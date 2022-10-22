//! This module contains everything pertaining to links IDs, the unique
//! identifiers used in this project to identify individual links.
//!
//! A links ID is a randomly generated 40 bit / 5 byte integer. It is usually
//! encoded as an 8 character string, starting with a digit (`0-9`) followed by
//! 7 base 38 characters. The character set used for base 38 is a modified
//! [`nanoid-dictionary nolookalikesSafe`](https://github.com/CyberAP/nanoid-dictionary#nolookalikessafe),
//! with 2 additional characters - `X` and `x`. The full charset is (in order):
//! `6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz`.

use std::{
	fmt::{Debug, Display, Error as FmtError, Formatter},
	str::FromStr,
};

use fred::{
	error::{RedisError, RedisErrorKind},
	types::{FromRedis, RedisValue},
};
use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
use serde_derive::{Deserialize, Serialize};

/// The error returned by fallible conversions into IDs.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
	/// The provided value is too large to be represented in an `Id`.
	#[error("value is too large")]
	TooLarge,
	/// The provided value is too small to be represented in an `Id`.
	#[error("value is too small")]
	TooSmall,
	/// The provided value is improperly formatted.
	#[error("value is in an invalid format")]
	InvalidFormat,
}

/// The character set used for base 10 encoding (digits). Used by the first
/// character of the Id string representation.
pub const BASE_10_CHARSET: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// The character set used for the base 38 encoding of the last 7 characters of
/// the string representation of an Id.
pub const BASE_38_CHARSET: [char; 38] = [
	'6', '7', '8', '9', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'T',
	'W', 'X', 'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 't', 'w', 'x', 'z',
];

/// Reverse base 38 character set used for decoding encoded IDs.
/// The numeric value of a base 38 character is available at `ascii - 54`.
pub const REVERSE_CHARSET: [u8; 69] = [
	0, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 4, 5, 6, 0, 7, 8, 9, 0, 10, 11, 12, 13, 14, 0, 15, 16, 17,
	0, 18, 0, 0, 19, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 21, 22, 23, 0, 24, 25, 26, 0, 27, 28, 0, 29,
	30, 0, 31, 32, 33, 0, 34, 0, 0, 35, 36, 0, 37,
];

/// The offset from the ASCII value of a base 38 character to its index in
/// `REVERSE_CHARSET`. To get the index of a character in `REVERSE_CHARSET`,
/// subtract `REVERSE_CHARSET_BASE_38_OFFSET` from its value (`ascii - this`).
pub const REVERSE_CHARSET_BASE_38_OFFSET: usize = 54;

/// The 40 bit ID used to identify links in links.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct Id([u8; 5]);

impl Id {
	/// The number of bits in an Id.
	pub const BITS: usize = 40;
	/// The number of bytes in an Id.
	pub const BYTES: usize = 5;
	/// The number of characters in the usual Id representation.
	pub const CHARS: usize = 8;
	/// The maximum value of an Id when represented as a number.
	#[allow(clippy::cast_possible_truncation)]
	pub const MAX: u64 = 2u64.pow(Self::BITS as u32) - 1;
	/// The minimum value of an Id when represented as a number
	pub const MIN: u64 = 0;

	/// Check if a string representation of an Id is valid.
	#[must_use]
	pub fn is_valid(id: &str) -> bool {
		lazy_static! {
			static ref RE: Regex =
				RegexBuilder::new("^[0123456789][6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz]{7}$")
					.case_insensitive(false)
					.dot_matches_new_line(false)
					.ignore_whitespace(false)
					.multi_line(false)
					.octal(false)
					.unicode(true)
					.build()
					.unwrap();
			static ref MAX: String = Id::try_from(Id::MAX).unwrap().to_string();
		}

		RE.is_match(id) && id <= MAX.as_str()
	}

	/// Create a new random Id.
	#[must_use]
	pub fn new() -> Self {
		Self(rand::random())
	}

	/// Convert this `Id` into a `u64`.
	#[must_use]
	pub fn to_u64(self) -> u64 {
		let mut buf = [0u8; 8];
		buf[3..].copy_from_slice(&self.0);

		u64::from_be_bytes(buf)
	}
}

impl Debug for Id {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
		if formatter.alternate() {
			formatter.debug_tuple("Id").field(&self.0).finish()
		} else {
			formatter
				.debug_tuple("Id")
				.field(&self.to_string())
				.finish()
		}
	}
}

impl Display for Id {
	#[allow(clippy::cast_possible_truncation)]
	fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
		let mut buf = ['\0'; Self::CHARS];
		let num = self.to_u64();

		buf[0] = BASE_10_CHARSET[((num / 38u64.pow(Self::CHARS as u32 - 1)) % 10) as usize];

		for (i, char) in buf.iter_mut().enumerate().skip(1) {
			let index = (num / 38u64.pow((Self::CHARS - 1 - i) as u32)) % 38;

			*char = BASE_38_CHARSET[index as usize];
		}

		let buf: String = buf.iter().collect();
		formatter.write_str(&buf)
	}
}

impl Default for Id {
	fn default() -> Self {
		Self::new()
	}
}

impl FromStr for Id {
	type Err = ConversionError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::try_from(s)
	}
}

impl FromRedis for Id {
	fn from_value(value: RedisValue) -> Result<Self, RedisError> {
		match value {
			RedisValue::String(s) => Ok(Self::try_from(&*s)
				.map_err(|e| RedisError::new(RedisErrorKind::Parse, e.to_string()))?),
			RedisValue::Bytes(b) => Ok(Self::try_from(&*b)
				.map_err(|e| RedisError::new(RedisErrorKind::Parse, e.to_string()))?),
			_ => Err(RedisError::new(
				RedisErrorKind::Parse,
				"can't convert this type into an ID",
			)),
		}
	}
}

impl From<[u8; 5]> for Id {
	fn from(bytes: [u8; 5]) -> Self {
		Self(bytes)
	}
}

impl TryFrom<&[u8]> for Id {
	type Error = ConversionError;

	fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
		match bytes.len() {
			5 => Ok(Self(
				bytes.try_into().map_err(|_| Self::Error::InvalidFormat)?,
			)),
			0..=4 => Err(Self::Error::TooSmall),
			_ => Err(Self::Error::TooLarge),
		}
	}
}

impl TryFrom<&str> for Id {
	type Error = ConversionError;

	fn try_from(string: &str) -> Result<Self, Self::Error> {
		#[allow(
			clippy::cast_possible_truncation,
			clippy::cast_lossless,
			clippy::cast_sign_loss,
			clippy::cast_possible_wrap
		)]
		if string.len() < Self::CHARS {
			Err(ConversionError::TooSmall)
		} else if string.len() > Self::CHARS {
			Err(ConversionError::TooLarge)
		} else if !Self::is_valid(string) {
			Err(ConversionError::InvalidFormat)
		} else {
			// Panic: `string` has already been checked to have only valid characters, so
			// this always succeeds.
			let mut num = string.chars().next().unwrap().to_digit(10).unwrap() as u64
				* 38u64.pow((Self::CHARS - 1) as u32);

			#[allow(clippy::cast_possible_truncation)]
			for (i, char) in string.chars().rev().enumerate().take(Self::CHARS - 1) {
				// Panic (indexing): `string` has already been checked to have only valid
				// characters, so this always succeeds.
				num += REVERSE_CHARSET[char.encode_utf8(&mut [0u8]).as_bytes()[0] as usize
					- REVERSE_CHARSET_BASE_38_OFFSET] as u64
					* 38u64.pow(i as u32);
			}

			Self::try_from(num)
		}
	}
}

impl TryFrom<String> for Id {
	type Error = ConversionError;

	fn try_from(string: String) -> Result<Self, Self::Error> {
		Self::try_from(string.as_str())
	}
}

impl TryFrom<&String> for Id {
	type Error = ConversionError;

	fn try_from(string: &String) -> Result<Self, Self::Error> {
		Self::try_from(string.as_str())
	}
}

impl TryFrom<u64> for Id {
	type Error = ConversionError;

	fn try_from(num: u64) -> Result<Self, Self::Error> {
		if num > Self::MAX {
			Err(ConversionError::TooLarge)
		} else {
			let mut buf = [0u8; Self::BYTES];
			buf.copy_from_slice(&num.to_be_bytes()[3..]);

			Ok(Self(buf))
		}
	}
}

impl From<Id> for u64 {
	fn from(id: Id) -> Self {
		id.to_u64()
	}
}

impl From<Id> for String {
	fn from(id: Id) -> Self {
		id.to_string()
	}
}

impl From<Id> for [u8; 5] {
	fn from(id: Id) -> [u8; 5] {
		id.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn is_valid() {
		assert!(Id::is_valid("1wqLjdjd")); // one valid Id
		assert!(Id::is_valid("06789BCD")); // another Id
		assert!(Id::is_valid(&Id::new().to_string())); // any generated Id is valid
		assert!(Id::is_valid("06666666")); // Id::MIN as an Id
		assert!(Id::is_valid("9dDbKpJP")); // Id::MAX as an Id
		assert!(!Id::is_valid("𝟵𝚍𝐃𝐛𝓚𝐩𝐉𝑷")); // characters that may look good, but aren't
		assert!(!Id::is_valid("9dDbKpJQ")); // Id::MAX + 1 as an Id (too large)
		assert!(!Id::is_valid("00000000")); // Invalid base 38 characters
		assert!(!Id::is_valid("xHJ6CH79")); // invalid base 10 character
		assert!(!Id::is_valid("06789BC")); // too short
		assert!(!Id::is_valid("06789BCDD")); // too long
		assert!(!Id::is_valid("9pqrtwxz")); // too large for 40 bits
		assert!(!Id::is_valid("")); // empty string
		assert!(!Id::is_valid("an invalid id")); // not even close
	}

	#[test]
	fn new() {
		assert_ne!(Id::new(), Id::new());
	}

	#[test]
	fn to_u64() {
		assert_eq!(
			Id([0x01, 0x02, 0x03, 0x04, 0x05]).to_u64(),
			0x01_02_03_04_05_u64
		);

		let id = Id::new();
		assert_eq!(Id::try_from(id.to_u64()).unwrap(), id);
	}

	#[test]
	fn to_string() {
		assert_eq!(Id([0x21, 0x22, 0x23, 0x24, 0x25]).to_string(), "1HJ6CH79");
		assert_eq!(Id([0x00, 0x22, 0x44, 0x66, 0x88]).to_string(), "06FHjHkx");
	}

	#[test]
	fn default() {
		assert_ne!(Id::default(), Id::default());
	}

	#[test]
	fn from_redis() {
		assert_eq!(
			Id([0x11, 0x33, 0x55, 0x77, 0x99]),
			Id::from_value(RedisValue::from_static_str("0fXMgWQz")).unwrap()
		);

		assert_eq!(
			Id([0x00, 0x22, 0x44, 0x66, 0x88]),
			Id::from_value(RedisValue::from_static(&[0x00, 0x22, 0x44, 0x66, 0x88])).unwrap()
		);

		assert!(Id::from_value(RedisValue::Null).is_err());
	}

	#[test]
	fn serde() {
		assert_eq!(
			serde_json::to_string(&Id([0x73, 0x65, 0x72, 0x64, 0x65])).unwrap(),
			r#""4Ld9TJrd""#
		);

		assert_eq!(
			serde_json::from_str::<Id>(r#""4Ld9TJrd""#).unwrap(),
			Id([0x73, 0x65, 0x72, 0x64, 0x65])
		);
	}

	#[test]
	fn from_bytes() {
		assert_eq!(
			Id([0x01, 0x02, 0x03, 0x04, 0x05]),
			Id::from([0x01, 0x02, 0x03, 0x04, 0x05])
		);

		let id = Id::new();
		assert_eq!(Id::from(id.0), id);
	}

	#[test]
	fn try_from_bytes() {
		assert_eq!(
			Id([0x01, 0x02, 0x03, 0x04, 0x05]),
			Id::try_from(&[0x01_u8, 0x02_u8, 0x03_u8, 0x04_u8, 0x05_u8][..]).unwrap()
		);

		assert!(Id::try_from(&[0_u8, 1_u8, 2_u8, 3_u8, 4_u8, 5_u8][..]).is_err());
		assert!(Id::try_from(&[0_u8, 1_u8, 2_u8, 3_u8][..]).is_err());
		assert!(Id::try_from(&b""[..]).is_err());
		assert!(Id::try_from(&b"This is not a valid binary representation of an ID"[..]).is_err());

		let id = Id::new();
		assert_eq!(Id::try_from(id.0).unwrap(), id);
	}

	#[test]
	fn try_from_string() {
		assert_eq!(
			Id([0x31, 0x32, 0x33, 0x34, 0x35]),
			Id::try_from("1qDhG8Tr").unwrap()
		);
		assert_eq!(
			Id([0x11, 0x33, 0x55, 0x77, 0x99]),
			Id::try_from("0fXMgWQz").unwrap()
		);
	}

	#[test]
	fn to_from_string() {
		for _ in 0..10000 {
			let id = Id::new();
			assert_eq!(id, Id::try_from(id.to_string()).unwrap());
		}
	}

	#[test]
	fn try_from_u64() {
		assert_eq!(
			Id([0x41, 0x42, 0x43, 0x44, 0x45]),
			Id::try_from(0x41_42_43_44_45_u64).unwrap()
		);

		let id = Id::new();
		assert_eq!(Id::try_from(id.to_u64()).unwrap(), id);
	}
}
