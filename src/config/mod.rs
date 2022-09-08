//! Links server configuration handling
//!
//! The links redirector server currently accepts the following configuration
//! options:
//!
//! - `log_level` - Tracing log level. Possible values: `trace`, `debug`,
//!   `info`, `warn`, `error`. **Default `info`**.
//! - `token` - RPC API authentication token, should be long and random.
//!   **Default [randomly generated string]**.
//! - `tls_enable` - Whether to enable TLS for HTTPS and RPC. **Default
//!   `false`**.
//! - `tls_key` - TLS private key file path. Required if TLS is enabled. **No
//!   default**.
//! - `tls_cert` - TLS certificate file path. Required if TLS is enabled. **No
//!   default**.
//! - `hsts` - HTTP strict transport security setting. Possible values:
//!   `disable`, `enable`, `includeSubDomains`, `preload`. **Default `enable`**.
//! - `hsts_max_age` - The HSTS max-age setting (in seconds). **Default
//!   `63072000` (2 years)**.
//! - `https_redirect` - Whether to redirect HTTP requests to HTTPS before the
//!   external redirect. **Default `false`**.
//! - `send_alt_svc` - Whether to send the Alt-Svc HTTP header (`Alt-Svc:
//!   h2=":443"; ma=31536000`). **Default `false`**.
//! - `send_server` - Whether to send the Server HTTP header (`Server:
//!   hyperlinks/[VERSION]`). **Default `true`**.
//! - `send_csp` - Whether to send the Content-Security-Policy HTTP header.
//!   **Default `true`**.
//! - `store` - The store backend type to use. See store documentation.
//!   **Default `memory`**.
//! - `store_config` - Store backend configuration. Depends on the store backend
//!   used. **Default empty**.

mod global;
mod partial;

pub use self::{
	global::{Config, Hsts, Redirector, Tls},
	partial::{IntoPartialError, LogLevel as PartialLogLevel, Partial, PartialHsts},
};
