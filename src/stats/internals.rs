//! Types that make up a links [`Statistic`]

use std::fmt::{Display, Formatter, Result as FmtResult};

use serde_derive::{Deserialize, Serialize};
use time::{
	format_description::well_known::{
		iso8601::{Config as TimeFormatConfig, EncodedConfig, TimePrecision},
		Iso8601,
	},
	macros::datetime,
	Duration, OffsetDateTime,
};

#[cfg(doc)]
use crate::stats::Statistic;

/// The data for a statistic
///
/// This struct holds the data associated with a statistic, that along with the
/// statistic's type and link comprises one full [`Statistic`]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StatisticData {
	data: String,
}

impl From<&str> for StatisticData {
	fn from(s: &str) -> Self {
		Self { data: s.into() }
	}
}

impl From<String> for StatisticData {
	fn from(s: String) -> Self {
		Self { data: s }
	}
}

impl Display for StatisticData {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_str(&self.data)
	}
}

/// The timestamp for a statistic
///
/// This timestamp is generally represented as the start of the period it
/// represents as an RFC3339/ISO8601 string (with the date, time with second
/// precision, and time zone `Z` for UTC), e.g. `2022-10-01T16:30:00Z`
///
/// Internally, this stores the number of 15 minute periods since the beginning
/// of the year 2000 UTC (e.g. on 2000-01-01 the period between 00:00:00.000 and
/// 00:14:59.999 UTC is 0 and 15:30:00.000 to 15:44:59.999 UTC is 62)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct StatisticTime {
	intervals: u32,
}

impl StatisticTime {
	/// The datetime representing the beginning of `0` in [`StatisticTime`]
	pub const EPOCH: OffsetDateTime = datetime!(2000-01-01 00:00:00 UTC);
	/// The resolution of a [`StatisticTime`] (15 minutes) in seconds
	pub const RESOLUTION_SECS: i64 = 15 * 60;

	/// Get the [`StatisticTime`] for now (the current time)
	#[must_use]
	pub fn now() -> Self {
		let intervals =
			(OffsetDateTime::now_utc() - Self::EPOCH).whole_seconds() / Self::RESOLUTION_SECS;

		Self {
			intervals: intervals.try_into().unwrap_or(u32::MAX),
		}
	}
}

impl From<OffsetDateTime> for StatisticTime {
	fn from(dt: OffsetDateTime) -> Self {
		let intervals = (dt - Self::EPOCH).whole_seconds() / Self::RESOLUTION_SECS;

		Self {
			intervals: intervals.try_into().unwrap_or(u32::MAX),
		}
	}
}

impl From<StatisticTime> for OffsetDateTime {
	fn from(st: StatisticTime) -> OffsetDateTime {
		let seconds = i64::from(st.intervals) * StatisticTime::RESOLUTION_SECS;

		StatisticTime::EPOCH + Duration::seconds(seconds)
	}
}

impl TryFrom<&str> for StatisticTime {
	type Error = time::Error;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		const TIME_FORMAT_CONFIG: EncodedConfig = TimeFormatConfig::DEFAULT
			.set_time_precision(TimePrecision::Second {
				decimal_digits: None,
			})
			.encode();

		let dt = OffsetDateTime::parse(s, &Iso8601::<TIME_FORMAT_CONFIG>)?;

		Ok(dt.into())
	}
}

impl From<StatisticTime> for String {
	fn from(st: StatisticTime) -> Self {
		st.to_string()
	}
}

impl Display for StatisticTime {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		// This doesn't use `time`'s datetime formatting, because that can fail
		let dt = OffsetDateTime::from(*self);

		let (year, month, day) = dt.to_calendar_date();
		let month = month as u8;

		let (hour, minute, second) = dt.to_hms();

		fmt.write_fmt(format_args!(
			"{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
		))
	}
}

/// The type of a links statistic
///
/// Each of the variants of this enum is one type of statistic, that along with
/// the statistic's data and link comprises one full [`Statistic`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StatisticType {
	/// Total number of requests
	///
	/// # Data
	/// This statistic type does not have any additional data
	Request,
	/// Number of requests to the specified host/domain
	///
	/// # Data
	/// The value of the [HTTP `Host` request header][host] or the [`:authority`
	/// pseudo-header field][authority], e.g. `example.com` or `10.0.0.25:8000`
	///
	/// [host]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Host
	/// [authority]: https://www.rfc-editor.org/rfc/rfc7540#section-8.1.2.3
	HostRequest,
	/// Number of requests with the specified [SNI]
	///
	/// # Data
	/// The value of the TLS [SNI], e.g. `example.com` or `www.links.example`
	///
	/// [SNI]: https://www.cloudflare.com/learning/ssl/what-is-sni/
	SniRequest,
	/// Number of requests that resulted in the given HTTP status code being
	/// returned
	///
	/// # Data
	/// The HTTP status code number, e.g. `404` or `308`
	StatusCode,
	/// Number of requests that used the given HTTP version
	///
	/// # Data
	/// The HTTP protocol version used for the request, e.g. `HTTP/1.0` or
	/// `HTTP/2`
	HttpVersion,
	/// Number of requests that used the given TLS version
	///
	/// # Data
	/// The TLS version (see [`ProtocolVersion`]) used for the request, e.g.
	/// `TLSv1.3` or `TLSv1.2`
	///
	/// [`ProtocolVersion`]: https://docs.rs/rustls/latest/rustls/enum.ProtocolVersion.html
	TlsVersion,
	/// Number of requests that used the provided TLS cipher suite
	///
	/// # Data
	/// The TLS cipher suite (see [`CipherSuite`]) used for the request, e.g.
	/// `TLS13_AES_256_GCM_SHA384` or `TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256`
	///
	/// [`CipherSuite`]: https://docs.rs/rustls/latest/rustls/enum.CipherSuite.html
	TlsCipherSuite,
	/// Number of requests by the provided user agent/browser
	///
	/// # Data
	/// The content of the [`Sec-CH-UA` HTTP header][sec-ch-ua], or in case that
	/// header is not available, the [`User-Agent` header][user-agent]
	///
	/// As recommended by the appropriate [standard], the data for this
	/// statistic is the entire value of the header. The header is not parsed
	/// into its individual components, instead it is simply copied verbatim.
	///
	/// [sec-ch-ua]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Sec-CH-UA
	/// [user-agent]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/User-Agent
	/// [standard]: https://wicg.github.io/ua-client-hints/#marketshare-analytics-use-case
	UserAgent,
	/// Number of requests by a user agent based on preference for a "mobile
	/// experience"
	///
	/// # Data
	/// The content of the [`Sec-CH-UA-Mobile` HTTP header][header], e.g. `?0`
	/// (for false/no) or `?1` (for true/yes)
	///
	/// [header]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Sec-CH-UA-Mobile
	UserAgentMobile,
	/// Number of requests by a user agent on the specified platform/operating
	/// system
	///
	/// # Data
	/// The content of the [`Sec-CH-UA-Platform` HTTP header][header], e.g.
	/// `Android` or `Windows`
	///
	/// [header]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Sec-CH-UA-Platform
	UserAgentPlatform,
}

impl TryFrom<&str> for StatisticType {
	type Error = ParseStatisticTypeError;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		match s {
			"request" => Ok(Self::Request),
			"host_request" => Ok(Self::HostRequest),
			"sni_request" => Ok(Self::SniRequest),
			"status_code" => Ok(Self::StatusCode),
			"http_version" => Ok(Self::HttpVersion),
			"tls_version" => Ok(Self::TlsVersion),
			"tls_cipher_suite" => Ok(Self::TlsCipherSuite),
			"user_agent" => Ok(Self::UserAgent),
			"user_agent_mobile" => Ok(Self::UserAgentMobile),
			"user_agent_platform" => Ok(Self::UserAgentPlatform),
			_ => Err(ParseStatisticTypeError),
		}
	}
}

impl From<StatisticType> for &str {
	fn from(stat_type: StatisticType) -> &'static str {
		match stat_type {
			StatisticType::Request => "request",
			StatisticType::HostRequest => "host_request",
			StatisticType::SniRequest => "sni_request",
			StatisticType::StatusCode => "status_code",
			StatisticType::HttpVersion => "http_version",
			StatisticType::TlsVersion => "tls_version",
			StatisticType::TlsCipherSuite => "tls_cipher_suite",
			StatisticType::UserAgent => "user_agent",
			StatisticType::UserAgentMobile => "user_agent_mobile",
			StatisticType::UserAgentPlatform => "user_agent_platform",
		}
	}
}

impl Display for StatisticType {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_str((*self).into())
	}
}

/// The error returned by fallible conversions into a [`StatisticType`]
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the provided value was not a valid statistic type")]
pub struct ParseStatisticTypeError;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn statistic_data() {
		assert_eq!(
			r#""Some Arbitrary Text Data""#,
			serde_json::to_string_pretty(&StatisticData::from("Some Arbitrary Text Data")).unwrap()
		);

		assert_eq!(
			serde_json::from_str::<StatisticData>(r#""!@#$%^&*()\" ᓚᘏᗢ""#).unwrap(),
			StatisticData::from(r#"!@#$%^&*()" ᓚᘏᗢ"#)
		);

		assert_ne!(
			r#""slightly different""#,
			serde_json::to_string_pretty(&StatisticData::from("hardly different")).unwrap()
		);

		assert_eq!(
			"Some Arbitrary Text Data",
			StatisticData::from("Some Arbitrary Text Data").to_string()
		);
	}

	#[test]
	fn statistic_time() {
		assert_ne!(StatisticTime::now(), StatisticTime::EPOCH.into());

		assert_eq!(
			StatisticTime::try_from(StatisticTime::now().to_string().as_str()).unwrap(),
			StatisticTime::now()
		);

		assert_eq!(
			StatisticTime::try_from("2022-10-08T16:30:00Z").unwrap(),
			StatisticTime::try_from("2022-10-08T16:34:25.159Z").unwrap()
		);

		assert_ne!(
			OffsetDateTime::from(StatisticTime::try_from("2022-10-08T16:34:25.159Z").unwrap()),
			datetime!(2022-10-08 16:34:25.159 UTC)
		);

		assert_eq!(
			OffsetDateTime::from(StatisticTime::try_from("2022-10-08T16:34:25.159Z").unwrap()),
			datetime!(2022-10-08 16:30:00.000 UTC)
		);

		assert!(dbg!(StatisticTime::now().to_string()).ends_with(":00Z"));
		assert_eq!(dbg!(StatisticTime::now().to_string()).len(), 20);

		let stat_time = StatisticTime::from(datetime!(2022-09-30 15:24:38 +2));

		assert_eq!(dbg!(stat_time.to_string()), "2022-09-30T13:15:00Z");
		assert_eq!(
			stat_time,
			StatisticTime::try_from(stat_time.to_string().as_str()).unwrap()
		);
		assert_eq!(
			stat_time,
			serde_json::from_str(&serde_json::to_string(&stat_time).unwrap()).unwrap()
		);

		let stat_time = StatisticTime::now();

		assert_eq!(
			stat_time,
			StatisticTime::try_from(stat_time.to_string().as_str()).unwrap()
		);
		assert_eq!(
			stat_time,
			serde_json::from_str(&serde_json::to_string(&stat_time).unwrap()).unwrap()
		);
	}

	#[test]
	fn statistic_type() {
		assert_eq!(
			StatisticType::HostRequest,
			serde_json::from_str(
				&serde_json::to_string_pretty(&StatisticType::HostRequest).unwrap()
			)
			.unwrap()
		);

		assert_eq!(
			r#""user_agent_platform""#,
			serde_json::to_string_pretty(&StatisticType::UserAgentPlatform).unwrap()
		);

		assert_eq!(
			StatisticType::HttpVersion,
			serde_json::from_str(r#""http_version""#).unwrap()
		);

		assert!(serde_json::from_str::<StatisticType>(r#""an_invalid_type""#).is_err());
	}
}
