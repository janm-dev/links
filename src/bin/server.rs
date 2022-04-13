//! # links server
//!
//! The links server is what actually redirects requests to their proper
//! destinations, interacts with (and sometimes is) the backend store for
//! redirections, and (soon) collects statistics about redirects. It
//! accomplishes this with two (or three) external interfaces: an HTTP/HTTPS
//! server, a gRPC server, and (sometimes) a connection to a backend store.
//!
//! ## The HTTP server
//! Links uses [hyper](https://hyper.rs/) with [(maybe) hyper-rustls] for
//! HTTP/1.0, HTTP/1.1, and HTTP/2. It listens for incoming requests and
//! redirects them (using the 302 (TODO) status code for GET requests and 307
//! for everything else).
//!
//! ## The gRPC server
//! Links runs a gRPC server via [tonic](https://github.com/hyperium/tonic) to
//! provide seamless access to the backend store for tasks such as setting a
//! redirect. The server is authenticated with JWTs (TODO). The protcol
//! definition can be found in [`proto/links.proto`](../proto/links.proto).
//!
//! ## The store backend
//! Links can use many (TODO) databases and datastores as store backends,
//! providing flexibility with the storage setup. Currently in-memory,
//! in-memory with file backup (TODO), and redis (TODO) backends are supported.

use hyper::{server::conn::Http, service::service_fn, Body, Request};
use links::api::{Api, LinksServer};
use links::redirector::redirector;
use links::store::get;
use std::net::SocketAddr;
use tokio::{net::TcpListener, spawn, try_join};
use tonic::transport::Server as RpcServer;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	// Create a tracing subscriber to collect and show logs
	//TODO: make this configurable
	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(Level::INFO)
		.finish();

	// Set the subscriber as the global default so all logs are sent there
	tracing::subscriber::set_global_default(tracing_subscriber)
		.expect("setting tracing default subscriber failed");

	// Listen on all addresses, on port 80 (http)
	let http_addr = SocketAddr::from(([0, 0, 0, 0], 80));
	// Listen on all addresses, on port 530 (gRPC)
	let rpc_addr = SocketAddr::from(([0, 0, 0, 0], 530));

	info!(%http_addr, %rpc_addr, "Starting links");

	// Get and initialize the links store
	//TODO: make this configurable
	let store = get("memory").await?;

	// Create the rpc service
	let rpc_service = Api::new(store);

	// Start the rpc api server
	let rpc_handle = spawn(async move {
		// Start the rpc server
		let rpc_server = RpcServer::builder()
			.add_service(LinksServer::new(rpc_service).send_gzip().accept_gzip())
			.serve(rpc_addr);

		// Log any server errors during requests
		if let Err(e) = rpc_server.await {
			error!(error = ?e, "RPC server error: {}", e);
		}
	});

	// Start the http server
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
						service_fn(|req: Request<Body>| redirector(req, store)),
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
