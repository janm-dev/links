//! Links server certificate handling.

use std::{
	fmt::{Debug, Formatter, Result as FmtResult},
	fs,
	io::Error as IoError,
	path::Path,
	sync::Arc,
};

use parking_lot::RwLock;
use tokio_rustls::rustls::{
	server::{ClientHello, ResolvesServerCert},
	sign::{self, CertifiedKey, SignError},
	Certificate, PrivateKey,
};
use tracing::error;

/// The error returned by [`get_certkey`].
#[derive(Debug, thiserror::Error)]
pub enum CertKeyError {
	/// The certificate or key file could not be read
	#[error("the certificate or key file could not be read")]
	Read(#[from] IoError),
	/// The private key file does not contain a valid private key
	#[error("the private key file does not contain a valid private key")]
	NoKey,
	/// The private key is invalid or unsupported
	#[error("the private key is invalid or unsupported")]
	InvalidKey(#[from] SignError),
}

/// Read a `CertifiedKey` from cert and key files.
///
/// # IO
/// This function performs synchronous (blocking) file IO.
///
/// # Errors
/// This function returns an error if:
/// - The certificate or key could not be read from their files
/// - The certificate or key could not be parsed or are otherwise invalid
/// - The certificate and key don't match (TODO)
pub fn get_certkey(
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

/// A [`ResolvesServerCert`] implementation, resolving a single `CertifiedKey`,
/// updatable on the fly. When this is used with the [`ResolvesServerCert`]
/// trait and the current certificate is `None`, the TLS handshake will be
/// aborted.
///
/// [`ResolvesServerCert`]: https://docs.rs/rustls/latest/rustls/server/trait.ResolvesServerCert.html
pub struct CertificateResolver {
	/// Current [`CertifiedKey`] value
	current: RwLock<Option<Arc<CertifiedKey>>>,
}

impl CertificateResolver {
	/// Create a new `CertificateResolver` from a [`CertifiedKey`]. The provided
	/// cert-key pair will be returned by calls to `get` or `resolve` (via the
	/// `ResolvesServerCert` trait), and can be replaced using `update`.
	#[must_use]
	pub const fn new(certkey: Option<Arc<CertifiedKey>>) -> Self {
		Self {
			current: RwLock::new(certkey),
		}
	}

	/// Get the current `CertifiedKey`
	pub fn get(&self) -> Option<Arc<CertifiedKey>> {
		self.current.read().clone()
	}

	/// Update the stored cert-key pair. All future calls to `get` will return
	/// this new `CertifiedKey`.
	pub fn update(&self, certkey: Option<Arc<CertifiedKey>>) {
		*self.current.write() = certkey;
	}
}

impl ResolvesServerCert for CertificateResolver {
	fn resolve(&self, _client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
		self.get()
	}
}

impl Debug for CertificateResolver {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		f.debug_struct("CertificateResolver")
			.field("current", &"Arc<[REDACTED]>")
			.finish()
	}
}
