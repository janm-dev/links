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

use std::{
	sync::{
		atomic::{AtomicUsize, Ordering},
		mpsc::{self, RecvTimeoutError},
		Arc,
	},
	time::Duration,
};

use anyhow::anyhow;
use links::{
	certs::{get_certkey, CertificateResolver},
	config::{Config, Tls},
	server::{
		store_setup, Listener, PlainHttpAcceptor, PlainRpcAcceptor, TlsHttpAcceptor, TlsRpcAcceptor,
	},
	store::Current,
	util::{SERVER_HELP, SERVER_NAME},
};
use notify::{RecursiveMode, Watcher};
use pico_args::Arguments;
use tokio::runtime::Builder;
use tracing::{debug, error, info, Level};
use tracing_subscriber::{filter::FilterFn, prelude::*, FmtSubscriber};

#[cfg(not(coverage))]
const WATCHER_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(coverage)]
const WATCHER_TIMEOUT: Duration = Duration::from_millis(50);

/// Run the links redirector server using configuration from the provided
/// command line arguments. This is essentially the entire server binary, but
/// exposed via `lib.rs` to aid in integration tests.
///
/// # Errors
/// Returns an error if setup fails, or an unexpected and unrecoverable runtime
/// error occurs.
fn main() -> Result<(), anyhow::Error> {
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

	let server = &*SERVER_NAME;
	info!(%server, "Starting links");

	// Parse cli args
	let mut args = Arguments::from_env();

	if args.contains(["-h", "--help"]) {
		println!("{}", SERVER_HELP);
		Err(anyhow!(""))?;
	}

	info!("Getting server configuration");

	// Configure the server
	let config = Config::new_static(args.opt_value_from_str(["-c", "--config"])?);

	debug!(?config, "Server configuration parsed");

	// Set a tracing filter which can change the minimum log level on the fly.
	let tracing_filter = FilterFn::new(move |metadata| metadata.level() <= &config.log_level());

	// Create the permanent global tracing subscriber to collect and show logs
	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(Level::TRACE)
		.finish()
		.with(tracing_filter);

	drop(subscriber_guard);
	tracing::subscriber::set_global_default(tracing_subscriber)
		.expect("setting tracing default subscriber failed");

	// Set up the TLS certificate resolver
	let cert_resolver = if let Tls::Enable {
		key_file,
		cert_file,
	} = config.tls()
	{
		debug!(
			"Using cert file: \"{}\", key file \"{}\"",
			cert_file.to_string_lossy(),
			key_file.to_string_lossy()
		);

		let certkey = get_certkey(cert_file, key_file)?;

		Arc::new(CertificateResolver::new(Some(Arc::new(certkey))))
	} else {
		Arc::new(CertificateResolver::new(None))
	};

	// Start tokio async runtime
	let rt = Builder::new_multi_thread()
		.enable_all()
		.thread_name_fn(|| {
			static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
			let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
			format!("links-worker-{id:#04x}")
		})
		.build()
		.expect("async runtime initialization");

	// Initialize the store
	let store = rt.block_on(store_setup(config, args.contains("--example-redirect")))?;
	let current_store = Current::new_static(store);

	// Initialize all acceptors
	let plain_http_acceptor = PlainHttpAcceptor::new(config, current_store);
	let tls_http_acceptor = TlsHttpAcceptor::new(config, current_store, cert_resolver.clone());
	let plain_rpc_acceptor = PlainRpcAcceptor::new(config, current_store);
	let tls_rpc_acceptor = TlsRpcAcceptor::new(config, current_store, cert_resolver.clone());

	// Set up listeners
	let _listeners = vec![
		rt.block_on(Listener::new(([0, 0, 0, 0], 80), plain_http_acceptor))?,
		rt.block_on(Listener::new(([0, 0, 0, 0], 443), tls_http_acceptor))?,
		if let Tls::Disable = config.tls() {
			rt.block_on(Listener::new(([0, 0, 0, 0], 530), plain_rpc_acceptor))?
		} else {
			rt.block_on(Listener::new(([0, 0, 0, 0], 530), tls_rpc_acceptor))?
		},
	];

	let (watcher_tx, watcher_rx) = mpsc::channel();
	let mut file_watcher = notify::recommended_watcher(move |res| match res {
		Ok(event) => {
			if let Err(err) = watcher_tx.send(event) {
				error!(?err, "File watching error");
			};
		}
		Err(err) => {
			error!(?err, "File watching error");
		}
	})?;

	if let Tls::Enable {
		key_file,
		cert_file,
	} = config.tls()
	{
		file_watcher.watch(&key_file, RecursiveMode::NonRecursive)?;
		file_watcher.watch(&cert_file, RecursiveMode::NonRecursive)?;
	}

	info!(%config, "Links redirector server started");

	loop {
		match watcher_rx.recv_timeout(WATCHER_TIMEOUT) {
			Ok(event) => {
				debug!(?event, "Received file event from watcher");

				if let Tls::Enable {
					key_file,
					cert_file,
				} = config.tls()
				{
					info!("Updating TLS certificate and key");
					cert_resolver.update(get_certkey(&cert_file, &key_file).ok().map(Arc::new));
				} else {
					debug!("Not updating TLS certificate and key because TLS is disabled")
				}
			}
			Err(RecvTimeoutError::Disconnected) => error!("File watching error"),
			Err(RecvTimeoutError::Timeout) => (),
		}

		// During coverage-collecting tests, in order to collect correct coverage
		// data, use stdin to stop the server instead of relying on a kill signal,
		// which also stops coverage reporting
		#[cfg(coverage)]
		{
			use std::io::{self, Read};

			use tracing::warn;

			// Wait until "x" is received on stdin, then stop the server
			let mut buf = [0u8];
			io::stdin().read_exact(&mut buf[..]).unwrap();

			if buf == *b"x" {
				warn!("Stopping server");
				break Ok(());
			}
		}
	}
}
