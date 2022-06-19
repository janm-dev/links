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

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use pico_args::Arguments;
use rand::{distributions::Alphanumeric, Rng};
use tokio::{net::TcpListener, spawn, try_join};
use tokio_rustls::{
	rustls::{server::ResolvesServerCert, ServerConfig},
	TlsAcceptor,
};
use tonic::{codegen::InterceptedService, transport::Server as RpcServer};
use tracing::{debug, error, info, Level};

use crate::{
	api::{self, Api, LinksServer},
	config::CertificateResolver,
	id::Id,
	normalized::{Link, Normalized},
	redirector::{https_redirector, redirector, Config},
	store::Store,
	util::SERVER_HELP,
};

/// Run the links redirector server using configuration from the provided
/// command line arguments (with the executable name removed). This is
/// essentially the entire server binary, but exposed via `lib.rs` to aid in
/// integration tests.
///
/// # What this function *doesn't* do
/// - Set up a default tracing subscriber. This would otherwise interfere with
///   integration tests.
/// - Parse CLI arguments from source. The arguments must be passed to this
///   function in a `pico_args::Arguments` struct, but will thereafter be fully
///   handled. This would otherwise also interfere with integration tests.
///
/// # Errors
/// Returns an error if setup fails, or an unexpected and unrecoverable runtime
/// error occurs.
#[allow(clippy::too_many_lines)] // TODO: consider refactoring so that this is not necessary
#[allow(clippy::similar_names)] // Caused by `http_addr` and `https_addr`, with no real alternatives
pub async fn run(mut args: Arguments, log_level: Level) -> Result<(), anyhow::Error> {
	// Parse cli args
	if args.contains(["-h", "--help"]) {
		print!("{}", SERVER_HELP);
		std::process::exit(0);
	}

	let enable_tls = args.contains(["-t", "--tls-enable"]);
	let redirect_https = args.contains(["-r", "--redirect-https"]);

	// Set redirector config from args
	let mut config = Config::default();
	config.enable_hsts ^= args.contains("--disable-hsts");
	config.preload_hsts ^= args.contains("--preload-hsts");
	config.enable_alt_svc ^= args.contains("--enable-alt-svc");
	config.enable_server ^= args.contains("--disable-server");
	config.enable_csp ^= args.contains("--disable-csp");
	config.hsts_age = args
		.opt_value_from_str("--hsts-age")?
		.unwrap_or(config.hsts_age);
	let config = config;

	// Get API auth secret from args (or generate a random one)
	let api_secret = Box::leak(Box::new(
		args.opt_value_from_str(["-a", "--api-secret"])?
			.unwrap_or_else(|| {
				let secret = rand::thread_rng()
					.sample_iter(&Alphanumeric)
					.take(32)
					.map(char::from)
					.collect::<String>();
				info!("No API secret provided, generated new secret: \"{secret}\"");
				secret
			}),
	));
	debug!("Using API secret: \"{api_secret}\"");

	// Get TLS cert and key
	let cert_resolver = if enable_tls {
		let cert_path = args
			.opt_value_from_fn::<_, _, anyhow::Error>(["-c", "--tls-cert"], |s| {
				Ok(PathBuf::from(s))
			})?
			.unwrap_or_else(|| PathBuf::from("./cert.pem"));
		let key_path = args
			.opt_value_from_fn::<_, _, anyhow::Error>(["-k", "--tls-key"], |s| {
				Ok(PathBuf::from(s))
			})?
			.unwrap_or_else(|| PathBuf::from("./key.pem"));

		debug!(
			"Using cert file: \"{}\", key file \"{}\"",
			cert_path.clone().to_string_lossy(),
			key_path.clone().to_string_lossy()
		);

		let resolver: Arc<dyn ResolvesServerCert + 'static> =
			Arc::new(CertificateResolver::new(key_path, cert_path).await?);

		Some(resolver)
	} else {
		None
	};

	// Listen on all addresses, on port 80 (HTTP)
	let http_addr = SocketAddr::from(([0, 0, 0, 0], 80));
	// Listen on all addresses, on port 443 (HTTPS)
	let https_addr = SocketAddr::from(([0, 0, 0, 0], 443));
	// Listen on all addresses, on port 530 (gRPC)
	let rpc_addr = SocketAddr::from(([0, 0, 0, 0], 530));

	// Initialize the store
	let store = Store::new_static(
		&args
			.opt_value_from_str::<_, String>(["-s", "--store"])?
			.unwrap_or_else(|| "memory".to_string()),
		&mut args,
	)
	.await?;

	if args.contains("--example-redirect") {
		store
			.set_redirect(Id::try_from(Id::MAX)?, Link::new("https://example.com/")?)
			.await?;
		store
			.set_vanity(Normalized::new("example"), Id::try_from(Id::MAX)?)
			.await?;
	}

	let server = &*crate::util::SERVER_NAME;
	info!(%server, %http_addr, %https_addr, %rpc_addr, %log_level, store = store.backend_name(), %enable_tls, "Starting links");

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
		let rpc_service = LinksServer::new(Api::new(store)).send_gzip().accept_gzip();
		let rpc_service = RpcServer::builder()
			.add_service(InterceptedService::new(
				rpc_service,
				api::get_auth_checker(api_secret),
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
		let rpc_service = LinksServer::new(Api::new(store)).send_gzip().accept_gzip();
		let rpc_service = RpcServer::builder()
			.add_service(InterceptedService::new(
				rpc_service,
				api::get_auth_checker(api_secret),
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

	// Start the HTTP server
	let http_handle = if redirect_https {
		let http_listener = TcpListener::bind(http_addr).await?;
		let mut http_handler = Http::new();
		http_handler.http1_only(false).http2_only(false);
		let http_service = service_fn(move |req: Request<Body>| https_redirector(req, config));

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

				spawn(async move {
					if let Err(http_err) = handler.serve_connection(tcp_stream, http_service).await
					{
						error!(?http_err, "Error while serving HTTP connection");
					}
				});
			}
		})
	} else {
		let http_listener = TcpListener::bind(http_addr).await?;
		let mut http_handler = Http::new();
		http_handler.http1_only(false).http2_only(false);
		let http_service = service_fn(move |req: Request<Body>| redirector(req, store, config));

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

				spawn(async move {
					if let Err(http_err) = handler.serve_connection(tcp_stream, http_service).await
					{
						error!(?http_err, "Error while serving HTTP connection");
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
		let https_service = service_fn(move |req: Request<Body>| redirector(req, store, config));

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
