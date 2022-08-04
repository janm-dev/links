//! A fully in-memory [`StoreBackend`] implementation, storing all data in RAM
//! with no other backups. This is mostly intended for tests, as it doesn't
//! depend on any state being persisted between links shutdown and startup, nor
//! does it depend on any external resources or services.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::instrument;

use crate::{
	id::Id,
	normalized::{Link, Normalized},
	store::{BackendType, StoreBackend},
};

/// A fully in-memory `StoreBackend` implementation useful for testing. Not
/// recommended for production, as this lacks any data persistence or backups.
///
/// # Configuration
///
/// **Store backend name:**
/// `memory`
///
/// **Configuration:**
/// *none*
#[derive(Debug)]
pub struct Store {
	redirects: RwLock<HashMap<Id, Link>>,
	vanity: RwLock<HashMap<Normalized, Id>>,
}

#[async_trait]
impl StoreBackend for Store {
	fn store_type() -> BackendType
	where
		Self: Sized,
	{
		BackendType::Memory
	}

	fn get_store_type(&self) -> BackendType {
		BackendType::Memory
	}

	#[instrument(level = "trace", ret, err)]
	async fn new(_config: &HashMap<String, String>) -> Result<Self> {
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
	use std::collections::HashMap;

	use super::Store;
	use crate::store::{tests, StoreBackend as _};

	async fn get_store() -> Store {
		Store::new(&HashMap::from([])).await.unwrap()
	}

	#[test]
	fn store_type() {
		tests::store_type::<Store>();
	}

	#[tokio::test]
	async fn get_store_type() {
		tests::get_store_type::<Store>(&get_store().await);
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
