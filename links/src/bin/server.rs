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
	thread,
	time::{Duration, Instant},
};

use anyhow::anyhow;
use crossbeam_channel::unbounded;
use links::{
	certs::CertificateResolver,
	config::{CertConfigUpdate, CertificateWatcher, Config, LogLevel},
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
	let mut cert_watcher = CertificateWatcher::new()?;
	let (cert_config_updates_tx, cert_config_updates_rx) = unbounded();
	let certs = config.certificates();
	let cert_resolver = Arc::new(CertificateResolver::new());

	for source in certs {
		cert_watcher.send_config_update(CertConfigUpdate::SourceAdded(source.clone()));
		cert_config_updates_tx
			.send(CertConfigUpdate::SourceAdded(source))
			.expect("Certificate configuration update unsuccessful");
	}

	if let default @ DefaultCertificateSource::Some { .. } = config.default_certificate() {
		cert_watcher.send_config_update(CertConfigUpdate::DefaultUpdated(default.clone()));

		cert_config_updates_tx
			.send(CertConfigUpdate::DefaultUpdated(default))
			.expect("Certificate configuration update unsuccessful");
	}

	// Start tokio async runtime
	let rt = Builder::new_multi_thread()
		.enable_all()
		.thread_name_fn(|| {
			static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
			let id = ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
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

	let cert_watcher_updates_tx = cert_watcher.get_config_sender();
	thread::scope(|scope| {
		// The `links-config` thread is responsible for updating the server's
		// configuration when it is changed
		thread::Builder::new()
			.name("links-config".to_string())
			.spawn_scoped(scope, move || loop {
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

				if last_file_event.is_none()
					|| last_file_event.unwrap().elapsed() < watcher_debounce
				{
					continue;
				}

				// Reset file event debouncing timeout
				last_file_event = None;

				// Retain some old config options, then update config
				let old_default_cert = config.default_certificate();
				let old_certs = config.certificates();
				let old_store = (config.store(), config.store_config());
				let old_listeners = config.listeners();
				config.update();
				let new_default_cert = config.default_certificate();
				let new_certs = config.certificates();
				let new_store = (config.store(), config.store_config());
				let new_listeners = config.listeners();

				// If the default TLS certificate source changed, update it
				if old_default_cert != new_default_cert {
					debug!("Updating default certificate source");

					cert_watcher_updates_tx
						.send(CertConfigUpdate::DefaultUpdated(new_default_cert.clone()))
						.expect("Certificate configuration update unsuccessful");

					cert_config_updates_tx
						.send(CertConfigUpdate::DefaultUpdated(new_default_cert))
						.expect("Certificate configuration update unsuccessful");
				}

				// If TLS certificate sources changed, update them
				if old_certs != new_certs {
					debug!("Updating certificate sources");

					// Unwatch and remove removed sources
					for source in old_certs.iter().filter(|c| !new_certs.contains(c)) {
						debug!(
							?source,
							"Removing certificate source for [{}]",
							source
								.domains
								.iter()
								.map(ToString::to_string)
								.collect::<Vec<_>>()
								.join(", ")
						);

						cert_watcher_updates_tx
							.send(CertConfigUpdate::SourceRemoved(source.clone()))
							.expect("Certificate configuration update unsuccessful");

						cert_config_updates_tx
							.send(CertConfigUpdate::SourceRemoved(source.clone()))
							.expect("Certificate configuration update unsuccessful");
					}

					// Watch added sources and add their certs/keys
					for source in new_certs.iter().filter(|c| !old_certs.contains(c)) {
						debug!(
							?source,
							"Adding certificate source for [{}]",
							source
								.domains
								.iter()
								.map(ToString::to_string)
								.collect::<Vec<_>>()
								.join(", ")
						);

						cert_watcher_updates_tx
							.send(CertConfigUpdate::SourceAdded(source.clone()))
							.expect("Certificate configuration update unsuccessful");

						cert_config_updates_tx
							.send(CertConfigUpdate::SourceAdded(source.clone()))
							.expect("Certificate configuration update unsuccessful");
					}
				} else {
					debug!("Certificate config not changed, continuing with existing cert sources");
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

				info!(?config, "Configuration reloaded");
			})
			.expect("error spawning configuration-reloading thread");

		// The `links-cert-updates` thread is responsible for updating the certificate
		// resolver when the underlying certificate sources are updated
		let resolver = Arc::clone(&cert_resolver);
		thread::Builder::new()
			.name("links-cert-updates".to_string())
			.spawn_scoped(scope, move || loop {
				let (sources, default) = cert_watcher.watch(watcher_debounce);
				debug!(?sources, "Certificate source update received from watcher");

				if let Some(default) = default.into_cs() {
					debug!(?default, "Updating default certificate");

					match default.get_certkey() {
						Ok(ck) => resolver.set_default(Some(Arc::new(ck))),
						Err(err) => error!(%err, "Couldn't get default TLS certificate / key"),
					}
				}

				for source in sources {
					debug!(?source, "Updating certificate source");

					let certkey = match source.get_certkey().map(Arc::new) {
						Ok(certkey) => certkey,
						Err(error) => {
							error!(%error, "Couldn't get TLS certificate / key");
							continue;
						}
					};

					for domain in source.domains {
						debug!("Updating certificate for {domain}");
						resolver.set(domain, Arc::clone(&certkey));
					}
				}

				info!("TLS certificates reloaded");
			})
			.expect("error spawning certificate-reloading thread");

		// The `links-cert-reconfig` thread is responsible for updating the certificate
		// resolver when certificate source configuration is updated
		thread::Builder::new()
			.name("links-cert-reconfig".to_string())
			.spawn_scoped(scope, move || loop {
				let update = cert_config_updates_rx
					.recv()
					.expect("Certificate configuration channel closed");
				debug!(?update, "Certificate source config update received");

				match update {
					CertConfigUpdate::DefaultUpdated(default) => {
						if let Some(source) = default.into_cs() {
							match source.get_certkey() {
								Ok(cert) => {
									cert_resolver.set_default(Some(Arc::new(cert)));
									info!(?source, "Default certificate updated");
								}
								Err(err) => {
									error!(%err, "Error updating default certificate");
								}
							}
						} else {
							cert_resolver.set_default(None);
							info!("Default certificate removed");
						}
					}
					CertConfigUpdate::SourceAdded(source) => {
						match source.get_certkey().map(Arc::new) {
							Ok(certkey) => {
								for domain in &source.domains {
									debug!("Setting certificate for {domain}");
									cert_resolver.set(domain.clone(), Arc::clone(&certkey));
								}

								info!(?source, "Certificate updated");
							}
							Err(err) => {
								error!(%err, ?source, "Error updating certificate");
							}
						}
					}
					CertConfigUpdate::SourceRemoved(source) => {
						for domain in &source.domains {
							debug!("Removing certificate for {domain}");
							cert_resolver.remove(domain);
						}

						info!(?source, "Certificate removed");
					}
				}
			})
			.expect("error spawning certificate-reloading thread");

		info!(%config, "Links redirector server started");
	});

	unreachable!("The server stopped unexpectedly")
}
