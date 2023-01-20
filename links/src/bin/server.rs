//! # links server
//!
//! The links server is what actually redirects requests to their proper
//! destinations, interacts with (and sometimes is) the backend store for
//! redirections, and collects statistics about redirects. It accomplishes this
//! with two (or three) external interfaces: an HTTP server, a RPC server, and
//! (usually) a connection to a backend store.
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
	time::{Duration, Instant},
};

use anyhow::anyhow;
use links::{
	certs::{get_certkey, CertificateResolver},
	config::{Config, LogLevel, Tls},
	server::{
		store_setup, Listener, PlainHttpAcceptor, PlainRpcAcceptor, Protocol, TlsHttpAcceptor,
		TlsRpcAcceptor,
	},
	store::Current,
	util::{stringify_map, SERVER_HELP, SERVER_NAME},
};
use notify::{RecursiveMode, Watcher};
use pico_args::Arguments;
use tokio::runtime::Builder;
use tracing::{debug, error, info, Level};
use tracing_subscriber::{filter::DynFilterFn, prelude::*, FmtSubscriber};

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

	info!(server = SERVER_NAME, "Starting links");

	// Parse cli args
	let mut args = Arguments::from_env();

	if args.contains(["-h", "--help"]) {
		println!("{SERVER_HELP}");
		Err(anyhow!(""))?;
	}

	info!("Getting server configuration");

	// Configure the server
	let config = Config::new_static(args.opt_value_from_str(["-c", "--config"])?);

	debug!(?config, "Server configuration parsed");

	// Set a tracing filter which can change the minimum log level on the fly.
	let tracing_filter = DynFilterFn::new(move |metadata, _| {
		let log_level = config.log_level();
		let level = metadata.level();
		if log_level == LogLevel::Verbose {
			let module = metadata.module_path();
			level <= &Level::INFO
				|| (module.is_some()
					&& (module.unwrap().starts_with("links::") || module.unwrap() == "server")
					&& level <= &Level::DEBUG)
		} else {
			level <= &Level::from(log_level)
		}
	});

	// Create the permanent global tracing subscriber to collect and show logs
	let (non_blocking, _tracing_appender_guard) = tracing_appender::non_blocking(std::io::stdout());
	let tracing_subscriber = FmtSubscriber::builder()
		.with_level(true)
		.with_max_level(Level::TRACE)
		.with_writer(non_blocking)
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
	let mut listeners = Vec::new();

	for addr in config.listeners() {
		listeners.push(match addr.protocol {
			Protocol::Http => {
				rt.block_on(Listener::new(addr.address, addr.port, plain_http_acceptor))?
			}
			Protocol::Https => {
				rt.block_on(Listener::new(addr.address, addr.port, tls_http_acceptor))?
			}
			Protocol::Grpc => {
				rt.block_on(Listener::new(addr.address, addr.port, plain_rpc_acceptor))?
			}
			Protocol::Grpcs => {
				rt.block_on(Listener::new(addr.address, addr.port, tls_rpc_acceptor))?
			}
		})
	}

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

	if let Some(config_file) = config.file() {
		file_watcher.watch(config_file, RecursiveMode::NonRecursive)?;
	}

	if let Tls::Enable {
		key_file,
		cert_file,
	} = config.tls()
	{
		file_watcher.watch(&key_file, RecursiveMode::NonRecursive)?;
		file_watcher.watch(&cert_file, RecursiveMode::NonRecursive)?;
	}

	let mut last_file_event = None;
	let watcher_timeout = Duration::from_millis(
		args.opt_value_from_str("--watcher-timeout")
			.unwrap_or_default()
			.unwrap_or(10000u64),
	);

	let watcher_debounce = Duration::from_millis(
		args.opt_value_from_str("--watcher-debounce")
			.unwrap_or_default()
			.unwrap_or(1000u64),
	);

	// During coverage-collecting tests, in order to collect correct coverage
	// data, use stdin to stop the server instead of relying on a kill signal,
	// which also stops coverage reporting
	#[cfg(coverage)]
	let rx = {
		use std::{
			io::{self, Read},
			thread,
		};

		let (tx, rx) = mpsc::channel::<u8>();

		thread::spawn(move || {
			let mut buf = [0u8];
			io::stdin().read_exact(&mut buf[..]).unwrap();
			tx.send(buf[0]).unwrap();
		});

		rx
	};

	info!(%config, "Links redirector server started");

	loop {
		match watcher_rx.recv_timeout(if last_file_event.is_none() {
			watcher_timeout
		} else {
			watcher_debounce.min(watcher_timeout) / 4
		}) {
			Ok(event) => {
				debug!(?event, "Received file event from watcher");
				last_file_event = Some(Instant::now());
			}
			Err(RecvTimeoutError::Disconnected) => error!("File watching error"),
			Err(RecvTimeoutError::Timeout) => (),
		}

		if last_file_event.is_some() && last_file_event.unwrap().elapsed() > watcher_debounce {
			// Reset file event debouncing timeout
			last_file_event = None;

			// Retain some old config options, then update config
			let old_tls = config.tls();
			let old_store = (config.store(), config.store_config());
			let old_listeners = config.listeners();
			config.update();
			let new_tls = config.tls();
			let new_store = (config.store(), config.store_config());
			let new_listeners = config.listeners();

			// If TLS file paths changed, watch those instead of the old ones
			if old_tls != new_tls {
				if let Tls::Enable {
					key_file,
					cert_file,
				} = old_tls
				{
					match file_watcher.unwatch(&key_file) {
						Ok(_) => (),
						Err(err) => error!(?err, "File watching error"),
					}
					match file_watcher.unwatch(&cert_file) {
						Ok(_) => (),
						Err(err) => error!(?err, "File watching error"),
					}
				}

				if let Tls::Enable {
					key_file,
					cert_file,
				} = new_tls
				{
					info!(
						"Using new TLS files: {} (cert) and {} (key)",
						cert_file.to_string_lossy(),
						key_file.to_string_lossy()
					);

					match file_watcher.watch(&key_file, RecursiveMode::NonRecursive) {
						Ok(_) => (),
						Err(err) => error!(?err, "File watching error"),
					}
					match file_watcher.watch(&cert_file, RecursiveMode::NonRecursive) {
						Ok(_) => (),
						Err(err) => error!(?err, "File watching error"),
					}
				}
			}

			// Update the cert resolver with new TLS cert and key
			if let Tls::Enable {
				key_file,
				cert_file,
			} = config.tls()
			{
				info!("Updating TLS certificate and key");
				cert_resolver.update(get_certkey(cert_file, key_file).ok().map(Arc::new));
			} else {
				info!("TLS is disabled, removing any old certificates");
				cert_resolver.update(None);
			}

			// If the store type or config changed, create a new store to replace the
			// existing one
			if old_store != new_store {
				info!(
					"Updating store: {} ({})",
					new_store.0,
					stringify_map(&new_store.1)
				);

				match rt.block_on(store_setup(config, false)) {
					Ok(store) => current_store.update(store),
					Err(err) => {
						error!(?err, "Error creating new store, retaining old store")
					}
				}
			} else {
				debug!("Store config not changed, continuing with existing store");
			}

			// Update listeners per the new config
			listeners.retain(|l| new_listeners.contains(&l.listen_address()));

			for addr in new_listeners {
				if !old_listeners.contains(&addr) {
					listeners.push(match addr.protocol {
						Protocol::Http => {
							match rt.block_on(Listener::new(
								addr.address,
								addr.port,
								plain_http_acceptor,
							)) {
								Ok(listener) => listener,
								Err(err) => {
									error!("Error creating new listener on \"{addr}\": {err}");
									continue;
								}
							}
						}
						Protocol::Https => match rt.block_on(Listener::new(
							addr.address,
							addr.port,
							tls_http_acceptor,
						)) {
							Ok(listener) => listener,
							Err(err) => {
								error!("Error creating new listener on \"{addr}\": {err}");
								continue;
							}
						},
						Protocol::Grpc => match rt.block_on(Listener::new(
							addr.address,
							addr.port,
							plain_rpc_acceptor,
						)) {
							Ok(listener) => listener,
							Err(err) => {
								error!("Error creating new listener on \"{addr}\": {err}");
								continue;
							}
						},
						Protocol::Grpcs => match rt.block_on(Listener::new(
							addr.address,
							addr.port,
							tls_rpc_acceptor,
						)) {
							Ok(listener) => listener,
							Err(err) => {
								error!("Error creating new listener on \"{addr}\": {err}");
								continue;
							}
						},
					})
				}
			}

			debug!(
				"Updated listeners, currently active: {:?}",
				listeners
					.iter()
					.map(|l| l.listen_address())
					.collect::<Vec<_>>()
			);

			info!("Configuration and TLS cert/key reloaded");
		}

		#[cfg(coverage)]
		{
			use std::sync::mpsc::TryRecvError;

			match rx.try_recv() {
				Ok(b) if b == b'x' => {
					tracing::warn!("Stopping server");
					return Ok(());
				}
				Err(TryRecvError::Disconnected) => panic!("Server stopping listening error"),
				_ => (),
			}
		}
	}
}
