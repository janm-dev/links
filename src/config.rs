//! Links server certificate handling.

use std::{
	fmt::{Debug, Formatter},
	fs,
	io::Error as IoError,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		mpsc::{self, RecvTimeoutError},
		Arc,
	},
	thread,
	time::Duration,
};

use arc_swap::ArcSwap;
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use tokio::task::spawn_blocking;
use tokio_rustls::rustls::{
	server::{ClientHello, ResolvesServerCert},
	sign::{self, CertifiedKey, SignError},
	Certificate, PrivateKey,
};
use tracing::{debug, error, info};

/// The error returned by [`get_certkey`].
#[derive(Debug, thiserror::Error)]
enum CertKeyError {
	#[error("the certificate or key file could not be read")]
	Read(#[from] IoError),
	#[error("the private key file does not contain a valid private key")]
	NoKey,
	#[error("the private key is invalid or unsupported")]
	InvalidKey(#[from] SignError),
}

/// Read a `CertifiedKey` from cert and key files. Note that all file IO
/// performed in this function is blocking.
///
/// # Errors
/// This function returns an error if:
/// - The certificate or key could not be read from their files
/// - The certificate or key could not be parsed or are otherwise invalid
/// - The certificate and key don't match (TODO)
fn get_certkey(
	cert_path: impl AsRef<Path>,
	key_path: impl AsRef<Path>,
) -> Result<CertifiedKey, CertKeyError> {
	let certs = fs::read(&cert_path)?;
	let key = fs::read(&key_path)?;

	let certs: Vec<Certificate> = rustls_pemfile::certs(&mut &certs[..])?
		.into_iter()
		.map(Certificate)
		.collect();
	let key = rustls_pemfile::pkcs8_private_keys(&mut &key[..])?
		.into_iter()
		.map(PrivateKey)
		.next()
		.ok_or(CertKeyError::NoKey)?;

	let cert_key = CertifiedKey::new(certs, sign::any_supported_type(&key)?);

	// Check if the certificate matches the key
	// TODO: Waiting on <https://github.com/rustls/rustls/issues/618>,
	// TODO: <https://github.com/briansmith/webpki/issues/35>,
	// TODO: <https://github.com/briansmith/ring/issues/419>,
	// TODO: or similar.

	Ok(cert_key)
}

/// The delay to use for grouping events within the file change watcher. Should
/// be slightly longer than the longest period between the start of writing the
/// first TLS-related file (cert or key) and the end of writing the other file.
const WATCHER_DELAY: Duration = Duration::from_secs(10);

/// The event-receipt-timeout to use. This is the time between
/// [`terminator`][CertificateResolver] checks in the file-watching thread.
const WATCHER_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`ResolvesServerCert`](https://docs.rs/rustls/latest/rustls/server/trait.ResolvesServerCert.html)
/// implementation, resolving a single `CertifiedKey`, updated from certificate
/// and key files. The files are watched on a separate thread, and updated when
/// they are changed.
pub struct CertificateResolver {
	/// Whether the file-watching thread should stop.
	terminator: Arc<AtomicBool>,
	/// Current [`CertifiedKey`] value.
	/// May be up to [`WATCHER_DELAY`] out of date.
	current: Arc<ArcSwap<CertifiedKey>>,
}

impl CertificateResolver {
	/// Create a new `CertificateResolver` from the key and cert paths. The
	/// provided paths will be read from to get the initial `CertifiedKey`, and
	/// will then be watched for changes on a newly spawned thread. When the
	/// TLS cert or key files change, the stored certified key will be updated.
	///
	/// # Errors
	/// This function returns an error if:
	/// - The file watcher could not be instantiated
	/// - The file watcher could not watch either of the file paths provided
	/// - There was an issue with the certificate or key (see [`get_certkey`])
	pub async fn new<P: Into<PathBuf> + Send>(key_path: P, cert_path: P) -> anyhow::Result<Self> {
		let key_path = key_path.into();
		let cert_path = cert_path.into();

		let (watcher_tx, watcher_rx) = mpsc::channel();
		let mut watcher = notify::watcher(watcher_tx, WATCHER_DELAY)?;

		watcher.watch(&key_path, RecursiveMode::NonRecursive)?;
		watcher.watch(&cert_path, RecursiveMode::NonRecursive)?;

		let key = key_path.clone();
		let cert = cert_path.clone();
		let cert_key = spawn_blocking(move || get_certkey(&cert, &key)).await??;

		let cert_key = Arc::new(ArcSwap::from_pointee(cert_key));
		let current = Arc::clone(&cert_key);

		let terminator = Arc::new(AtomicBool::new(false));
		let terminate = Arc::clone(&terminator);
		thread::spawn(
			#[allow(clippy::cognitive_complexity)]
			move || {
				info!("TLS cert and key file watcher starting");

				while !terminate.load(Ordering::Relaxed) {
					match watcher_rx.recv_timeout(WATCHER_TIMEOUT) {
						Ok(DebouncedEvent::Write(path)) => {
							info!(
								"Detected write to file \"{}\", updating TLS cert and key",
								path.to_string_lossy()
							);

							let new = match get_certkey(&cert_path, &key_path) {
								Ok(ck) => ck,
								Err(e) => {
									error!("Error while updating TLS certificate and key: {e}");
									continue;
								}
							};

							cert_key.store(Arc::new(new));

							info!("Successfully updated TLS certificate and key");
						}
						Err(RecvTimeoutError::Timeout) => {
							debug!("Still watching for changes to TLS cert and key files");
						}
						Err(e) => {
							error!("Error while waiting for changes to TLC cert/key files: {e}");
						}
						_ => (),
					}
				}

				info!("Terminating TLS cert and key file watcher");
				drop(watcher);
			},
		);

		Ok(Self {
			terminator,
			current,
		})
	}
}

impl ResolvesServerCert for CertificateResolver {
	fn resolve(&self, _client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
		Some(self.current.load_full())
	}
}

impl Drop for CertificateResolver {
	fn drop(&mut self) {
		self.terminator.store(true, Ordering::Relaxed);
	}
}

impl Debug for CertificateResolver {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CertificateResolver")
			.field("terminator", &self.terminator)
			.field("current", &"Arc<[REDACTED]>")
			.finish()
	}
}
