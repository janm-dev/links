//! A Redis-backed [`StoreBackend`] implementation, storing all data on a Redis
//! server at the provided address. This store backend is recommended for most
//! situations, as the data is stored in a persistent\* and distributed\*, but
//! still very high-performance way, which also allows any\* number of links
//! instances to connect and share the same underlying data source.
//!
//! \* If Redis is configured appropriately.
//!
//! This is tested with and developed against Redis 6.2 and 7.0. Older Redis
//! versions may be supported in the future. Newer Redis versions will be
//! supported as they are released.
//!
//! On Redis, redirects and vanity paths are stored in the specified database
//! with keys in the following format:
//! - `links:redirect:[ID]` for redirects (with string values of URLs)
//! - `links:vanity:[vanity]` for vanity paths (with string values of IDs)
//! - `links:stat:[link]:[type]:[time]:[data]` for statistics (with int values)
//!
//! Some extra metadata is also needed for certain operations:
//! - `links:stat-all` set of all statistics (json)
//! - `links:stat-link:[link]` set of all statistics with that link (json)
//! - `links:stat-type:[type]` set of all statistics with that type (json)
//! - `links:stat-time:[time]` set of all statistics with that time (json)
//! - `links:stat-data:[data]` set of all statistics with that data (json)

use std::{
	collections::HashMap,
	fmt::{Debug, Formatter, Result as FmtResult},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use fred::{
	pool::RedisPool,
	prelude::*,
	types::{ArcStr, RespVersion, Server, TlsConnector},
};
use links_id::Id;
use links_normalized::{Link, Normalized};
use tokio::try_join;
use tracing::instrument;

use super::BackendType;
use crate::{
	stats::{Statistic, StatisticDescription, StatisticValue},
	store::StoreBackend,
};

/// A Redis-backed `StoreBackend` implementation. The best option for most
/// links deployments.
///
/// # Configuration
///
/// **Store backend name:**
/// `redis`
///
/// **Configuration:**
/// - `cluster`: Use Redis cluster mode. If this is enabled, cluster information
///   will be requested from Redis nodes (which will fail if the server isn't in
///   cluster mode). *`true` / `false`*. **Default `false`**.
/// - `connect`: Connection information in the format of `host:port` to connect
///   to. When using Redis in cluster mode, you can configure multiple
///   `host:port` pairs seperated by commas for different nodes (i.e.
///   `host1:port1,host2:port2,host3:port3`), but only one is required (the
///   others will be automatically discovered). Note that this is not a full
///   URL, just the host and port.
/// - `username`: The username to use for the connection, when using ACLs on the
///   server. Don't specify this when using password-based auth.
/// - `password`: The password to use for the Redis connection. This can either
///   be the user's password (when using ACLs) or the global server password
///   when using password-based authentication.
/// - `database`: The database number to use for the Redis connection. **Default
///   `0`**.
/// - `tls`: Enable TLS (using system root CAs) when communicating with the
///   Redis server. *`true` / `false`*. **Default `false`**.
/// - `pool_size`: The number of connections to use in the connection pool.
///   **Default `8`**.
pub struct Store {
	pool: RedisPool,
}

impl Debug for Store {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		f.debug_struct("Store").finish_non_exhaustive()
	}
}

#[async_trait]
impl StoreBackend for Store {
	fn store_type() -> BackendType
	where
		Self: Sized,
	{
		BackendType::Redis
	}

	fn get_store_type(&self) -> BackendType {
		BackendType::Redis
	}

	#[instrument(level = "trace", ret, err)]
	async fn new(config: &HashMap<String, String>) -> Result<Self> {
		let server_config = if config.get("cluster").map_or(Ok(false), |s| s.parse())? {
			ServerConfig::Clustered {
				hosts: config
					.get("connect")
					.ok_or_else(|| anyhow!("missing connect option"))?
					.split(',')
					.map(|s| {
						s.trim()
							.split_once(':')
							.map(|v| {
								let host = ArcStr::from(v.0);

								Ok(Server {
									host: host.clone(),
									port: v.1.parse::<u16>()?,
									tls_server_name: Some(host),
								})
							})
							.ok_or_else(|| anyhow!("couldn't parse connect value"))?
					})
					.collect::<Result<_, anyhow::Error>>()?,
			}
		} else {
			let (host, port) = config
				.get("connect")
				.map(|s| {
					s.split_once(':')
						.map::<Result<_, anyhow::Error>, _>(|v| {
							Ok((ArcStr::from(v.0), v.1.parse::<u16>()?))
						})
						.ok_or_else(|| anyhow!("couldn't parse connect value"))?
				})
				.ok_or_else(|| anyhow!("missing connect option"))??;

			ServerConfig::Centralized {
				server: Server {
					host: host.clone(),
					port,
					tls_server_name: Some(host),
				},
			}
		};

		let pool_config = RedisConfig {
			username: config.get("username").map(String::clone),
			password: config.get("password").map(String::clone),
			server: server_config,
			version: RespVersion::RESP3,
			database: config.get("database").map(|s| s.parse()).transpose()?,
			tracing: TracingConfig {
				enabled: true,
				..Default::default()
			},
			tls: if config.get("tls").map_or(Ok(false), |s| s.parse())? {
				Some(TlsConnector::default_rustls()?.into())
			} else {
				None
			},
			..RedisConfig::default()
		};

		let pool = RedisPool::new(
			pool_config,
			None,
			Some(ReconnectPolicy::new_constant(0, 100)),
			config
				.get("pool_size")
				.map(|s| s.parse())
				.transpose()?
				.unwrap_or(8),
		)?;

		pool.connect();
		pool.wait_for_connect().await?;

		Ok(Self { pool })
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_redirect(&self, from: Id) -> Result<Option<Link>> {
		Ok(self.pool.get(format!("links:redirect:{from}")).await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_redirect(&self, from: Id, to: Link) -> Result<Option<Link>> {
		Ok(self
			.pool
			.set(
				format!("links:redirect:{from}"),
				to.into_string(),
				None,
				None,
				true,
			)
			.await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_redirect(&self, from: Id) -> Result<Option<Link>> {
		Ok(self.pool.getdel(format!("links:redirect:{from}")).await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		Ok(self.pool.get(format!("links:vanity:{from}")).await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn set_vanity(&self, from: Normalized, to: Id) -> Result<Option<Id>> {
		Ok(self
			.pool
			.set(
				format!("links:vanity:{from}"),
				to.to_string(),
				None,
				None,
				true,
			)
			.await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_vanity(&self, from: Normalized) -> Result<Option<Id>> {
		Ok(self.pool.getdel(format!("links:vanity:{from}")).await?)
	}

	#[instrument(level = "trace", ret, err)]
	async fn get_statistics(
		&self,
		description: StatisticDescription,
	) -> Result<Vec<(Statistic, StatisticValue)>> {
		let mut keys = Vec::with_capacity(5);

		keys.push("links:stat-all".to_string());

		if let Some(link) = description.link {
			keys.push(format!("links:stat-link:{link}"));
		}

		if let Some(stat_type) = description.stat_type {
			keys.push(format!("links:stat-type:{stat_type}"));
		}

		if let Some(data) = description.data {
			keys.push(format!("links:stat-data:{data}"));
		}

		if let Some(time) = description.time {
			keys.push(format!("links:stat-time:{time}"));
		}

		let stats: Vec<Statistic> = self
			.pool
			.sinter::<Vec<String>, _>(keys)
			.await?
			.into_iter()
			.filter_map(|s| serde_json::from_str(&s).ok())
			.collect();

		let stat_keys = stats
			.iter()
			.map(
				|Statistic {
				     link,
				     stat_type,
				     time,
				     data,
				 }| format!("links:stat:{link}:{stat_type}:{time}:{data}"),
			)
			.collect::<Vec<String>>();

		let values: Vec<Option<u64>> = if stat_keys.is_empty() {
			Vec::new()
		} else {
			self.pool.mget(stat_keys).await?
		};

		let res = stats
			.into_iter()
			.zip(values.into_iter())
			.filter_map(|(s, v)| Some((s, StatisticValue::new(v?)?)))
			.collect();

		Ok(res)
	}

	#[instrument(level = "trace", ret, err)]
	async fn incr_statistic(&self, statistic: Statistic) -> Result<Option<StatisticValue>> {
		let stat_json = serde_json::to_string(&statistic)?;

		let Statistic {
			link,
			stat_type,
			data,
			time,
		} = statistic;

		let values: Vec<RedisValue> = self
			.pool
			.incr(format!("links:stat:{link}:{stat_type}:{time}:{data}"))
			.await?;

		try_join!(
			self.pool
				.sadd::<(), _, _>("links:stat-all".to_string(), &stat_json),
			self.pool
				.sadd::<(), _, _>(format!("links:stat-link:{link}"), &stat_json),
			self.pool
				.sadd::<(), _, _>(format!("links:stat-type:{stat_type}"), &stat_json),
			self.pool
				.sadd::<(), _, _>(format!("links:stat-data:{data}"), &stat_json),
			self.pool
				.sadd::<(), _, _>(format!("links:stat-time:{time}"), &stat_json),
		)?;

		Ok(values
			.first()
			.and_then(RedisValue::as_u64)
			.and_then(StatisticValue::new))
	}

	#[instrument(level = "trace", ret, err)]
	async fn rem_statistics(
		&self,
		description: StatisticDescription,
	) -> Result<Vec<(Statistic, StatisticValue)>> {
		let mut keys = Vec::with_capacity(5);

		keys.push("links:stat-all".to_string());

		if let Some(link) = description.link {
			keys.push(format!("links:stat-link:{link}"));
		}

		if let Some(stat_type) = description.stat_type {
			keys.push(format!("links:stat-type:{stat_type}"));
		}

		if let Some(data) = description.data {
			keys.push(format!("links:stat-data:{data}"));
		}

		if let Some(time) = description.time {
			keys.push(format!("links:stat-time:{time}"));
		}

		let stats_json: Vec<String> = self.pool.sinter(keys.clone()).await?;
		let stats: Vec<Statistic> = stats_json
			.iter()
			.filter_map(|s| serde_json::from_str(s).ok())
			.collect();

		let stat_keys = stats
			.iter()
			.map(
				|Statistic {
				     link,
				     stat_type,
				     time,
				     data,
				 }| format!("links:stat:{link}:{stat_type}:{time}:{data}"),
			)
			.collect::<Vec<String>>();

		let values: Vec<Option<u64>> = if stat_keys.is_empty() {
			Vec::new()
		} else {
			let values = self.pool.mget(stat_keys.clone()).await?;
			self.pool.del(stat_keys).await?;
			for key in keys {
				self.pool.srem(key, stats_json.clone()).await?;
			}
			values
		};

		let res = stats
			.into_iter()
			.zip(values.into_iter())
			.filter_map(|(s, v)| Some((s, StatisticValue::new(v?)?)))
			.collect();

		Ok(res)
	}
}

/// Note:
/// These tests require a running Redis 7.0 server. Because of this, they only
/// run if the `test-redis` feature is enabled. To run all tests including
/// these, use `cargo test --features test-redis`. You can run a Redis server
/// with Docker using `docker run -p 6379:6379 --rm redis:7.0-alpine` (replace
/// `7.0` with another version if necessary). It is highly recommended **not**
/// to run these tests on a production Redis server.
#[cfg(all(test, feature = "test-redis"))]
mod tests {
	use std::collections::HashMap;

	use super::Store;
	use crate::store::{tests, StoreBackend as _};

	async fn get_store() -> Store {
		Store::new(&HashMap::from_iter([(
			"connect".to_string(),
			"localhost:6379".to_string(),
		)]))
		.await
		.unwrap()
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

	#[tokio::test]
	async fn get_statistics() {
		tests::get_statistics(&get_store().await).await;
	}

	#[tokio::test]
	async fn incr_statistic() {
		tests::incr_statistic(&get_store().await).await;
	}

	#[tokio::test]
	async fn rem_statistics() {
		tests::rem_statistics(&get_store().await).await;
	}
}
