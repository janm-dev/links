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
//! redirect. The server is authenticated with JWTs (TODO). The protocol
//! definition can be found in [`proto/links.proto`](../proto/links.proto).
//!
//! ## The store backend
//! Links can use many (TODO) databases and data stores as store backends,
//! providing flexibility with the storage setup. Currently in-memory,
//! in-memory with file backup (TODO), and redis (TODO) backends are supported.

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use links::api::{Api, LinksServer};
use links::redirector::{redirector, Config};
use links::store::Store;
use std::net::SocketAddr;
use tokio::{net::TcpListener, spawn, try_join};
use tonic::transport::Server as RpcServer;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

const HELP: &str = r#"links server

USAGE:
    server [FLAGS] [OPTIONS] [STORE CONFIG]

FLAGS (all default off):
 -h --help                   Print this and exit
    --disable-hsts           Disable the Strict-Transport-Security header
    --preload-hsts           Enable HSTS preloading and include subdomains (WARNING: Be very careful about enabling this. Requires hsts-age of at least 1 year.)
    --enable-alt-svc         Enable the Alt-Svc header advertising HTTP/2 support on port 443
    --disable-server         Disable the Server HTTP header
    --disable-csp            Disable the Content-Security-Policy header

OPTIONS:
 -s --store STORE            Store type to use ("memory" *)
 -l --log LEVEL              Log level ("trace" / "debug" / "info" * / "warning")
    --hsts-age SECONDS       HSTS header max-age (default 2 years)

STORE CONFIG:
    --store-[CONFIG] VALUE   Store-specific configuration, see the store docs.

* Default value for this option
"#;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	// Parse cli args
	let mut args = pico_args::Arguments::from_env();

	if args.contains(["-h", "--help"]) {
		print!("{}", HELP);
		std::process::exit(0);
	}

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

	// Listen on all addresses, on port 80 (HTTP)
	let http_addr = SocketAddr::from(([0, 0, 0, 0], 80));
	// Listen on all addresses, on port 530 (gRPC)
	let rpc_addr = SocketAddr::from(([0, 0, 0, 0], 530));

	info!(%http_addr, %rpc_addr, %log_level, "Starting links");

	// Initialize the store
	let store = Store::new_static(
		&args
			.opt_value_from_str::<_, String>(["-s", "--store"])?
			.unwrap_or_else(|| "memory".to_string()),
		&mut args,
	)
	.await?;

	// Create the gRPC service
	let rpc_service = Api::new(store);

	// Start the gRPC API server
	let rpc_handle = spawn(async move {
		// Start the gRPC server
		let rpc_server = RpcServer::builder()
			.add_service(LinksServer::new(rpc_service).send_gzip().accept_gzip())
			.serve(rpc_addr);

		// Log any server errors during requests
		if let Err(e) = rpc_server.await {
			error!(error = ?e, "RPC server error: {}", e);
		}
	});

	// Start the HTTP server
	let tcp_listener = TcpListener::bind(http_addr).await?;
	let http_handle = spawn(async move {
		loop {
			let tcp_stream = match tcp_listener.accept().await {
				Ok((tcp_stream, _)) => tcp_stream,
				Err(tcp_err) => {
					error!(?tcp_err, "Error while accepting HTTP connection");
					continue;
				}
			};

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
	});

	// Wait until the first unhandled error (if any) and exit
	try_join!(rpc_handle, http_handle)?;

	Ok(())
}
