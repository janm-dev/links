//! Types for domain names, either as a reference identifier or a presented
//! identifier, with support for internationalized domain names
//!
//! Rules for domain names as implemented here (based on a mix of [RFC 952],
//! [RFC 1034], [RFC 1123], [RFC 2181], the [WHATWG URL specification][whatwg
//! url], [browser][chrome] [implementations][firefox], and [browser
//! bugs][bugzilla]) are:
//! - A domain name has a maximum total length (including '.'s) of 253
//!   characters/octets/bytes ([RFC 1034] section 3.1, [Unicode TR46] section 4)
//! - Domain name labels (the things seperated by '.') have a maximum length of
//!   63 characters/octets/bytes each, not including the separators ([RFC 1034]
//!   section 3.5, [RFC 1123] section 2.1, [RFC 2181] section 11)
//! - A domain name label must consist of only ASCII letters (`'a'..='z' |
//!   'A'..='Z'`), digits (`'0'..='9'`), and hyphens (`'-'`). A label can not
//!   start or end with a hyphen. ([RFC 952] section B, [RFC 1034] section 3.5,
//!   [RFC 1123] section 2.1)
//! - Additionally, a label can also contain underscores (`'_'`) in the same
//!   places as letters for compatibility reasons. ([Additional discussion
//!   around underscores in a Firefox bug][bugzilla], implementations in
//!   [Chromium][chrome] and [Firefox][firefox])
//! - A wildcard (`"*"`) can only comprise the entire left-most label of a
//!   domain name and matches exactly one label. ([RFC 2818] section 3.1, [RFC
//!   6125] section 6.4.3; i.e. `"*.example.com"` is valid and matches
//!   `"foo.example.com"` and `"bar.example.com"` but does not match
//!   `"foo.bar.example.com"` or `"example.com"`, and all of the following are
//!   invalid: `"*.*.example.com"`, `"foo.*.example.com"`, `"fo*.example.com"`,
//!   `"*oo.example.com"`, `"f*o.example.com"`)
//! - No special treatment is given to wildcards on top-level domains (e.g.
//!   `"*.com"`), or on other public suffixes (e.g. "`*.co.uk`" or
//!   `"*.pvt.k12.ma.us"`), which allows some potentially invalid wildcard
//!   domains; full wildcard domains (`"*"`) are not allowed
//! - [Percent-encoded domain names][whatwg url] (e.g. `"e%78ample.com"`) are
//!   not supported, and `'%'` is treated as an invalid character
//!
//! [RFC 952]: https://www.rfc-editor.org/rfc/rfc952
//! [RFC 1034]: https://www.rfc-editor.org/rfc/rfc1034
//! [RFC 1123]: https://www.rfc-editor.org/rfc/rfc1123
//! [RFC 2181]: https://www.rfc-editor.org/rfc/rfc2181
//! [RFC 5890]: https://www.rfc-editor.org/rfc/rfc5890
//! [RFC 6125]: https://www.rfc-editor.org/rfc/rfc6125
//! [Unicode TR46]: https://www.unicode.org/reports/tr46/tr46-29.html
//! [bugzilla]: https://bugzilla.mozilla.org/show_bug.cgi?id=1136616
//! [whatwg url]: https://url.spec.whatwg.org/#host-parsing
//! [chrome]: https://github.com/chromium/chromium/blob/18095fefc0746e934e623019294b10844d8ec989/net/base/url_util.cc#L359-L377
//! [firefox]: https://searchfox.org/mozilla-central/rev/23690c9281759b41eedf730d3dcb9ae04ccaddf8/security/nss/lib/mozpkix/lib/pkixnames.cpp#1979-1997
//! [reference identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=for%20each%20domain.-,reference%20identifier,-%3A%20%20An%20identifier%2C%20constructed
//! [presented identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=context%20of%20PKIX.-,presented%20identifier,-%3A%20%20An%20identifier%20that

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::{
	cmp::Ordering,
	fmt::{Debug, Display, Formatter, Result as FmtResult, Write},
};
#[cfg(feature = "std")]
use std::error::Error;

/// A domain name label, stored in lowercase in its ASCII-encoded form
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label(String);

impl Label {
	/// Create a new [`Label`] from the given [ACE] input, checking if all
	/// characters in the label are valid, and that the length is appropriate.
	/// This function does not do any conversions, it only checks the input for
	/// ASCII validity. The validity of A-labels is not checked and fake
	/// A-labels (labels starting with "xn--", while not being valid punycode)
	/// are accepted.
	///
	/// # Errors
	///
	/// This function returns a [`ParseError`] if parsing of the label fails
	///
	/// [ACE]: https://datatracker.ietf.org/doc/html/rfc3490#section-2
	pub(crate) fn new_ace(mut label: String) -> Result<Self, ParseError> {
		if label.is_empty() {
			return Err(ParseError::LabelEmpty);
		}

		if label.len() > 63 {
			return Err(ParseError::LabelTooLong);
		}

		if let Some(invalid) = label
			.chars()
			.find(|&c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
		{
			return Err(ParseError::InvalidChar(invalid));
		}

		if label.starts_with('-') || label.ends_with('-') {
			return Err(ParseError::InvalidHyphen);
		}

		label.make_ascii_lowercase();

		Ok(Self(label))
	}

	/// Create a new [`Label`] from the given possibly-internationalized input
	/// label, parsing and encoding the input into an A-label if necessary. If
	/// the input contains a fake A-label, [`ParseError::Idna`] is returned.
	///
	/// # Errors
	///
	/// This function returns a [`ParseError`] if parsing of the label fails
	pub(crate) fn new_idn(label: &str) -> Result<Self, ParseError> {
		let label = idna::domain_to_ascii(label)?;
		Self::new_ace(label)
	}

	/// Get the internal string representing this label
	///
	/// The returned value is an ASCII lowercase string, with non-ASCII
	/// characters encoded using punycode. The string does not contain any `.`s.
	/// It may however be a "fake A-label", i.e. start with "xn--", but
	/// not be valid punycode.
	///
	/// # Example
	///
	/// ```rust
	/// # use links_domainmap::{Domain, Label, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let domain = Domain::presented("παράδειγμα.EXAMPLE.com")?;
	/// for label in domain.labels() {
	/// 	assert!(label.as_str().is_ascii());
	/// 	assert!(label
	/// 		.as_str()
	/// 		.chars()
	/// 		.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
	/// }
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn as_str(&self) -> &str {
		self.as_ref()
	}
}

impl Display for Label {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		if fmt.alternate() {
			let (res, err) = idna::domain_to_unicode(self.as_str());

			// Try encoding as Unicode, but use the original label if that fails
			let res = if err.is_err() {
				self.as_str()
			} else {
				res.as_str()
			};

			fmt.write_str(res)
		} else {
			fmt.write_str(self.as_str())
		}
	}
}

impl AsRef<str> for Label {
	fn as_ref(&self) -> &str {
		self.0.as_ref()
	}
}

/// A domain name split into individual labels (not including the root label).
/// Labels are stored in most-significant-first order, i.e. `"www.example.com."`
/// would be stored as `["com", "example", "www"]`. Labels are stored in their
/// ASCII-encoded form (A-labels for internationalized domain name labels). If
/// the left-most label is equal to `'*'`, `is_wildcard` is set to true, and the
/// label itself is *not* stored in `labels`.
///
/// See the [library documentation][crate] for details about syntax rules for
/// domain names.
///
/// # `Eq` vs `matches()`
///
/// The [`PartialEq`] / [`Eq`] implementation for [`Domain`] is equivalent to a
/// string comparison of the domains (e.g. `"example.com" == "example.com"`, but
/// `"*.example.com" != "www.example.com"`). [`Domain::matches`], on the other
/// hand checks if a [reference identifier] domain matches a given [presented
/// identifier] domain (e.g. `"example.com".matches("example.com")`, and
/// `"*.example.com".matches("www.example.com")`). Care should be taken to use
/// the appropriate method in each situation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Domain {
	/// Indicates whether the domain is a wildcard, i.e. that the left-most
	/// label is exactly equal to `"*"`
	is_wildcard: bool,
	/// The labels of the domain, in right-to-left (most-significant-first)
	/// order, not including the seperators or wildcard label (if any)
	labels: Vec<Label>,
}

impl Domain {
	/// Create a new `Domain` from a [reference identifier], without allowing
	/// wildcards. Note that this function assumes the input is already
	/// [ACE][IDNA]-encoded and it does not check the validity of A-labels,
	/// allowing so-called "[fake A-labels][IDNA]" (labels starting with "xn--",
	/// while not being valid punycode).
	///
	/// [presented identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=context%20of%20PKIX.-,presented%20identifier,-%3A%20%20An%20identifier%20that
	/// [IDNA]: https://www.rfc-editor.org/rfc/rfc5890#section-2.3.2.1
	///
	/// # Errors
	///
	/// Returns a [`ParseError`] if the parsing of the domain name fails. See
	/// the documentation for the error type for an explanation of possible
	/// error variants.
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let example = Domain::reference(&"www.example.com".to_string())?;
	/// assert!(!example.is_wildcard());
	/// assert_eq!(example.labels().len(), 3);
	/// assert_eq!(example.labels()[0].as_ref(), "com");
	/// assert_eq!(example.labels()[1].as_ref(), "example");
	/// assert_eq!(example.labels()[2].as_ref(), "www");
	///
	/// let wildcard = Domain::reference(&"*.example.com".to_string());
	/// assert!(wildcard.is_err());
	/// assert!(matches!(wildcard, Err(ParseError::InvalidChar('*'))));
	/// # Ok(())
	/// # }
	/// ```
	///
	/// ```rust
	/// # use links_domainmap::{Domain, DomainMap};
	/// # use std::error::Error;
	/// #
	/// # struct StubClientHello;
	/// # impl StubClientHello {
	/// # 	fn server_name(&self) -> Option<&str> {
	/// # 		Some("example.com")
	/// # 	}
	/// # }
	/// # fn get_default_cert() -> Option<()> {
	/// # 	None
	/// # }
	/// # fn test() -> Option<()> {
	/// # let client_hello = StubClientHello;
	/// # let mut certificates = DomainMap::<()>::new();
	/// # certificates.set(Domain::presented("example.com").unwrap(), ());
	/// if let Some(server_name) = client_hello.server_name() {
	/// 	let domain = Domain::reference(&server_name).ok()?;
	/// 	let certificate = certificates.get(&domain)?;
	/// 	Some(certificate.clone())
	/// } else {
	/// 	get_default_cert()
	/// }
	/// # }
	/// # test().unwrap();
	/// ```
	pub fn reference(input: &str) -> Result<Self, ParseError> {
		const SEPERATOR: char = '.';

		let input = input.strip_suffix(SEPERATOR).unwrap_or(input);

		if input.is_empty() {
			return Err(ParseError::Empty);
		}

		if input.len() > 253 {
			return Err(ParseError::TooLong);
		}

		let labels = input
			.split(SEPERATOR)
			.rev()
			.map(|l| Label::new_ace(l.into()))
			.collect::<Result<Vec<_>, _>>()?;

		Ok(Self {
			is_wildcard: false,
			labels,
		})
	}

	/// Create a new `Domain` from a [presented identifier], while also checking
	/// for wildcards. This function accepts and encodes ASCII labels, A-labels,
	/// or U-labels, or a mix of them. If the leftmost label is "*", then the
	/// domain name is considered a wildcard domain, and `is_wildcard` is set to
	/// true. Additionally, this function also accepts absolute domain names
	/// (i.e. domain names ending with a '.'), which is [not allowed in
	/// certificates][RFC 5280].
	///
	/// [presented identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=context%20of%20PKIX.-,presented%20identifier,-%3A%20%20An%20identifier%20that
	/// [RFC 5280]: https://www.rfc-editor.org/rfc/rfc5280
	///
	/// # Errors
	///
	/// Returns a [`ParseError`] if the parsing of the domain name fails.
	/// See the documentation for the error type for an explanation of possible
	/// error variants.
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let example = Domain::presented(&"www.example.com".to_string())?;
	/// assert!(!example.is_wildcard());
	/// assert_eq!(example.labels().len(), 3);
	/// assert_eq!(example.labels()[0].as_ref(), "com");
	/// assert_eq!(example.labels()[1].as_ref(), "example");
	/// assert_eq!(example.labels()[2].as_ref(), "www");
	///
	/// let idn = Domain::presented(&"παράδειγμα.例子.example.com".to_string())?;
	/// assert!(!idn.is_wildcard());
	/// assert_eq!(idn.labels().len(), 4);
	/// assert_eq!(idn.labels()[0].as_ref(), "com");
	/// assert_eq!(idn.labels()[1].as_ref(), "example");
	/// assert_eq!(idn.labels()[2].as_ref(), "xn--fsqu00a");
	/// assert_eq!(idn.labels()[3].as_ref(), "xn--hxajbheg2az3al");
	///
	/// let wildcard = Domain::presented(&"*.example.com".to_string())?;
	/// assert!(wildcard.is_wildcard());
	/// assert_eq!(wildcard.labels().len(), 2);
	/// assert_eq!(wildcard.labels()[0].as_ref(), "com");
	/// assert_eq!(wildcard.labels()[1].as_ref(), "example");
	///
	/// let wildcard_idn = Domain::presented(&"*.приклад.com".to_string())?;
	/// assert!(wildcard_idn.is_wildcard());
	/// assert_eq!(wildcard_idn.labels().len(), 2);
	/// assert_eq!(wildcard_idn.labels()[0].as_ref(), "com");
	/// assert_eq!(wildcard_idn.labels()[1].as_ref(), "xn--80aikifvh");
	/// # Ok(())
	/// # }
	/// ```
	///
	/// ```rust
	/// # use links_domainmap::{Domain, DomainMap, ParseError};
	/// #
	/// # struct StubConfigSource;
	/// # impl StubConfigSource {
	/// # 	fn get_config(&self) -> Vec<(String, ())> {
	/// # 		vec![("example.com".to_string(), ()), ("przykład。com".to_string(), ())]
	/// # 	}
	/// # }
	/// # fn main() -> Result<(), ParseError> {
	/// # let tls_config = StubConfigSource;
	/// # let mut certificates = DomainMap::<()>::new();
	/// for (domain_name, certificate) in tls_config.get_config() {
	/// 	let domain = Domain::presented(&domain_name)?;
	/// 	let certificate = certificates.set(domain, certificate);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub fn presented(input: &str) -> Result<Self, ParseError> {
		const SEPERATORS: &[char] = &['\u{002e}', '\u{3002}', '\u{ff0e}', '\u{ff61}'];

		let input = input.strip_suffix(SEPERATORS).unwrap_or(input);

		if input.is_empty() {
			return Err(ParseError::Empty);
		}

		let mut labels = input.split(SEPERATORS).peekable();

		let is_wildcard = *labels.peek().ok_or(ParseError::Empty)? == "*";

		if is_wildcard {
			// Skip the `"*"` label
			labels.next();
		}

		let labels = labels
			.rev()
			.map(Label::new_idn)
			.collect::<Result<Vec<_>, _>>()?;

		if labels.is_empty() {
			// Input was `"*"`, an invalid input considered empty by this crate
			return Err(ParseError::Empty);
		}

		let wildcard_len = if is_wildcard { "*.".len() } else { 0 };
		if labels.iter().map(|l| l.0.len() + 1).sum::<usize>() - 1 + wildcard_len > 253 {
			return Err(ParseError::TooLong);
		}

		Ok(Self {
			is_wildcard,
			labels,
		})
	}

	/// Whether this `Domain` represents a wildcard, i.e. the left-most label is
	/// "*". If this is `true`, this domain matches another non-wildcard domain,
	/// if this domain's labels are a prefix of the other domain's, and the
	/// other domain has exactly one extra label, e.g. if this domain has the
	/// labels `["com", "example"]`, then it would match another domain with
	/// labels `["com", "example", "foo"]` or `["com", "example", "bar"]`, but
	/// not `["com", "example"]` or `["com", "example", "bar", "foo"]`.
	#[must_use]
	pub const fn is_wildcard(&self) -> bool {
		self.is_wildcard
	}

	/// Get the labels of this `Domain`. The labels are in right-to-left /
	/// most-significant-first order, i.e. `"www.example.com"` would have the
	/// labels `["com", "example", "www"]`. If this domain is a [wildcard
	/// domain][`Domain::is_wildcard`], the wildcard label is not included in
	/// the returned slice. See [`Domain`]'s documentation for details.
	#[must_use]
	pub fn labels(&self) -> &[Label] {
		self.labels.as_slice()
	}

	/// Check whether this [`Domain`] matches the given [presented identifier].
	/// This domain is treated as a [reference identifier], and therefore if its
	/// `is_wildcard` property is set, this function returns `None`.
	///
	/// [presented identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=context%20of%20PKIX.-,presented%20identifier,-%3A%20%20An%20identifier%20that
	/// [reference identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=for%20each%20domain.-,reference%20identifier,-%3A%20%20An%20identifier%2C%20constructed
	#[must_use]
	pub fn matches(&self, presented: &Self) -> Option<bool> {
		if self.is_wildcard() {
			return None;
		}

		if presented.is_wildcard() {
			Some(presented.labels() == &self.labels()[..self.labels().len() - 1])
		} else {
			Some(presented.labels() == self.labels())
		}
	}
}

/// Format a [`Domain`] with the given formatter. Use alternate formatting
/// (`"{:#}"`) to encode labels into Unicode; by default internationalized
/// labels are formatted in their ASCII compatible encoding form.
impl Display for Domain {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		if self.is_wildcard() {
			fmt.write_str("*.")?;
		}

		Display::fmt(
			self.labels
				.last()
				.expect("a domain always has at least one label"),
			fmt,
		)?;

		for label in self.labels.iter().rev().skip(1) {
			fmt.write_char('.')?;
			Display::fmt(label, fmt)?;
		}

		Ok(())
	}
}

impl PartialOrd for Domain {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Domain {
	fn cmp(&self, other: &Self) -> Ordering {
		match self.labels.cmp(&other.labels) {
			Ordering::Equal => match (self.is_wildcard, other.is_wildcard) {
				(true, false) => Ordering::Greater,
				(false, true) => Ordering::Less,
				_ => Ordering::Equal,
			},
			ord => ord,
		}
	}
}

/// An error encountered while parsing a domain name
#[derive(Debug)]
pub enum ParseError {
	/// The domain name has no non-wildcard labels
	Empty,
	/// The length of the domain exceeds 253
	TooLong,
	/// The label is empty
	LabelEmpty,
	/// The length of the label exceeds 63
	LabelTooLong,
	/// The input contains an invalid character
	InvalidChar(char),
	/// A label has a hyphen at the start or end
	InvalidHyphen,
	/// An error occurred during processing of an internationalized input
	Idna(idna::Errors),
}

impl PartialEq for ParseError {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Empty, Self::Empty)
			| (Self::TooLong, Self::TooLong)
			| (Self::LabelEmpty, Self::LabelEmpty)
			| (Self::LabelTooLong, Self::LabelTooLong)
			| (Self::InvalidHyphen, Self::InvalidHyphen)
			| (Self::Idna(_), Self::Idna(_)) => true,
			(Self::InvalidChar(a), Self::InvalidChar(b)) => a == b,
			_ => false,
		}
	}
}

impl Display for ParseError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Empty => f.write_str("the domain name has no non-wildcard labels"),
			Self::TooLong => f.write_str("the length of the domain exceeds 253"),
			Self::LabelEmpty => f.write_str("the label is empty"),
			Self::LabelTooLong => f.write_str("the length of the label exceeds 63"),
			Self::InvalidChar(char) => f.write_fmt(format_args!(
				"the input contains the invalid character '{char}'"
			)),
			Self::InvalidHyphen => f.write_str("a label has a hyphen at the start or end"),
			Self::Idna(errors) => f.write_fmt(format_args!(
				"an error occurred during processing of an internationalized input: {errors}"
			)),
		}
	}
}

#[cfg(feature = "std")]
impl Error for ParseError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			Self::Idna(errors) => Some(errors),
			_ => None,
		}
	}
}

impl From<idna::Errors> for ParseError {
	fn from(value: idna::Errors) -> Self {
		Self::Idna(value)
	}
}

#[cfg(test)]
mod tests {
	#[cfg(not(feature = "std"))]
	use alloc::{boxed::Box, collections::BTreeMap, format, string::ToString};
	#[cfg(feature = "std")]
	use std::{
		collections::{BTreeMap, HashMap},
		error::Error,
	};

	use super::*;
	#[allow(clippy::wildcard_imports)]
	use crate::tests::*;

	#[test]
	fn domain_reference() {
		for (input, expected) in DOMAIN_REFERENCE {
			let res = Domain::reference(input).map(|d| &*Box::leak(d.to_string().into_boxed_str()));

			assert_eq!(res, *expected);
		}
	}

	#[test]
	fn domain_presented() {
		for (input, expected) in DOMAIN_PRESENTED {
			let res = Domain::presented(input).map(|d| &*Box::leak(d.to_string().into_boxed_str()));

			assert_eq!(res, *expected);
		}

		assert!(Domain::presented("xn--example.com").is_err());

		#[cfg(feature = "std")]
		assert!(Domain::presented("xn--example.com")
			.unwrap_err()
			.source()
			.unwrap()
			.is::<idna::Errors>());
	}

	#[test]
	fn domain_matches() {
		for &(reference, presented, expected, _) in DOMAIN_MATCHES_EQ {
			let reference = Domain::reference(reference).unwrap();
			let presented = Domain::presented(presented).unwrap();
			let res = reference.matches(&presented).unwrap();

			assert_eq!(res, expected);
		}

		for &(a, b, expected) in DOMAIN_PRESENTED_MATCHES_PRESENTED {
			let a = Domain::presented(a).unwrap();
			let b = Domain::presented(b).unwrap();
			let res = a.matches(&b);

			assert_eq!(res, expected);
		}

		for &(a, b, expected) in DOMAIN_REFERENCE_MATCHES_REFERENCE {
			let a = Domain::reference(a).unwrap();
			let b = Domain::reference(b).unwrap();
			let res = a.matches(&b);

			assert_eq!(res, expected);
		}
	}

	#[test]
	fn domain_eq() {
		for &(reference, presented, _, expected) in DOMAIN_MATCHES_EQ {
			let reference = Domain::reference(reference).unwrap();
			let presented = Domain::presented(presented).unwrap();
			let res = reference == presented;

			assert_eq!(res, expected);

			assert_eq!(reference == presented, presented == reference);
		}

		for &(a, b, expected) in DOMAIN_PRESENTED_EQ_PRESENTED {
			let a = Domain::presented(a).unwrap();
			let b = Domain::presented(b).unwrap();

			let res = a == b;
			assert_eq!(res, expected);

			let res = b == a;
			assert_eq!(res, expected);
		}

		for &(a, b, expected) in DOMAIN_REFERENCE_EQ_REFERENCE {
			let a = Domain::reference(a).unwrap();
			let b = Domain::reference(b).unwrap();

			let res = a == b;
			assert_eq!(res, expected);

			let res = b == a;
			assert_eq!(res, expected);
		}
	}

	#[test]
	fn domain_display() {
		for &(input, to_string, regular_format, alternate_format) in DOMAIN_DISPLAY {
			if let Ok(presented) = Domain::presented(input) {
				assert_eq!(presented.to_string(), to_string);
				assert_eq!(format!("{presented}"), regular_format);
				assert_eq!(format!("{presented:#}"), alternate_format);
			};

			if let Ok(reference) = Domain::reference(input) {
				assert_eq!(reference.to_string(), to_string);
				assert_eq!(format!("{reference}"), regular_format);
				assert_eq!(format!("{reference:#}"), alternate_format);
			};
		}
	}

	#[test]
	fn domain_misc_traits() {
		let domain = Domain::presented("example.com").unwrap();
		let wildcard = Domain::presented("*.example.com").unwrap();
		let foo = Domain::presented("foo.example.com").unwrap();
		let a = Domain::presented("foo.an-example.com").unwrap();

		let cloned = domain.clone();
		assert!(domain == cloned);

		assert_eq!(format!("{domain:?}"), format!("{cloned:?}"));
		assert!(format!("{domain:?}").contains("Domain"));

		assert!(domain == domain);
		assert!(wildcard == wildcard);
		assert!(foo != Domain::presented("com.example.foo").unwrap());

		assert!(domain < wildcard);
		assert!(domain < foo);
		assert!(wildcard < foo);
		assert!(a < domain);
		assert!(a < foo);
		assert!(a < wildcard);

		assert!(wildcard > domain);
		assert!(foo > domain);
		assert!(foo > wildcard);
		assert!(domain > a);
		assert!(foo > a);
		assert!(wildcard > a);

		for (a, b) in [&domain, &wildcard, &foo, &a]
			.into_iter()
			.zip([&domain, &wildcard, &foo, &a])
		{
			assert_eq!(a.partial_cmp(b), Some(a.cmp(b)));
			assert_eq!(a.partial_cmp(b), b.partial_cmp(a).map(Ordering::reverse));
		}

		let mut btree_map = BTreeMap::<_, usize>::new();
		btree_map.insert(domain.clone(), 3);
		assert_eq!(btree_map.get(&domain), Some(&3));

		#[cfg(feature = "std")]
		{
			let mut hash_map = HashMap::<_, usize>::new();
			hash_map.insert(cloned, 3);
			assert_eq!(hash_map.get(&domain), Some(&3));
		}
	}

	#[test]
	fn parseerror_error() {
		assert!(Domain::presented("xn--example.com").is_err());
		#[cfg(feature = "std")]
		assert!(Domain::presented("xn--example.com")
			.unwrap_err()
			.source()
			.unwrap()
			.is::<idna::Errors>());

		assert!(Domain::presented("example..com").is_err());
		#[cfg(feature = "std")]
		assert!(Domain::presented("example..com")
			.unwrap_err()
			.source()
			.is_none());

		assert_eq!(
			Domain::presented("www.a$df.com").unwrap_err(),
			Domain::presented("a$df.com").unwrap_err(),
		);
		assert_eq!(
			Domain::presented("xn--example.com").unwrap_err(),
			Domain::presented("foo.xn--example.com").unwrap_err()
		);
		assert_ne!(
			Domain::presented("www.a$df.com").unwrap_err(),
			Domain::presented("a#df.com").unwrap_err(),
		);
		assert_ne!(
			Domain::presented("xn--example.com").unwrap_err(),
			Domain::presented("foo.*.com").unwrap_err()
		);
	}

	#[test]
	fn parseerror_debug_display() {
		format!("{:?}", ParseError::Empty).contains("Empty");
		format!("{}", ParseError::Empty).contains("the domain name has no non-wildcard labels");

		format!("{:?}", ParseError::TooLong).contains("TooLong");
		format!("{}", ParseError::TooLong).contains("the length of the domain exceeds 253");

		format!("{:?}", ParseError::LabelEmpty).contains("LabelEmpty");
		format!("{}", ParseError::LabelEmpty).contains("the label is empty");

		format!("{:?}", ParseError::LabelTooLong).contains("LabelTooLong");
		format!("{}", ParseError::LabelTooLong).contains("the length of the label exceeds 63");

		format!("{:?}", ParseError::InvalidChar(' ')).contains("InvalidChar");
		format!("{}", ParseError::InvalidChar(' '))
			.contains("the input contains the invalid character ' '");

		format!("{:?}", ParseError::InvalidHyphen).contains("InvalidHyphen");
		format!("{}", ParseError::InvalidHyphen)
			.contains("a label has a hyphen at the start or end");

		format!("{:?}", Domain::presented("xn--example").unwrap_err()).contains("Idna");
		format!("{}", Domain::presented("xn--example").unwrap_err())
			.contains("an error occurred during processing of an internationalized input: ");
	}
}
