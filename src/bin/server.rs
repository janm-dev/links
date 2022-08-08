//! # links server
//!
//! The links server is what actually redirects requests to their proper
//! destinations, interacts with (and sometimes is) the backend store for
//! redirections, and (soon) collects statistics about redirects. It
//! accomplishes this with two (or three) external interfaces: an HTTP server,
//! a RPC server, and (usually) a connection to a backend store.
//!
//! ## The HTTP server
//! Links uses [hyper](https://hyper.rs/) for HTTP/1.0, HTTP/1.1, and HTTP/2.
//! It listens for incoming requests and redirects them (using the 302 status
//! code for GET requests and 307 for everything else).
//!
//! ## The RPC server
//! Links runs a RPC server via [tonic](https://github.com/hyperium/tonic) to
//! provide seamless access to the backend store for tasks such as setting a
//! redirect. The server is authenticated with a shared token. The protocol
//! definition can be found in [`proto/links.proto`](../proto/links.proto).
//!
//! ## The store backend
//! Links can use many different databases and data stores as store backends,
//! providing flexibility with the storage setup. Currently in-memory and Redis
//! backends are supported.

use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use links::{
	api::{self, Api, LinksServer},
	certs::CertificateResolver,
	config::{Config, Tls},
	id::Id,
	normalized::{Link, Normalized},
	redirector::{https_redirector, redirector},
	store::Store,
	util::{SERVER_HELP, SERVER_NAME},
};
use pico_args::Arguments;
use tokio::{net::TcpListener, spawn, try_join};
use tokio_rustls::{
	rustls::{server::ResolvesServerCert, ServerConfig},
	TlsAcceptor,
};
use tonic::{
	codec::CompressionEncoding, codegen::InterceptedService, transport::Server as RpcServer,
};
use tracing::{debug, error, info, Level};
use tracing_subscriber::{filter::FilterFn, prelude::*, FmtSubscriber};

/// Run the links redirector server using configuration from the provided
/// command line arguments. This is essentially the entire server binary, but
/// exposed via `lib.rs` to aid in integration tests.
///
/// # Errors
/// Returns an error if setup fails, or an unexpected and unrecoverable runtime
/// error occurs.
#[allow(clippy::too_many_lines)] // TODO: consider refactoring so that this is not necessary
#[allow(clippy::similar_names)] // Caused by `http_addr` and `https_addr`, with no real alternatives
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	// Create a temporary tracing subscriber to collect and show logs on startup
	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(if cfg!(debug_assertions) {
			Level::DEBUG
		} else {
			Level::INFO
		})
		.finish();

	// Set the subscriber as the current default so logs are sent there
	let subscriber_guard = tracing::subscriber::set_default(tracing_subscriber);

	info!("Getting server configuration");

	// Parse cli args
	let mut args = Arguments::from_env();

	if args.contains(["-h", "--help"]) {
		print!("{}", SERVER_HELP);
		std::process::exit(0);
	}

	// Configure the server
	let config = Config::new(args.opt_value_from_str(["-c", "--config"])?);

	debug!(?config, "Server configuration parsed");

	// Set a tracing filter which can change the minimum log level on the fly.
	let server_config = config.clone();
	let tracing_filter =
		FilterFn::new(move |metadata| metadata.level() <= &server_config.log_level());

	// Create the permanent global tracing subscriber to collect and show logs
	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(Level::TRACE)
		.finish()
		.with(tracing_filter);

	drop(subscriber_guard);
	tracing::subscriber::set_global_default(tracing_subscriber)
		.expect("setting tracing default subscriber failed");

	// Get TLS cert and key
	let cert_resolver = match config.tls() {
		Tls::Enable {
			key_file,
			cert_file,
		}
		| Tls::Force {
			key_file,
			cert_file,
		} => {
			debug!(
				"Using cert file: \"{}\", key file \"{}\"",
				cert_file.to_string_lossy(),
				key_file.to_string_lossy()
			);

			// This does synchronous file IO inside of an async function, which
			// would usually be bad. However, here this is done only on server
			// startup, and using `tokio::task::spawn_blocking` causes weird issues
			// in tests on linux.
			let resolver: Arc<dyn ResolvesServerCert + 'static> =
				Arc::new(CertificateResolver::new(key_file, cert_file)?);

			Some(resolver)
		}
		_ => None,
	};

	// Listen on all addresses, on port 80 (HTTP)
	let http_addr = SocketAddr::from(([0, 0, 0, 0], 80));
	// Listen on all addresses, on port 443 (HTTPS)
	let https_addr = SocketAddr::from(([0, 0, 0, 0], 443));
	// Listen on all addresses, on port 530 (gRPC)
	let rpc_addr = SocketAddr::from(([0, 0, 0, 0], 530));

	// Initialize the store
	let store = Store::new_static(config.store(), &config.store_config()).await?;

	if args.contains("--example-redirect") {
		store
			.set_redirect(Id::try_from(Id::MAX)?, Link::new("https://example.com/")?)
			.await?;
		store
			.set_vanity(Normalized::new("example"), Id::try_from(Id::MAX)?)
			.await?;
	}

	let server = &*SERVER_NAME;
	info!(%server, %config , "Starting links");

	// Start the gRPC API server
	let rpc_handle = if let Some(ref resolver) = cert_resolver {
		let mut rpc_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(Arc::clone(resolver));
		rpc_config.alpn_protocols = vec![b"h2".to_vec()];

		let rpc_config = Arc::new(rpc_config);
		let rpc_listener = TcpListener::bind(rpc_addr).await?;
		let rpc_acceptor = TlsAcceptor::from(rpc_config);
		let mut rpc_handler = Http::new();
		rpc_handler.http2_only(true);
		let rpc_service = LinksServer::new(Api::new(store))
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip);
		let rpc_service = RpcServer::builder()
			.add_service(InterceptedService::new(
				rpc_service,
				api::get_auth_checker(config.clone()),
			))
			.into_service();

		spawn(async move {
			loop {
				let tcp_stream = match rpc_listener.accept().await {
					Ok((tcp_stream, _)) => tcp_stream,
					Err(tcp_err) => {
						error!(?tcp_err, "Error while accepting gRPC connection");
						continue;
					}
				};

				let handler = rpc_handler.clone();
				let service = rpc_service.clone();
				let acceptor = rpc_acceptor.clone();

				spawn(async move {
					let tls_stream = match acceptor.accept(tcp_stream).await {
						Ok(tls_stream) => tls_stream,
						Err(tls_err) => {
							error!(?tls_err, "Error while initiating gRPC connection");
							return;
						}
					};

					if let Err(rpc_err) = handler.serve_connection(tls_stream, service).await {
						error!(?rpc_err, "Error while serving gRPC connection");
					}
				});
			}
		})
	} else {
		let rpc_listener = TcpListener::bind(rpc_addr).await?;
		let mut rpc_handler = Http::new();
		rpc_handler.http2_only(true);
		let rpc_service = LinksServer::new(Api::new(store))
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip);
		let rpc_service = RpcServer::builder()
			.add_service(InterceptedService::new(
				rpc_service,
				api::get_auth_checker(config.clone()),
			))
			.into_service();

		spawn(async move {
			loop {
				let tcp_stream = match rpc_listener.accept().await {
					Ok((tcp_stream, _)) => tcp_stream,
					Err(tcp_err) => {
						error!(?tcp_err, "Error while accepting gRPC connection");
						continue;
					}
				};

				let handler = rpc_handler.clone();
				let service = rpc_service.clone();

				spawn(async move {
					if let Err(rpc_err) = handler.serve_connection(tcp_stream, service).await {
						error!(?rpc_err, "Error while serving gRPC connection");
					}
				});
			}
		})
	};

	let redirector_config = config.redirector();

	// Start the HTTP server
	let http_handle = {
		let http_listener = TcpListener::bind(http_addr).await?;
		let mut http_handler = Http::new();
		http_handler.http1_only(false).http2_only(false);
		let https_service =
			service_fn(move |req: Request<Body>| https_redirector(req, redirector_config));
		let redirector_service =
			service_fn(move |req: Request<Body>| redirector(req, store, redirector_config));

		let config = config.clone();

		spawn(async move {
			loop {
				let tcp_stream = match http_listener.accept().await {
					Ok((tcp_stream, _)) => tcp_stream,
					Err(tcp_err) => {
						error!(?tcp_err, "Error while accepting HTTP connection");
						continue;
					}
				};

				let handler = http_handler.clone();
				let config = config.clone();

				spawn(async move {
					match config.tls() {
						Tls::Force {
							key_file: _,
							cert_file: _,
						} => {
							if let Err(http_err) =
								handler.serve_connection(tcp_stream, https_service).await
							{
								error!(?http_err, "Error while redirecting HTTP connection");
							}
						}
						_ => {
							if let Err(http_err) = handler
								.serve_connection(tcp_stream, redirector_service)
								.await
							{
								error!(?http_err, "Error while serving HTTP connection");
							}
						}
					}
				});
			}
		})
	};

	// Start the HTTPS server
	let https_handle = if let Some(ref resolver) = cert_resolver {
		let mut https_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_cert_resolver(Arc::clone(resolver));
		https_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

		let https_config = Arc::new(https_config);
		let https_listener = TcpListener::bind(https_addr).await?;
		let https_acceptor = TlsAcceptor::from(https_config);
		let mut https_handler = Http::new();
		https_handler.http1_only(false).http2_only(false);
		let https_service =
			service_fn(move |req: Request<Body>| redirector(req, store, redirector_config));

		spawn(async move {
			loop {
				let tcp_stream = match https_listener.accept().await {
					Ok((tcp_stream, _)) => tcp_stream,
					Err(tcp_err) => {
						error!(?tcp_err, "Error while accepting HTTPS connection");
						continue;
					}
				};

				let handler = https_handler.clone();
				let acceptor = https_acceptor.clone();

				spawn(async move {
					let tls_stream = match acceptor.accept(tcp_stream).await {
						Ok(tls_stream) => tls_stream,
						Err(tls_err) => {
							error!(?tls_err, "Error while initiating HTTPS connection");
							return;
						}
					};

					if let Err(rpc_err) = handler.serve_connection(tls_stream, https_service).await
					{
						error!(?rpc_err, "Error while serving HTTPS connection");
					}
				});
			}
		})
	} else {
		spawn(async {})
	};

	info!("Links redirector server started");

	// Wait until the first unhandled error (if any) and exit
	try_join!(rpc_handle, http_handle, https_handle)?;

	Ok(())
}
