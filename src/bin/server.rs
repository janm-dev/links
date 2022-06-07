//! # links server
//!
//! The links server is what actually redirects requests to their proper
//! destinations, interacts with (and sometimes is) the backend store for
//! redirections, and (soon) collects statistics about redirects. It
//! accomplishes this with two (or three) external interfaces: an HTTP server,
//! a gRPC server, and (usually) a connection to a backend store.
//!
//! ## The HTTP server
//! Links uses [hyper](https://hyper.rs/) for HTTP/1.0, HTTP/1.1, and HTTP/2.
//! It listens for incoming requests and redirects them (using the 302 status
//! code for GET requests and 307 for everything else).
//!
//! ## The gRPC server
//! Links runs a gRPC server via [tonic](https://github.com/hyperium/tonic) to
//! provide seamless access to the backend store for tasks such as setting a
//! redirect. The server is authenticated with a shared token. The protocol
//! definition can be found in [`proto/links.proto`](../proto/links.proto).
//!
//! ## The store backend
//! Links can use many different databases and data stores as store backends,
//! providing flexibility with the storage setup. Currently in-memory and Redis
//! backends are supported.

use anyhow::anyhow;
use hyper::{server::conn::Http, service::service_fn, Body, Request};
use links::api::{self, Api, LinksServer};
use links::redirector::{https_redirector, redirector, Config};
use links::store::Store;
use links::util::SERVER_HELP;
use rand::{distributions::Alphanumeric, Rng};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{fs, net::TcpListener, spawn, try_join};
use tokio_rustls::{
	rustls::{Certificate, PrivateKey, ServerConfig},
	TlsAcceptor,
};
use tonic::{
	codegen::InterceptedService,
	transport::Server as RpcServer,
	transport::{Identity, ServerTlsConfig},
};
use tracing::{debug, error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	// Parse cli args
	let mut args = pico_args::Arguments::from_env();

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

	// Create a tracing subscriber to collect and show logs
	let log_level = args
		.opt_value_from_str(["-l", "--log"])?
		.unwrap_or(Level::INFO);

	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(log_level)
		.finish();

	// Set the subscriber as the global default so all logs are sent there
	tracing::subscriber::set_global_default(tracing_subscriber)
		.expect("setting tracing default subscriber failed");

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
	let cert_key = if enable_tls {
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
			cert_path.clone().into_os_string().to_string_lossy(),
			key_path.clone().into_os_string().to_string_lossy()
		);

		let certs = fs::read(cert_path).await.map_err(|e| {
			error!("Unable to read TLS certificates");
			e
		})?;
		let key = fs::read(key_path).await.map_err(|e| {
			error!("Unable to read TLS private key");
			e
		})?;

		Some((certs, key))
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

	// Configure TLS for HTTPS and gRPC
	let tls_config = if let Some((ref certs, ref key)) = cert_key {
		let certs: Vec<Certificate> = rustls_pemfile::certs(&mut &certs[..])?
			.into_iter()
			.map(Certificate)
			.collect();
		let key = rustls_pemfile::pkcs8_private_keys(&mut &key[..])?
			.into_iter()
			.map(PrivateKey)
			.next()
			.ok_or_else(|| anyhow!("no TLS private key found"))?;

		let mut tls_config = ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_single_cert(certs, key)?;
		tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

		Some(Arc::new(tls_config))
	} else {
		None
	};

	info!(%http_addr, %https_addr, %rpc_addr, %log_level, store = store.backend_name(), %enable_tls, "Starting links");

	// Create the gRPC service
	let rpc_service = Api::new(store);

	// Start the gRPC API server
	let rpc_service = LinksServer::new(rpc_service).send_gzip().accept_gzip();
	let mut rpc_server = RpcServer::builder();
	if let Some((certs, key)) = cert_key {
		let rpc_tls_config = ServerTlsConfig::new().identity(Identity::from_pem(certs, key));
		rpc_server = rpc_server.tls_config(rpc_tls_config)?
	}

	let rpc_server = rpc_server
		.add_service(InterceptedService::new(
			rpc_service,
			api::get_auth_checker(api_secret),
		))
		.serve(rpc_addr);

	let rpc_handle = spawn(async move {
		// Start the gRPC server and log any server errors during requests
		if let Err(e) = rpc_server.await {
			error!(error = ?e, "RPC server error: {}", e);
		}
	});

	// Start the HTTP server
	let http_listener = TcpListener::bind(http_addr).await?;
	let http_handle = spawn(async move {
		loop {
			let tcp_stream = match http_listener.accept().await {
				Ok((tcp_stream, _)) => tcp_stream,
				Err(tcp_err) => {
					error!(?tcp_err, "Error while accepting HTTP connection");
					continue;
				}
			};

			if redirect_https {
				spawn(async move {
					if let Err(http_err) = Http::new()
						.http1_only(false)
						.http2_only(false)
						.serve_connection(
							tcp_stream,
							service_fn(|req: Request<Body>| https_redirector(req, config)),
						)
						.await
					{
						error!(?http_err, "Error while redirecting HTTP connection");
					}
				});
			} else {
				spawn(async move {
					if let Err(http_err) = Http::new()
						.http1_only(false)
						.http2_only(false)
						.serve_connection(
							tcp_stream,
							service_fn(|req: Request<Body>| redirector(req, store, config)),
						)
						.await
					{
						error!(?http_err, "Error while serving HTTP connection");
					}
				});
			}
		}
	});

	// Start the HTTPS server
	let https_handle = if let Some(tls_config) = tls_config {
		let https_listener = TcpListener::bind(https_addr).await?;
		let https_acceptor = TlsAcceptor::from(tls_config);

		spawn(async move {
			loop {
				let tcp_stream = match https_listener.accept().await {
					Ok((tcp_stream, _)) => tcp_stream,
					Err(tcp_err) => {
						error!(?tcp_err, "Error while accepting HTTPS connection");
						continue;
					}
				};

				let tls_stream = match https_acceptor.accept(tcp_stream).await {
					Ok(tls_stream) => tls_stream,
					Err(tls_err) => {
						error!(?tls_err, "Error while initiating HTTPS connection");
						continue;
					}
				};

				spawn(async move {
					if let Err(https_err) = Http::new()
						.http1_only(false)
						.http2_only(false)
						.serve_connection(
							tls_stream,
							service_fn(|req: Request<Body>| redirector(req, store, config)),
						)
						.await
					{
						error!(?https_err, "Error while serving HTTPS connection");
					}
				});
			}
		})
	} else {
		spawn(async {})
	};

	// Wait until the first unhandled error (if any) and exit
	try_join!(rpc_handle, http_handle, https_handle)?;

	Ok(())
}
