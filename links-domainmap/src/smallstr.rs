//! A small string for internal usage by this crate
//!
//! This implementation is guided primarily by performance while avoiding unsafe
//! code and keeping things simple. That means we need:
//! - fast conversions from an `std::string::String` (due to
//!   `idna::domain_to_ascii`'s return type)
//! - `Deref` and `DerefMut` into a `str`
//! - avoiding allocations for short strings
//!
//! The current implementation provides roughly no change in performance in
//! `Domain::presented`, but approximately 30% better performance in
//! `Domain::reference` in the `domain` benchmark, with large overall
//! improvements in `DomainMap` benchmarks.

#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SmallStr {
	Stack(heapless::String<16>),
	Heap(String),
}

impl AsRef<str> for SmallStr {
	fn as_ref(&self) -> &str {
		match self {
			Self::Stack(s) => s.as_str(),
			Self::Heap(s) => s.as_str(),
		}
	}
}

impl AsMut<str> for SmallStr {
	fn as_mut(&mut self) -> &mut str {
		match self {
			Self::Stack(s) => s.as_mut(),
			Self::Heap(s) => s.as_mut(),
		}
	}
}

impl Deref for SmallStr {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Stack(s) => s,
			Self::Heap(s) => s,
		}
	}
}

impl DerefMut for SmallStr {
	fn deref_mut(&mut self) -> &mut Self::Target {
		match self {
			Self::Stack(s) => s,
			Self::Heap(s) => s,
		}
	}
}

impl From<String> for SmallStr {
	fn from(value: String) -> Self {
		Self::Heap(value)
	}
}

impl From<&str> for SmallStr {
	#[allow(clippy::ignored_unit_patterns)]
	fn from(value: &str) -> Self {
		value
			.try_into()
			.map_or_else(|_| Self::Heap(value.into()), Self::Stack)
	}
}
