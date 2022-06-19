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

use links::server;
use pico_args::Arguments;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	// Get command line arguments
	let mut args = Arguments::from_env();

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

	// Run the server
	server::run(args, log_level).await
}
