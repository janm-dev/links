//! This crate contains two normalized string datastructures:
//! - [`Normalized`], which represents unicode normalized strings
//! - [`Link`], which represents valid normalized redirection target URLs
//!
//! [`Normalized`] strings are case-insensitive and ignore whitespace and
//! control characters. They also perform [NFKC] normalization on the input.
//! They are used as vanity paths by links.
//!
//! [`Link`]s are normalized (in the URI sense) `http`/`https` URLs used as
//! redirect destinations by links.
//!
//! [NFKC]: https://www.unicode.org/reports/tr15/#Norm_Forms

use std::{
	convert::Infallible,
	fmt::{Display, Error as FmtError, Formatter},
	str::FromStr,
};

#[cfg(feature = "fred")]
use fred::{
	error::{RedisError, RedisErrorKind},
	types::{FromRedis, RedisValue},
};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use uriparse::{Scheme, URIReference};

/// A normalized string used for vanity paths. Allows for storing and comparing
/// vanity paths in a normalized, case-insensitive way. Also filters out
/// whitespace and control characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct Normalized(String);

impl Normalized {
	/// Create a new `Normalized` string, normalizing, filtering, and
	/// lowercasing the provided string.
	#[must_use]
	pub fn new(string: &str) -> Self {
		Self(
			string
				.nfkc()
				.filter(|c| !c.is_control())
				.filter(|c| !c.is_whitespace())
				.collect::<String>()
				.to_lowercase(),
		)
	}

	/// Returns the string this `Normalized` wraps, consuming `self`.
	#[must_use]
	// False positive, see https://github.com/rust-lang/rust-clippy/issues/4979
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_string(self) -> String {
		self.0
	}
}

impl Display for Normalized {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
		formatter.write_str(&self.0)
	}
}

impl FromStr for Normalized {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self::from(s))
	}
}

#[cfg(feature = "fred")]
impl FromRedis for Normalized {
	fn from_value(value: RedisValue) -> Result<Self, RedisError> {
		value.into_string().map_or_else(
			|| {
				Err(RedisError::new(
					RedisErrorKind::Parse,
					"can't convert this type into a Normalized",
				))
			},
			|s| Ok(Self::from(&*s)),
		)
	}
}

impl From<String> for Normalized {
	fn from(string: String) -> Self {
		Self::new(string.as_str())
	}
}

impl From<&String> for Normalized {
	fn from(string: &String) -> Self {
		Self::new(string.as_str())
	}
}

impl From<&str> for Normalized {
	fn from(string: &str) -> Self {
		Self::new(string)
	}
}

impl From<Normalized> for String {
	fn from(normalized: Normalized) -> Self {
		normalized.into_string()
	}
}

/// The error returned by fallible conversions into `Link`s.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
	/// The provided value is not a valid URL.
	#[error("url is invalid")]
	Invalid,
	/// The URL is relative (i.e. does not have a scheme and/or host).
	#[error("url is not absolute")]
	Relative,
	/// The URL has a scheme that is not `http` or `https`.
	#[error("url has a non-http/https scheme")]
	Scheme,
	/// The URL contains a password, which is considered potentially unsafe.
	#[error("url has credentials")]
	Unsafe,
}

/// A normalized URL used as the redirect destination. This ensures that the
/// link is a valid absolute HTTP(S) URL. The resulting `Link` is guaranteed to
/// have an `http` or `https` scheme, be an absolute URL, not have a password,
/// and be properly percent encoded. Note that this doesn't aim to make invalid
/// URLs valid (e.g. by percent encoding non-ascii characters), but may
/// normalize the provided URL (e.g. by decoding percent-encoded non-reserved
/// characters or by lowercasing the host). `Link` should not be used to create
/// a new, valid, properly encoded URL from user input, only to verify one, as
/// it doesn't provide much useful feedback or help with encoding an almost
/// valid URL, nor does it do much useful guesswork.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct Link(String);

impl Link {
	/// Valid Link URL schemes
	const VALID_SCHEMES: &'static [&'static str] = &["https", "http"];

	/// Create a new Link, checking the provided string.
	///
	/// # Errors
	/// This returns an error if the passed `url` is invalid
	/// (`LinkError::Invalid`), has a password (`LinkError::Unsafe`), has an
	/// invalid scheme (`LinkError::Scheme`, valid schemes are `http` and
	/// `https`), or is not absolute (`LinkError::Relative`).
	pub fn new(url: &str) -> Result<Self, LinkError> {
		let mut url = match URIReference::try_from(url) {
			Ok(url) => url,
			Err(_) => return Err(LinkError::Invalid),
		};

		if url.has_password() {
			return Err(LinkError::Unsafe);
		}

		url.normalize();

		if !Self::VALID_SCHEMES.contains(&url.scheme().map_or("", Scheme::as_str)) {
			return Err(LinkError::Scheme);
		}

		if url.is_uri() && url.has_authority() {
			Ok(Self(url.to_string()))
		} else {
			Err(LinkError::Relative)
		}
	}

	/// Create a new Link without performing any checks.
	///
	/// # Safety
	/// This makes no guarantees about the contents of the Link, the validity
	/// of the link must be ensured some other way before calling this.
	#[must_use]
	pub const fn new_unchecked(url: String) -> Self {
		Self(url)
	}

	/// Returns the string this `Link` wraps, consuming `self`.
	#[must_use]
	// False positive, see https://github.com/rust-lang/rust-clippy/issues/4979
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_string(self) -> String {
		self.0
	}
}

impl Display for Link {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
		formatter.write_str(&self.0)
	}
}

impl FromStr for Link {
	type Err = LinkError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_from(s)
	}
}

#[cfg(feature = "fred")]
impl FromRedis for Link {
	fn from_value(value: RedisValue) -> Result<Self, RedisError> {
		match value {
			RedisValue::String(s) => Ok(Self::try_from(&*s)
				.map_err(|e| RedisError::new(RedisErrorKind::Parse, e.to_string()))?),
			_ => Err(RedisError::new(
				RedisErrorKind::Parse,
				"can't convert this type into a Link",
			)),
		}
	}
}

impl TryFrom<String> for Link {
	type Error = LinkError;

	fn try_from(string: String) -> Result<Self, Self::Error> {
		Self::new(string.as_str())
	}
}

impl TryFrom<&String> for Link {
	type Error = LinkError;

	fn try_from(string: &String) -> Result<Self, Self::Error> {
		Self::new(string.as_str())
	}
}

impl TryFrom<&str> for Link {
	type Error = LinkError;

	fn try_from(string: &str) -> Result<Self, Self::Error> {
		Self::new(string)
	}
}

impl From<Link> for String {
	fn from(link: Link) -> Self {
		link.into_string()
	}
}

#[cfg(test)]
#[allow(clippy::unicode_not_nfc)]
mod tests {
	use std::cmp::Ordering;

	use super::*;

	#[test]
	fn normalized_new() {
		assert_eq!(Normalized::new("BiGbIrD"), Normalized::new("bigbird"));
		assert_eq!(Normalized::new("Big Bird	"), Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ"));

		let ohm = "Ω";
		let omega = "Ω";
		assert_ne!(ohm, omega);
		assert_eq!(Normalized::new(ohm), Normalized::new(omega));

		let letters = "ffi";
		let ligature = "ﬃ";
		assert_ne!(letters, ligature);
		assert_eq!(Normalized::new(letters), Normalized::new(ligature));
	}

	#[test]
	fn normalized_from_string() {
		let ohm = "Ω";
		let omega = "Ω";
		let letters = "ffi";
		let ligature = "ﬃ";

		assert_ne!(ohm, omega);
		assert_ne!(letters, ligature);

		assert_eq!(
			Normalized::from("BiGbIrD".to_string()),
			Normalized::from("bigbird".to_string())
		);
		assert_eq!(
			Normalized::from("Big Bird	".to_string()),
			Normalized::from(" ᴮᴵᴳᴮᴵᴿᴰ".to_string())
		);
		assert_eq!(
			Normalized::from(ohm.to_string()),
			Normalized::from(omega.to_string())
		);
		assert_eq!(
			Normalized::from(letters.to_string()),
			Normalized::from(ligature.to_string())
		);

		assert_eq!(
			Normalized::from(&"BiGbIrD".to_string()),
			Normalized::from(&"bigbird".to_string())
		);
		assert_eq!(
			Normalized::from(&"Big Bird	".to_string()),
			Normalized::from(&" ᴮᴵᴳᴮᴵᴿᴰ".to_string())
		);
		assert_eq!(
			Normalized::from(&ohm.to_string()),
			Normalized::from(&omega.to_string())
		);
		assert_eq!(
			Normalized::from(&letters.to_string()),
			Normalized::from(&ligature.to_string())
		);

		assert_eq!(Normalized::new("BiGbIrD"), Normalized::new("bigbird"));
		assert_eq!(Normalized::new("Big Bird	"), Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ"));
		assert_eq!(Normalized::new(ohm), Normalized::new(omega));
		assert_eq!(Normalized::new(letters), Normalized::new(ligature));

		assert_eq!(
			"BiGbIrD".parse::<Normalized>().unwrap(),
			Normalized::new("bigbird")
		);
		assert_eq!(
			"Big Bird	".parse::<Normalized>().unwrap(),
			Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ")
		);
		assert_eq!(ohm.parse::<Normalized>().unwrap(), Normalized::new(omega));
		assert_eq!(
			letters.parse::<Normalized>().unwrap(),
			Normalized::new(ligature)
		);
	}

	#[test]
	fn normalized_into_string() {
		assert_eq!(
			Normalized::new("BiGbIrD").into_string(),
			Normalized::new("bigbird").into_string()
		);

		assert_eq!(
			Normalized::new("BiGbIrD").to_string(),
			Normalized::new("bigbird").to_string()
		);
	}

	#[test]
	#[cfg(feature = "fred")]
	fn normalized_from_redis() {
		assert_eq!(
			Normalized::from_value(RedisValue::from_static_str("BiG bIrD"))
				.unwrap()
				.into_string(),
			"bigbird".to_string()
		);

		assert_eq!(
			Normalized::from_value(RedisValue::Integer(42))
				.unwrap()
				.into_string(),
			"42".to_string()
		);

		assert_eq!(
			Normalized::from_value(RedisValue::Null).unwrap_err().kind(),
			&RedisErrorKind::Parse
		);
	}

	#[test]
	fn normalized_serde() {
		assert_eq!(
			Normalized::new("BiGbIrD"),
			serde_json::from_str::<Normalized>(r#"" ᴮᴵᴳᴮᴵᴿᴰ""#).unwrap()
		);

		assert_eq!(
			Normalized::new("BiGbIrD"),
			serde_json::from_str::<Normalized>(
				&serde_json::to_string(&Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ")).unwrap()
			)
			.unwrap()
		);
	}

	#[test]
	#[allow(clippy::redundant_clone)]
	fn normalized_cmp() {
		assert_eq!(
			Normalized::new("Big Bird	").cmp(&Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ")),
			Ordering::Equal
		);
		assert_eq!(
			Normalized::new("SmaLlbIrD").cmp(&Normalized::new("smolbird")),
			Ordering::Less
		);
		assert_eq!(
			Normalized::new(" ˢᴹᵒᶫᴮᴵᴿᴰ").cmp(&Normalized::new("Small Bird	")),
			Ordering::Greater
		);

		assert_eq!(
			Normalized::new("Big Bird	").partial_cmp(&Normalized::new(" ᴮᴵᴳᴮᴵᴿᴰ")),
			Some(Ordering::Equal)
		);
		assert_eq!(
			Normalized::new("SmaLlbIrD").partial_cmp(&Normalized::new("smolbird")),
			Some(Ordering::Less)
		);
		assert_eq!(
			Normalized::new(" ˢᴹᵒᶫᴮᴵᴿᴰ").partial_cmp(&Normalized::new("Small Bird	")),
			Some(Ordering::Greater)
		);

		let ohm = "Ω";
		let omega = "Ω";
		assert_ne!(
			ohm.cmp(omega),
			Normalized::new(ohm).cmp(&Normalized::new(omega))
		);
		assert_ne!(
			ohm.partial_cmp(omega),
			Normalized::new(ohm).partial_cmp(&Normalized::new(omega))
		);
		assert!(Normalized::new(ohm) == Normalized::new(omega).clone());
		assert!(Normalized::new(ohm).clone() == Normalized::new(omega));

		let letters = "ffi";
		let ligature = "ﬃ";
		assert_ne!(
			letters.cmp(ligature),
			Normalized::new(letters).cmp(&Normalized::new(ligature))
		);
		assert_ne!(
			letters.partial_cmp(ligature),
			Normalized::new(letters).partial_cmp(&Normalized::new(ligature))
		);
		assert!(Normalized::new(letters) == Normalized::new(ligature).clone());
		assert!(Normalized::new(letters).clone() == Normalized::new(ligature));
	}

	#[test]
	fn link_new() {
		assert_eq!(
			Link::new("http://example.com").unwrap().into_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			Link::new("http://example.com").unwrap(),
			Link::new_unchecked("http://example.com/".to_string())
		);

		assert_eq!(
			Link::new("https://example.com/test?test=test#test")
				.unwrap()
				.into_string(),
			"https://example.com/test?test=test#test".to_string()
		);

		assert_eq!(
			Link::new("https://example.com/test?test=test#test").unwrap(),
			Link::new_unchecked("https://example.com/test?test=test#test".to_string())
		);

		assert_eq!(
			Link::new("HTtPS://eXaMpLe.com?").unwrap().into_string(),
			"https://example.com/?".to_string()
		);

		assert_eq!(
			Link::new("https://username@example.com/")
				.unwrap()
				.into_string(),
			"https://username@example.com/".to_string()
		);

		assert_eq!(
			Link::new("https://example.com/th%69%73/%69%73?a=test")
				.unwrap()
				.into_string(),
			"https://example.com/this/is?a=test".to_string()
		);

		assert_eq!(
			Link::new(
				"https://%65%78%61%6d%70%6c%65.%63%6f%6d/%74%68%69%73/%69%73?%61=%74%65%73%74"
			)
			.unwrap()
			.into_string(),
			"https://example.com/this/is?a=test".to_string()
		);

		assert_eq!(
			Link::new("https://example.com/%E1%B4%AE%E1%B4%B5%E1%B4%B3%E1%B4%AE%E1%B4%B5%E1%B4%BF%E1%B4%B0").unwrap().into_string(),
			"https://example.com/%E1%B4%AE%E1%B4%B5%E1%B4%B3%E1%B4%AE%E1%B4%B5%E1%B4%BF%E1%B4%B0".to_string()
		);

		assert_eq!(
			Link::new("https://xn--xmp-qla7xe00a.xn--m-uga3d/")
				.unwrap()
				.into_string(),
			"https://xn--xmp-qla7xe00a.xn--m-uga3d/".to_string()
		);

		assert!(Link::new("").is_err());

		assert!(Link::new("/test").is_err());

		assert!(Link::new("http:/test").is_err());

		assert!(Link::new("example.com/test").is_err());

		assert!(Link::new("//example.com/test").is_err());

		assert!(Link::new("ftp://example.com").is_err());

		assert!(Link::new("https_colon_slash_slash_example_dot_com_slash_test").is_err());

		assert!(Link::new("https://username:password@example.com").is_err());

		assert!(Link::new("https://êxämpłé.ćóm/ᴮᴵᴳ ᴮᴵᴿᴰ").is_err());
	}

	#[test]
	#[cfg(feature = "fred")]
	fn link_from_redis() {
		assert_eq!(
			Link::from_value(RedisValue::from_static_str(
				"https://EXAMPLE.com/test?test=test#test"
			))
			.unwrap(),
			Link::new("https://example.COM/test?test=test#test").unwrap()
		);

		assert!(Link::from_value(RedisValue::from_static_str(
			"https_colon_slash_slash_example_dot_com_slash_test"
		))
		.is_err());

		assert_eq!(
			Link::from_value(RedisValue::Null).unwrap_err().kind(),
			&RedisErrorKind::Parse
		);
	}

	#[test]
	fn link_serde() {
		assert_eq!(
			serde_json::from_str::<Link>(r#""https://EXAMPLE.com/test?test=test#test""#).unwrap(),
			Link::new("https://example.COM/test?test=test#test").unwrap()
		);

		assert_eq!(
			serde_json::from_str::<Link>(
				&serde_json::to_string(
					&Link::new("https://EXAMPLE.com/test?test=test#test").unwrap()
				)
				.unwrap()
			)
			.unwrap(),
			Link::new("https://example.COM/test?test=test#test").unwrap()
		);

		assert!(serde_json::from_str::<Link>(
			r#""https_colon_slash_slash_example_dot_com_slash_test""#
		)
		.is_err());
	}

	#[test]
	fn link_cmp() {
		assert_eq!(
			Link::new("https://example.com/test?test=test#test")
				.unwrap()
				.cmp(&Link::new("https://example.com/test?test=test#test").unwrap()),
			Ordering::Equal
		);

		assert_eq!(
			Link::new("https://example.com/test?test=test#test")
				.unwrap()
				.partial_cmp(&Link::new("https://example.com/test?test=test#test").unwrap()),
			Some(Ordering::Equal)
		);

		assert_eq!(
			Link::new("https://example.com/test?test=test#test")
				.unwrap()
				.into_string()
				.cmp(
					&Link::new("https://xn--xmp-qla7xe00a.xn--m-uga3d/")
						.unwrap()
						.into_string()
				),
			Ordering::Less
		);

		assert_eq!(
			Link::new("https://xn--xmp-qla7xe00a.xn--m-uga3d/")
				.unwrap()
				.into_string()
				.partial_cmp(
					&Link::new("https://example.com/test?test=test#test")
						.unwrap()
						.into_string()
				),
			Some(Ordering::Greater)
		);
	}

	#[test]
	fn link_to_from_string() {
		assert_eq!(
			Link::new("http://example.com").unwrap().to_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			Link::try_from("http://example.com").unwrap().to_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			Link::try_from("http://example.com".to_string())
				.unwrap()
				.to_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			Link::try_from(&"http://example.com".to_string())
				.unwrap()
				.to_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			"http://example.com".parse::<Link>().unwrap().into_string(),
			"http://example.com/".to_string()
		);

		assert_eq!(
			Link::new("https://example.com/test?test=test#test")
				.unwrap()
				.to_string(),
			"https://example.com/test?test=test#test".to_string()
		);

		assert_eq!(
			"https://example.com/test?test=test#test"
				.parse::<Link>()
				.unwrap()
				.into_string(),
			"https://example.com/test?test=test#test".to_string()
		);
	}
}
