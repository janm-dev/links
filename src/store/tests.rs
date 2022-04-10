//! Generic tests for each [`Store`] implementation. These test functions have
//! the same name as the `Store` function that they are testing.

use super::*;
use crate::id::Id;
use crate::normalized::{Link, Normalized};

#[cfg(test)]
#[tokio::test]
async fn test_get() {
	for store_name in STORES {
		assert!(get(store_name).await.is_ok());
		assert_eq!(&get(store_name).await.unwrap().backend_name(), store_name);
	}

	assert!(get("some other non-existant store name").await.is_err());
}

pub async fn get_redirect(store: &impl Store) {
	let id = Id::from([0x10, 0x20, 0x30, 0x40, 0x50]);
	let link = Link::new("https://example.com/test").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(Id::new()).await.unwrap(), None);
	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
}

pub async fn set_redirect(store: &impl Store) {
	let id = Id::from([0x11, 0x21, 0x31, 0x41, 0x51]);
	let link = Link::new("https://example.com/test").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link));
}

pub async fn rem_redirect(store: &impl Store) {
	let id = Id::from([0x12, 0x22, 0x32, 0x42, 0x52]);
	let link = Link::new("https://example.com/test").unwrap();

	store.set_redirect(id, link.clone()).await.unwrap();

	assert_eq!(store.get_redirect(id).await.unwrap(), Some(link.clone()));
	store.rem_redirect(id).await.unwrap();
	assert_eq!(store.get_redirect(id).await.unwrap(), None);
}

pub async fn get_vanity(store: &impl Store) {
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

pub async fn set_vanity(store: &impl Store) {
	let vanity = Normalized::new("Example Test");
	let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

	store.set_vanity(vanity.clone(), id).await.unwrap();

	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
}

pub async fn rem_vanity(store: &impl Store) {
	let vanity = Normalized::new("Example Test");
	let id = Id::from([0x13, 0x23, 0x33, 0x43, 0x53]);

	store.set_vanity(vanity.clone(), id).await.unwrap();

	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), Some(id));
	store.rem_vanity(vanity.clone()).await.unwrap();
	assert_eq!(store.get_vanity(vanity.clone()).await.unwrap(), None);
}
