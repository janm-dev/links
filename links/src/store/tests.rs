//! Generic tests for the each [`StoreBackend`] implementation. These test
//! functions have the same name as the function that they are testing.

use links_id::Id;
use links_normalized::{Link, Normalized};

use super::*;
use crate::stats::{StatisticData, StatisticTime, StatisticType};

pub fn store_type<S: StoreBackend>() {
	let name = S::store_type().as_str();

	assert!(!name.is_empty());
	assert!(name.is_ascii());
	assert!(name
		.chars()
		.all(|c| (c.is_ascii_alphabetic() && c.is_ascii_lowercase())
			|| c.is_ascii_digit()
			|| c == '_'));
}

pub fn get_store_type<S: StoreBackend>(store: &S) {
	let name = store.get_store_type().as_str();

	assert!(!name.is_empty());
	assert!(name.is_ascii());
	assert!(name
		.chars()
		.all(|c| (c.is_ascii_alphabetic() && c.is_ascii_lowercase())
			|| c.is_ascii_digit()
			|| c == '_'));
	assert_eq!(name, S::store_type().as_str());
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

pub async fn get_statistics(store: &impl StoreBackend) {
	let id = Id::from([0x16, 0x26, 0x36, 0x46, 0x56]);
	let vanity = Normalized::new("Statistics Test One");

	let statistic_a = Statistic {
		link: id.into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let statistic_b = Statistic {
		link: vanity.clone().into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let desc_a = StatisticDescription {
		link: Some(id.into()),
		..Default::default()
	};

	let desc_b = StatisticDescription {
		link: Some(vanity.into()),
		..Default::default()
	};

	let res_a = store.get_statistics(desc_a.clone()).await;
	let res_b = store.get_statistics(desc_b.clone()).await;

	store.incr_statistic(statistic_a.clone()).await.unwrap();
	store.incr_statistic(statistic_b.clone()).await.unwrap();

	let res_c = store.get_statistics(desc_a).await.unwrap();
	let res_d = store.get_statistics(desc_b).await.unwrap();

	assert!(res_a.unwrap().is_empty());
	assert!(res_b.unwrap().is_empty());
	assert_eq!(res_c.len(), 1);
	assert_eq!(res_d.len(), 1);
	assert_eq!(res_c[0], (statistic_a, StatisticValue::new(1).unwrap()));
	assert_eq!(res_d[0], (statistic_b, StatisticValue::new(1).unwrap()));
}

pub async fn incr_statistic(store: &impl StoreBackend) {
	let id = Id::from([0x17, 0x27, 0x37, 0x47, 0x57]);
	let vanity = Normalized::new("Statistics Test Two");

	let statistic_a = Statistic {
		link: id.into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let statistic_b = Statistic {
		link: vanity.into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let res_a = store.incr_statistic(statistic_a).await;
	let res_b = store.incr_statistic(statistic_b).await;

	assert!(matches!(res_a, Ok(Some(StatisticValue { .. }))));
	assert!(matches!(res_b, Ok(Some(StatisticValue { .. }))));
}

pub async fn rem_statistics(store: &impl StoreBackend) {
	let vanity = Normalized::new("Statistics Test Three");
	let id = Id::from([0x18, 0x28, 0x38, 0x48, 0x58]);

	let statistic_a = Statistic {
		link: id.into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let statistic_b = Statistic {
		link: vanity.clone().into(),
		stat_type: StatisticType::Request,
		data: StatisticData::default(),
		time: StatisticTime::now(),
	};

	let desc_a = StatisticDescription {
		link: Some(id.into()),
		..Default::default()
	};

	let desc_b = StatisticDescription {
		link: Some(vanity.into()),
		..Default::default()
	};

	let res_a = store.get_statistics(desc_a.clone()).await;
	let res_b = store.get_statistics(desc_b.clone()).await;

	store.incr_statistic(statistic_a.clone()).await.unwrap();
	store.incr_statistic(statistic_b.clone()).await.unwrap();

	let res_c = store.get_statistics(desc_a.clone()).await.unwrap();
	let res_d = store.get_statistics(desc_b.clone()).await.unwrap();

	store.rem_statistics(desc_a.clone()).await.unwrap();
	store.rem_statistics(desc_b.clone()).await.unwrap();

	let res_e = store.get_statistics(desc_a).await.unwrap();
	let res_f = store.get_statistics(desc_b).await.unwrap();

	assert!(res_a.unwrap().is_empty());
	assert!(res_b.unwrap().is_empty());
	assert_eq!(res_c.len(), 1);
	assert_eq!(res_d.len(), 1);
	assert_eq!(res_c[0], (statistic_a, StatisticValue::new(1).unwrap()));
	assert_eq!(res_d[0], (statistic_b, StatisticValue::new(1).unwrap()));
	assert!(res_e.is_empty());
	assert!(res_f.is_empty());
}
