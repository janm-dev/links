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
//! - `tls_enable` - Whether to enable TLS for HTTPS and RPC. **Default
//!   `false`**.
//! - `tls_key` - TLS private key file path. Required if TLS is enabled. **No
//!   default**.
//! - `tls_cert` - TLS certificate file path. Required if TLS is enabled. **No
//!   default**.
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
	error::Error,
	fmt::{Debug, Display, Formatter, Result as FmtResult},
	net::{AddrParseError, IpAddr, Ipv4Addr, Ipv6Addr},
	num::ParseIntError,
	str::FromStr,
};

use serde_derive::{Deserialize, Serialize};
use tracing::Level;

pub use self::{
	global::{Config, Hsts, Redirector, Tls},
	partial::{IntoPartialError, Partial, PartialHsts},
};
use crate::server::{IntoProtocolError, Protocol};

/// The error returned by fallible conversions into [`ListenAddress`],
/// containing the invalid input value
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntoListenAddressError {
	/// General listener address parse error
	#[error("\"{0}\" is not a valid listen address")]
	General(String),
	/// Parse error from the protocol
	#[error("{0}")]
	Protocol(#[from] IntoProtocolError),
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
			self.address.map_or("".to_string(), |a| match a {
				IpAddr::V4(a) => a.to_string(),
				IpAddr::V6(a) => format!("[{a}]"),
			}),
			self.port.map_or("".to_string(), |n| n.to_string())
		))
	}
}

impl PartialEq for ListenAddress {
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
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// The error returned by fallible conversions from a string to [`LogLevel`].
/// Includes the original input string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogLevelParseError(String);

impl Error for LogLevelParseError {}

impl Display for LogLevelParseError {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_fmt(format_args!("unknown log level: {}", self.0))
	}
}

impl FromStr for LogLevel {
	type Err = LogLevelParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"trace" => Ok(Self::Trace),
			"debug" => Ok(Self::Debug),
			"verbose" => Ok(Self::Verbose),
			"info" => Ok(Self::Info),
			"warn" | "warning" => Ok(Self::Warn),
			"error" => Ok(Self::Error),
			_ => Err(LogLevelParseError(s.to_string())),
		}
	}
}

impl Display for LogLevel {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_str(match self {
			Self::Trace => "trace",
			Self::Debug => "debug",
			Self::Verbose => "verbose",
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
		assert_eq!("warning".parse(), Ok(LogLevel::Warn));

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
