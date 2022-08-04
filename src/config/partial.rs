//! Links server configuration as seen by the user

use std::{
	collections::HashMap,
	env,
	error::Error,
	ffi::OsStr,
	fmt::Display,
	fs,
	io::Error as IoError,
	path::{Path, PathBuf},
	str::FromStr,
};

use pico_args::Arguments;
use serde_derive::{Deserialize, Serialize};
use serde_json::Error as JsonError;
use serde_yaml::Error as YamlError;
use thiserror::Error;
use toml::de::Error as TomlError;
use tracing::{instrument, Level};

use crate::{
	config::global::{Config, Hsts, Tls},
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
	/// TLS configuration (HTTPS and gRPC)
	pub tls: Option<PartialTls>,
	/// TLS key file path, required when TLS is enabled or forced
	pub tls_key: Option<PathBuf>,
	/// TLS certificate file path, required when TLS is enabled or forced
	pub tls_cert: Option<PathBuf>,
	/// HTTP Strict Transport Security setting on redirect
	pub hsts: Option<PartialHsts>,
	/// HTTP Strict Transport Security `max_age` header attribute (retention
	/// time in seconds)
	pub hsts_max_age: Option<u32>,
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
		Ok(toml::from_str(toml)?)
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

	/// Parse command-line arguments into a [`Partial`]. Store configuration is
	/// parsed from a json string (the value of `--store-config`).
	#[must_use]
	#[instrument(level = "debug", ret)]
	pub fn from_args() -> Self {
		let mut args = Arguments::from_env();

		let store_config = args
			.opt_value_from_fn("--store-config", |s| serde_json::from_str(s))
			.ok()
			.flatten();

		Self {
			log_level: args.opt_value_from_str("--log-level").unwrap_or(None),
			token: args.opt_value_from_str("--token").unwrap_or(None),
			tls: args.opt_value_from_str("--tls").unwrap_or(None),
			tls_key: args.opt_value_from_str("--tls-key").unwrap_or(None),
			tls_cert: args.opt_value_from_str("--tls-cert").unwrap_or(None),
			hsts: args.opt_value_from_str("--hsts").unwrap_or(None),
			hsts_max_age: args.opt_value_from_str("--hsts-max-age").unwrap_or(None),
			send_alt_svc: args.opt_value_from_str("--send-alt-svc").unwrap_or(None),
			send_server: args.opt_value_from_str("--send-server").unwrap_or(None),
			send_csp: args.opt_value_from_str("--send-csp").unwrap_or(None),
			store: args.opt_value_from_str("--store").unwrap_or(None),
			store_config,
		}
	}

	/// Parse environment variables with the prefix `LINKS_` into a [`Partial`].
	/// Store configuration is parsed from a json string (the value of
	/// `LINKS_STORE_CONFIG`).
	#[must_use]
	#[instrument(level = "debug", ret)]
	pub fn from_env_vars() -> Self {
		let store_config = env::var("LINKS_STORE_CONFIG")
			.map_or(None, |s| serde_json::from_str(&s).ok())
			.flatten();

		Self {
			log_level: parse_env_var("LINKS_LOG_LEVEL"),
			token: parse_env_var("LINKS_TOKEN"),
			tls: parse_env_var("LINKS_TLS"),
			tls_key: parse_env_var("LINKS_TLS_KEY"),
			tls_cert: parse_env_var("LINKS_TLS_CERT"),
			hsts: parse_env_var("LINKS_HSTS"),
			hsts_max_age: parse_env_var("LINKS_HSTS_MAX_AGE"),
			send_alt_svc: parse_env_var("LINKS_SEND_ALT_SVC"),
			send_server: parse_env_var("LINKS_SEND_SERVER"),
			send_csp: parse_env_var("LINKS_SEND_CSP"),
			store: parse_env_var("LINKS_STORE"),
			store_config,
		}
	}

	/// Get the log level as a `tracing::Level`, if present
	#[must_use]
	pub fn log_level(&self) -> Option<Level> {
		self.log_level.map(Into::into)
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

	/// Get TLS configuration information from this partial config, if present
	#[must_use]
	pub fn tls(&self) -> Option<Tls> {
		match self.tls? {
			PartialTls::Disable => Some(Tls::Disable),
			PartialTls::Enable => Some(Tls::Enable {
				key_file: self.tls_key.as_ref()?.clone(),
				cert_file: self.tls_cert.as_ref()?.clone(),
			}),
			PartialTls::Force => Some(Tls::Force {
				key_file: self.tls_key.as_ref()?.clone(),
				cert_file: self.tls_cert.as_ref()?.clone(),
			}),
		}
	}
}

impl From<&Config> for Partial {
	fn from(config: &Config) -> Self {
		let (tls, key_file, cert_file) = match config.tls() {
			Tls::Disable => (PartialTls::Disable, None, None),
			Tls::Enable {
				ref key_file,
				ref cert_file,
			} => (
				PartialTls::Enable,
				Some(key_file.clone()),
				Some(cert_file.clone()),
			),
			Tls::Force {
				ref key_file,
				ref cert_file,
			} => (
				PartialTls::Force,
				Some(key_file.clone()),
				Some(cert_file.clone()),
			),
		};

		let (hsts, hsts_max_age) = match config.hsts() {
			Hsts::Disable => (PartialHsts::Disable, None),
			Hsts::Enable(max_age) => (PartialHsts::Enable, Some(max_age)),
			Hsts::IncludeSubDomains(max_age) => (PartialHsts::IncludeSubDomains, Some(max_age)),
			Hsts::Preload(max_age) => (PartialHsts::Preload, Some(max_age)),
		};

		Self {
			log_level: Some((config.log_level()).into()),
			token: Some(config.token()),
			tls: Some(tls),
			tls_key: key_file,
			tls_cert: cert_file,
			hsts: Some(hsts),
			hsts_max_age,
			send_alt_svc: Some(config.send_alt_svc()),
			send_server: Some(config.send_server()),
			send_csp: Some(config.send_csp()),
			store: Some(config.store()),
			store_config: Some(config.store_config()),
		}
	}
}

/// Log level as seen from the user's perspective.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
	/// Lowest log level. Log everything, including very verbose debug/trace
	/// info. May expose private/secret information in logs.
	Trace,
	/// Log most things, including more verbose debug info. May expose
	/// private/secret information in logs.
	Debug,
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

/// The error returned by fallible conversions from a string to [`LogLevel`].
/// Includes the original input string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogLevelParseError(String);

impl Error for LogLevelParseError {}

impl Display for LogLevelParseError {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("unknown log level: {}", self.0))
	}
}

impl FromStr for LogLevel {
	type Err = LogLevelParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"trace" => Ok(Self::Trace),
			"debug" => Ok(Self::Debug),
			"info" => Ok(Self::Info),
			"warn" | "warning" => Ok(Self::Warn),
			"error" => Ok(Self::Error),
			_ => Err(LogLevelParseError(s.to_string())),
		}
	}
}

impl Display for LogLevel {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(match self {
			Self::Trace => "trace",
			Self::Debug => "debug",
			Self::Info => "info",
			Self::Warn => "warn",
			Self::Error => "error",
		})
	}
}

impl From<LogLevel> for Level {
	fn from(log_level: LogLevel) -> Self {
		match log_level {
			LogLevel::Trace => Level::TRACE,
			LogLevel::Debug => Level::DEBUG,
			LogLevel::Info => Level::INFO,
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

/// TLS enabling options as seen from the user's perspective.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
#[serde(rename_all = "snake_case")]
pub enum PartialTls {
	/// Disable TLS. Only HTTP (no HTTPS) and unencrypted gRPC will be
	/// available.
	#[default]
	Disable,
	/// Enable TLS. HTTP, HTTPS, and encrypted gRPC will be available.
	Enable,
	/// Enable and force TLS. HTTP, HTTPS, and encrypted gRPC will be available,
	/// however HTTP will only redirect to HTTPS (before any external redirect).
	Force,
}

/// The error returned by fallible conversions from a string to [`PartialTls`].
/// Includes the original input string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub struct PartialTlsParseError(String);

impl Error for PartialTlsParseError {}

impl Display for PartialTlsParseError {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("unknown TLS option: {}", self.0))
	}
}

impl FromStr for PartialTls {
	type Err = PartialTlsParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"disable" | "off" => Ok(Self::Disable),
			"enable" | "on" => Ok(Self::Enable),
			"force" => Ok(Self::Force),
			_ => Err(PartialTlsParseError(s.to_string())),
		}
	}
}

impl Display for PartialTls {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(match self {
			Self::Disable => "disable",
			Self::Enable => "enable",
			Self::Force => "force",
		})
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
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
#[serde(rename_all = "snake_case")]
pub enum PartialHsts {
	/// Don't send the HTTP Strict Transport Security header
	Disable,
	/// Send the HSTS header without the `preload` or `includeSubDomains`
	/// attributes.
	#[default]
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

/// The error returned by fallible conversions from a string to [`PartialHsts`].
/// Includes the original input string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
pub struct PartialHstsParseError(String);

impl Error for PartialHstsParseError {}

impl Display for PartialHstsParseError {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("unknown HSTS option: {}", self.0))
	}
}

impl FromStr for PartialHsts {
	type Err = PartialHstsParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"disable" | "off" => Ok(Self::Disable),
			"enable" | "on" => Ok(Self::Enable),
			"include" | "includeSubDomains" => Ok(Self::IncludeSubDomains),
			"preload" => Ok(Self::Preload),
			_ => Err(PartialHstsParseError(s.to_string())),
		}
	}
}

impl Display for PartialHsts {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(match self {
			Self::Disable => "disable",
			Self::Enable => "enable",
			Self::IncludeSubDomains => "include",
			Self::Preload => "preload",
		})
	}
}
