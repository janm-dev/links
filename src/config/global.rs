//! Global redirector server configuration.

use std::{collections::HashMap, fmt::Display, path::PathBuf, sync::Arc};

use parking_lot::RwLock;
use rand::{distributions::Alphanumeric, Rng};
use tracing::{debug, instrument, warn, Level};

use crate::{config::partial::Partial, store::BackendType, util::A_YEAR};

/// Global configuration for the links redirector server. This is the more
/// idiomatic, easier to use (in rust code), and shareable-across-threads
/// version, which can be updated from a [`Partial`].
#[derive(Debug)]
pub struct Config {
	inner: RwLock<ConfigInner>,
	file: Option<PathBuf>,
}

impl Config {
	/// Create a new `Config` instance using the provided file path as the
	/// configuration file. Configuration data is parsed from environment
	/// variables, the config file, and command-line arguments, in that order.
	/// If there is an error with the configuration file or any other
	/// configuration source, no error is emitted. Instead, a warning is logged,
	/// and the other configuration sources are used.
	///
	/// # IO
	/// This function performs synchronous file IO, and should therefore not be
	/// used inside of an asynchronous context.
	#[must_use]
	pub fn new(file: Option<PathBuf>) -> Self {
		let config = ConfigInner::default();

		let config = Self {
			inner: RwLock::new(config),
			file,
		};
		config.update();
		config
	}

	/// Create a new static reference to a new `Config` instance using the
	/// provided file path as the configuration file. Configuration data is
	/// parsed from environment variables, the config file, and command-line
	/// arguments, in that order. If there is an error with the configuration
	/// file or any other configuration source, no error is emitted. Instead, a
	/// warning is logged, and the other configuration sources are used.
	///
	/// # Memory
	/// Because this function leaks memory with no (safe) way of freeing it,
	/// care should be taken not to call this function an unbounded number of
	/// times.
	///
	/// # IO
	/// This function performs synchronous file IO, and should therefore not be
	/// used inside of an asynchronous context.
	#[must_use]
	pub fn new_static(file: Option<PathBuf>) -> &'static Self {
		Box::leak(Box::new(Self::new(file)))
	}

	/// Update this config from environment variables, config file, and
	/// command-line arguments. This function starts with defaults for each
	/// option, then updates those from environment variables, then from the
	/// config file, then from command-line arguments, and finally overwrites
	/// this `Config`'s options with those newly-parsed ones.
	///
	/// # IO
	/// This function performs synchronous file IO, and should therefore not be
	/// used inside of an asynchronous context.
	#[instrument]
	pub fn update(&self) {
		let mut config = ConfigInner::default();

		config.update_from_partial(&Partial::from_env_vars());

		if let Some(ref file) = *self.file() {
			match Partial::from_file(file) {
				Ok(partial) => config.update_from_partial(&partial),
				Err(err) => warn!("Could not read configuration from file: {err}"),
			}
		}

		config.update_from_partial(&Partial::from_args());

		debug!(new_config = ?config, "Configuration reloaded");

		*self.inner.write() = config;
	}

	/// Generate a redirector configuration from the options defined in this
	/// global links config.
	#[must_use]
	pub fn redirector(&self) -> Redirector {
		Redirector {
			hsts: self.hsts(),
			send_alt_svc: self.send_alt_svc(),
			send_server: self.send_server(),
			send_csp: self.send_csp(),
		}
	}

	/// Get the configured log level
	#[must_use]
	pub fn log_level(&self) -> Level {
		self.inner.read().log_level
	}

	/// Get the RPC API token
	#[must_use]
	pub fn token(&self) -> Arc<str> {
		Arc::clone(&self.inner.read().token)
	}

	/// Get the TLS configuration
	#[must_use]
	pub fn tls(&self) -> Tls {
		self.inner.read().tls.clone()
	}

	/// Get the `hsts` configuration option
	#[must_use]
	pub fn hsts(&self) -> Hsts {
		self.inner.read().hsts
	}

	/// Get the `send_alt_svc` configuration option
	#[must_use]
	pub fn send_alt_svc(&self) -> bool {
		self.inner.read().send_alt_svc
	}

	/// Get the `send_server` configuration option
	#[must_use]
	pub fn send_server(&self) -> bool {
		self.inner.read().send_server
	}

	/// Get the `send_csp` configuration option
	#[must_use]
	pub fn send_csp(&self) -> bool {
		self.inner.read().send_csp
	}

	/// Get the store type
	#[must_use]
	pub fn store(&self) -> BackendType {
		self.inner.read().store
	}

	/// Get the store backend configuration
	#[must_use]
	pub fn store_config(&self) -> HashMap<String, String> {
		self.inner.read().store_config.clone()
	}

	/// Get the configuration file path
	#[must_use]
	const fn file(&self) -> &Option<PathBuf> {
		&self.file
	}
}

impl Display for Config {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_struct("Config")
			.field("log_level", &(self.log_level()).to_string())
			.field(
				"token",
				&(self.token())
					.chars()
					.take(3)
					.chain("...".chars())
					.collect::<String>(),
			)
			.field("tls", &self.tls())
			.field("hsts", &self.hsts())
			.field("send_alt_svc", &self.send_alt_svc())
			.field("send_server", &self.send_server())
			.field("send_csp", &self.send_csp())
			.field("store", &self.store())
			.field("store_config", &self.store_config())
			.field("file", &self.file())
			.finish()
	}
}

/// Actual configuration storage inside of a [`Config`]
#[derive(Debug)]
struct ConfigInner {
	/// Minimum level of logs to be collected/displayed. Debug and trace levels
	/// may expose secret information, so are not recommended for production
	/// deployments.
	pub log_level: Level,
	/// API token, used for authentication of gRPC clients
	pub token: Arc<str>,
	/// TLS configuration (HTTPS and gRPC)
	pub tls: Tls,
	/// HTTP Strict Transport Security setting on redirect
	pub hsts: Hsts,
	/// Send the `Alt-Svc` header advertising `h2` (HTTP/2.0 with TLS) support
	/// on port 443
	pub send_alt_svc: bool,
	/// Send the `Server` header
	pub send_server: bool,
	/// Send the `Content-Security-Policy` header
	pub send_csp: bool,
	/// The store backend type
	pub store: BackendType,
	/// The store backend configuration
	pub store_config: HashMap<String, String>,
}

impl ConfigInner {
	/// Update the config from a [`Partial`]. This overwrites all fields
	/// of this [`Config`] from the provided [`Partial`], if they are set in
	/// that partial config.
	fn update_from_partial(&mut self, partial: &Partial) {
		if let Some(log_level) = partial.log_level() {
			self.log_level = log_level;
		}

		if let Some(ref token) = partial.token {
			self.token = Arc::from(token.as_str());
		}

		if let Some(tls) = partial.tls() {
			self.tls = tls;
		}

		if let Some(hsts) = partial.hsts() {
			self.hsts = hsts;
		}

		if let Some(send_alt_svc) = partial.send_alt_svc {
			self.send_alt_svc = send_alt_svc;
		}

		if let Some(send_server) = partial.send_server {
			self.send_server = send_server;
		}

		if let Some(send_csp) = partial.send_csp {
			self.send_csp = send_csp;
		}

		if let Some(store) = partial.store {
			self.store = store;
		}

		if let Some(ref store_config) = partial.store_config {
			self.store_config
				.extend(store_config.iter().map(|(k, v)| (k.clone(), v.clone())));
		}
	}
}

impl Default for ConfigInner {
	fn default() -> Self {
		Self {
			log_level: Level::INFO,
			token: rand::thread_rng()
				.sample_iter(&Alphanumeric)
				.take(32)
				.map(char::from)
				.collect::<String>()
				.into(),
			tls: Tls::default(),
			hsts: Hsts::default(),
			send_alt_svc: false,
			send_server: true,
			send_csp: true,
			store: BackendType::default(),
			store_config: HashMap::with_capacity(0),
		}
	}
}

/// Configuration of a redirector. Can be generated from a [`Config`]. This is
/// separate from the actual `Config`, because it shouldn't/can't change during
/// the course of processing a redirect.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Redirector {
	/// HTTP Strict Transport Security configuration
	pub hsts: Hsts,
	/// Send the `Alt-Svc` header advertising `h2` (HTTP/2.0 with TLS) support
	/// on port 443
	pub send_alt_svc: bool,
	/// Send the `Server` header
	pub send_server: bool,
	/// Send the `Content-Security-Policy` header
	pub send_csp: bool,
}

/// HTTP Strict Transport Security configuration settings and `max-age` in
/// seconds for the links redirector.
///
/// The `max-age` indicates for how long the server's HSTS setting should be
/// saved by browsers, with 2 years (63072000 seconds) being recommended. For
/// preloading to work, `max-age` must be at least 1 year (31536000 seconds).
/// Setting `max-age` to 0 will clear a browser's HSTS setting for the
/// redirection website on next request, allowing it to perform HTTP (without
/// TLS) requests again.
///
/// # Caution:
/// The `IncludeSubDomains` and `Preload` settings may have lasting unintended
/// effects on unrelated HTTP servers (current and future) running on subdomains
/// of the links host, and may even render those websites unusable for months or
/// years by requiring browsers to use HTTPS (with TLS) *exclusively* when doing
/// HTTP requests to those domains. The `Enable` setting, however, only impacts
/// the exact domain it is used from, so should only impact the links redirector
/// server itself. It is recommended to start testing HSTS (especially
/// `IncludeSubDomains` and `Preload`) with a short `max-age` initially, and to
/// test any possible impact on other websites hosted on the same domain and on
/// its subdomains.
///
/// See also:
/// - <https://hstspreload.org/>
/// - <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Strict-Transport-Security>
/// - <https://en.wikipedia.org/wiki/HTTP_Strict_Transport_Security>
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Hsts {
	/// Don't send the HTTP Strict Transport Security header
	Disable,
	/// Send the HSTS header without the `preload` or `includeSubDomains`
	/// attributes
	Enable(u32),
	/// Send the HSTS header with the `includeSubDomains` attribute, but without
	/// `preload`
	///
	/// # Caution:
	/// This may have temporary unintended effects on unrelated HTTP servers
	/// running on subdomains of the links host. Make sure that this won't cause
	/// any problems before enabling it and try a short max-age first.
	/// More info on <https://hstspreload.org/>,
	/// <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Strict-Transport-Security>,
	/// and <https://en.wikipedia.org/wiki/HTTP_Strict_Transport_Security>.
	IncludeSubDomains(u32),
	/// Send the HSTS header with the `preload` and `includeSubDomains`
	/// attributes
	///
	/// # Caution:
	/// This may have lasting unintended effects on unrelated HTTP servers
	/// (current and future) running on subdomains of the links host, and may
	/// even render those websites unusable for months or years.
	///
	/// Read <https://hstspreload.org/>,
	/// <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Strict-Transport-Security>,
	/// and <https://en.wikipedia.org/wiki/HTTP_Strict_Transport_Security>, and
	/// make sure that this won't cause any problems before enabling it.
	Preload(u32),
}

impl Default for Hsts {
	fn default() -> Self {
		Self::Enable(2 * A_YEAR)
	}
}

/// TLS settings for the links redirector server. Applies to both HTTP(S) and
/// the RPC API.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Tls {
	/// Disable TLS. With this option only HTTP *without* TLS (no HTTPS) are
	/// supported, and gRPC traffic is unencrypted. Not recommended.
	#[default]
	Disable,
	/// Enable TLS. Both HTTP and HTTPS are supported, gRPC traffic is
	/// encrypted.
	Enable {
		/// Path to the file containing a PEM-encoded TLS private key. All file
		/// formats supported by [`rustls-pemfile`](https://docs.rs/rustls-pemfile/)
		/// are supported.
		key_file: PathBuf,
		/// Path to the file containing a PEM-encoded TLS certificate. All file
		/// formats supported by [`rustls-pemfile`](https://docs.rs/rustls-pemfile/)
		/// are supported.
		cert_file: PathBuf,
	},
	/// Enable TLS, and redirect HTTP traffic to HTTPS. Both HTTP and HTTPS are
	/// supported, but the HTTP server *only* redirects to HTTPS, before any
	/// external redirect happens. gRPC traffic is encrypted. Recommended.
	Force {
		/// Path to the file containing a PEM-encoded TLS private key. All
		/// formats supported by [`rustls-pemfile`](https://docs.rs/rustls-pemfile/)
		/// are supported.
		key_file: PathBuf,
		/// Path to the file containing a PEM-encoded TLS certificate. All
		/// formats supported by [`rustls-pemfile`](https://docs.rs/rustls-pemfile/)
		/// are supported.
		cert_file: PathBuf,
	},
}