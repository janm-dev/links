//! Links redirector server utilities, including:
//! - Listeners, which listen for incoming network traffic
//! - Acceptors, which accept incoming connections and direct it to the handlers
//! - Handlers, which handle requests, doing HTTP redirects or RPC calls
//! - miscellaneous functions used by the server binary
//!
//! # Listeners
//! A listener controls a network socket and calls its associated acceptor when
//! a new connection is received. One listener exists for every socket that the
//! links redirector server listens on. Listeners can be added and removed while
//! the server is active.
//!
//! # Acceptors
//! Acceptors are the bridge between listeners and handlers. They hold
//! (references to) all necessary state used by themselves and the handlers. One
//! acceptor exists for every combination of listener + encryption + handler
//! type, e.g. an unencrypted TCP acceptor for HTTP, an encrypted TCP/TLS
//! acceptor for RPC, etc. Acceptors are predefined for the lifetime of the
//! redirector server, and initialized at server startup. Acceptors initially
//! process incoming connections (doing handshakes and de/en-cryption if
//! necessary), and then call their associated handler.
//!
//! # Handlers
//! Handlers are responsible for all application logic. A handler is an async
//! function called from an acceptor. There is one predefined handler for each
//! kind of request: currently one external HTTP redirector, one HTTP to HTTPS
//! redirector, and one RPC handler.

use std::{
	fmt::{Debug, Formatter, Result as FmtResult},
	net::{IpAddr, Ipv6Addr, SocketAddr},
	os::raw::c_int,
	sync::Arc,
	thread,
};

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use links_id::Id;
use links_normalized::{Link, Normalized};
use parking_lot::Mutex;
use socket2::{Domain, Protocol as SocketProtocol, Socket, Type};
use strum::{Display as EnumDisplay, EnumString};
use tokio::{
	io::{AsyncRead, AsyncWrite, Error as IoError},
	net::{TcpListener, TcpStream},
	spawn,
	task::JoinHandle,
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tonic::{
	codegen::{CompressionEncoding, InterceptedService},
	transport::{server::Routes, Server as RpcServer},
};
use tracing::{debug, error, trace, warn};

use crate::{
	api::{self, Api, LinksServer},
	certs::CertificateResolver,
	config::{Config, ListenAddress},
	redirector::{https_redirector, redirector},
	stats::ExtraStatisticInfo,
	store::{Current, Store},
};

/// A handler that does external HTTP redirects using information from the
/// provided store. Extra information for statistics can be passed via
/// `stat_info`.
pub async fn http_handler(
	stream: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
	store: Store,
	config: &'static Config,
	stat_info: ExtraStatisticInfo,
) {
	let redirector_service = service_fn(move |req: Request<Body>| {
		redirector(req, store.clone(), config.redirector(), stat_info.clone())
	});

	if let Err(err) = Http::new()
		.serve_connection(stream, redirector_service)
		.await
	{
		error!(?err, "Error while handling HTTP connection");
	}
}

/// Number of incoming connections that can be kept in the TCP socket backlog of
/// a listener (see `listen`'s [linux man page] or [winsock docs] for details)
///
/// [linux man page]: https://linux.die.net/man/2/listen
/// [winsock docs]: https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-listen
const LISTENER_TCP_BACKLOG_SIZE: c_int = 1024;

/// A handler that redirects incoming requests to their original URL, but with
/// the HTTPS scheme instead.
///
/// # Warning
/// This function does not know the original URL scheme. If used as the handler
/// for HTTPS requests, this might create a redirect loop.
pub async fn http_to_https_handler(
	stream: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
	config: &'static Config,
) {
	let redirector_service =
		service_fn(move |req: Request<Body>| https_redirector(req, config.redirector()));

	if let Err(err) = Http::new()
		.serve_connection(stream, redirector_service)
		.await
	{
		error!(?err, "Error while handling HTTP connection");
	}
}

/// Handler processing RPC API calls.
pub async fn rpc_handler(
	stream: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
	service: Routes,
) {
	if let Err(rpc_err) = Http::new()
		.http2_only(true)
		.serve_connection(stream, service)
		.await
	{
		error!(?rpc_err, "Error while handling gRPC connection");
	}
}

/// A trait for defining links server acceptors.
///
/// For more info about acceptors in general, please see the [module-level
/// documentation][mod].
///
/// [mod]: crate::server
#[async_trait::async_trait]
pub trait Acceptor<S: AsyncRead + AsyncWrite + Send + Unpin + 'static>:
	Send + Sync + 'static
{
	/// Accept an incoming connection in `stream` from `remote_addr` to
	/// `local_addr`. This function should [spawn a task][spawn] to handle the
	/// request using this acceptor's associated handler.
	///
	/// [spawn]: tokio::task
	async fn accept(&self, stream: S, local_addr: SocketAddr, remote_addr: SocketAddr);

	/// Get the [`Protocol`] that this acceptor processes
	fn protocol(&self) -> Protocol;
}

/// An acceptor for plaintext (unencrypted) HTTP requests. Supports HTTP/1.0,
/// HTTP/1.1, and HTTP/2 (in its rare unencrypted variety usually not found in
/// any browsers).
#[derive(Debug)]
pub struct PlainHttpAcceptor {
	config: &'static Config,
	current_store: &'static Current,
}

impl PlainHttpAcceptor {
	/// Create a new [`PlainHttpAcceptor`] with the provided [`Config`] and
	/// [`Current`]
	///
	/// # Memory
	/// This function leaks memory, and should therefore not be called an
	/// unbounded number of times
	pub fn new(config: &'static Config, current_store: &'static Current) -> &'static Self {
		Box::leak(Box::new(Self {
			config,
			current_store,
		}))
	}
}

#[async_trait::async_trait]
impl Acceptor<TcpStream> for PlainHttpAcceptor {
	async fn accept(&self, stream: TcpStream, local_addr: SocketAddr, remote_addr: SocketAddr) {
		let config = self.config;
		let current_store = self.current_store;

		spawn(async move {
			trace!("New plain connection from {remote_addr} on {local_addr}");

			if config.https_redirect() {
				http_to_https_handler(stream, config).await;
			} else {
				http_handler(
					stream,
					current_store.get(),
					config,
					ExtraStatisticInfo::default(),
				)
				.await;
			}
		});
	}

	fn protocol(&self) -> Protocol {
		Protocol::Http
	}
}

/// An acceptor for TLS-encrypted HTTPS requests. Supports HTTP/1.0, HTTP/1.1,
/// and HTTP/2.
pub struct TlsHttpAcceptor {
	config: &'static Config,
	current_store: &'static Current,
	tls_acceptor: TlsAcceptor,
}

impl TlsHttpAcceptor {
	/// Create a new [`TlsHttpAcceptor`] with the provided [`Config`],
	/// [`Current`], and a reference-counted (via [`Arc`])
	/// [`CertificateResolver`]
	///
	/// # Memory
	/// This function leaks memory, and should therefore not be called an
	/// unbounded number of times
	pub fn new(
		config: &'static Config,
		current_store: &'static Current,
		cert_resolver: Arc<CertificateResolver>,
	) -> &'static Self {
		let mut server_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(cert_resolver);
		server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

		let server_config = Arc::new(server_config);
		let tls_acceptor = TlsAcceptor::from(server_config);

		Box::leak(Box::new(Self {
			config,
			current_store,
			tls_acceptor,
		}))
	}
}

#[async_trait::async_trait]
impl Acceptor<TcpStream> for TlsHttpAcceptor {
	async fn accept(&self, stream: TcpStream, local_addr: SocketAddr, remote_addr: SocketAddr) {
		let config = self.config;
		let current_store = self.current_store;
		let tls_acceptor = self.tls_acceptor.clone();

		spawn(async move {
			trace!("New TLS connection from {remote_addr} on {local_addr}");

			match tls_acceptor.accept(stream).await {
				Ok(stream) => {
					let tls_conn = stream.get_ref().1;
					let extra_info = ExtraStatisticInfo {
						tls_sni: tls_conn.server_name().map(Arc::from),
						tls_version: tls_conn.protocol_version(),
						tls_cipher_suite: tls_conn.negotiated_cipher_suite(),
					};

					http_handler(stream, current_store.get(), config, extra_info).await;
				}
				Err(err) => warn!("Error accepting incoming TLS connection: {err:?}"),
			}
		});
	}

	fn protocol(&self) -> Protocol {
		Protocol::Https
	}
}

impl Debug for TlsHttpAcceptor {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		#[derive(Debug)]
		struct TlsAcceptor {}

		fmt.debug_struct("TlsHttpAcceptor")
			.field("config", self.config)
			.field("current_store", self.current_store)
			.field("tls_acceptor", &TlsAcceptor {})
			.finish()
	}
}

/// An acceptor for plaintext (unencrypted) RPC calls. Supports `gRPC` over
/// unencrypted HTTP/2.
#[derive(Debug)]
pub struct PlainRpcAcceptor {
	service: Mutex<Routes>,
}

impl PlainRpcAcceptor {
	/// Create a new [`PlainRpcAcceptor`] with the provided [`Config`] and
	/// [`Current`]
	///
	/// # Memory
	/// This function leaks memory, and should therefore not be called an
	/// unbounded number of times
	pub fn new(config: &'static Config, current_store: &'static Current) -> &'static Self {
		let service = RpcServer::builder()
			.add_service(InterceptedService::new(
				LinksServer::new(Api::new(current_store))
					.send_compressed(CompressionEncoding::Gzip)
					.accept_compressed(CompressionEncoding::Gzip),
				api::get_auth_checker(config),
			))
			.into_service();

		Box::leak(Box::new(Self {
			service: Mutex::new(service),
		}))
	}
}

#[async_trait::async_trait]
impl Acceptor<TcpStream> for PlainRpcAcceptor {
	async fn accept(&self, stream: TcpStream, local_addr: SocketAddr, remote_addr: SocketAddr) {
		let service = self.service.lock().clone();

		spawn(async move {
			trace!("New plain connection from {remote_addr} on {local_addr}");

			rpc_handler(stream, service).await;
		});
	}

	fn protocol(&self) -> Protocol {
		Protocol::Grpc
	}
}

/// An acceptor for TLS-encrypted RPC calls. Supports `gRPC` over
/// HTTP/2 with HTTPS.
pub struct TlsRpcAcceptor {
	service: Arc<Mutex<Routes>>,
	tls_acceptor: TlsAcceptor,
}

impl TlsRpcAcceptor {
	/// Create a new [`TlsRpcAcceptor`] with the provided [`Config`],
	/// [`Current`], and a reference-counted (via [`Arc`])
	/// [`CertificateResolver`]
	///
	/// # Memory
	/// This function leaks memory, and should therefore not be called an
	/// unbounded number of times
	pub fn new(
		config: &'static Config,
		current_store: &'static Current,
		cert_resolver: Arc<CertificateResolver>,
	) -> &'static Self {
		let mut server_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(cert_resolver);
		server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

		let server_config = Arc::new(server_config);
		let tls_acceptor = TlsAcceptor::from(server_config);

		let service = RpcServer::builder()
			.add_service(InterceptedService::new(
				LinksServer::new(Api::new(current_store))
					.send_compressed(CompressionEncoding::Gzip)
					.accept_compressed(CompressionEncoding::Gzip),
				api::get_auth_checker(config),
			))
			.into_service();

		Box::leak(Box::new(Self {
			service: Arc::new(Mutex::new(service)),
			tls_acceptor,
		}))
	}
}

#[async_trait::async_trait]
impl Acceptor<TcpStream> for TlsRpcAcceptor {
	async fn accept(&self, stream: TcpStream, local_addr: SocketAddr, remote_addr: SocketAddr) {
		let tls_acceptor = self.tls_acceptor.clone();
		let service = self.service.lock().clone();

		spawn(async move {
			trace!("New TLS connection from {remote_addr} on {local_addr}");

			match tls_acceptor.accept(stream).await {
				Ok(stream) => rpc_handler(stream, service).await,
				Err(err) => warn!("Error accepting incoming TLS connection: {err:?}"),
			}
		});
	}

	fn protocol(&self) -> Protocol {
		Protocol::Grpcs
	}
}

impl Debug for TlsRpcAcceptor {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		#[derive(Debug)]
		struct TlsAcceptor {}

		fmt.debug_struct("TlsRpcAcceptor")
			.field("service", &self.service)
			.field("tls_acceptor", &TlsAcceptor {})
			.finish()
	}
}

/// The protocols that links redirector servers can listen on
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, EnumDisplay)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum Protocol {
	/// HTTP/1.0, HTTP/1.1, and HTTP/2 (h2c) over TCP (unencrypted)
	Http,
	/// HTTP/1.0, HTTP/1.1, and HTTP/2 (h2) over TCP with TLS
	Https,
	/// gRPC over HTTP/2 (h2c) over TCP (unencrypted)
	Grpc,
	/// gRPC over HTTP/2 (h2) over TCP with TLS
	Grpcs,
}

impl Protocol {
	/// Default port for the `grpcs` protocol
	pub const GRPCS_DEFAULT_PORT: u16 = 530;
	/// Default port for the `grpc` protocol
	pub const GRPC_DEFAULT_PORT: u16 = 50051;
	/// Default port for the `https` protocol
	pub const HTTPS_DEFAULT_PORT: u16 = 443;
	/// Default port for the `http` protocol
	pub const HTTP_DEFAULT_PORT: u16 = 80;

	/// Get the default port for this [`Protocol`]
	#[must_use]
	pub const fn default_port(self) -> u16 {
		match self {
			Self::Http => Self::HTTP_DEFAULT_PORT,
			Self::Https => Self::HTTPS_DEFAULT_PORT,
			Self::Grpc => Self::GRPC_DEFAULT_PORT,
			Self::Grpcs => Self::GRPCS_DEFAULT_PORT,
		}
	}
}

/// A links redirector listener. This listens for incoming network connections
/// on a specified address using a specified protocol in an async task in the
/// background. On drop, the async task is aborted in order to stop listening.
#[derive(Debug)]
pub struct Listener {
	/// The address this listener will listen on. No address indicates that this
	/// listener will accept all traffic on any address (IPv4 and IPv6),
	/// `0.0.0.0` means any IPv4 address (but not IPv6), `[::]` means any IPv6
	/// address (but not IPv4).
	pub addr: Option<IpAddr>,
	/// The port this listener will listen on. Currently, this is a TCP port,
	/// but may in the future also additionally indicate a UDP port.
	pub port: u16,
	/// The protocol of the acceptor/handler this listener uses to process
	/// requests
	pub proto: Protocol,
	handle: JoinHandle<()>,
}

impl Listener {
	/// Create a new [`Listener`] on the specified address, which will use the
	/// specified acceptor to accept incoming connections. If no address is
	/// specified, the listener will listen on all IPv4 and IPv6 addresses.
	/// Address `0.0.0.0` can be used to listen on all IPv4 (but not IPv6)
	/// addresses, and address `[::]` can be used to listen on all IPv6 (but not
	/// IPv4) addresses. If the port is not specified, the protocol's default
	/// port will be used (see [`Protocol`] for details).
	///
	/// **Note:**
	/// Support for dual stack sockets (IPv4 and IPv6 in one socket, available
	/// in links via an empty address) is not universal on all platforms (such
	/// as some BSDs). On those platforms, an empty address and `[::]` will
	/// behave the same, i.e. an empty address will only listen on IPv6, not
	/// IPv4. To get the desired result (IPv4 *and* IPv6), you must use two
	/// listeners, one listening on `0.0.0.0` and the other on `[::]`.
	///
	/// # Drop
	/// When dropped, a listener will wait until its internal task is fully
	/// cancelled, which can take some time to complete. Dropping a listener
	/// should therefore be considered blocking, and only done in synchronous
	/// contexts or via the [`spawn_blocking` function][spawn_blocking].
	/// Additionally, because the `drop` function blocks its thread until the
	/// async runtime completes the cancellation of the task in the background,
	/// a listener requires more than one thread to drop, and can not
	/// successfully be dropped inside of a single-threaded tokio runtime (the
	/// entire program will block indefinitely).
	///
	/// [spawn_blocking]: fn@tokio::task::spawn_blocking
	///
	/// # Errors
	/// This function returns an error if it can not set up the listening
	/// socket.
	#[allow(clippy::unused_async)] // TODO
	pub async fn new(
		addr: Option<IpAddr>,
		port: Option<u16>,
		acceptor: &'static impl Acceptor<TcpStream>,
	) -> Result<Self, IoError> {
		let proto = acceptor.protocol();
		let port = port.unwrap_or_else(|| proto.default_port());
		let socket_addr = (addr.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED)), port).into();

		let socket = Socket::new(
			Domain::for_address(socket_addr),
			Type::STREAM,
			Some(SocketProtocol::TCP),
		)?;

		// `SO_REUSEADDR` has different meanings across platforms:
		// - On Windows, it allows multiple listeners per socket (which is very bad)
		// - On Unix-like OSs, it allows a process to bind to a recently-closed socket
		//   (which can occasionally speed up socket initialization)
		socket.set_reuse_address(cfg!(unix))?;
		// Set the socket into IPv6-only mode if the address is configured as IPv6 (even
		// if it's `[::]`). This is done because the default depends on the OS and
		// sometimes user configuration, and we want consistency across platforms.
		if socket_addr.is_ipv6() {
			socket.set_only_v6(addr.is_some())?;
		}
		// Required for Tokio to properly use async listeners
		socket.set_nonblocking(true)?;
		// Improves latency when sending responses
		socket.set_nodelay(true)?;

		socket.bind(&socket_addr.into())?;
		socket.listen(LISTENER_TCP_BACKLOG_SIZE)?;
		let listener = TcpListener::from_std(socket.into())?;

		let handle = spawn(async move {
			loop {
				match listener.accept().await {
					Ok((stream, remote_addr)) => {
						acceptor.accept(stream, socket_addr, remote_addr).await;
					}
					Err(err) => {
						warn!("Error accepting TCP connection on {socket_addr}: {err:?}");
						continue;
					}
				}
			}
		});

		debug!("Opened new listener on {}", ListenAddress {
			protocol: proto,
			address: addr,
			port: Some(port),
		});

		Ok(Self {
			addr,
			port,
			proto,
			handle,
		})
	}

	/// Get the [`ListenAddress`] of this listener
	#[must_use]
	pub const fn listen_address(&self) -> ListenAddress {
		ListenAddress {
			protocol: self.proto,
			address: self.addr,
			port: Some(self.port),
		}
	}
}

impl Drop for Listener {
	/// Cancel the task responsible for listening
	///
	/// # Blocking
	/// This functions blocks the current thread until the task is fully
	/// aborted. Additionally, if used in the context of a single-threaded tokio
	/// runtime, this function can completely block the entire program.
	fn drop(&mut self) {
		trace!("Closing listener on {}", self.listen_address());

		self.handle.abort();

		while !self.handle.is_finished() {
			thread::yield_now();
		}

		debug!("Closed listener on {}", self.listen_address());
	}
}

/// Set up the links store, optionally setting an example redirect
/// (`example` -> `9dDbKpJP` -> `https://example.com/`).
///
/// # Errors
/// This function returns an error if construction of the [`Store`] (using
/// `Store::new`) fails or if the example redirect can not be set when
/// requested.
pub async fn store_setup(config: &Config, example_redirect: bool) -> Result<Store, anyhow::Error> {
	let store = Store::new(config.store(), &config.store_config()).await?;

	if example_redirect {
		store
			.set_redirect(Id::try_from(Id::MAX)?, Link::new("https://example.com/")?)
			.await?;
		store
			.set_vanity(Normalized::new("example"), Id::try_from(Id::MAX)?)
			.await?;
	}

	Ok(store)
}

#[cfg(test)]
mod tests {
	use std::time::{Duration, Instant};

	use super::*;

	/// A mock [`Acceptor`] that does nothing, while pretending to do HTTP
	#[derive(Debug, Copy, Clone)]
	struct UnAcceptor;

	#[async_trait::async_trait]
	impl Acceptor<TcpStream> for UnAcceptor {
		async fn accept(&self, _: TcpStream, _: SocketAddr, _: SocketAddr) {
			spawn(async {});
		}

		fn protocol(&self) -> Protocol {
			Protocol::Http
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn listener_new_drop() {
		let addr = Some([127, 0, 0, 1].into());
		let port = Some(8000);

		let listener = Listener::new(addr, port, &UnAcceptor).await.unwrap();

		let start = Instant::now();
		drop(listener);
		let duration = start.elapsed();

		let _listener = Listener::new(addr, port, &UnAcceptor).await.unwrap();

		assert!(
			dbg!(duration) < Duration::from_millis(if cfg!(debug_assertions) { 100 } else { 1 })
		);
	}

	#[tokio::test]
	async fn fn_store_setup() {
		let with_example = store_setup(&Config::new(None), true).await.unwrap();
		let without_example = store_setup(&Config::new(None), false).await.unwrap();

		assert_eq!(
			with_example.get_vanity("example".into()).await.unwrap(),
			Some(Id::MAX.try_into().unwrap())
		);

		assert_eq!(
			without_example.get_vanity("example".into()).await.unwrap(),
			None
		);
	}
}
