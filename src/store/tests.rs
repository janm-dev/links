//! Generic tests for the each [`StoreBackend`] implementation. These test
//! functions have the same name as the function that they are testing.

use super::*;
use crate::{
	id::Id,
	normalized::{Link, Normalized},
};

pub fn backend_name<S: StoreBackend>() {
	let name = S::backend_name();

	assert!(!name.is_empty());
	assert!(name.is_ascii());
	assert!(name
		.chars()
		.all(|c| (c.is_ascii_alphabetic() && c.is_ascii_lowercase())
			|| c.is_ascii_digit()
			|| c == '_'));
}

pub fn get_backend_name<S: StoreBackend>(store: &S) {
	let name = store.get_backend_name();

	assert!(!name.is_empty());
	assert!(name.is_ascii());
	assert!(name
		.chars()
		.all(|c| (c.is_ascii_alphabetic() && c.is_ascii_lowercase())
			|| c.is_ascii_digit()
			|| c == '_'));
	assert_eq!(name, S::backend_name());
}

pub async fn get_redirect(store: &impl StoreBackend) {
	let id = Id::from([0x10, 0x20, 0x30, 0x40, 0x50]);
	let link = Link::new("https://example.com/test/1").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(Id::new()).await.unwrap(), None);
	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
}

pub async fn set_redirect(store: &impl StoreBackend) {
	let id = Id::from([0x11, 0x21, 0x31, 0x41, 0x51]);
	let link = Link::new("https://example.com/test/2").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
}

pub async fn rem_redirect(store: &impl StoreBackend) {
	let id = Id::from([0x12, 0x22, 0x32, 0x42, 0x52]);
	let link = Link::new("https://example.com/test/3").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link.clone()));
	store.rem_redirect(id).await.unwrap();
	assert_eq!(store.get_redirect(id).await.unwrap(), None);
}

pub async fn get_vanity(store: &impl StoreBackend) {
	let vanity = Normalized::new("Example Test One");
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

pub async fn set_vanity(store: &impl StoreBackend) {
	let vanity = Normalized::new("Example Test Two");
	let id = Id::from([0x14, 0x24, 0x34, 0x44, 0x54]);

	store.set_vanity(vanity.clone(), id).await.unwrap();

	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
}

pub async fn rem_vanity(store: &impl StoreBackend) {
	let vanity = Normalized::new("Example Test Three");
	let id = Id::from([0x15, 0x25, 0x35, 0x45, 0x55]);

	store.set_vanity(vanity.clone(), id).await.unwrap();

	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
	store.rem_vanity(vanity.clone()).await.unwrap();
	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), None);
}
