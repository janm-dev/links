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
//! - `links:stat:*` reserved for statistics

use std::fmt::{Debug, Formatter, Result as FmtResult};

use anyhow::Result;
use async_trait::async_trait;
use fred::{
	pool::RedisPool,
	prelude::*,
	types::{RespVersion, TlsConfig},
};
use pico_args::Arguments;
use tracing::instrument;

use crate::{
	id::Id,
	normalized::{Link, Normalized},
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
/// **Command-line flags:**
/// - `--store-cluster`: Use Redis cluster mode. If this is present, cluster
///   information will be requested from Redis nodes (which will fail if the
///   server isn't in cluster mode). If this flag is not present, only one
///   single Redis server will be used.
/// - `--store-tls`: Enable TLS (using system root CAs) when communicating with
///   the Redis server.
///
/// **Command-line options:**
/// - `--store-connect`: Connection information in the format of `host:port` to
///   connect to. When using Redis in cluster mode, you can pass this option
///   multiple times for different nodes, but only one is required (the others
///   will be automatically discovered). Note that this is not a full URL.
/// - `--store-username`: The username to use for the connection, when using
///   ACLs on the server. Don't specify this when using password-based auth.
/// - `--store-password`: The password to use for the Redis connection. This can
///   either be the user's password (when using ACLs) or the global server
///   password when using password-based authentication.
/// - `--store-pool-size`: The number of connections to use in the connection
///   pool. **Default `8`**.
/// - `--store-database`: The database number to use for the Redis connection.
///   **Default `0`**.
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
	fn backend_name() -> &'static str
	where
		Self: Sized,
	{
		"redis"
	}

	fn get_backend_name(&self) -> &'static str {
		"redis"
	}

	#[instrument(level = "trace", ret, err)]
	async fn new(config: &mut Arguments) -> Result<Self> {
		let server_config = if config.contains("--store-cluster") {
			ServerConfig::Clustered {
				hosts: config.values_from_fn::<_, _, anyhow::Error>("--store-connect", |s| {
					s.split_once(':')
						.map(|v| Ok((v.0.to_string(), v.1.parse::<u16>()?)))
						.ok_or_else(|| anyhow::anyhow!("couldn't parse --store-connect value"))?
				})?,
			}
		} else {
			let (host, port) =
				config.value_from_fn::<_, _, anyhow::Error>("--store-connect", |s| {
					s.split_once(':')
						.map(|v| Ok((v.0.to_string(), v.1.parse::<u16>()?)))
						.ok_or_else(|| anyhow::anyhow!("couldn't parse --store-connect value"))?
				})?;

			ServerConfig::Centralized { host, port }
		};

		let pool_config = RedisConfig {
			username: config.opt_value_from_fn::<_, _, anyhow::Error>("--store-username", |s| {
				Ok(s.to_string())
			})?,
			password: config.opt_value_from_fn::<_, _, anyhow::Error>("--store-password", |s| {
				Ok(s.to_string())
			})?,
			server: server_config,
			version: RespVersion::RESP3,
			database: config.opt_value_from_fn("--store-database", |s| str::parse::<u8>(s))?,
			tracing: true,
			tls: if config.contains("--store-tls") {
				Some(TlsConfig::default())
			} else {
				None
			},
			..RedisConfig::default()
		};

		let pool = RedisPool::new(
			pool_config,
			config
				.opt_value_from_fn("--store-pool-size", str::parse)?
				.unwrap_or(8),
		)?;

		pool.connect(Some(ReconnectPolicy::new_constant(0, 100)));
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
}

/// Note:
/// These tests require a running Redis 7.0 server. Because of this, they only
/// run if the `redis-tests` feature is enabled. To run all tests including
/// these, use `cargo test --features redis-tests` You can run a Redis server
/// with Docker using `docker run -p 6379:6379 --rm redis:7.0-alpine` (replace
/// `7.0` with another version if necessary). It is highly recommended **not**
/// to run these tests on a production Redis server.
#[cfg(all(test, feature = "redis-tests"))]
mod tests {
	use std::{ffi::OsString, str::FromStr};

	use pico_args::Arguments;

	use super::Store;
	use crate::store::{tests, StoreBackend as _};

	async fn get_store() -> Store {
		Store::new(&mut Arguments::from_vec(vec![
			OsString::from_str("--store-connect").unwrap(),
			OsString::from_str("localhost:6379").unwrap(),
		]))
		.await
		.unwrap()
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
