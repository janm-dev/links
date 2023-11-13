//! A map with [domain name][Domain] keys, with support for wildcards

#[cfg(not(feature = "std"))]
use alloc::vec::{IntoIter as VecIter, Vec};
use core::{
	fmt::Debug,
	hash::{Hash, Hasher},
	mem,
	slice::{Iter as SliceIter, IterMut as SliceIterMut},
};
#[cfg(feature = "std")]
use std::vec::IntoIter as VecIter;

use crate::Domain;

/// A map with [domain name][Domain] keys, with support for wildcards
///
/// A [`DomainMap<T>`] holds "[reference identifiers]" (domain names possibly
/// with wildcards) as [`Domain`]s. A [`DomainMap`] can be indexed using a
/// [`Domain`], which stores either a "[reference identifier]" (for matching
/// methods, e.g. `get` or `get_mut`) or a "[presented identifier]" (for
/// equality-comparing methods, e.g. `get_eq` or `remove`).
///
/// Currently, this is implemented using an associative array, but this may
/// change in the future.
///
/// # Examples
///
/// ```rust
/// use links_domainmap::{Domain, DomainMap};
///
/// # use links_domainmap::ParseError;
/// # fn main() -> Result<(), ParseError> {
/// // Create a new `DomainMap` with `u32` values
/// let mut domainmap = DomainMap::<u32>::new(); // or `with_capacity()`
///
/// // Set a value for `example.com`
/// domainmap.set(Domain::presented("example.com")?, 5);
///
/// // Set a value for the wildcard domain `*.example.net`
/// domainmap.set(Domain::presented("*.example.net")?, 100);
///
/// // Get the value for the domain matching `example.com`
/// assert_eq!(domainmap.get(&Domain::reference("example.com")?), Some(&5));
///
/// // Get the value for the domain matching `foo.example.net`
/// assert_eq!(
/// 	domainmap.get(&Domain::reference("foo.example.net")?),
/// 	Some(&100)
/// );
///
/// // Get the value for the domain `*.example.net` (using `==` internally)
/// assert_eq!(
/// 	domainmap.get_eq(&Domain::presented("*.example.net")?),
/// 	Some(&100)
/// );
///
/// // Try to get the value for the domain matching `a.b.c.example.net`
/// assert_eq!(
/// 	domainmap.get(&Domain::reference("a.b.c.example.net")?),
/// 	None // Wildcards only work for one label
/// );
///
/// // Update the value for `example.com`
/// let old_value = domainmap.set(Domain::presented("example.com")?, 50);
/// assert_eq!(old_value, Some(5));
///
/// // Modify the value for the domain matching `foo.example.net`
/// let val = domainmap.get_mut(&Domain::reference("foo.example.net")?);
/// if let Some(val) = val {
/// 	*val += 1;
/// 	assert_eq!(val, &101);
/// }
///
/// // Set a value for `www.example.net`, overriding the wildcard `*.example.net`
/// domainmap.set(Domain::presented("www.example.net")?, 250);
///
/// // The wildcard still exists, but is overridden for `www.example.net`
/// assert_eq!(
/// 	domainmap.get(&Domain::reference("www.example.net")?),
/// 	Some(&250)
/// );
/// assert_eq!(
/// 	domainmap.get(&Domain::reference("other.example.net")?),
/// 	Some(&101)
/// );
///
/// // Remove the entry for `example.com`
/// let old_value = domainmap.remove(&Domain::presented("example.com")?);
/// assert_eq!(old_value, Some(50));
/// assert_eq!(
/// 	domainmap.get(&Domain::reference("example.com")?),
/// 	None // Not in the map anymore
/// );
///
/// // Show the amount of key-value pairs in the map
/// assert_eq!(domainmap.len(), 2); // `*.example.net` and `www.example.net`
///
/// // Clear the map
/// domainmap.clear();
/// assert!(domainmap.is_empty());
/// # Ok(())
/// # }
/// ```
///
/// [reference identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=for%20each%20domain.-,reference%20identifier,-%3A%20%20An%20identifier%2C%20constructed
/// [presented identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=context%20of%20PKIX.-,presented%20identifier,-%3A%20%20An%20identifier%20that
#[derive(Debug, Clone)]
pub struct DomainMap<T> {
	data: Vec<(Domain, T)>,
}

impl<T> DomainMap<T> {
	/// Create a new empty [`DomainMap`]
	#[must_use]
	pub const fn new() -> Self {
		Self { data: Vec::new() }
	}

	/// Create a new empty [`DomainMap`] with enough capacity for at least `cap`
	/// key-value pairs
	#[must_use]
	pub fn with_capacity(cap: usize) -> Self {
		Self {
			data: Vec::with_capacity(cap),
		}
	}

	/// Set the value for the given domain, adding a new entry if the domain was
	/// not already in the map, and returning the old value otherwise
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("example.com")?, get_certificate());
	///
	/// domainmap.set(Domain::presented("*.example.com")?, get_certificate());
	///
	/// assert!(domainmap.get(&Domain::presented("example.com")?).is_some());
	/// # Ok(())
	/// # }
	/// ```
	pub fn set(&mut self, domain: Domain, value: T) -> Option<T> {
		for (k, v) in &mut self.data {
			if *k == domain {
				return Some(mem::replace(v, value));
			}
		}

		self.data.push((domain, value));
		None
	}

	/// Get the value matching the [reference identifier] domain
	///
	/// If there is a value for a wildcard domain matching the given domain, and
	/// for the given domain itself, the specific (non-wildcard) domain's value
	/// is always returned, regardless of insertion order
	///
	/// [reference identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=for%20each%20domain.-,reference%20identifier,-%3A%20%20An%20identifier%2C%20constructed
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("*.example.com")?, get_certificate());
	///
	/// assert!(domainmap.get(&Domain::reference("example.com")?).is_none());
	///
	/// assert!(domainmap
	/// 	.get(&Domain::reference("www.example.com")?)
	/// 	.is_some());
	/// # Ok(())
	/// # }
	/// ```
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<u64>::new();
	///
	/// domainmap.set(Domain::presented("foo.example.com")?, 10);
	/// domainmap.set(Domain::presented("*.example.com")?, 50);
	///
	/// assert_eq!(
	/// 	domainmap.get(&Domain::reference("bar.example.com")?),
	/// 	Some(&50)
	/// );
	///
	/// assert_eq!(
	/// 	domainmap.get(&Domain::reference("foo.example.com")?),
	/// 	Some(&10)
	/// );
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn get(&self, domain: &Domain) -> Option<&T> {
		let mut wildcard_result = None;

		for (k, v) in &self.data {
			if domain.matches(k).unwrap_or(false) {
				if k.is_wildcard() {
					wildcard_result = Some(v);
				} else {
					return Some(v);
				}
			}
		}

		wildcard_result
	}

	/// Get a mutable reference to the value matching the [reference identifier]
	///
	/// If there is a value for a wildcard domain matching the given domain, and
	/// for the given domain itself, the specific (non-wildcard) domain's value
	/// is always returned, regardless of insertion order
	///
	/// [reference identifier]: https://www.rfc-editor.org/rfc/rfc6125#section-1.8:~:text=for%20each%20domain.-,reference%20identifier,-%3A%20%20An%20identifier%2C%20constructed
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("*.example.com")?, get_certificate());
	///
	/// assert!(domainmap
	/// 	.get_mut(&Domain::reference("example.com")?)
	/// 	.is_none());
	///
	/// assert!(domainmap
	/// 	.get_mut(&Domain::reference("www.example.com")?)
	/// 	.is_some());
	/// # Ok(())
	/// # }
	/// ```
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<u64>::new();
	///
	/// domainmap.set(Domain::presented("foo.example.com")?, 10);
	/// domainmap.set(Domain::presented("*.example.com")?, 50);
	///
	/// assert_eq!(
	/// 	domainmap.get_mut(&Domain::reference("bar.example.com")?),
	/// 	Some(&mut 50)
	/// );
	///
	/// assert_eq!(
	/// 	domainmap.get_mut(&Domain::reference("foo.example.com")?),
	/// 	Some(&mut 10)
	/// );
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn get_mut(&mut self, domain: &Domain) -> Option<&mut T> {
		let mut wildcard_result = None;

		for (k, v) in &mut self.data {
			if domain.matches(k).unwrap_or(false) {
				if k.is_wildcard() {
					wildcard_result = Some(v);
				} else {
					return Some(v);
				}
			}
		}

		wildcard_result
	}

	/// Get the value for the given domain, checking using `==` instead of
	/// matching
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("*.example.com")?, get_certificate());
	///
	/// assert!(domainmap
	/// 	.get_eq(&Domain::presented("www.example.com")?)
	/// 	.is_none());
	///
	/// assert!(domainmap
	/// 	.get_eq(&Domain::presented("*.example.com")?)
	/// 	.is_some());
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn get_eq(&self, domain: &Domain) -> Option<&T> {
		for (k, v) in &self.data {
			if domain == k {
				return Some(v);
			}
		}

		None
	}

	/// Remove the given domain from the map, returning its value, if any
	///
	/// Note that unlike `DomainMap::get`, this method compares the domain using
	/// `==` instead of checking for a match
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("*.example.com")?, get_certificate());
	///
	/// assert!(domainmap
	/// 	.remove(&Domain::presented("example.com")?)
	/// 	.is_none());
	///
	/// assert!(domainmap
	/// 	.remove(&Domain::presented("*.example.com")?)
	/// 	.is_some());
	///
	/// assert!(domainmap
	/// 	.remove(&Domain::presented("*.example.com")?)
	/// 	.is_none());
	/// # Ok(())
	/// # }
	/// ```
	pub fn remove(&mut self, domain: &Domain) -> Option<T> {
		for (i, (k, _)) in self.data.iter_mut().enumerate() {
			if k == domain {
				let (_, v) = self.data.swap_remove(i);
				return Some(v);
			}
		}

		None
	}

	/// Clear the [`DomainMap`], removing all contents
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// domainmap.set(Domain::presented("example.com")?, get_certificate());
	///
	/// assert!(domainmap.get(&Domain::reference("example.com")?).is_some());
	///
	/// domainmap.clear();
	///
	/// assert!(domainmap.get(&Domain::reference("example.com")?).is_none());
	/// # Ok(())
	/// # }
	/// ```
	pub fn clear(&mut self) {
		self.data.clear();
	}

	/// Get the number of key-value pairs in the [`DomainMap`]
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// assert_eq!(domainmap.len(), 0);
	///
	/// domainmap.set(Domain::presented("example.com")?, get_certificate());
	///
	/// assert_eq!(domainmap.len(), 1);
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn len(&self) -> usize {
		self.data.len()
	}

	/// Check whether the [`DomainMap`] is empty, i.e. if its length is 0
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # struct Certificate;
	/// # fn get_certificate() -> Certificate { Certificate }
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<Certificate>::new();
	///
	/// assert!(domainmap.is_empty());
	///
	/// domainmap.set(Domain::presented("example.com")?, get_certificate());
	///
	/// assert!(!domainmap.is_empty());
	/// # Ok(())
	/// # }
	/// ```
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Return an iterator over references to this map's key-value pairs in
	/// unspecified order
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<u32>::new();
	/// domainmap.set(Domain::presented("example.com")?, 1);
	/// let mut iterator = domainmap.iter();
	///
	/// assert_eq!(
	/// 	iterator.next(),
	/// 	Some((&Domain::presented("example.com")?, &1))
	/// );
	/// assert_eq!(iterator.next(), None);
	/// # Ok(())
	/// # }
	/// ```
	pub fn iter(&self) -> impl Iterator<Item = (&Domain, &T)> {
		<&Self as IntoIterator>::into_iter(self)
	}

	/// Return an iterator over mutable references to this map's key-value pairs
	/// in unspecified order
	///
	/// # Examples
	///
	/// ```rust
	/// # use links_domainmap::{DomainMap, Domain, ParseError};
	/// # fn main() -> Result<(), ParseError> {
	/// let mut domainmap = DomainMap::<u32>::new();
	/// domainmap.set(Domain::presented("example.com")?, 1);
	/// let mut iterator = domainmap.iter_mut();
	///
	/// assert_eq!(
	/// 	iterator.next(),
	/// 	Some((&Domain::presented("example.com")?, &mut 1))
	/// );
	/// assert_eq!(iterator.next(), None);
	/// # Ok(())
	/// # }
	/// ```
	pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Domain, &mut T)> {
		<&mut Self as IntoIterator>::into_iter(self)
	}
}

impl<T> Default for DomainMap<T> {
	fn default() -> Self {
		Self::with_capacity(4)
	}
}

impl<T: PartialEq> PartialEq for DomainMap<T> {
	fn eq(&self, other: &Self) -> bool {
		if self.len() != other.len() {
			return false;
		}

		self.iter()
			.all(|(key, value)| other.get_eq(key).map_or(false, |v| value == v))
	}
}

impl<T: Eq> Eq for DomainMap<T> {}

impl<T: Hash> Hash for DomainMap<T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		let mut sorted = self.data.iter().collect::<Vec<&(Domain, T)>>();
		sorted.sort_unstable_by_key(|(domain, _)| domain);

		for element in sorted {
			(*element).hash(state);
		}
	}
}

impl<T> FromIterator<(Domain, T)> for DomainMap<T> {
	fn from_iter<I: IntoIterator<Item = (Domain, T)>>(iter: I) -> Self {
		let iter = iter.into_iter();
		let mut map = Self::with_capacity(iter.size_hint().0);

		for (domain, value) in iter {
			map.set(domain, value);
		}

		map
	}
}

impl<T> Extend<(Domain, T)> for DomainMap<T> {
	fn extend<I: IntoIterator<Item = (Domain, T)>>(&mut self, iter: I) {
		for (domain, value) in iter {
			self.set(domain, value);
		}
	}
}

impl<T> IntoIterator for DomainMap<T> {
	type IntoIter = IntoIter<T>;
	type Item = (Domain, T);

	fn into_iter(self) -> Self::IntoIter {
		IntoIter {
			inner: self.data.into_iter(),
		}
	}
}

impl<'a, T: 'a> IntoIterator for &'a DomainMap<T> {
	type IntoIter = Iter<'a, T>;
	type Item = (&'a Domain, &'a T);

	fn into_iter(self) -> Self::IntoIter {
		Iter {
			inner: self.data.iter(),
		}
	}
}

impl<'a, T: 'a> IntoIterator for &'a mut DomainMap<T> {
	type IntoIter = IterMut<'a, T>;
	type Item = (&'a Domain, &'a mut T);

	fn into_iter(self) -> Self::IntoIter {
		IterMut {
			inner: self.data.iter_mut(),
		}
	}
}

pub struct IntoIter<T> {
	inner: VecIter<(Domain, T)>,
}

impl<T> Iterator for IntoIter<T> {
	type Item = (Domain, T);

	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next()
	}
}

pub struct Iter<'a, T: 'a> {
	inner: SliceIter<'a, (Domain, T)>,
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
	type Item = (&'a Domain, &'a T);

	#[allow(clippy::map_identity)] // false positive, the map is from `&(k, v)` to `(&k, &v)`
	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next().map(|(k, v)| (k, v))
	}
}

pub struct IterMut<'a, T: 'a> {
	inner: SliceIterMut<'a, (Domain, T)>,
}

impl<'a, T: 'a> Iterator for IterMut<'a, T> {
	type Item = (&'a Domain, &'a mut T);

	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next().map(|(k, v)| (&*k, v))
	}
}

#[cfg(test)]
mod tests {
	#[cfg(not(feature = "std"))]
	use alloc::format;
	#[cfg(feature = "std")]
	use std::collections::HashMap;

	use super::*;

	#[test]
	fn domainmap_new() {
		for map in [
			DomainMap::<()>::new(),
			DomainMap::<()>::default(),
			DomainMap::<()>::with_capacity(8),
		] {
			assert!(map.is_empty());
			assert_eq!(map.len(), 0);

			assert!(map
				.get(&Domain::reference("example.com").unwrap())
				.is_none());
		}
	}

	#[test]
	fn domainmap_get() {
		let mut map = DomainMap::<u32>::new();

		assert_eq!(map.get(&Domain::reference("example.com").unwrap()), None);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			None
		);

		map.set(Domain::presented("example.com").unwrap(), 1);
		map.set(Domain::presented("*.example.com").unwrap(), 10);

		assert_eq!(
			map.get(&Domain::reference("example.com").unwrap()),
			Some(&1)
		);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			Some(&10)
		);
	}

	#[test]
	fn domainmap_get_mut() {
		let mut map = DomainMap::<u32>::new();

		assert_eq!(
			map.get_mut(&Domain::reference("example.com").unwrap()),
			None
		);
		assert_eq!(
			map.get_mut(&Domain::reference("foo.example.com").unwrap()),
			None
		);

		map.set(Domain::presented("example.com").unwrap(), 1);
		map.set(Domain::presented("*.example.com").unwrap(), 10);

		assert_eq!(
			map.get_mut(&Domain::reference("example.com").unwrap()),
			Some(&mut 1)
		);

		let val = map
			.get_mut(&Domain::reference("example.com").unwrap())
			.unwrap();
		*val += 100;

		let foo_val = map
			.get_mut(&Domain::reference("foo.example.com").unwrap())
			.unwrap();
		*foo_val += 100;

		assert_eq!(
			map.get_mut(&Domain::reference("example.com").unwrap()),
			Some(&mut 101)
		);
		assert_eq!(
			map.get_mut(&Domain::reference("foo.example.com").unwrap()),
			Some(&mut 110)
		);
	}

	#[test]
	fn domainmap_get_eq() {
		let mut map = DomainMap::<u32>::new();

		assert_eq!(
			map.get_eq(&Domain::presented("*.example.com").unwrap()),
			None
		);
		assert_eq!(
			map.get_eq(&Domain::reference("foo.example.com").unwrap()),
			None
		);

		map.set(Domain::presented("*.example.com").unwrap(), 1);

		assert_eq!(
			map.get_eq(&Domain::presented("*.example.com").unwrap()),
			Some(&1)
		);
		assert_eq!(
			map.get_eq(&Domain::reference("foo.example.com").unwrap()),
			None
		);
	}

	#[test]
	fn domainmap_set() {
		let mut map = DomainMap::<u32>::new();

		assert_eq!(map.set(Domain::presented("example.com").unwrap(), 1), None);
		assert_eq!(
			map.set(Domain::presented("*.example.com").unwrap(), 10),
			None
		);

		assert_eq!(
			map.get(&Domain::reference("example.com").unwrap()),
			Some(&1)
		);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			Some(&10)
		);

		assert_eq!(
			map.set(Domain::presented("*.example.com").unwrap(), 20),
			Some(10)
		);
		assert_eq!(
			map.set(Domain::presented("example.com").unwrap(), 2),
			Some(1)
		);
		assert_eq!(
			map.set(Domain::presented("foo.example.com").unwrap(), 200),
			None
		);

		assert_eq!(
			map.get(&Domain::reference("example.com").unwrap()),
			Some(&2)
		);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			Some(&200)
		);
	}

	#[test]
	fn domainmap_remove() {
		let mut map = DomainMap::<u32>::new();

		map.set(Domain::presented("example.com").unwrap(), 1);
		map.set(Domain::presented("*.example.com").unwrap(), 10);

		assert_eq!(map.len(), 2);

		assert_eq!(
			map.remove(&Domain::presented("example.com").unwrap()),
			Some(1)
		);
		assert_eq!(
			map.remove(&Domain::presented("foo.example.com").unwrap()),
			None
		);

		assert_eq!(map.len(), 1);
		assert_eq!(map.get(&Domain::reference("example.com").unwrap()), None);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			Some(&10)
		);

		assert_eq!(
			map.remove(&Domain::presented("*.example.com").unwrap()),
			Some(10)
		);

		assert_eq!(map.len(), 0);
		assert_eq!(
			map.get(&Domain::reference("foo.example.com").unwrap()),
			None
		);
	}

	#[test]
	fn domainmap_len_clear() {
		let mut map = DomainMap::<u32>::new();

		assert!(map.is_empty());
		assert_eq!(map.len(), 0);

		map.set(Domain::presented("example.com").unwrap(), 1);
		map.set(Domain::presented("*.example.com").unwrap(), 10);
		map.set(Domain::presented("foo.example.com").unwrap(), 100);

		assert!(!map.is_empty());
		assert_eq!(map.len(), 3);

		map.clear();

		assert!(map.is_empty());
		assert_eq!(map.len(), 0);
	}

	#[test]
	fn domainmap_iter() {
		let mut map = DomainMap::<u32>::new();

		let a = Domain::presented("example.com").unwrap();
		let b = Domain::presented("*.example.com").unwrap();
		let c = Domain::presented("foo.example.com").unwrap();

		map.set(a.clone(), 1);
		map.set(b.clone(), 10);
		map.set(c.clone(), 100);

		for (domain, value) in map.iter() {
			// Iteration order is not specified
			assert!(
				(*domain == a && *value == 1)
					|| (*domain == b && *value == 10)
					|| (*domain == c && *value == 100)
			);
		}

		for (domain, value) in map.iter_mut() {
			// Iteration order is not specified
			assert!(
				(*domain == a && *value == 1)
					|| (*domain == b && *value == 10)
					|| (*domain == c && *value == 100)
			);

			*value += 1000;
		}

		#[allow(clippy::explicit_into_iter_loop)]
		for (domain, value) in map.into_iter() {
			// Iteration order is not specified
			assert!(
				(domain == a && value == 1001)
					|| (domain == b && value == 1010)
					|| (domain == c && value == 1100)
			);

			drop(domain);
		}

		map = DomainMap::<u32>::new();

		let a = Domain::presented("example.net").unwrap();
		let b = Domain::presented("*.example.net").unwrap();
		let c = Domain::presented("foo.example.net").unwrap();

		map.set(a.clone(), 1);
		map.set(b.clone(), 10);
		map.set(c.clone(), 100);

		for (domain, value) in &map {
			// Iteration order is not specified
			assert!(
				(*domain == a && *value == 1)
					|| (*domain == b && *value == 10)
					|| (*domain == c && *value == 100)
			);
		}

		for (domain, value) in &mut map {
			// Iteration order is not specified
			assert!(
				(*domain == a && *value == 1)
					|| (*domain == b && *value == 10)
					|| (*domain == c && *value == 100)
			);

			*value += 1000;
		}

		for (domain, value) in map {
			// Iteration order is not specified
			assert!(
				(domain == a && value == 1001)
					|| (domain == b && value == 1010)
					|| (domain == c && value == 1100)
			);

			drop(domain);
		}
	}

	#[test]
	#[allow(clippy::many_single_char_names)]
	fn domainmap_from_iter_extend() {
		let a = Domain::presented("example.net").unwrap();
		let b = Domain::presented("*.example.net").unwrap();
		let c = Domain::presented("foo.example.net").unwrap();
		let e = Domain::presented("example.com").unwrap();
		let d = Domain::presented("bar.example.com").unwrap();

		let mut map = DomainMap::<u32>::from_iter([(a, 1), (b, 10), (c, 100)]);

		assert_eq!(map.len(), 3);

		map.extend([(e, 1000), (d, 10000)]);

		assert_eq!(map.len(), 5);
	}

	#[test]
	fn domainmap_misc_traits() {
		let mut map = DomainMap::<u32>::new();
		let mut other = DomainMap::<u32>::new();

		let a = Domain::presented("example.com").unwrap();
		let b = Domain::presented("*.example.com").unwrap();
		let c = Domain::presented("foo.example.com").unwrap();

		map.set(a.clone(), 1);
		map.set(b.clone(), 10);
		map.set(c.clone(), 100);

		other.set(c, 100);
		other.set(b, 10);
		other.set(a, 1);

		assert!(format!("{map:?}").contains("DomainMap"));

		let cloned = map.clone();
		assert!(map == cloned);
		assert!(map == other);
		assert!(map == map);
		assert!(map != DomainMap::new());

		#[cfg(feature = "std")]
		{
			let mut hash_map = HashMap::<_, usize>::new();
			hash_map.insert(cloned, 3);
			assert_eq!(hash_map.get(&map), Some(&3));
			assert_eq!(hash_map.get(&other), Some(&3));
		}
	}
}
