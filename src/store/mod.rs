//! This module contains all things relating to the way redirects, vanity
//! paths, and statistics are stored in links. For details about configuring
//! each store backend, see that backend's documentation.

mod backend;
mod memory;

#[cfg(test)]
mod tests;

pub use memory::Store as Memory;

use crate::id::Id;
use crate::normalized::{Link, Normalized};
use anyhow::{bail, Result};
use backend::StoreBackend;
use pico_args::Arguments;
use tracing::instrument;

/// A wrapper around any `StoreBackend`, providing access to the underlying
/// store along some with extra things like logging.
#[derive(Debug)]
pub struct Store {
	store: Box<dyn StoreBackend>,
}

impl Store {
	/// Create a new instance of this `Store`. Configuration is
	/// backend-specific and is provided as a collection of `pico-args`
	/// arguments beginning with `--store-`.
	///
	/// # Errors
	/// This function returns an error if the store could not be initialized.
	/// This may happen if the configuration is invalid or for other
	/// backend-specific reasons (such as a file not being createable or a
	/// network connection not being establishable, etc.).
	#[instrument(level = "debug", ret, err)]
	pub async fn new(name: &str, config: &mut Arguments) -> Result<Self> {
		if name == Memory::backend_name() {
			Ok(Self {
				store: Box::new(Memory::new(config).await?),
			})
		} else {
			bail!(format!("Unknown store \"{name}\""));
		}
	}

	/// Create a new static reference to a new instance of this `Store`.
	/// Configuration is backend-specific and is provided as a collection of
	/// `pico-args` arguments beginning with `--store-`.
	///
	/// # Errors
	/// This function returns an error if the store could not be initialized.
	/// This may happen if the configuration is invalid or for other
	/// backend-specific reasons (such as a file not being createable or a
	/// network connection not being establishable, etc.).
	#[instrument(level = "trace")]
	pub async fn new_static(name: &str, config: &mut Arguments) -> Result<&'static Self> {
		Ok(&*Box::leak(Box::new(Self::new(name, config).await?)))
	}

	/// Get the underlying implementation's name. The name (used in e.g. the
	/// configuration) of the backend store implementing this trait must be a
	/// human-readable name using only 'a'-'z', '0'-'9', and '_'.
	#[must_use]
	pub fn backend_name(&self) -> &'static str {
		self.store.get_backend_name()
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
}

#[cfg(test)]
mod store_tests {
	use super::*;

	#[tokio::test]
	async fn new() {
		Store::new("memory", &mut Arguments::from_vec(vec![]))
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn new_static() {
		Store::new_static("memory", &mut Arguments::from_vec(vec![]))
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn backend_name() {
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
			.await
			.unwrap();

		let name = store.backend_name();

		assert_eq!(name, "memory");
	}

	#[tokio::test]
	async fn get_redirect() {
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
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
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
			.await
			.unwrap();

		let id = Id::from([0x11, 0x21, 0x31, 0x41, 0x51]);
		let link = Link::new("https://example.com/test").unwrap();

		store.set_redirect(id, link.clone()).await.unwrap();

		assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
	}

	#[tokio::test]
	async fn rem_redirect() {
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
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
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
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
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
			.await
			.unwrap();

		let vanity = Normalized::new("Example Test");
		let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

		store.set_vanity(vanity.clone(), id).await.unwrap();

		assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
	}

	#[tokio::test]
	async fn rem_vanity() {
		let store = Store::new_static("memory", &mut Arguments::from_vec(vec![]))
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
