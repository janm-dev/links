//! Links server certificate handling.

use std::{
	fmt::{Debug, Formatter, Result as FmtResult},
	sync::{Arc, RwLock},
};

use links_domainmap::{Domain, DomainMap};
use tokio_rustls::rustls::{
	server::{ClientHello, ResolvesServerCert},
	sign::CertifiedKey,
};
use tracing::debug;

/// A [`ResolvesServerCert`] implementor, resolving TLS certificates based on
/// the domain name using `links-domainmap`. The default certificate for unknown
/// or unrecognized domain names can be specified using `default`.
///
/// [`ResolvesServerCert`]: https://docs.rs/rustls/latest/rustls/server/trait.ResolvesServerCert.html
pub struct CertificateResolver {
	/// The map containing all certificates
	certs: RwLock<DomainMap<Arc<CertifiedKey>>>,
	/// Default certificate/key for unknown and unrecognized domain names
	default: RwLock<Option<Arc<CertifiedKey>>>,
}

impl CertificateResolver {
	/// Create a new empty `CertificateResolver` from a [`CertifiedKey`]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			certs: RwLock::new(DomainMap::new()),
			default: RwLock::new(None),
		}
	}

	/// Get the default `CertifiedKey` if one is configured
	///
	/// # Panics
	/// This function panics if the lock inside of `self` is poisoned
	fn get_default(&self) -> Option<Arc<CertifiedKey>> {
		self.default
			.read()
			.expect("lock is poisoned")
			.as_ref()
			.map(Arc::clone)
	}

	/// Get the matching `CertifiedKey` for the given reference identifier
	/// domain name
	///
	/// # Panics
	/// This function panics if the lock inside of `self` is poisoned
	pub fn get(&self, domain: Option<&Domain>) -> Option<Arc<CertifiedKey>> {
		domain.map_or_else(
			|| self.get_default(),
			|domain| {
				self.certs
					.read()
					.expect("lock is poisoned")
					.get(domain)
					.map_or_else(|| self.get_default(), |certkey| Some(Arc::clone(certkey)))
			},
		)
	}

	/// Set the cert-key pair for the given domain. All future calls to `get` or
	/// `resolve` with this domain name will return this new `CertifiedKey`.
	///
	/// # Panics
	/// This function panics if the lock inside of `self` is poisoned
	pub fn set(&self, domain: Domain, certkey: Arc<CertifiedKey>) {
		self.certs
			.write()
			.expect("lock is poisoned")
			.set(domain, certkey);
	}

	/// Set the default cert-key pair for unknown or unrecognized domains. All
	/// future calls to `get_default` or `resolve` without a domain name or a
	/// domain name not found in any other certificate sources will return this
	/// new `CertifiedKey`. Setting the default certificate to `None` will
	/// reject requests for unknown or unrecognized domains.
	///
	/// # Panics
	/// This function panics if the lock inside of `self` is poisoned
	pub fn set_default(&self, certkey: Option<Arc<CertifiedKey>>) {
		*self.default.write().expect("lock is poisoned") = certkey;
	}

	/// Remove the cert-key pair for the given domain. All future calls to `get`
	/// or `resolve` with this domain name will return nothing.
	///
	/// # Panics
	/// This function panics if the lock inside of `self` is poisoned
	pub fn remove(&self, domain: &Domain) {
		self.certs.write().expect("lock is poisoned").remove(domain);
	}
}

impl Default for CertificateResolver {
	fn default() -> Self {
		Self::new()
	}
}

impl ResolvesServerCert for CertificateResolver {
	fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
		let cert = self.get(
			client_hello
				.server_name()
				.map(Domain::reference)
				.and_then(Result::ok)
				.as_ref(),
		);

		if cert.is_none() {
			debug!(
				"Certificate for {:?} not resolved",
				client_hello.server_name()
			);
		}

		cert
	}
}

impl Debug for CertificateResolver {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		f.debug_struct("CertificateResolver")
			.field("current", &"Arc<[REDACTED]>")
			.finish()
	}
}
