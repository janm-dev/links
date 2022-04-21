//! A fully in-memory [`StoreBackend`] implementation, storing all data in RAM
//! with no other backups. This is mostly intended for tests, as it doesn't
//! depend on any state being persisted between links shutdown and startup, nor
//! does it depend on any external resources or services.
//!
//! # Configuration
//! **Store backend name:**
//! `memory`
//! **Command-line flags:**
//! *none*
//! **Command-line options:**
//! *none*

use crate::id::Id;
use crate::normalized::{Link, Normalized};
use crate::store::StoreBackend;
use anyhow::Result;
use async_trait::async_trait;
use pico_args::Arguments;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::instrument;

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
		let redirects = self.redirects.read().expect("redirects lock is poisoned");
		Ok(redirects.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write().expect("redirects lock is poisoned");
		Ok(redirects.insert(from, to))
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_redirect(&self, from: Id) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write().expect("redirects lock is poisoned");
		Ok(redirects.remove(&from))
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let vanity = self.vanity.read().expect("vanity lock is poisoned");
		Ok(vanity.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write().expect("vanity lock is poisoned");
		Ok(vanity.insert(from, to))
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write().expect("vanity lock is poisoned");
		Ok(vanity.remove(&from))
	}
}

#[cfg(test)]
mod tests {
	use super::Store;
	use crate::store::tests;
	use crate::store::StoreBackend as _;
	use pico_args::Arguments;

	#[test]
	fn backend_name() {
		tests::backend_name::<Store>();
	}

	#[tokio::test]
	async fn get_backend_name() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::get_backend_name::<Store>(&store);
	}

	#[tokio::test]
	async fn get_redirect() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::get_redirect(&store).await;
	}

	#[tokio::test]
	async fn set_redirect() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::set_redirect(&store).await;
	}

	#[tokio::test]
	async fn rem_redirect() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::rem_redirect(&store).await;
	}

	#[tokio::test]
	async fn get_vanity() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::get_vanity(&store).await;
	}

	#[tokio::test]
	async fn set_vanity() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::set_vanity(&store).await;
	}

	#[tokio::test]
	async fn rem_vanity() {
		let store = Store::new(&mut Arguments::from_vec(vec![])).await.unwrap();
		tests::rem_vanity(&store).await;
	}
}
