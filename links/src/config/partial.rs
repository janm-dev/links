//! Links server configuration as seen by the user

use std::{
	collections::HashMap, env, ffi::OsStr, fs, io::Error as IoError, path::Path, str::FromStr,
};

use basic_toml::Error as TomlError;
use pico_args::Arguments;
use serde::{Deserialize, Serialize};
use serde_json::Error as JsonError;
use serde_yaml::Error as YamlError;
use strum::{Display as EnumDisplay, EnumString};
use thiserror::Error;
use tracing::{instrument, warn};

use crate::{
	config::{global::Hsts, CertificateSource, DefaultCertificateSource, ListenAddress, LogLevel},
	stats::StatisticCategories,
	store::BackendType,
};

/// The error returned by fallible conversions into a [`Partial`]
#[derive(Debug, Error)]
pub enum IntoPartialError {
	/// Failed to parse from toml
	#[error("failed to parse from toml")]
	Toml(#[from] TomlError),
	/// Failed to parse from yaml
	#[error("failed to parse from yaml")]
	Yaml(#[from] YamlError),
	/// Failed to parse from json
	#[error("failed to parse from json")]
	Json(#[from] JsonError),
	/// Failed to read config file
	#[error("failed to read config file")]
	Io(#[from] IoError),
	/// File extension unknown, could not determine format
	#[error("file extension unknown, could not determine format")]
	UnknownExtension,
}

/// Parse the provided environment variable, returning `Some(...)` if it is
/// present, has a value, and was successfully parsed, and `None` otherwise
fn parse_env_var<T: FromStr>(key: &'static str) -> Option<T> {
	env::var(key).map_or(None, |s| s.parse().ok())
}

/// Links redirector configuration as seen from the user's perspective. This is
/// easier to parse, but less idiomatic and not as easy to use as [`Config`]. As
/// this is a representation of links' configuration from one source only, all
/// fields are optional, which allows incremental updates to the actual
/// [`Config`] struct.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Partial {
	/// Minimum level of logs to be collected/displayed. Debug and trace levels
	/// may expose secret information, so are not recommended for production
	/// deployments.
	pub log_level: Option<LogLevel>,
	/// API token, used for authentication of gRPC clients
	pub token: Option<String>,
	/// Listener addresses, see [`ListenAddress`] for details
	pub listeners: Option<Vec<ListenAddress>>,
	/// What types of statistics should be collected
	pub statistics: Option<StatisticCategories>,
	/// Default TLS certificate and key source
	pub default_certificate: Option<DefaultCertificateSource>,
	/// TLS certificate and key sources
	pub certificates: Option<Vec<CertificateSource>>,
	/// HTTP Strict Transport Security setting on redirect
	pub hsts: Option<PartialHsts>,
	/// HTTP Strict Transport Security `max_age` header attribute (retention
	/// time in seconds)
	pub hsts_max_age: Option<u32>,
	/// Redirect from HTTP to HTTPS before the external redirect
	pub https_redirect: Option<bool>,
	/// Send the `Alt-Svc` header advertising `h2` (HTTP/2.0 with TLS) support
	/// on port 443
	pub send_alt_svc: Option<bool>,
	/// Send the `Server` header
	pub send_server: Option<bool>,
	/// Send the `Content-Security-Policy` header
	pub send_csp: Option<bool>,
	/// The store backend type
	pub store: Option<BackendType>,
	/// The store backend configuration. All of these options are
	/// backend-specific, and have ASCII alphanumeric string keys in
	/// `snake_case` (lower case, words seperated by underscores), without any
	/// hyphens (`-`), i.e. only lowercase `a-z`, `0-9`, and `_` are
	/// allowed. The values are UTF-8 strings in any format.
	pub store_config: Option<HashMap<String, String>>,
}

impl Partial {
	/// Parse a [`Partial`] from a [toml](https://toml.io/en/) string
	///
	/// # Errors
	/// Returns a `FromFileError::Toml` if deserialization fails.
	pub fn from_toml(toml: &str) -> Result<Self, IntoPartialError> {
		Ok(basic_toml::from_str(toml)?)
	}

	/// Parse a [`Partial`] from a [yaml](https://yaml.org/) string
	///
	/// # Errors
	/// Returns a `FromFileError::Yaml` if deserialization fails.
	pub fn from_yaml(yaml: &str) -> Result<Self, IntoPartialError> {
		Ok(serde_yaml::from_str(yaml)?)
	}

	/// Parse a [`Partial`] from a [json](https://json.org/) string
	///
	/// # Errors
	/// Returns a `FromFileError::Json` if deserialization fails.
	pub fn from_json(json: &str) -> Result<Self, IntoPartialError> {
		Ok(serde_json::from_str(json)?)
	}

	/// Read and parse a configuration file into a [`Partial`]. The format of
	/// the file is determined from its extension:
	/// - `*.toml` files are parsed as [toml](https://toml.io/en/)
	/// - `*.yaml` and `*.yml` files are parsed as [yaml](https://yaml.org/)
	/// - `*.json` files are parsed as [json](https://json.org/)
	///
	/// # IO
	/// This function performs synchronous file IO, and should not be used in an
	/// asynchronous context.
	///
	/// # Errors
	/// Returns an error when reading of parsing the file fails.
	#[instrument(level = "debug", ret, err)]
	pub fn from_file(path: &Path) -> Result<Self, IntoPartialError> {
		let parse = match path.extension().map(OsStr::to_str) {
			Some(Some("toml")) => Self::from_toml,
			Some(Some("yaml" | "yml")) => Self::from_yaml,
			Some(Some("json")) => Self::from_json,
			_ => return Err(IntoPartialError::UnknownExtension),
		};

		parse(&fs::read_to_string(path)?)
	}

	/// Parse command-line arguments into a [`Partial`]. Listeners and store
	/// configuration are parsed from json strings.
	#[must_use]
	#[instrument(level = "debug", ret)]
	pub fn from_args() -> Self {
		let mut args = Arguments::from_env();

		let listeners = args
			.opt_value_from_fn("--listeners", |s| serde_json::from_str(s))
			.map_err(|err| {
				warn!(
					%err,
					"Error parsing configuration from command-line argument '--listeners'"
				);
			})
			.ok()
			.flatten();

		let statistics = args
			.opt_value_from_fn("--statistics", |s| serde_json::from_str(s))
			.map_err(|err| {
				warn!(
					%err,
					"Error parsing configuration from command-line argument '--statistics'"
				);
			})
			.ok()
			.flatten();

		let default_certificate = args
			.opt_value_from_fn("--default-certificate", |s| serde_json::from_str(s))
			.map_err(|err| {
				warn!(
					%err,
					"Error parsing configuration from command-line argument '--default-certificate'"
				);
			})
			.ok()
			.flatten();

		let certificates = args
			.opt_value_from_fn("--certificates", |s| serde_json::from_str(s))
			.map_err(|err| {
				warn!(
					%err,
					"Error parsing configuration from command-line argument '--certificates'"
				);
			})
			.ok()
			.flatten();

		let store_config = args
			.opt_value_from_fn("--store-config", |s| serde_json::from_str(s))
			.map_err(|err| {
				warn!(
					%err,
					"Error parsing configuration from command-line argument '--store-config'"
				);
			})
			.ok()
			.flatten();

		Self {
			log_level: args.opt_value_from_str("--log-level").unwrap_or(None),
			token: args.opt_value_from_str("--token").unwrap_or(None),
			listeners,
			statistics,
			default_certificate,
			certificates,
			hsts: args.opt_value_from_str("--hsts").unwrap_or(None),
			hsts_max_age: args.opt_value_from_str("--hsts-max-age").unwrap_or(None),
			https_redirect: args.opt_value_from_str("--https-redirect").unwrap_or(None),
			send_alt_svc: args.opt_value_from_str("--send-alt-svc").unwrap_or(None),
			send_server: args.opt_value_from_str("--send-server").unwrap_or(None),
			send_csp: args.opt_value_from_str("--send-csp").unwrap_or(None),
			store: args.opt_value_from_str("--store").unwrap_or(None),
			store_config,
		}
	}

	/// Parse environment variables with the prefix `LINKS_` into a [`Partial`].
	/// Listeners and store configuration are parsed from json strings.
	#[must_use]
	#[instrument(level = "debug", ret)]
	pub fn from_env_vars() -> Self {
		let listeners = env::var("LINKS_LISTENERS")
			.map_or(None, |s| {
				serde_json::from_str(&s)
					.map_err(|err| {
						warn!(
							%err,
							"Error parsing configuration from environment variable 'LINKS_LISTENERS'"
						);
					})
					.ok()
			})
			.flatten();

		let statistics = env::var("LINKS_STATISTICS")
			.map_or(None, |s| {
				serde_json::from_str(&s)
					.map_err(|err| {
						warn!(
							%err,
							"Error parsing configuration from environment variable 'LINKS_STATISTICS'"
						);
					})
					.ok()
			})
			.flatten();

		let default_certificate = env::var("LINKS_DEFAULT_CERTIFICATE")
			.map_or(None, |s| {
				serde_json::from_str(&s)
					.map_err(|err| {
						warn!(
							%err,
							"Error parsing configuration from environment variable 'LINKS_DEFAULT_CERTIFICATE'"
						);
					})
					.ok()
			})
			.flatten();

		let certificates = env::var("LINKS_CERTIFICATES")
			.map_or(None, |s| {
				serde_json::from_str(&s)
					.map_err(|err| {
						warn!(
							%err,
							"Error parsing configuration from environment variable 'LINKS_CERTIFICATES'"
						);
					})
					.ok()
			})
			.flatten();

		let store_config = env::var("LINKS_STORE_CONFIG")
			.map_or(None, |s| {
				serde_json::from_str(&s)
					.map_err(|err| {
						warn!(
							%err,
							"Error parsing configuration from environment variable 'LINKS_STORE_CONFIG'"
						);
					})
					.ok()
			})
			.flatten();

		Self {
			log_level: parse_env_var("LINKS_LOG_LEVEL"),
			token: parse_env_var("LINKS_TOKEN"),
			listeners,
			statistics,
			default_certificate,
			certificates,
			hsts: parse_env_var("LINKS_HSTS"),
			hsts_max_age: parse_env_var("LINKS_HSTS_MAX_AGE"),
			https_redirect: parse_env_var("LINKS_HTTPS_REDIRECT"),
			send_alt_svc: parse_env_var("LINKS_SEND_ALT_SVC"),
			send_server: parse_env_var("LINKS_SEND_SERVER"),
			send_csp: parse_env_var("LINKS_SEND_CSP"),
			store: parse_env_var("LINKS_STORE"),
			store_config,
		}
	}

	/// Get HSTS configuration information from this partial config, if present
	#[must_use]
	pub fn hsts(&self) -> Option<Hsts> {
		match self.hsts? {
			PartialHsts::Disable => Some(Hsts::Disable),
			PartialHsts::Enable => Some(Hsts::Enable(self.hsts_max_age?)),
			PartialHsts::IncludeSubDomains => Some(Hsts::IncludeSubDomains(self.hsts_max_age?)),
			PartialHsts::Preload => Some(Hsts::Preload(self.hsts_max_age?)),
		}
	}
}

/// HSTS enabling options as seen from the user's perspective.
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
#[derive(
	Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, EnumString, EnumDisplay,
)]
#[allow(clippy::module_name_repetitions)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum PartialHsts {
	/// Don't send the HTTP Strict Transport Security header
	#[strum(serialize = "disable", serialize = "off")]
	Disable,
	/// Send the HSTS header without the `preload` or `includeSubDomains`
	/// attributes.
	#[default]
	#[strum(serialize = "enable", serialize = "on")]
	Enable,
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
	#[strum(
		serialize = "includeSubDomains",
		serialize = "include",
		to_string = "include"
	)]
	IncludeSubDomains,
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
	/// and <https://en.wikipedia.org/wiki/HTTP_Strict_Transport_Security> first,
	/// and make sure that this won't cause any problems before enabling it.
	Preload,
}
