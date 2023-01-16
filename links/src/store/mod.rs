//! This module contains all things relating to the way redirects, vanity
//! paths, and statistics are stored in links. For details about configuring
//! each store backend, see that backend's documentation.

pub mod backend;
mod memory;
mod redis;

#[cfg(test)]
mod tests;

use std::{
	collections::HashMap,
	error::Error,
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr,
	sync::Arc,
};

use anyhow::Result;
use backend::StoreBackend;
use links_id::Id;
use links_normalized::{Link, Normalized};
use parking_lot::RwLock;
use serde_derive::{Deserialize, Serialize};
use tokio::spawn;
use tracing::{debug, instrument, trace};

pub use self::{memory::Store as Memory, redis::Store as Redis};
use crate::stats::{Statistic, StatisticDescription, StatisticValue};

/// The type of store backend used by the links redirector server. All variants
/// must have a canonical human-readable string representation using only
/// 'a'-'z', '0'-'9', and '_'.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
	/// A fully in-memory store backend, storing all data in RAM
	/// with no other backups, but without any external dependencies. Not
	/// recommended outside of tests.
	#[default]
	Memory,
	/// A store backend which stores all data using a Redis 6.2+ server.
	Redis,
}

impl BackendType {
	const fn to_str(self) -> &'static str {
		match self {
			Self::Memory => "memory",
			Self::Redis => "redis",
		}
	}
}

impl FromStr for BackendType {
	type Err = IntoBackendTypeError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"memory" => Ok(Self::Memory),
			"redis" => Ok(Self::Redis),
			s => Err(IntoBackendTypeError(s.to_string())),
		}
	}
}

impl Display for BackendType {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_str(self.to_str())
	}
}

/// The error returned by fallible conversions into a [`BackendType`]. Contains
/// the original input string.
#[derive(Debug, Clone)]
pub struct IntoBackendTypeError(String);

impl Display for IntoBackendTypeError {
	fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
		fmt.write_fmt(format_args!("unrecognized store backend type {}", self.0))
	}
}

impl Error for IntoBackendTypeError {}

/// A holder for a [`Store`], which allows the store to be updated on the fly.
#[derive(Debug)]
pub struct Current {
	store: RwLock<Store>,
}

impl Current {
	/// Create a new [`Current`]. The store passed into this function will be
	/// returned by calls to `get`.
	#[must_use]
	pub const fn new(store: Store) -> Self {
		Self {
			store: RwLock::new(store),
		}
	}

	/// Create a new static reference to a new [`Current`]. The store passed
	/// into this function will be returned by calls to `get`.
	///
	/// # Memory
	/// This function leaks memory. Make sure it is not called an unbounded
	/// number of times.
	#[must_use]
	pub fn new_static(store: Store) -> &'static Self {
		Box::leak(Box::new(Self::new(store)))
	}

	/// Get the current [`Store`]. The returned store itself will remain
	/// unchanged even if the [`Current`] is updated.
	pub fn get(&self) -> Store {
		self.store.read().clone()
	}

	/// Update the store inside this [`Current`]. All future calls to `get` will
	/// return this store instead, but all still-active stores will remain
	/// unchanged.
	pub fn update(&self, store: Store) {
		*self.store.write() = store;
	}

	/// Get the current store's backend name. This is (slightly) more efficient
	/// than `current.get().backend_name()`, because it doesn't need to update
	/// the store's internal reference count.
	pub fn backend_name(&self) -> &'static str {
		self.store.read().backend_name()
	}
}

/// A wrapper around any [`StoreBackend`], providing access to the underlying
/// store along some with extra things like logging.
#[derive(Debug, Clone)]
pub struct Store {
	store: Arc<dyn StoreBackend>,
}

impl Store {
	/// Create a new instance of this `Store`. Configuration is
	/// backend-specific and is provided as a `HashMap` from string keys to
	/// string values, that are parsed by the backend as needed.
	///
	/// # Errors
	/// This function returns an error if the store could not be initialized.
	/// This may happen if the configuration is invalid or for other
	/// backend-specific reasons (such as a file not being createable or a
	/// network connection not being establishable, etc.).
	#[instrument(level = "debug", ret, err)]
	pub async fn new(store_type: BackendType, config: &HashMap<String, String>) -> Result<Self> {
		match store_type {
			BackendType::Memory => Ok(Self {
				store: Arc::new(Memory::new(config).await?),
			}),
			BackendType::Redis => Ok(Self {
				store: Arc::new(Redis::new(config).await?),
			}),
		}
	}

	/// Get the underlying implementation's name. The name (used in e.g. the
	/// configuration) of the backend store implementing this trait must be a
	/// human-readable name using only 'a'-'z', '0'-'9', and '_'.
	#[must_use]
	pub fn backend_name(&self) -> &'static str {
		self.store.get_store_type().to_str()
	}

	/// Get a redirect. Returns the full `to` link corresponding to the `from`
	/// links ID. A link not existing is not an error, if no matching link is
	/// found, `Ok(None)` is returned.
	///
	/// # Error
	/// An error is only returned if something actually fails; if we don't know
	/// if a link exists or not, or what it is. A link not existing is not
	/// considered an error.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn get_redirect(&self, from: Id) -> Result<Option<Link>> {
		self.store.get_redirect(from).await
	}

	/// Set a redirect. `from` is the ID of the link, while `to` is the full
	/// destination link. If a mapping with this ID already exists, it must be
	/// changed to the new one, returning the old one.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an `Err` is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>> {
		self.store.set_redirect(from, to).await
	}

	/// Remove a redirect. `from` is the ID of the links link to be removed.
	/// Returns the old value of the mapping or `None` if there was no such
	/// mapping.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely removed /
	/// processed / saved, and will be unavailable on next request.
	/// If an `Err` is returned, the value must not have been removed /
	/// modified, insofar as that is possible to determine from the backend.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn rem_redirect(&self, from: Id) -> Result<Option<Link>> {
		self.store.rem_redirect(from).await
	}

	/// Get a vanity path's ID. Returns the ID of the `to` link corresponding
	/// to the `from` vanity path. An ID not existing is not an error, if no
	/// matching ID is found, `None` is returned.
	///
	/// # Error
	/// An error is only returned if something actually fails; if we don't know
	/// if a link exists or not, or what it is. A link not existing is not
	/// considered an error.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		self.store.get_vanity(from).await
	}

	/// Set a vanity path for an ID. `from` is the vanity path of the links ID,
	/// while `to` is the ID itself. If a vanity link with this path already
	/// exists, it must be changed to the new one, returning the old one.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an `Err` is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>> {
		self.store.set_vanity(from, to).await
	}

	/// Remove a vanity path. `from` is the vanity path to be removed. Returns
	/// the old value of the mapping or `None` if there was no such mapping.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely removed /
	/// processed / saved, and will be unavailable on next request.
	/// If an `Err` is returned, the value must not have been removed /
	/// modified, insofar as that is possible to determine from the backend.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		self.store.rem_vanity(from).await
	}

	/// Get statistics' values by their description. Returns all matching
	/// [statistics][`Statistic`] and their values for the provided [statistic
	/// description][`StatisticDescription`]. Statistics not having been
	/// collected is not an error, if no matching statistics are found, an empty
	/// iterator is returned.
	///
	/// # Error
	/// An error is only returned if something fails when it should have worked.
	/// A statistic not existing or the store not supporting statistics is not
	/// considered an error.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn get_statistics(
		&self,
		description: StatisticDescription,
	) -> Result<impl Iterator<Item = (Statistic, StatisticValue)>> {
		Ok(self.store.get_statistics(description).await?.into_iter())
	}

	/// Increment multiple statistics' count for the given id and/or vanity
	/// path. Each of the provided [statistic][`Statistic`]s' values for the
	/// provided [id][`Id`] and [vanity path][`Normalized`] are incremented by 1
	/// in a spawned tokio task in the background.
	///
	/// # Error
	/// This function failing in any way is not considered an error, because
	/// statistics are done on a best-effort basis. However, any errors that
	/// occur are logged.
	pub fn incr_statistics<I>(&self, statistics: I)
	where
		I: IntoIterator<Item = Statistic> + Send + 'static,
		<I as IntoIterator>::IntoIter: Send,
	{
		let store = self.store.clone();
		spawn(async move {
			for stat in statistics {
				match store.incr_statistic(stat.clone()).await {
					Ok(val) => trace!(?val, ?stat, "statistic incremented"),
					Err(err) => debug!(?err, ?stat, "statistic incrementing failed"),
				}
			}
		});
	}

	/// Remove statistics by their description. Deletes all
	/// [statistics][`Statistic`] that match the provided
	/// [description][`StatisticDescription`] and returns their values before
	/// they were deleted, if they're available. A statistic not having been
	/// collected is not an error, if no matching statistics are found, an empty
	/// iterator is returned.
	///
	/// # Error
	/// An error is only returned if something fails when it should have worked.
	/// A statistic not existing or the store not supporting statistics is not
	/// considered an error.
	#[instrument(level = "debug", skip(self), fields(name = self.backend_name()), ret, err)]
	pub async fn rem_statistics(
		&self,
		description: StatisticDescription,
	) -> Result<impl Iterator<Item = (Statistic, StatisticValue)>> {
		Ok(self.store.rem_statistics(description).await?.into_iter())
	}
}

#[cfg(test)]
mod store_tests {
	use super::*;

	#[tokio::test]
	async fn current() {
		let id = Id::from([1, 2, 3, 4, 5]);
		let link = Link::from_str("https://example.com").unwrap();
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();
		store.set_redirect(id, link.clone()).await.unwrap();

		let current = Current::new(store.clone());
		let static_current = Current::new_static(store);

		assert_eq!(
			current.get().get_redirect(id).await.unwrap(),
			Some(link.clone())
		);
		assert_eq!(
			static_current.get().get_redirect(id).await.unwrap(),
			Some(link)
		);

		let new_store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();
		current.update(new_store.clone());
		static_current.update(new_store.clone());

		assert_eq!(current.get().get_redirect(id).await.unwrap(), None);
		assert_eq!(static_current.get().get_redirect(id).await.unwrap(), None);
	}

	#[test]
	fn type_to_from() {
		assert_eq!(
			BackendType::Memory,
			BackendType::Memory.to_str().parse().unwrap()
		);

		assert_eq!(
			BackendType::Redis,
			BackendType::Redis.to_str().parse().unwrap()
		);
	}

	#[tokio::test]
	async fn cheap_clone() {
		let store_a = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();
		let store_b = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();
		let store_c = store_a.clone();
		let id = Id::from([0, 1, 2, 3, 4]);

		store_a
			.set_redirect(id, Link::new("https://example.com/test").unwrap())
			.await
			.unwrap();

		assert_eq!(
			store_a.get_redirect(id).await.unwrap(),
			store_c.get_redirect(id).await.unwrap(),
		);
		assert_ne!(
			store_a.get_redirect(id).await.unwrap(),
			store_b.get_redirect(id).await.unwrap(),
		);
	}

	#[tokio::test]
	async fn new() {
		Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn backend_name() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let name = store.backend_name();

		assert_eq!(name, "memory");
	}

	#[tokio::test]
	async fn get_redirect() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let id = Id::from([0x10, 0x20, 0x30, 0x40, 0x50]);
		let link = Link::new("https://example.com/test").unwrap();

		store.set_redirect(id, link.clone()).await.unwrap();

		assert_eq!(store.get_redirect(Id::new()).await.unwrap(), None);
		assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
	}

	#[tokio::test]
	async fn set_redirect() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let id = Id::from([0x11, 0x21, 0x31, 0x41, 0x51]);
		let link = Link::new("https://example.com/test").unwrap();

		store.set_redirect(id, link.clone()).await.unwrap();

		assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
	}

	#[tokio::test]
	async fn rem_redirect() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let id = Id::from([0x12, 0x22, 0x32, 0x42, 0x52]);
		let link = Link::new("https://example.com/test").unwrap();

		store.set_redirect(id, link.clone()).await.unwrap();

		assert_eq!(store.get_redirect(id).await.unwrap(), Some(link.clone()));
		store.rem_redirect(id).await.unwrap();
		assert_eq!(store.get_redirect(id).await.unwrap(), None);
	}

	#[tokio::test]
	async fn get_vanity() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let vanity = Normalized::new("Example Test");
		let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

		store.set_vanity(vanity.clone(), id).await.unwrap();

		assert_eq!(
			store
				.get_vanity(Normalized::new("Doesn't exist."))
				.await
				.unwrap(),
			None
		);
		assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
	}

	#[tokio::test]
	async fn set_vanity() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let vanity = Normalized::new("Example Test");
		let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

		store.set_vanity(vanity.clone(), id).await.unwrap();

		assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
	}

	#[tokio::test]
	async fn rem_vanity() {
		let store = Store::new("memory".parse().unwrap(), &HashMap::new())
			.await
			.unwrap();

		let vanity = Normalized::new("Example Test");
		let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

		store.set_vanity(vanity.clone(), id).await.unwrap();

		assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
		store.rem_vanity(vanity.clone()).await.unwrap();
		assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), None);
	}
}
