//! A fully in-memory [`Store`] implementation, storing all data in RAM with no
//! other backups. This is mostly intended for tests, as it doesn't depend on
//! any state being persisted between links shutdown and startup, nor does it
//! depend on any external resources or services.

use crate::id::Id;
use crate::normalized::{Link, Normalized};
use crate::store::Store as StoreTrait;
use anyhow::Result;
use async_trait::async_trait;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::instrument;

#[derive(Debug)]
pub struct Store {
	redirects: RwLock<HashMap<Id, Link>>,
	vanity: RwLock<HashMap<Normalized, Id>>,
}

#[async_trait]
impl StoreTrait for Store {
	const BACKEND_NAME: &'static str = "memory";

	#[instrument(level = "debug", fields(store = Self::BACKEND_NAME), ret, err)]
	async fn new() -> Result<&'static Self> {
		lazy_static! {
			static ref STORE: Store = Store {
				redirects: RwLock::new(HashMap::new()),
				vanity: RwLock::new(HashMap::new()),
			};
		}
		Ok(&STORE)
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn get_redirect(&self, from: Id) -> Result<Option<Link>> {
		let redirects = self.redirects.read().expect("redirects lock is poisoned");
		Ok(redirects.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write().expect("redirects lock is poisoned");
		Ok(redirects.insert(from, to))
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn rem_redirect(&self, from: Id) -> Result<Option<Link>> {
		let mut redirects = self.redirects.write().expect("redirects lock is poisoned");
		Ok(redirects.remove(&from))
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let vanity = self.vanity.read().expect("vanity lock is poisoned");
		Ok(vanity.get(&from).map(ToOwned::to_owned))
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write().expect("vanity lock is poisoned");
		Ok(vanity.insert(from, to))
	}

	#[instrument(level = "debug", skip(self), fields(store = Self::BACKEND_NAME), ret, err)]
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		let mut vanity = self.vanity.write().expect("vanity lock is poisoned");
		Ok(vanity.remove(&from))
	}
}

#[cfg(test)]
mod tests {
	use super::Store as MemoryStore;
	use crate::store::tests;
	use crate::store::Store as _;

	#[tokio::test]
	async fn get_redirect() {
		let store = MemoryStore::new().await.unwrap();
		tests::get_redirect(store).await;
	}

	#[tokio::test]
	async fn set_redirect() {
		let store = MemoryStore::new().await.unwrap();
		tests::set_redirect(store).await;
	}

	#[tokio::test]
	async fn rem_redirect() {
		let store = MemoryStore::new().await.unwrap();
		tests::rem_redirect(store).await;
	}

	#[tokio::test]
	async fn get_vanity() {
		let store = MemoryStore::new().await.unwrap();
		tests::get_vanity(store).await;
	}

	#[tokio::test]
	async fn set_vanity() {
		let store = MemoryStore::new().await.unwrap();
		tests::set_vanity(store).await;
	}

	#[tokio::test]
	async fn rem_vanity() {
		let store = MemoryStore::new().await.unwrap();
		tests::rem_vanity(store).await;
	}
}
