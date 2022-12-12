//! Miscellaneous statistics-related types

use std::{
	convert::Infallible,
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr,
	sync::Arc,
};

use hyper::Version;
use serde::Serialize;
use serde_derive::{Deserialize, Serialize};
use tokio_rustls::rustls::{ProtocolVersion, SupportedCipherSuite};

use super::StatisticType;
use crate::{id::Id, normalized::Normalized};

/// Extra statistics-related information passed to the links HTTP redirector for
/// collection
#[derive(Debug, Clone, Default)]
pub struct ExtraStatisticInfo {
	/// The server name indication from TLS, if available
	pub tls_sni: Option<Arc<str>>,
	/// The version of TLS used, if any
	pub tls_version: Option<ProtocolVersion>,
	/// The negotiated TLS cipher suite, if any
	pub tls_cipher_suite: Option<SupportedCipherSuite>,
}

/// A links ID or vanity path
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdOrVanity {
	/// A links [`Id`]
	Id(Id),
	/// A links vanity path as a [`Normalized`]
	Vanity(Normalized),
}

impl Display for IdOrVanity {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_str(&match self {
			Self::Id(id) => id.to_string(),
			Self::Vanity(vanity) => vanity.to_string(),
		})
	}
}

impl From<&str> for IdOrVanity {
	fn from(s: &str) -> Self {
		Id::from_str(s).map_or_else(|_| Self::Vanity(Normalized::new(s)), Self::Id)
	}
}

impl From<String> for IdOrVanity {
	fn from(s: String) -> Self {
		Id::from_str(s.as_str()).map_or_else(|_| Self::Vanity(Normalized::from(s)), Self::Id)
	}
}

impl From<Id> for IdOrVanity {
	fn from(id: Id) -> Self {
		Self::Id(id)
	}
}

impl From<Normalized> for IdOrVanity {
	fn from(vanity: Normalized) -> Self {
		Self::Vanity(vanity)
	}
}

impl From<&IdOrVanity> for IdOrVanity {
	fn from(iov: &Self) -> Self {
		iov.clone()
	}
}

/// Which categories of statistics are to be collected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Vec<&str>", into = "Vec<&'static str>")]
#[non_exhaustive]
#[allow(clippy::struct_excessive_bools)]
pub struct StatisticCategories {
	/// Collect [`StatisticType::Request`]
	pub redirect: bool,
	/// Collect [`StatisticType::HostRequest`], [`StatisticType::SniRequest`],
	/// and [`StatisticType::StatusCode`]
	pub basic: bool,
	/// Collect [`StatisticType::HttpVersion`], [`StatisticType::TlsVersion`],
	/// and [`StatisticType::TlsCipherSuite`]
	pub protocol: bool,
	/// Collect [`StatisticType::UserAgent`],
	/// [`StatisticType::UserAgentMobile`],
	/// and [`StatisticType::UserAgentPlatform`]
	pub user_agent: bool,
}

impl StatisticCategories {
	/// All categories enabled
	pub const ALL: Self = Self {
		redirect: true,
		basic: true,
		protocol: true,
		user_agent: true,
	};
	/// No categories enabled
	pub const NONE: Self = Self {
		redirect: false,
		basic: false,
		protocol: false,
		user_agent: false,
	};

	/// Whether this [`StatisticCategories`] struct specifies that a statistic
	/// with the provided [`StatisticType`] should be collected
	#[must_use]
	pub const fn specifies(self, stat_type: StatisticType) -> bool {
		#[allow(clippy::enum_glob_use)]
		use StatisticType::*;

		match stat_type {
			Request => self.redirect,
			HostRequest | SniRequest | StatusCode => self.basic,
			HttpVersion | TlsVersion | TlsCipherSuite => self.protocol,
			UserAgent | UserAgentMobile | UserAgentPlatform => self.user_agent,
		}
	}

	/// Convert this [`StatisticCategories`] into a `Vec` of the names of its
	/// enabled categories
	///
	/// # Example
	/// ```rust
	/// # use links::stats::StatisticCategories;
	/// let mut categories = StatisticCategories::NONE;
	///
	/// categories.redirect = true;
	/// categories.basic = false;
	/// categories.protocol = true;
	/// categories.user_agent = false;
	///
	/// assert_eq!(categories.to_names(), vec!["redirect", "protocol"]);
	/// ```
	#[must_use]
	pub fn to_names(self) -> Vec<&'static str> {
		let mut names = Vec::with_capacity(4);

		if self.redirect {
			names.push("redirect");
		}

		if self.basic {
			names.push("basic");
		}

		if self.protocol {
			names.push("protocol");
		}

		if self.user_agent {
			names.push("user-agent");
		}

		names
	}

	/// Convert a list of category names into a [`StatisticCategories`].
	/// Unrecognized category names are ignored.
	///
	/// # Example
	/// ```rust
	/// # use links::stats::StatisticCategories;
	/// let list = ["redirect", "protocol", "invalid"];
	/// let mut categories = StatisticCategories::from_names(list);
	///
	/// assert!(categories.redirect);
	/// assert!(!categories.basic);
	/// assert!(categories.protocol);
	/// assert!(!categories.user_agent);
	/// ```
	#[must_use]
	pub fn from_names<L, T>(categories: L) -> Self
	where
		L: AsRef<[T]>,
		T: AsRef<str>,
	{
		let mut cats = Self::NONE;

		for cat in categories.as_ref() {
			match cat.as_ref() {
				"redirect" => cats.redirect = true,
				"basic" => cats.basic = true,
				"protocol" => cats.protocol = true,
				"user-agent" => cats.user_agent = true,
				_ => (),
			}
		}

		cats
	}
}

impl Default for StatisticCategories {
	fn default() -> Self {
		Self {
			redirect: true,
			basic: true,
			protocol: true,
			user_agent: false,
		}
	}
}

impl From<Vec<&str>> for StatisticCategories {
	fn from(names: Vec<&str>) -> Self {
		Self::from_names(names)
	}
}

impl From<StatisticCategories> for Vec<&'static str> {
	fn from(cats: StatisticCategories) -> Self {
		cats.to_names()
	}
}

/// An HTTP protocol version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(from = "&str")]
#[non_exhaustive]
pub enum HttpVersion {
	/// HTTP version 0.9
	V09,
	/// HTTP version 1.0
	V10,
	/// HTTP version 1.1
	V11,
	/// HTTP version 2
	V2,
	/// HTTP version 3
	V3,
	/// Unknown HTTP version
	Unknown,
}

impl HttpVersion {
	/// Get a string representing the HTTP version
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			HttpVersion::V09 => "HTTP/0.9",
			HttpVersion::V10 => "HTTP/1.0",
			HttpVersion::V11 => "HTTP/1.1",
			HttpVersion::V2 => "HTTP/2",
			HttpVersion::V3 => "HTTP/3",
			HttpVersion::Unknown => "HTTP/???",
		}
	}
}

impl FromStr for HttpVersion {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(s.into())
	}
}

impl From<&str> for HttpVersion {
	fn from(s: &str) -> Self {
		match s.trim_start_matches("HTTP/").trim_start_matches("http/") {
			"0.9" => Self::V09,
			"1.0" => Self::V10,
			"1.1" => Self::V11,
			"2" => Self::V2,
			"3" => Self::V3,
			_ => Self::Unknown,
		}
	}
}

impl Serialize for HttpVersion {
	fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		ser.serialize_str(self.as_str())
	}
}

impl From<Version> for HttpVersion {
	fn from(v: Version) -> Self {
		match v {
			Version::HTTP_09 => Self::V09,
			Version::HTTP_10 => Self::V10,
			Version::HTTP_11 => Self::V11,
			Version::HTTP_2 => Self::V2,
			Version::HTTP_3 => Self::V3,
			_ => Self::Unknown,
		}
	}
}

impl TryFrom<HttpVersion> for Version {
	type Error = UnknownHttpVersionError;

	fn try_from(v: HttpVersion) -> Result<Self, Self::Error> {
		match v {
			HttpVersion::V09 => Ok(Self::HTTP_09),
			HttpVersion::V10 => Ok(Self::HTTP_10),
			HttpVersion::V11 => Ok(Self::HTTP_11),
			HttpVersion::V2 => Ok(Self::HTTP_2),
			HttpVersion::V3 => Ok(Self::HTTP_3),
			HttpVersion::Unknown => Err(UnknownHttpVersionError),
		}
	}
}

/// The error returned when attempting to convert an [`HttpVersion::Unknown`] to
/// another type that can't represent that value
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the HTTP version is unknown")]
pub struct UnknownHttpVersionError;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn id_or_vanity() {
		assert_eq!(
			IdOrVanity::Id([0x11, 0x33, 0x55, 0x77, 0x99].into()),
			IdOrVanity::try_from("0fXMgWQz").unwrap()
		);

		assert_eq!(
			IdOrVanity::Vanity("example-test".into()),
			IdOrVanity::try_from("example-test").unwrap()
		);
	}

	#[test]
	fn statistic_categories() {
		assert_eq!(Vec::<&str>::new(), StatisticCategories::NONE.to_names());

		assert_eq!(
			StatisticCategories::from_names(Vec::<&str>::new()),
			StatisticCategories::NONE
		);

		assert_eq!(
			vec!["redirect", "basic", "protocol"],
			StatisticCategories::default().to_names()
		);

		let names = vec!["protocol", "user-agent"];
		assert_eq!(names, StatisticCategories::from_names(&names).to_names());

		let names = vec!["protocol", "user-agent"];
		assert_eq!(
			names,
			Vec::<&str>::from(StatisticCategories::from(names.clone()))
		);

		let categories = StatisticCategories::default();
		assert!(categories.specifies(StatisticType::Request));
		assert!(categories.specifies(StatisticType::HostRequest));
		assert!(categories.specifies(StatisticType::SniRequest));
		assert!(categories.specifies(StatisticType::HttpVersion));
		assert!(!categories.specifies(StatisticType::UserAgent));
		assert!(!categories.specifies(StatisticType::UserAgentPlatform));

		assert_eq!(
			serde_json::from_str::<StatisticCategories>(r#"["redirect", "basic", "protocol"]"#)
				.unwrap(),
			StatisticCategories::default()
		);

		assert_eq!(
			serde_json::from_str::<StatisticCategories>(
				&serde_json::to_string(&StatisticCategories::ALL).unwrap()
			)
			.unwrap(),
			StatisticCategories::ALL
		);
	}

	#[test]
	fn http_version() {
		assert_eq!(
			HttpVersion::from_str("HTTP/1.1").unwrap().as_str(),
			"HTTP/1.1"
		);

		assert_eq!(
			HttpVersion::try_from(HttpVersion::V2.as_str()).unwrap(),
			HttpVersion::V2
		);

		assert!(Version::try_from(HttpVersion::Unknown).is_err());

		assert_eq!(
			Version::try_from(HttpVersion::V10).unwrap(),
			Version::HTTP_10
		);
	}
}
