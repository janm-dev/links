//! This module contains a trait and its implementations for a unified
//! links and statistics store used as links' backend store. This store can be
//! something simple like in-memory hashmaps, an interface to something more
//! complex and feature-rich like Redis, or anything in between. The aim of
//! this `Store` trait is to make it easy to swap between different storage
//! backends, and to make developing them fast.
//!
//! The store has three main functions:
//! - store redirects, mapping from IDs to links
//! - store vanity links, mapping from vanity urls to IDs
//! - store statistics

mod memory;

#[cfg(test)]
mod tests;

pub use memory::Store as MemoryStore;
//pub use redis::Store as RedisStore;

use crate::id::Id;
use crate::normalized::{Link, Normalized};
use anyhow::{bail, Result};
use async_trait::async_trait;
use core::fmt::Debug;

pub const STORES: &[&str] = &[MemoryStore::BACKEND_NAME];

/// Get the specified Store implementation by its name. Note that this may or
/// may not create a new store/connection, or reuse an old one. For
/// consistency, this should only ever by called once.
///
/// # Errors
/// This returns an error if the provided `name` is not recognized as the name
/// of any known store backend. The error contains information about which
/// stores are available.
pub async fn get(name: &str) -> Result<&'static impl Store> {
	match name {
		MemoryStore::BACKEND_NAME => MemoryStore::new().await,
		//RedisStore::BACKEND_NAME => RedisStore::new().await,
		_ => {
			let stores = STORES.join(", ");
			bail!(format!(
				"Store \"{name}\" not found. Available store backends: {stores}"
			))
		}
	}
}

/// The link and statistics store trait used by links.
#[async_trait]
pub trait Store: Debug + Send + Sync {
	/// The name (used in e.g. the configuration file) of the backend store
	/// implementing this trait. Must be a human-readable name using only
	/// 'a'-'z', '0'-'9', and '_'.
	const BACKEND_NAME: &'static str;

	/// Get this implementation's backend name.
	///
	/// `store.backend_name()` is equivalent to `Store::BACKEND_NAME`.
	fn backend_name(&self) -> &'static str {
		Self::BACKEND_NAME
	}

	/// Create a new instance of this `Store`. Any configuration must be taken
	/// from environment variables within this function.
	async fn new() -> Result<&'static Self>;

	/// Get a redirect. Returns the full `to` link corresponding to the `from`
	/// links ID. A link not existing is not an error, if no matching link is
	/// found, `Ok(None)` is returned.
	///
	/// # Error
	/// An error is only returned if something actually fails; if we don't know
	/// if a link exists or not, or what it is. A link not existing is not
	/// considered an error.
	async fn get_redirect(&self, from: Id) -> Result<Option<Link>>;

	/// Set a redirect. `from` is the ID of the link, while `to` is the full
	/// destination link. If a mapping with this ID already exists, it must be
	/// changed to the new one, returning the old one.
	///
	/// # Storage Guarantees
	/// If an Ok is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an Err is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>>;

	/// Remove a redirect. `from` is the ID of the links link to be removed.
	/// Returns the old value of the mapping or `None` if there was no such
	/// mapping.
	///
	/// # Storage Guarantees
	/// If an Ok is returned, the new value was definitely removed / processed
	/// / saved, and will be unavailable on next request.
	/// If an Err is returned, the value must not have been removed / modified,
	/// insofar as that is possible to determine from the backend.
	async fn rem_redirect(&self, from: Id) -> Result<Option<Link>>;

	/// Get a vanity path's ID. Returns the ID of the `to` link corresponding
	/// to the `from` vanity path. An ID not existing is not an error, if no
	/// matching ID is found, `None` is returned.
	///
	/// # Error
	/// An error is only returned if something actually fails; if we don't know
	/// if a link exists or not, or what it is. A link not existing is not
	/// considered an error.
	async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>>;

	/// Set a vanity path for an ID. `from` is the vanity path of the links ID,
	/// while `to` is the ID itself. If a vanity link with this path already
	/// exists, it must be changed to the new one, returning the old one.
	///
	/// # Storage Guarantees
	/// If an Ok is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an Err is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>>;

	/// Remove a vanity path. `from` is the vanity path to be removed. Returns
	/// the old value of the mapping or `None` if there was no such mapping.
	///
	/// # Storage Guarantees
	/// If an Ok is returned, the new value was definitely removed / processed
	/// / saved, and will be unavailable on next request.
	/// If an Err is returned, the value must not have been removed / modified,
	/// insofar as that is possible to determine from the backend.
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>>;
}
