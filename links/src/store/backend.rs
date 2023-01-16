//! This module contains a trait and its implementations for a unified
//! redirect, vanity path, and statistics store used as links' backend store.
//! This store can be something simple like in-memory hashmaps, an interface to
//! something more complex and feature-rich like Redis, or anything in between.
//! The aim of this [`StoreBackend`] trait is to make it easy to swap between
//! different storage backends, and to make developing them fast. For details
//! about configuring each store backend, see that backend's documentation.

use core::fmt::Debug;
use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use links_id::Id;

use crate::{
	normalized::{Link, Normalized},
	stats::{Statistic, StatisticDescription, StatisticValue},
	store::BackendType,
};

/// The redirect, vanity path, and statistics store trait used by links.
#[async_trait]
#[allow(clippy::module_name_repetitions)]
pub trait StoreBackend: Debug + Send + Sync {
	/// Get this implementation's backend store type. This is used in
	/// e.g. the configuration.
	fn store_type() -> BackendType
	where
		Self: Sized;

	/// Get this implementation's backend store type. This can be used on trait
	/// objects, but is otherwise equivalent to calling `Self::store_type()`.
	fn get_store_type(&self) -> BackendType;

	/// Create a new instance of this `StoreBackend`. Configuration is provided
	/// as a collection of `pico-args` arguments beginning with `--store-`. For
	/// details about configuring each store backend, see that backend's
	/// documentation.
	async fn new(config: &HashMap<String, String>) -> Result<Self>
	where
		Self: Sized;

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
	/// If an `Ok` is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an `Err` is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>>;

	/// Remove a redirect. `from` is the ID of the links link to be removed.
	/// Returns the old value of the mapping or `None` if there was no such
	/// mapping.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely removed /
	/// processed / saved, and will be unavailable on next request.
	/// If an `Err` is returned, the value must not have been removed /
	/// modified, insofar as that is possible to determine from the backend.
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
	/// If an `Ok` is returned, the new value was definitely set / processed /
	/// saved, and will be available on next request.
	/// If an `Err` is returned, the value must not have been set / modified,
	/// insofar as that is possible to determine from the backend.
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>>;

	/// Remove a vanity path. `from` is the vanity path to be removed. Returns
	/// the old value of the mapping or `None` if there was no such mapping.
	///
	/// # Storage Guarantees
	/// If an `Ok` is returned, the new value was definitely removed /
	/// processed / saved, and will be unavailable on next request.
	/// If an `Err` is returned, the value must not have been removed /
	/// modified, insofar as that is possible to determine from the backend.
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>>;

	/// Get statistics' values by their description. Returns all matching
	/// [`Statistic`]s and their values for the provided
	/// [`StatisticDescription`]. Statistics not having been collected is not an
	/// error, if no matching statistics are found, an empty [`Vec`] is
	/// returned.
	///
	/// By default this function returns an empty [`Vec`]
	///
	/// # Error
	/// An error is only returned if something fails when it should have worked.
	/// A statistic not existing or the store not supporting statistics is not
	/// considered an error.
	async fn get_statistics(
		&self,
		_description: StatisticDescription,
	) -> Result<Vec<(Statistic, StatisticValue)>> {
		Ok(Vec::new())
	}

	/// Increment a statistic's count. The provided [`Statistic`]'s value is
	/// incremented by 1. Returns the new value of the statistic after the
	/// increment, or `None` if the statistic wasn't recorded or its new value
	/// is not known.
	///
	/// By default this function does nothing and returns `Ok(None)`
	///
	/// # Error
	/// An error is only returned if something fails when it should have worked.
	/// A statistic not being recorded (immediately or ever) is not considered
	/// and error.
	async fn incr_statistic(&self, _statistic: Statistic) -> Result<Option<StatisticValue>> {
		Ok(None)
	}

	/// Remove statistics by their description. Deletes all [`Statistic`]s that
	/// match the provided [`StatisticDescription`] and returns their values
	/// before they were deleted, if they're available. A statistic not having
	/// been collected is not an error, if no matching statistics are found, an
	/// empty [`Vec`] is returned.
	///
	/// By default this function does nothing and returns an empty [`Vec`]
	///
	/// # Error
	/// An error is only returned if something fails when it should have worked.
	/// A statistic not existing or the store not supporting statistics is not
	/// considered an error.
	async fn rem_statistics(
		&self,
		_description: StatisticDescription,
	) -> Result<Vec<(Statistic, StatisticValue)>> {
		Ok(Vec::new())
	}
}
