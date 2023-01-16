//! Links statistics
//!
//! Statistics can be collected by the redirector server after every redirect
//! and have a numeric value indicating the number of requests performed by
//! someone/something that matches a particular [`Statistic`]. The value of the
//! statistic is simply incremented for every matching request.
//!
//! Statistics are represented as a key-value pair, where the key is [this
//! struct][`Statistic`] and the value is [a number][`StatisticValue`] that gets
//! incremented each time a statistic is collected. The actual internal
//! representation depends on the store backend, but could for example be a
//! string like `id-or-vanity:statistic-type:statistic-time:statistic-data`,
//! i.e. `example:user_agent_platform:2022-10-02T14:30:00Z:Windows`. It is also
//! important to note that statistics are collected individually, not in a
//! combined per-request object - they are simple counters, incremented per
//! request. This helps in preserving the users' privacy, because multiple
//! pieces of data can not reliably be correlated with each other, e.g. the
//! server may know that there were 22 requests from Firefox users and 19
//! requests using HTTP/2, but it can not know if any of these describe the same
//! request.
//!
//! Not all statistics are necessarily always collected. A store backend may not
//! support statistics, statistics may not be enabled in the configuration,
//! there may not be enough data to collect a specific statistic, or statistic
//! collection may fail. None of these situations are considered critical
//! errors; statistics are not an integral part of links.

mod internals;
mod misc;

use std::num::NonZeroU64;

use hyper::{http::HeaderValue, Request, StatusCode};
use serde_derive::{Deserialize, Serialize};

pub use self::{internals::*, misc::*};

/// A links statistic
///
/// Internally, a [`Statistic`] is made up of its [link][`IdOrVanity`] (e.g.
/// `07Qdzc9W` or `my-cool-link`), [type][`StatisticType`] (e.g. `HostRequest`
/// or `StatusCode`), [time][`StatisticTime`] (e.g. `2022-10-22T19:15:00Z`), and
/// [data][`StatisticData`] (e.g. `example.com` or `308`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Statistic {
	/// The ID or vanity path of the link that this statistic is about
	pub link: IdOrVanity,
	/// The type of this statistic
	#[serde(rename = "type")]
	pub stat_type: StatisticType,
	/// The data for this statistic
	pub data: StatisticData,
	/// The approximate time this statistic was collected at
	pub time: StatisticTime,
}

impl Statistic {
	/// Create a new [`Statistic`] from the provided information and the current
	/// time
	pub fn new(
		link: impl Into<IdOrVanity>,
		stat_type: StatisticType,
		data: impl Into<StatisticData>,
	) -> Self {
		Self {
			link: link.into(),
			stat_type,
			data: data.into(),
			time: StatisticTime::now(),
		}
	}

	/// Get all statistics from the provided [`ExtraStatisticInfo`] and other
	/// miscellaneous data. Only statistics specified by `categories` are
	/// returned.
	///
	/// The returned value is an iterator over statistics with some or all of
	/// the following types:
	/// - [`StatisticType::Request`]
	/// - [`StatisticType::StatusCode`]
	/// - [`StatisticType::SniRequest`]
	/// - [`StatisticType::TlsVersion`]
	/// - [`StatisticType::TlsCipherSuite`]
	pub fn get_misc(
		link: Option<&IdOrVanity>,
		stat_info: ExtraStatisticInfo,
		status_code: StatusCode,
		categories: StatisticCategories,
	) -> impl Iterator<Item = Statistic> {
		link.map_or_else(
			|| Vec::new().into_iter(),
			|link| {
				let mut stats = Vec::with_capacity(5);

				if categories.specifies(StatisticType::Request) {
					stats.push(Self::new(
						link,
						StatisticType::Request,
						StatisticData::default(),
					));
				}

				if categories.specifies(StatisticType::StatusCode) {
					stats.push(Self::new(
						link,
						StatisticType::StatusCode,
						status_code.as_str(),
					));
				}

				if categories.specifies(StatisticType::SniRequest) {
					if let Some(sni) = stat_info.tls_sni {
						stats.push(Self::new(link, StatisticType::SniRequest, sni.to_string()));
					}
				}

				if categories.specifies(StatisticType::TlsVersion) {
					if let Some(Some(version)) = stat_info.tls_version.map(|v| v.as_str()) {
						stats.push(Self::new(link, StatisticType::TlsVersion, version));
					}
				}

				if categories.specifies(StatisticType::TlsCipherSuite) {
					if let Some(Some(suite)) =
						stat_info.tls_cipher_suite.map(|s| s.suite().as_str())
					{
						stats.push(Self::new(link, StatisticType::TlsCipherSuite, suite));
					}
				}

				stats.into_iter()
			},
		)
	}

	/// Get all possible statistics from the provided HTTP request info. Only
	/// statistics specified by `categories` are returned.
	///
	/// The returned value is an iterator over statistics with some or all of
	/// the following types:
	/// - [`StatisticType::HostRequest`]
	/// - [`StatisticType::HttpVersion`]
	/// - [`StatisticType::UserAgent`]
	/// - [`StatisticType::UserAgentMobile`]
	/// - [`StatisticType::UserAgentPlatform`]
	pub fn from_req<T>(
		link: Option<&IdOrVanity>,
		req: &Request<T>,
		categories: StatisticCategories,
	) -> impl Iterator<Item = Statistic> {
		link.map_or_else(
			|| Vec::new().into_iter(),
			|link| {
				let mut stats = Vec::with_capacity(5);

				if categories.specifies(StatisticType::HttpVersion) {
					stats.push(Self::new(
						link,
						StatisticType::HttpVersion,
						HttpVersion::from(req.version()).as_str(),
					));
				}

				let headers = req.headers();

				if categories.specifies(StatisticType::HostRequest) {
					if let Some(Ok(host)) = req
						.uri()
						.host()
						.map(Ok)
						.or_else(|| headers.get("host").map(HeaderValue::to_str))
					{
						stats.push(Self::new(link, StatisticType::HostRequest, host));
					}
				}

				if categories.user_agent {
					if let Some(Ok(val)) = headers.get("sec-ch-ua").map(HeaderValue::to_str) {
						stats.push(Self::new(link, StatisticType::UserAgent, val));
					} else if let Some(Ok(val)) = headers.get("user-agent").map(HeaderValue::to_str)
					{
						stats.push(Self::new(link, StatisticType::UserAgent, val));
					}
				}

				if categories.specifies(StatisticType::UserAgentMobile) {
					if let Some(Ok(val)) = headers.get("sec-ch-ua-mobile").map(HeaderValue::to_str)
					{
						stats.push(Self::new(link, StatisticType::UserAgentMobile, val));
					}
				}

				if categories.specifies(StatisticType::UserAgentPlatform) {
					if let Some(Ok(val)) =
						headers.get("sec-ch-ua-platform").map(HeaderValue::to_str)
					{
						stats.push(Self::new(link, StatisticType::UserAgentPlatform, val));
					}
				}

				stats.into_iter()
			},
		)
	}
}

/// A description of one or more [`Statistic`]s, where some fields may be
/// omitted so that they act as a wildcard
///
/// This struct is intended for use with the links store and RPC API, and can
/// describe multiple statistics by specifying only some of a statistic's
/// fields. When a field is omitted, all values for it are accepted. Therefore,
/// for example to get all statistics for a given link regardless of type, data,
/// or time, only the `link` field of the [`StatisticDescription`] would be
/// `Some(...)`, while all others are `None`. If all fields are `None`, then all
/// statistics for all links are matched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatisticDescription {
	/// The ID or vanity path of the link that this statistic is about
	pub link: Option<IdOrVanity>,
	/// The type of this statistic, see [`StatisticType`]
	#[serde(rename = "type")]
	pub stat_type: Option<StatisticType>,
	/// The data for this statistic
	pub data: Option<StatisticData>,
	/// The approximate time this statistic was collected at
	pub time: Option<StatisticTime>,
}

impl StatisticDescription {
	/// Check whether the provided [`Statistic`] matches this description
	#[must_use]
	pub fn matches(&self, stat: &Statistic) -> bool {
		(self.link.is_none() || self.link.as_ref() == Some(&stat.link))
			&& (self.stat_type.is_none() || self.stat_type.as_ref() == Some(&stat.stat_type))
			&& (self.data.is_none() || self.data.as_ref() == Some(&stat.data))
			&& (self.time.is_none() || self.time.as_ref() == Some(&stat.time))
	}
}

/// The value of a links statistic
///
/// A [`StatisticValue`] represents the number of requests matching a particular
/// [`Statistic`]. This gets incremented (inside of the store) every time a
/// statistic is collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StatisticValue {
	count: NonZeroU64,
}

impl StatisticValue {
	/// Create a new [`StatisticValue`] with the provided count
	///
	/// This function returns `None` if the count is 0
	#[must_use]
	pub fn new(count: u64) -> Option<Self> {
		Some(Self {
			count: NonZeroU64::new(count)?,
		})
	}

	/// Get the numeric count of this statistic value
	///
	/// The returned value is never 0. If the returned value is used to
	/// construct a [`NonZeroU64`], you can instead use `get_nonzero` to get one
	/// directly.
	#[must_use]
	pub const fn get(self) -> u64 {
		self.count.get()
	}

	/// Get the numeric count of this statistic value as a [`NonZeroU64`]
	#[must_use]
	pub const fn get_nonzero(self) -> NonZeroU64 {
		self.count
	}

	/// Increment this [`StatisticValue`], returning the next value up
	#[must_use]
	pub fn increment(self) -> Self {
		// `NonZeroU64::saturating_add` has an MSRV of 1.64, so we do this instead.
		// Performance is identical in release builds (use show assembly):
		// https://play.rust-lang.org/?version=stable&mode=release&edition=2021&gist=eab44903e3eb0921a793167ff3ad2f79.
		let count = NonZeroU64::new(self.get().saturating_add(1)).expect("n + 1 != 0");

		Self { count }
	}
}

impl Default for StatisticValue {
	fn default() -> Self {
		Self::new(1).expect("1 is not 0")
	}
}

#[cfg(test)]
mod tests {
	use links_id::Id;
	use links_normalized::Normalized;
	use tokio_rustls::rustls::{ProtocolVersion, ALL_CIPHER_SUITES};

	use super::*;

	#[test]
	fn statistic() {
		assert_eq!(
			Statistic::new(Id::new(), StatisticType::Request, "").time,
			StatisticTime::now()
		);

		let id = Id::new();
		assert_eq!(
			Statistic::new(id, StatisticType::Request, "").link,
			id.into()
		);

		assert_eq!(
			Statistic::new(Id::new(), StatisticType::Request, "").stat_type,
			StatisticType::Request
		);

		assert_eq!(
			Statistic::new(Id::new(), StatisticType::Request, "this is a test").data,
			"this is a test".into()
		);

		assert_eq!(
			Statistic::new(Normalized::new("a link"), StatisticType::StatusCode, "501"),
			Statistic {
				link: IdOrVanity::Vanity(Normalized::new("a link")),
				stat_type: StatisticType::StatusCode,
				time: StatisticTime::now(),
				data: StatisticData::from("501")
			}
		);
	}

	#[test]
	fn statistic_collection() {
		let stats = Statistic::get_misc(
			Some(&Normalized::new("test").into()),
			ExtraStatisticInfo {
				tls_sni: Some("example.com".into()),
				tls_version: Some(ProtocolVersion::TLSv1_3),
				tls_cipher_suite: Some(ALL_CIPHER_SUITES[0]),
			},
			StatusCode::TEMPORARY_REDIRECT,
			StatisticCategories::ALL,
		)
		.map(|s| s.stat_type)
		.collect::<Vec<_>>();

		assert!(stats.contains(&StatisticType::Request));
		assert!(stats.contains(&StatisticType::StatusCode));
		assert!(stats.contains(&StatisticType::SniRequest));
		assert!(stats.contains(&StatisticType::TlsVersion));
		assert!(stats.contains(&StatisticType::TlsCipherSuite));

		let stats = Statistic::from_req(
			Some(&Normalized::new("test").into()),
			&Request::builder()
				.header("Host", "example.com")
				.header(
					"Sec-CH-UA",
					r#"" Not A;Brand";v="99", "Chromium";v="96", "Google Chrome";v="96""#,
				)
				.header("Sec-CH-UA-Mobile", "?0")
				.header("Sec-CH-UA-Platform", "Windows")
				.body(Vec::<u8>::new())
				.unwrap(),
			StatisticCategories::ALL,
		)
		.map(|s| s.stat_type)
		.collect::<Vec<_>>();

		assert!(stats.contains(&StatisticType::HostRequest));
		assert!(stats.contains(&StatisticType::HttpVersion));
		assert!(stats.contains(&StatisticType::UserAgent));
		assert!(stats.contains(&StatisticType::UserAgentMobile));
		assert!(stats.contains(&StatisticType::UserAgentPlatform));

		let mut stats = Statistic::from_req(
			Some(&Normalized::new("test").into()),
			&Request::builder()
				.header("Host", "example.com")
				.header(
					"Sec-CH-UA",
					r#"" Not A;Brand";v="99", "Chromium";v="96", "Google Chrome";v="96""#,
				)
				.header("Sec-CH-UA-Mobile", "?0")
				.header("Sec-CH-UA-Platform", "Windows")
				.body(Vec::<u8>::new())
				.unwrap(),
			StatisticCategories::NONE,
		);

		// Nothing collected
		assert!(stats.next().is_none());

		let stats = Statistic::from_req(
			Some(&Normalized::new("test").into()),
			&Request::builder()
				.header("Host", "example.com")
				.header(
					"Sec-CH-UA",
					r#"" Not A;Brand";v="99", "Chromium";v="96", "Google Chrome";v="96""#,
				)
				.header("Sec-CH-UA-Mobile", "?0")
				.header("Sec-CH-UA-Platform", "Windows")
				.body(Vec::<u8>::new())
				.unwrap(),
			StatisticCategories::default(),
		)
		.map(|s| s.stat_type)
		.collect::<Vec<_>>();

		assert!(stats.contains(&StatisticType::HostRequest));
		assert!(stats.contains(&StatisticType::HttpVersion));
		assert!(!stats.contains(&StatisticType::UserAgent));
		assert!(!stats.contains(&StatisticType::UserAgentMobile));
		assert!(!stats.contains(&StatisticType::UserAgentPlatform));
	}

	#[test]
	fn statistic_serde() {
		let vanity = Normalized::new("test-vanity");

		let stats = vec![
			Statistic::new(vanity.clone(), StatisticType::Request, ""),
			Statistic::new(
				vanity.clone(),
				StatisticType::UserAgent,
				r#"" Not A;Brand";v="99", "Chromium";v="96", "Google Chrome";v="96""#,
			),
			Statistic::new(vanity, StatisticType::HttpVersion, HttpVersion::V2.as_str()),
		];

		let json = serde_json::to_string(&stats).unwrap();

		let parsed_stats = serde_json::from_str::<Vec<Statistic>>(&json).unwrap();

		assert_eq!(stats, parsed_stats);

		let id = Id::from([1, 2, 3, 4, 5]);

		let stats = vec![
			Statistic::new(id, StatisticType::Request, ""),
			Statistic::new(
				id,
				StatisticType::UserAgent,
				r#"" Not A;Brand";v="99", "Chromium";v="96", "Google Chrome";v="96""#,
			),
			Statistic::new(id, StatisticType::HttpVersion, HttpVersion::V2.as_str()),
		];

		let json = serde_json::to_string_pretty(&stats).unwrap();

		let parsed_stats = serde_json::from_str::<Vec<Statistic>>(&json).unwrap();

		assert_eq!(stats, parsed_stats);
	}

	#[test]
	fn statistic_description() {
		let desc = StatisticDescription {
			link: None,
			stat_type: None,
			data: None,
			time: None,
		};

		assert!(desc.matches(&Statistic::new(Id::new(), StatisticType::Request, "")));
		assert!(desc.matches(&Statistic::new(
			Normalized::new("a test"),
			StatisticType::StatusCode,
			"400"
		)));

		let desc = StatisticDescription {
			link: Some(Normalized::new("a test").into()),
			stat_type: None,
			data: None,
			time: None,
		};

		assert!(!desc.matches(&Statistic::new(Id::new(), StatisticType::Request, "")));
		assert!(desc.matches(&Statistic::new(
			Normalized::new("a test"),
			StatisticType::StatusCode,
			"400"
		)));

		let desc = StatisticDescription {
			link: Some(Normalized::new("a test").into()),
			stat_type: Some(StatisticType::Request),
			data: None,
			time: None,
		};

		assert!(!desc.matches(&Statistic::new(Id::new(), StatisticType::Request, "")));
		assert!(!desc.matches(&Statistic::new(
			Normalized::new("a test"),
			StatisticType::StatusCode,
			"400"
		)));

		let desc = StatisticDescription {
			link: None,
			stat_type: Some(StatisticType::Request),
			data: None,
			time: None,
		};

		assert!(desc.matches(&Statistic::new(Id::new(), StatisticType::Request, "")));
		assert!(!desc.matches(&Statistic::new(
			Normalized::new("a test"),
			StatisticType::StatusCode,
			"400"
		)));

		let desc = StatisticDescription {
			link: None,
			stat_type: None,
			data: None,
			time: Some(StatisticTime::try_from("2020-01-01T12:34:56.789Z").unwrap()),
		};

		assert!(!desc.matches(&Statistic::new(Id::new(), StatisticType::Request, "")));
		assert!(!desc.matches(&Statistic::new(
			Normalized::new("a test"),
			StatisticType::StatusCode,
			"400"
		)));
	}

	#[test]
	fn statistic_value() {
		assert_eq!(StatisticValue::new(1), Some(StatisticValue::default()));
		assert!(StatisticValue::new(0).is_none());
		assert!(StatisticValue::new(1).is_some());
		assert!(StatisticValue::new(1_000_000).is_some());

		let stat_val = StatisticValue::default();
		assert_eq!(stat_val.get(), 1);
		let stat_val = stat_val.increment();
		assert_eq!(stat_val.get(), 2);

		let stat_val = StatisticValue::default();
		assert_eq!(stat_val.get_nonzero(), NonZeroU64::new(1).unwrap());
		let stat_val = stat_val.increment();
		assert_eq!(stat_val.get_nonzero(), NonZeroU64::new(2).unwrap());

		let stat_val = StatisticValue::new(u64::MAX - 1).unwrap();
		assert_eq!(stat_val.get(), u64::MAX - 1);
		let stat_val = stat_val.increment();
		assert_eq!(stat_val.get(), u64::MAX);
		let stat_val = stat_val.increment();
		assert_eq!(stat_val.get(), u64::MAX);
	}
}
