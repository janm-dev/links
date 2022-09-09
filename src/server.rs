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
	net::SocketAddr,
	sync::Arc,
	thread,
};

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use parking_lot::Mutex;
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
use tracing::{error, trace, warn};

use crate::{
	api::{self, Api, LinksServer},
	certs::CertificateResolver,
	config::Config,
	id::Id,
	normalized::{Link, Normalized},
	redirector::{https_redirector, redirector},
	store::{Current, Store},
};

/// A handler that does external HTTP redirects using information from the
/// provided store.
pub async fn http_handler(
	stream: impl AsyncRead + AsyncWrite + Send + Unpin + 'static,
	store: Store,
	config: &'static Config,
) {
	let redirector_service =
		service_fn(move |req: Request<Body>| redirector(req, store.clone(), config.redirector()));

	if let Err(err) = Http::new()
		.serve_connection(stream, redirector_service)
		.await
	{
		error!(?err, "Error while handling HTTP connection");
	}
}

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
}

/// An acceptor for plaintext (unencrypted) HTTP requests. Supports HTTP/1.0,
/// HTTP/1.1, and HTTP/2 (in its rare unencrypted variety usually not found in
/// any browsers).
#[derive(Debug, Copy, Clone)]
pub struct PlainHttpAcceptor {
	config: &'static Config,
	current_store: &'static Current,
}

impl PlainHttpAcceptor {
	/// Create a new [`PlainHttpAcceptor`] with the provided [`Config`] and
	/// [`Current`]
	pub const fn new(config: &'static Config, current_store: &'static Current) -> Self {
		Self {
			config,
			current_store,
		}
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
				http_handler(stream, current_store.get(), config).await;
			}
		});
	}
}

/// An acceptor for TLS-encrypted HTTPS requests. Supports HTTP/1.0, HTTP/1.1,
/// and HTTP/2.
#[derive(Clone)]
pub struct TlsHttpAcceptor {
	config: &'static Config,
	current_store: &'static Current,
	tls_acceptor: TlsAcceptor,
}

impl TlsHttpAcceptor {
	/// Create a new [`TlsHttpAcceptor`] with the provided [`Config`],
	/// [`Current`], and a reference-counted (via [`Arc`])
	/// [`CertificateResolver`]
	pub fn new(
		config: &'static Config,
		current_store: &'static Current,
		cert_resolver: Arc<CertificateResolver>,
	) -> Self {
		let mut server_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(cert_resolver);
		server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

		let server_config = Arc::new(server_config);
		let tls_acceptor = TlsAcceptor::from(server_config);

		Self {
			config,
			current_store,
			tls_acceptor,
		}
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
				Ok(stream) => http_handler(stream, current_store.get(), config).await,
				Err(err) => warn!("Error accepting incoming TLS connection: {err:?}"),
			}
		});
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
	pub fn new(config: &'static Config, current_store: &'static Current) -> Self {
		let service = RpcServer::builder()
			.add_service(InterceptedService::new(
				LinksServer::new(Api::new(current_store))
					.send_compressed(CompressionEncoding::Gzip)
					.accept_compressed(CompressionEncoding::Gzip),
				api::get_auth_checker(config),
			))
			.into_service();

		Self {
			service: Mutex::new(service),
		}
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
}

/// An acceptor for TLS-encrypted RPC calls. Supports `gRPC` over
/// HTTP/2 with HTTPS.
pub struct TlsRpcAcceptor {
	service: Mutex<Routes>,
	tls_acceptor: TlsAcceptor,
}

impl TlsRpcAcceptor {
	/// Create a new [`TlsRpcAcceptor`] with the provided [`Config`],
	/// [`Current`], and a reference-counted (via [`Arc`])
	/// [`CertificateResolver`]
	pub fn new(
		config: &'static Config,
		current_store: &'static Current,
		cert_resolver: Arc<CertificateResolver>,
	) -> Self {
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

		Self {
			service: Mutex::new(service),
			tls_acceptor,
		}
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

/// A links redirector listener. This listens for incoming network connections
/// on a specified address using a specified protocol in an async task in the
/// background. On drop, the async task is aborted in order to stop listening.
#[derive(Debug)]
pub struct Listener {
	/// The address of this listener's socket. `0.0.0.0` and `[::]` can be used
	/// as wildcards, accepting traffic to any address.
	pub addr: SocketAddr,
	handle: JoinHandle<()>,
}

impl Listener {
	/// Create a new [`Listener`] on the specified address, which will use the
	/// specified acceptor to accept incoming connections. The addresses
	/// `0.0.0.0` and `[::]` can be used to listen on all local addresses, and
	/// port `0` can be used to indicate an OS-assigned port (not generally
	/// recommended for a server).
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
	/// This function returns an error if it can not bind to the address.
	pub async fn new(
		addr: impl Into<SocketAddr> + Send,
		acceptor: impl Acceptor<TcpStream>,
	) -> Result<Self, IoError> {
		let addr = addr.into();
		let listener = TcpListener::bind(addr).await?;

		let handle = spawn(async move {
			loop {
				match listener.accept().await {
					Ok((stream, remote_addr)) => {
						acceptor.accept(stream, addr, remote_addr).await;
					}
					Err(err) => {
						warn!("Error accepting TCP connection on {addr}: {err:?}");
						continue;
					}
				}
			}
		});

		Ok(Self { addr, handle })
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
		self.handle.abort();

		while !self.handle.is_finished() {
			thread::yield_now();
		}
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
	use std::time::Instant;

	use super::*;

	/// A mock [`Acceptor`] that does nothing.
	#[derive(Debug, Copy, Clone)]
	struct UnAcceptor;

	#[async_trait::async_trait]
	impl Acceptor<TcpStream> for UnAcceptor {
		async fn accept(&self, _: TcpStream, _: SocketAddr, _: SocketAddr) {
			spawn(async {});
		}
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn listener() {
		let addr = ([127, 0, 0, 1], 8000);

		let listener = Listener::new(addr, UnAcceptor).await.unwrap();

		let start = Instant::now();
		drop(listener);
		let duration = start.elapsed();

		let _listener = Listener::new(addr, UnAcceptor).await.unwrap();

		assert!(duration.as_micros() < 1000);
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
