//! A fully in-memory [`StoreBackend`] implementation, storing all data in RAM
//! with no other backups. This is mostly intended for tests, as it doesn't
//! depend on any state being persisted between links shutdown and startup, nor
//! does it depend on any external resources or services.
//!
//! # Configuration
//!
//! **Store backend name:**
//! `memory`
//!
//! **Command-line flags:**
//! *none*
//!
//! **Command-line options:**
//! *none*

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use pico_args::Arguments;
use tracing::instrument;

use crate::{
	id::Id,
	normalized::{Link, Normalized},
	store::StoreBackend,
};

/// A fully in-memory `StoreBackend` implementation useful for testing. Not
/// recommended for production, as this lacks any data persistence or backups.
#[derive(Debug)]
pub struct Store {
	redirects: RwLock<HashMap<Id, Link>>,
	vanity: RwLock<HashMap<Normalized, Id>>,
}

#[async_trait]
impl StoreBackend for Store {
	fn backend_name() -> &'static str
	where
		Self: Sized,
	{
		"memory"
	}

	fn get_backend_name(&self) -> &'static str {
		"memory"
	}

	#[instrument(level = "trace", ret, err)]
	async fn new(_config: &mut Arguments) -> Result<Self> {
		Ok(Self {
			redirects: RwLock::new(HashMap::new()),
			vanity: RwLock::new(HashMap::new()),
		})
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_redirect(&self, from: Id) -> Result<Option<Link>> {
		let redirects = self.redirects.read();
		Ok(redirects.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write();
		Ok(redirects.insert(from, to))
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_redirect(&self, from: Id) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write();
		Ok(redirects.remove(&from))
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let vanity = self.vanity.read();
		Ok(vanity.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write();
		Ok(vanity.insert(from, to))
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write();
		Ok(vanity.remove(&from))
	}
}

#[cfg(test)]
mod tests {
	use pico_args::Arguments;

	use super::Store;
	use crate::store::{tests, StoreBackend as _};

	async fn get_store() -> Store {
		Store::new(&mut Arguments::from_vec(vec![])).await.unwrap()
	}

	#[test]
	fn backend_name() {
		tests::backend_name::<Store>();
	}

	#[tokio::test]
	async fn get_backend_name() {
		tests::get_backend_name::<Store>(&get_store().await);
	}

	#[tokio::test]
	async fn get_redirect() {
		tests::get_redirect(&get_store().await).await;
	}

	#[tokio::test]
	async fn set_redirect() {
		tests::set_redirect(&get_store().await).await;
	}

	#[tokio::test]
	async fn rem_redirect() {
		tests::rem_redirect(&get_store().await).await;
	}

	#[tokio::test]
	async fn get_vanity() {
		tests::get_vanity(&get_store().await).await;
	}

	#[tokio::test]
	async fn set_vanity() {
		tests::set_vanity(&get_store().await).await;
	}

	#[tokio::test]
	async fn rem_vanity() {
		tests::rem_vanity(&get_store().await).await;
	}
}
