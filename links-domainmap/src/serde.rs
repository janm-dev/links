//! Serialization and deserialization implementations using `serde` for
//! [`DomainMap`] and [`Domain`]

#[cfg(not(feature = "std"))]
use alloc::format;
use core::{
	any,
	fmt::{Formatter, Result as FmtResult},
	marker::PhantomData,
};

use serde::{
	de::{Error as SerdeError, MapAccess, Unexpected, Visitor},
	ser::SerializeMap,
	Deserialize, Deserializer, Serialize,
};

use crate::{Domain, DomainMap};

struct DomainMapVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for DomainMapVisitor<T> {
	type Value = DomainMap<T>;

	fn expecting(&self, f: &mut Formatter) -> FmtResult {
		f.write_fmt(format_args!(
			"a map with domain name keys and {} values",
			any::type_name::<T>()
		))
	}

	fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
	where
		A: MapAccess<'de>,
	{
		let mut domainmap = map
			.size_hint()
			.map_or_else(DomainMap::default, |cap| DomainMap::with_capacity(cap));

		while let Some((k, v)) = map.next_entry()? {
			domainmap.set(k, v);
		}

		Ok(domainmap)
	}
}

struct DomainVisitor;

impl<'de> Visitor<'de> for DomainVisitor {
	type Value = Domain;

	fn expecting(&self, f: &mut Formatter) -> FmtResult {
		f.write_str("a valid domain name")
	}

	fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
	where
		E: SerdeError,
	{
		Domain::presented(v)
			.or_else(|_| Domain::reference(v))
			.map_err(|_| SerdeError::invalid_value(Unexpected::Str(v), &self))
	}
}

impl<T: Serialize> Serialize for DomainMap<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let mut map = serializer.serialize_map(Some(self.len()))?;

		for (k, v) in self {
			map.serialize_entry(k, v)?;
		}

		map.end()
	}
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DomainMap<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_map(DomainMapVisitor::<T>(PhantomData))
	}
}

impl Serialize for Domain {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&format!("{self:#}"))
	}
}

impl<'de> Deserialize<'de> for Domain {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_str(DomainVisitor)
	}
}

#[cfg(test)]
mod tests {
	#[cfg(not(feature = "std"))]
	use alloc::string::ToString;
	use core::f32::consts::PI;

	use super::*;
	#[allow(clippy::wildcard_imports)]
	use crate::tests::*;

	#[test]
	fn domain_serde() {
		for &(input, _) in DOMAIN_REFERENCE {
			if let Ok(domain) = Domain::reference(input) {
				let res = serde_json::from_str(
					&serde_json::to_string(&domain).expect("couldn't serialize domain"),
				)
				.expect("couldn't deserialize domain");

				assert_eq!(
					domain, res,
					"deserialize(serialize(`{domain:?}`)) is `{res:?}`, expected `{domain:?}`"
				);
			}
		}

		for &(input, _) in DOMAIN_PRESENTED {
			if let Ok(domain) = Domain::presented(input) {
				let res = serde_json::from_str(
					&serde_json::to_string(&domain).expect("couldn't serialize domain"),
				)
				.expect("couldn't deserialize domain");

				assert_eq!(
					domain, res,
					"deserialize(serialize(`{domain:?}`)) is `{res:?}`, expected `{domain:?}`"
				);
			}
		}

		assert!(serde_json::from_str::<Domain>(r#"[1, 2, 3]"#)
			.unwrap_err()
			.to_string()
			.contains("a valid domain name"));
	}

	#[test]
	fn domainmap_serde() {
		let mut map = DomainMap::<usize>::new();
		let empty = DomainMap::<i8>::new();
		let mut example = DomainMap::<f32>::new();

		for (i, &(input, _)) in DOMAIN_PRESENTED.iter().enumerate() {
			if let Ok(domain) = Domain::presented(input) {
				map.set(domain, i);
			}
		}

		example.set(Domain::presented("example.com").unwrap(), PI);

		let ser = serde_json::to_string(&map).unwrap();
		let ser_empty = serde_json::to_string(&empty).unwrap();
		let ser_example = serde_json::to_string(&example).unwrap();
		let serde = serde_json::from_str::<DomainMap<usize>>(&ser).unwrap();

		for &(input, _) in DOMAIN_PRESENTED {
			if let Ok(domain) = Domain::presented(input) {
				assert_eq!(serde.get(&domain), map.get(&domain));
			}
		}

		assert_eq!(ser_empty, "{}");
		assert!(serde_json::from_str::<DomainMap<i8>>(&ser_empty)
			.unwrap()
			.is_empty());

		assert_eq!(ser_example, r#"{"example.com":3.1415927}"#);
		assert_eq!(
			serde_json::from_str::<DomainMap<f32>>(&ser_example)
				.unwrap()
				.get(&Domain::reference("example.com").unwrap()),
			Some(&PI)
		);

		assert!(serde_json::from_str::<DomainMap<u16>>(r#""string""#)
			.unwrap_err()
			.to_string()
			.contains("a map with domain name keys and u16 values"));
	}
}
