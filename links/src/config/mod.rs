//! Links server configuration handling
//!
//! The links redirector server currently accepts the following configuration
//! options:
//!
//! - `log_level` - Tracing log level. Possible values: `trace`, `debug`,
//!   `verbose`, `info`, `warn`, `error`. **Default `info`**.
//! - `token` - RPC API authentication token, should be long and random.
//!   **Default \[randomly generated string\]**.
//! - `listeners` - A list of listener addresses (strings) in the format of
//!   `protocol:ip-address:port` (see [`ListenAddress`] for details). **Default
//!   `http::`, `https::`, `grpc:[::1]:`, and `grpcs::`**.
//! - `statistics` - A list of statistics categories to be collected (see
//!   [statistics][`crate::stats`] for details). **Default `redirect`, `basic`,
//!   and `protocol`**.
//! - `default_certificate` - An optional TLS certificate/key source to be used
//!   for requests with an unknown/unrecognized domain names (see
//!   [certificates][`crate::certs`] for details). **Default `None`**.
//! - `certificates` - A list of TLS certificate/key sources (see
//!   [certificates][`crate::certs`] for details). **Default empty**.
//! - `hsts` - HTTP strict transport security setting. Possible values:
//!   `disable`, `enable`, `includeSubDomains`, `preload`. **Default `enable`**.
//! - `hsts_max_age` - The HSTS max-age setting (in seconds). **Default
//!   `63072000` (2 years)**.
//! - `https_redirect` - Whether to redirect HTTP requests to HTTPS before the
//!   external redirect. **Default `false`**.
//! - `send_alt_svc` - Whether to send the Alt-Svc HTTP header (`Alt-Svc:
//!   h2=":443"; ma=31536000`). **Default `false`**.
//! - `send_server` - Whether to send the Server HTTP header (`Server:
//!   hyperlinks/[VERSION]`). **Default `true`**.
//! - `send_csp` - Whether to send the Content-Security-Policy HTTP header.
//!   **Default `true`**.
//! - `store` - The store backend type to use. See store documentation.
//!   **Default `memory`**.
//! - `store_config` - Store backend configuration. Depends on the store backend
//!   used. **Default empty**.

mod global;
mod partial;

use std::{
	fmt::{Debug, Display, Formatter, Result as FmtResult},
	fs,
	io::Error as IoError,
	net::{AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr},
	num::ParseIntError,
	path::PathBuf,
	str::FromStr,
	sync::Mutex,
	time::Duration,
};

use crossbeam_channel::{select, unbounded, Receiver, Sender};
use links_domainmap::Domain;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use strum::{Display as EnumDisplay, EnumString, ParseError};
use tokio_rustls::rustls::{
	crypto::ring::sign,
	pki_types::{CertificateDer, PrivateKeyDer},
	sign::CertifiedKey,
	Error as RustlsError,
};
use tracing::{debug, error, Level};

pub use self::{
	global::{Config, Hsts, Redirector},
	partial::{IntoPartialError, Partial, PartialHsts},
};
use crate::{server::Protocol, util::Unpoison};

/// An update to certificate configuration
#[derive(Debug)]
pub enum CertConfigUpdate {
	/// The default certificate source was updated
	DefaultUpdated(DefaultCertificateSource),
	/// A certificate source was added and should start being watched
	SourceAdded(CertificateSource),
	/// A certificate source was removed and should stop being watched
	SourceRemoved(CertificateSource),
}

/// A watcher for updates to certificate sources
#[derive(Debug)]
pub struct CertificateWatcher {
	/// All currently-watched non-default certificate sources
	sources: Vec<CertificateSource>,
	/// The default certificate source
	default_source: DefaultCertificateSource,
	/// Underlying watcher for certificates read from files
	files_watcher: RecommendedWatcher,
	/// Receiver for file modification events from `files_watcher`
	files_rx: Receiver<Event>,
	/// Receiver for certificate source configuration updates
	config_rx: Receiver<CertConfigUpdate>,
	/// Sender for certificate source configuration updates, can be retrieved
	/// using [`Self::get_config_sender()`].
	config_tx: Sender<CertConfigUpdate>,
}

impl CertificateWatcher {
	/// Create a new [`CertificateWatcher`]
	///
	/// # Errors
	/// This function returns an error if the file watcher for `files`
	/// certificate sources could not be set up
	pub fn new() -> anyhow::Result<Self> {
		let (files_tx, files_rx) = unbounded();
		let (config_tx, config_rx) = unbounded();
		let files_watcher = notify::recommended_watcher(move |res| match res {
			Ok(ev) => {
				let _ = files_tx.send(ev).inspect_err(|err| {
					error!("the certificate file watching channel closed unexpectedly: {err}");
				});
			}
			Err(err) => error!(%err, "certificate file watching error"),
		})?;

		Ok(Self {
			sources: Vec::new(),
			default_source: DefaultCertificateSource::None,
			files_watcher,
			files_rx,
			config_rx,
			config_tx,
		})
	}

	/// Watch for changes to the certificates
	///
	/// Blocks until a change has occurred, returning all [`CertificateSource`]s
	/// that have changed and need to be reloaded. The first element in the
	/// tuple contains all updated sources from `certificates`, the second is
	/// the default certificate source (or `None` if it hasn't been updated).
	/// May return multiple certificate sources at once if one event caused all
	/// of them to need updating. May return a false positive (a certificate
	/// source may be returned when it hasn't actually changed).
	#[allow(clippy::missing_panics_doc)] // TODO
	pub fn watch(
		&mut self,
		debounce_time: Duration,
	) -> (Vec<CertificateSource>, DefaultCertificateSource) {
		let debounced = Mutex::new(Option::<(Vec<_>, _)>::None);

		let handle_files = |this: &mut Self, event| {
			debug!(?event, "Received file event from watcher");
			let file_sources = this
				.sources
				.iter()
				.filter(|s| match s.source {
					CertificateSourceType::Files { .. } => true,
				})
				.cloned()
				.collect();

			let mut db = debounced.lock().unpoison();
			if let Some(ref mut debounced) = *db {
				for source in file_sources {
					if !debounced.0.contains(&source) {
						debounced.0.push(source);
					}
				}

				if matches!(this.default_source, DefaultCertificateSource::Some {
					source: CertificateSourceType::Files { .. },
					..
				}) {
					debounced.1 = this.default_source.clone();
				}
			} else {
				*db = Some((
					file_sources,
					if matches!(this.default_source, DefaultCertificateSource::Some {
						source: CertificateSourceType::Files { .. },
						..
					}) {
						this.default_source.clone()
					} else {
						DefaultCertificateSource::None
					},
				));
			}
		};

		let handle_config = |this: &mut Self, msg| match msg {
			CertConfigUpdate::DefaultUpdated(default) => {
				if let Some((Err(err), source)) = this
					.default_source
					.clone()
					.into_cs()
					.map(|s| (s.unwatch(this), s))
				{
					error!(%err, ?source, "default certificate source could not be unwatched");
				}

				this.default_source = default;

				if let Some((Err(err), source)) = this
					.default_source
					.clone()
					.into_cs()
					.map(|s| (s.watch(this), s))
				{
					error!(%err, ?source, "default certificate source could not be watched");
				}
			}
			CertConfigUpdate::SourceRemoved(source) => {
				this.sources.retain(|s| s != &source);
				if let Err(err) = source.unwatch(this) {
					error!(%err, ?source, "certificate source could not be unwatched");
				}
			}
			CertConfigUpdate::SourceAdded(source) => {
				if let Err(err) = source.watch(this) {
					error!(%err, ?source, "certificate source could not be watched");
				}
				this.sources.push(source);
			}
		};

		loop {
			select! {
				recv(self.files_rx) -> msg => handle_files(self, msg.expect("certificate watcher channel closed")),
				recv(self.config_rx) -> msg => handle_config(self, msg.expect("certificate watcher channel closed")),
				default(debounce_time) => if debounced.lock().unpoison().is_some() {
					break debounced.into_inner().unpoison().expect("the option was just checked to be some");
				}
			}
		}
	}

	/// Get the sender for certificate source configuration updates
	#[must_use]
	pub fn get_config_sender(&self) -> Sender<CertConfigUpdate> {
		self.config_tx.clone()
	}

	/// Send a config update to be processed by this watcher
	pub fn send_config_update(&self, update: CertConfigUpdate) {
		if self.config_tx.send(update).is_err() {
			unreachable!("the receiver is owned by this watcher, so this channel can not be closed")
		}
	}
}

/// The source of the default certificate/key pair
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum DefaultCertificateSource {
	/// No default certificate
	None,
	/// The default certificate's source
	Some {
		/// The domains that this certificate will be gotten for, if applicable
		/// to the `source` type
		#[serde(default)]
		domains: Vec<Domain>,
		/// The type of certificate source and type-specific configuration
		#[serde(flatten)]
		source: CertificateSourceType,
	},
}

impl DefaultCertificateSource {
	/// Convert this [`DefaultCertificateSource`] into a [`CertificateSource`],
	/// setting the `domains` to an empty vec if not specified. Returns `None`
	/// if this is `DefaultCertificateSource::None`
	#[must_use]
	pub fn into_cs(self) -> Option<CertificateSource> {
		match self {
			Self::None => None,
			Self::Some { domains, source } => Some(CertificateSource { domains, source }),
		}
	}
}

/// The source of a certificate/key pair
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CertificateSource {
	/// The domains that this certificate will be used for
	pub domains: Vec<Domain>,
	/// The type of certificate source and type-specific configuration
	#[serde(flatten)]
	pub source: CertificateSourceType,
}

impl CertificateSource {
	/// Get the certificate and private key
	///
	/// # IO
	/// Depending on the type of this [`CertificateSource`], blocking IO may be
	/// performed. This function should not be called in async contexts.
	///
	/// # Errors
	/// This function may return various errors on failure, see
	/// [`CertificateAcquisitionError`] for more details
	pub fn get_certkey(&self) -> Result<CertifiedKey, CertificateAcquisitionError> {
		match &self.source {
			CertificateSourceType::Files { cert, key } => {
				let certs = fs::read(cert)?;
				let key = fs::read(key)?;

				let certs: Result<Vec<CertificateDer>, _> = rustls_pemfile::certs(&mut &certs[..])
					.map(|res| res.map(|der| CertificateDer::from(der.to_vec())))
					.collect();
				let certs = certs?;
				let key = rustls_pemfile::pkcs8_private_keys(&mut &key[..])
					.map(|res| {
						res.map(|der| {
							PrivateKeyDer::Pkcs8(der.secret_pkcs8_der().to_owned().into())
						})
					})
					.next()
					.ok_or(CertificateAcquisitionError::MissingKey)??;

				let cert_key = CertifiedKey::new(
					certs,
					sign::any_supported_type(&key)
						.map_err(CertificateAcquisitionError::InvalidKey)?,
				);

				let () = cert_key
					.keys_match()
					.map_err(CertificateAcquisitionError::KeyMismatch)?;

				Ok(cert_key)
			}
		}
	}

	/// Start watching for updates to the certificate source
	///
	/// # Errors
	/// This function returns an error if the certificate source could not
	/// successfully be watched due to e.g. file watching errors
	pub fn watch(&self, watcher: &mut CertificateWatcher) -> anyhow::Result<()> {
		match &self.source {
			CertificateSourceType::Files { cert, key } => {
				watcher
					.files_watcher
					.watch(cert, RecursiveMode::NonRecursive)?;
				watcher
					.files_watcher
					.watch(key, RecursiveMode::NonRecursive)?;
			}
		}

		Ok(())
	}

	/// Stop watching for updates to the certificate source
	///
	/// # Errors
	/// This function returns an error if the certificate source could not
	/// successfully be unwatched due to e.g. file watching errors
	pub fn unwatch(&self, watcher: &mut CertificateWatcher) -> anyhow::Result<()> {
		match &self.source {
			CertificateSourceType::Files { cert, key } => {
				watcher.files_watcher.unwatch(cert)?;
				watcher.files_watcher.unwatch(key)?;
			}
		}

		Ok(())
	}
}

/// The error returned when getting a certificate/key pair fails
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CertificateAcquisitionError {
	/// A filesystem error occurred (e.g. a file containing a certificate could
	/// not be read)
	#[error("A filesystem error occurred")]
	FileIo(#[from] IoError),
	/// The certificate can not be found
	#[error("No certificate found")]
	MissingCert,
	/// The key can not be found
	#[error("No key found")]
	MissingKey,
	/// The private key is invalid or unsupported
	#[error("The private key is invalid or unsupported")]
	InvalidKey(#[source] RustlsError),
	/// The private key does not match the certificate
	#[error("The private key does not match the certificate")]
	KeyMismatch(#[source] RustlsError),
}

/// The type of certificate source, for example certificate/key files, ACME,
/// secret storage, etc.
///
/// Each variant has a name (serialized as `source`), a list of domains for
/// which the certificate is to be used (serialized as `domains`), and any other
/// variant-specific configuration (serialized in `snake_case` with
/// appropriately typed values).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum CertificateSourceType {
	/// Use the certificate from the `cert` file and the private key from the
	/// `key` file. Currently only the PEM file format is supported.
	///
	/// # Example
	/// ```toml
	/// { source = "files", domains = ["example.com", "*.example.net"], cert = "./cert.pem", key = "./key.pem" }
	/// ```
	Files {
		/// The file path of the certificate file (PEM format)
		cert: PathBuf,
		/// The file path of the private key file (PEM format)
		key: PathBuf,
	},
}

/// The error returned by fallible conversions into [`ListenAddress`],
/// containing the invalid input value
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntoListenAddressError {
	/// General listener address parse error
	#[error("\"{0}\" is not a valid listen address")]
	General(String),
	/// Parse error from the protocol
	#[error("{0}")]
	Protocol(#[from] ParseError),
	/// Parse error from the IP address
	#[error("invalid IP address for listener: {0}")]
	Address(#[from] AddrParseError),
	/// Parse error from the port number
	#[error("invalid port number for listener: {0}")]
	Port(#[from] ParseIntError),
}

/// A listener's address, with the protocol, ip address, and port.
///
/// # String representation
/// A [`ListenAddress`] can be represented as a string in the format
/// `protocol:ip-address:port`.
///
/// The protocol is the string representation of a links-supported [`Protocol`]
/// (see its documentation for more info). The protocol is case-insensitive and
/// can not be omitted.
///
/// The ip address is an IPv4 address literal (e.g. `0.0.0.0`, `127.0.0.1`, or
/// `198.51.100.35`), an IPv6 address literal in square brackets (e.g. `[::]`,
/// `[::1]`, or `[2001:db8:6ca1:573d:15c7:075f:5de5:cd30]`), or can be omitted.
/// The listener will listen for incoming connections on the specified address.
/// Address `0.0.0.0` can be used to listen on all IPv4 (but not IPv6)
/// addresses, address `[::]` can be used to listen on all IPv6 (but not IPv4)
/// addresses. An empty (omitted) address can be used to listen on all
/// addresses, IPv4 and IPv6, though this requires OS support for dual stack
/// sockets (which is common but not universal). In the case of lack of such
/// support, an empty address is equivalent to `[::]`, and as such will listen
/// on all IPv6 (but not IPv4) addresses.
///
/// The port is a TCP/UDP port (currently only TCP is used, but UDP may be used
/// in the future for some protocols). An empty (omitted) port means the default
/// port for the specified protocol (see [`Protocol`]). Port `0` can be used to
/// request and ephemeral port from the operating system, however this is not
/// recommended for server applications such as links.
#[derive(Copy, Clone, Eq, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct ListenAddress {
	/// The protocol that the listener will process. See [`Protocol`] for
	/// details.
	pub protocol: Protocol,
	/// The address of the listener. An unspecified (omitted) address will
	/// listen on all addresses (OS support is not universal, see
	/// [`ListenAddress`]).
	pub address: Option<IpAddr>,
	/// The port (TCP and UDP) that the listener will use. An unspecified port
	/// means the default port of the protocol.
	pub port: Option<u16>,
}

impl Debug for ListenAddress {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		Display::fmt(self, fmt)
	}
}

impl Display for ListenAddress {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_fmt(format_args!(
			"{}:{}:{}",
			self.protocol,
			self.address.map_or(String::new(), |a| match a {
				IpAddr::V4(a) => a.to_string(),
				IpAddr::V6(a) => format!("[{a}]"),
			}),
			self.port.map_or(String::new(), |n| n.to_string())
		))
	}
}

impl PartialEq for ListenAddress {
	#[expect(
		clippy::suspicious_operation_groupings,
		reason = "This is correct, a `None` address is distinct from all `Some(_)` addresses, but \
		          a `None` port is just the default port for that protocol"
	)]
	fn eq(&self, other: &Self) -> bool {
		self.protocol == other.protocol
			&& self.address == other.address
			&& self.port.unwrap_or_else(|| self.protocol.default_port())
				== other.port.unwrap_or_else(|| other.protocol.default_port())
	}
}

impl FromStr for ListenAddress {
	type Err = IntoListenAddressError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let (protocol, rest) = s
			.split_once(':')
			.ok_or_else(|| IntoListenAddressError::General(s.to_string()))?;
		let (address, port) = rest
			.rsplit_once(':')
			.ok_or_else(|| IntoListenAddressError::General(s.to_string()))?;

		let address = if address.starts_with('[') && address.ends_with(']') {
			Some(Ipv6Addr::from_str(address.trim_start_matches('[').trim_end_matches(']'))?.into())
		} else if address.is_empty() {
			None
		} else {
			Some(Ipv4Addr::from_str(address)?.into())
		};

		Ok(Self {
			protocol: protocol.parse()?,
			address,
			port: match port {
				"" => None,
				s => Some(s.parse()?),
			},
		})
	}
}

impl TryFrom<&str> for ListenAddress {
	type Error = IntoListenAddressError;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		s.parse()
	}
}

impl From<ListenAddress> for String {
	fn from(address: ListenAddress) -> Self {
		address.to_string()
	}
}

/// Log level, corresponding roughly to `tracing`'s, but with the addition of
/// [`Verbose`][`LogLevel::Verbose`] between debug and info.
#[derive(
	Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, EnumString, EnumDisplay,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum LogLevel {
	/// Lowest log level. Log everything, including very verbose debug/trace
	/// info. May expose private/secret information in logs.
	Trace,
	/// Log most things, including more verbose debug info. May expose
	/// private/secret information in logs.
	Debug,
	/// Logs more verbose information (`debug`-level or higher) from links,
	/// while only logging `info`-level or higher information from dependencies.
	/// May expose private/secret information in logs.
	Verbose,
	/// Recommended log level. Logs general information, warnings, and errors.
	#[default]
	Info,
	/// Log only warnings and errors. Generally not recommended, as this hides a
	/// lot of useful information from logs.
	Warn,
	/// Log only critical errors. Generally not recommended, as this hides a lot
	/// of useful information from logs.
	Error,
}

impl From<LogLevel> for Level {
	fn from(log_level: LogLevel) -> Self {
		match log_level {
			LogLevel::Trace => Level::TRACE,
			LogLevel::Debug => Level::DEBUG,
			LogLevel::Verbose | LogLevel::Info => Level::INFO,
			LogLevel::Warn => Level::WARN,
			LogLevel::Error => Level::ERROR,
		}
	}
}

impl From<Level> for LogLevel {
	fn from(log_level: Level) -> Self {
		match log_level {
			Level::TRACE => LogLevel::Trace,
			Level::DEBUG => LogLevel::Debug,
			Level::INFO => LogLevel::Info,
			Level::WARN => LogLevel::Warn,
			Level::ERROR => LogLevel::Error,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn listen_address_parse() {
		assert_eq!(
			"http:0.0.0.0:80".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Http,
				address: Some([0, 0, 0, 0].into()),
				port: Some(80)
			})
		);

		assert_eq!(
			"http:[::]:80".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Http,
				address: Some([0, 0, 0, 0, 0, 0, 0, 0].into()),
				port: Some(80)
			})
		);

		assert_eq!(
			"https::".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Https,
				address: None,
				port: None
			})
		);

		assert_eq!(
			"grpc:127.0.0.1:".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Grpc,
				address: Some([127, 0, 0, 1].into()),
				port: None
			})
		);

		assert_eq!(
			"grpc:[::1]:".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Grpc,
				address: Some([0, 0, 0, 0, 0, 0, 0, 1].into()),
				port: None
			})
		);

		assert_eq!(
			"grpcs::530".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Grpcs,
				address: None,
				port: Some(530)
			})
		);

		assert_eq!(
			"GrPcS::530".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Grpcs,
				address: None,
				port: Some(530)
			})
		);

		assert_eq!(
			"GrPcS:127.0.5.4:00530".parse(),
			Ok(ListenAddress {
				protocol: Protocol::Grpcs,
				address: Some([127, 0, 5, 4].into()),
				port: Some(530)
			})
		);
	}

	#[test]
	fn listen_address_parse_invalid() {
		assert!(matches!(
			"http:[::1]".parse::<ListenAddress>(),
			Err(IntoListenAddressError::General(_)
				| IntoListenAddressError::Address(_)
				| IntoListenAddressError::Port(_))
		));

		assert!(matches!(
			"http:localhost:80".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Address(_))
		));

		assert!(matches!(
			"http:0.0.0.0".parse::<ListenAddress>(),
			Err(IntoListenAddressError::General(_))
		));

		assert!(matches!(
			":0.0.0.0:443".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Protocol(_))
		));

		assert!(matches!(
			"https:123.456.789.0:443".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Address(_))
		));

		assert!(matches!(
			"https:[::1]:123456789".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Port(_))
		));

		assert!(matches!(
			"https:[::1]:4a5".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Port(_))
		));

		assert!(matches!(
			"https:[::1]:0x1bb".parse::<ListenAddress>(),
			Err(IntoListenAddressError::Port(_))
		));
	}

	#[test]
	fn listen_address_to_from_string() {
		assert_eq!(
			"http::".parse::<ListenAddress>().unwrap().to_string(),
			"http::"
		);

		assert_eq!(
			"https:[::]:".parse::<ListenAddress>().unwrap().to_string(),
			"https:[::]:"
		);

		assert_eq!(
			"grpc::456".parse::<ListenAddress>().unwrap().to_string(),
			"grpc::456"
		);

		assert_eq!(
			"grpcs:0.0.0.0:789"
				.parse::<ListenAddress>()
				.unwrap()
				.to_string(),
			"grpcs:0.0.0.0:789"
		);

		assert_eq!(
			"grpcs:0.0.0.0:0000789"
				.parse::<ListenAddress>()
				.unwrap()
				.to_string(),
			"grpcs:0.0.0.0:789"
		);

		assert_eq!(
			"grpcs:[0000:0000:0000:0000:0000:0000:0000:0000]:789"
				.parse::<ListenAddress>()
				.unwrap()
				.to_string(),
			"grpcs:[::]:789"
		);
	}

	#[test]
	fn listen_address_eq() {
		assert_eq!(
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: None
			},
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: None
			}
		);

		assert_eq!(
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: None
			},
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: Some(Protocol::HTTP_DEFAULT_PORT)
			}
		);

		assert_ne!(
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: None
			},
			ListenAddress {
				protocol: Protocol::Https,
				address: None,
				port: None
			}
		);

		assert_ne!(
			ListenAddress {
				protocol: Protocol::Http,
				address: Some("::".parse().unwrap()),
				port: None
			},
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: None
			}
		);

		assert_ne!(
			ListenAddress {
				protocol: Protocol::Https,
				address: Some("::".parse().unwrap()),
				port: None
			},
			ListenAddress {
				protocol: Protocol::Http,
				address: None,
				port: Some(1000)
			}
		);
	}

	#[test]
	fn log_level() {
		assert_eq!("verbose".parse(), Ok(LogLevel::Verbose));
		assert_eq!("info".parse(), Ok(LogLevel::Info));
		assert_eq!("warn".parse(), Ok(LogLevel::Warn));

		assert_eq!("info".parse::<LogLevel>().map(Into::into), Ok(Level::INFO));
		assert_eq!(
			"verbose".parse::<LogLevel>().map(Into::into),
			Ok(Level::INFO)
		);
		assert_eq!(
			"error".parse::<LogLevel>().map(Into::into),
			Ok(Level::ERROR)
		);
	}
}
