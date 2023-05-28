//! A map with [domain name][Domain] keys, with support for wildcards
//!
//! A [`DomainMap<T>`] holds "[reference identifiers]" (domain names possibly
//! with wildcards) as [`Domain`]s. A [`DomainMap`] can be indexed using a
//! [`Domain`], which stores either a "[reference identifier]" (for matching
//! methods, e.g. `get` or `get_mut`) or a "[presented identifier]" (for
//! equality-comparing methods, e.g. `get_eq` or `remove`).
//!
//! # Cargo features
//!
//! - `std` (on by default): Enable features that require the standard library,
//!   such as the `std::error::Error` trait
//! - `serde`: Enable `serde` serialization and deserialization for `DomainMap`,
//!   `Domain`, and `Label`
//!
//! # Example usage
//!
//! ```rust
//! use links_domainmap::{Domain, DomainMap};
//!
//! # use links_domainmap::ParseError;
//! # fn main() -> Result<(), ParseError> {
//! // Create a new `DomainMap` with `u32` values
//! let mut domainmap = DomainMap::<u32>::new(); // or `with_capacity()`
//!
//! // Set a value for `example.com`
//! domainmap.set(Domain::presented("example.com")?, 5);
//!
//! // Set a value for the wildcard domain `*.example.net`
//! domainmap.set(Domain::presented("*.example.net")?, 100);
//!
//! // Get the value for the domain matching `example.com`
//! assert_eq!(domainmap.get(&Domain::reference("example.com")?), Some(&5));
//!
//! // Get the value for the domain matching `foo.example.net`
//! assert_eq!(
//! 	domainmap.get(&Domain::reference("foo.example.net")?),
//! 	Some(&100)
//! );
//!
//! // Get the value for the domain `*.example.net` (using `==` internally)
//! assert_eq!(
//! 	domainmap.get_eq(&Domain::presented("*.example.net")?),
//! 	Some(&100)
//! );
//!
//! // Try to get the value for the domain matching `a.b.c.example.net`
//! assert_eq!(
//! 	domainmap.get(&Domain::reference("a.b.c.example.net")?),
//! 	None // Wildcards only work for one label
//! );
//!
//! // Update the value for `example.com`
//! let old_value = domainmap.set(Domain::presented("example.com")?, 50);
//! assert_eq!(old_value, Some(5));
//!
//! // Modify the value for the domain matching `foo.example.net`
//! let val = domainmap.get_mut(&Domain::reference("foo.example.net")?);
//! if let Some(val) = val {
//! 	*val += 1;
//! 	assert_eq!(val, &101);
//! }
//!
//! // Set a value for `www.example.net`, overriding the wildcard `*.example.net`
//! domainmap.set(Domain::presented("www.example.net")?, 250);
//!
//! // The wildcard still exists, but is overridden for `www.example.net`
//! assert_eq!(
//! 	domainmap.get(&Domain::reference("www.example.net")?),
//! 	Some(&250)
//! );
//! assert_eq!(
//! 	domainmap.get(&Domain::reference("other.example.net")?),
//! 	Some(&101)
//! );
//!
//! // Remove the entry for `example.com`
//! let old_value = domainmap.remove(&Domain::presented("example.com")?);
//! assert_eq!(old_value, Some(50));
//! assert_eq!(
//! 	domainmap.get(&Domain::reference("example.com")?),
//! 	None // Not in the map anymore
//! );
//!
//! // Show the amount of key-value pairs in the map
//! assert_eq!(domainmap.len(), 2); // `*.example.net` and `www.example.net`
//!
//! // Clear the map
//! domainmap.clear();
//! assert!(domainmap.is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! # [`Domain`] name syntax
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

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(
	clippy::pedantic,
	clippy::cargo,
	clippy::nursery,
	missing_docs,
	rustdoc::missing_crate_level_docs
)]
#![allow(clippy::tabs_in_doc_comments, clippy::module_name_repetitions)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod domain;
mod map;

#[cfg(feature = "serde")]
mod serde;
#[cfg(test)]
mod tests;

pub use domain::{Domain, Label, ParseError};
pub use map::DomainMap;
