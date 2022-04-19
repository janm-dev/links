//! The main part of links. This module contains code relating to actually
//! redirecting requests.

use crate::id::Id;
use crate::normalized::Normalized;
use crate::store::Store;
use crate::util::{A_YEAR, SERVER_NAME};
use hyper::{header::HeaderValue, Body, Method, Request, Response, StatusCode};
use tokio::time::Instant;
use tracing::{debug, info, instrument, trace};

/// Configuration for how the redirector should behave.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Config {
	/// Whether to enable HTTP Strict Transport Security
	pub enable_hsts: bool,
	/// Whether to preload HTTP Strict Transport Security (also sets `includeSubDomains`)
	pub preload_hsts: bool,
	/// Value of `max-age` on the Strict Transport Security header in seconds
	pub hsts_age: u32,
	/// Whether to send the `Alt-Svc` header advertising `h2` on port 443
	pub enable_alt_svc: bool,
	/// Whether to send the `Server` header
	pub enable_server: bool,
	/// Whether to send the `Content-Security-Policy` header
	pub enable_csp: bool,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			enable_hsts: true,
			preload_hsts: false,
			hsts_age: 2 * A_YEAR,
			enable_alt_svc: false,
			enable_server: true,
			enable_csp: true,
		}
	}
}

/// Redirects the `req`uest to the appropriate target URL (if one is found in
/// the `store`) or returns a `404 Not Found` response. When redirecting, the
/// status code is `302 Found` when the method is GET, and `307 Temporary
/// Redirect` otherwise.
#[instrument(level = "trace", name = "request-details")]
#[instrument(level = "info", name = "request", skip_all, fields(http.version = ?req.version(), http.host = %req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]")), http.path = ?req.uri().path(), http.method = %req.method(), store = %store.backend_name()))]
pub async fn redirector(
	req: Request<Body>,
	store: &Store,
	config: Config,
) -> Result<Response<Body>, anyhow::Error> {
	let redirect_start = Instant::now();
	debug!(?req);

	let path = req.uri().path();
	let mut res = Response::new(Body::empty());

	// Set default response headers
	res.headers_mut()
		.insert("Referrer-Policy", HeaderValue::from_static("unsafe-url"));
	if config.enable_server {
		res.headers_mut()
			.insert("Server", HeaderValue::from_static(&SERVER_NAME));
	}
	if config.enable_csp {
		res.headers_mut().insert(
			"Content-Security-Policy",
			HeaderValue::from_static("default-src 'none'; sandbox allow-top-navigation"),
		);
	}
	if config.enable_hsts {
		res.headers_mut().insert(
			"Strict-Transport-Security",
			HeaderValue::from_str(&format!(
				"max-age={}{}",
				config.hsts_age,
				if config.preload_hsts {
					"; includeSubDomains; preload"
				} else {
					""
				}
			))
			.unwrap(),
		);
	}
	if config.enable_alt_svc {
		res.headers_mut().insert(
			"Alt-Svc",
			HeaderValue::from_static("h2=\":443\"; ma=31536000"),
		);
	}

	let id_or_vanity = path.trim_start_matches('/');

	let (id, vanity) = if Id::is_valid(id_or_vanity) {
		trace!("path is an ID");
		(Some(Id::try_from(id_or_vanity)?), None)
	} else {
		let vanity = Normalized::new(id_or_vanity);
		trace!("path is a vanity path, normalized to \"{}\"", &vanity);
		(store.get_vanity(vanity.clone()).await?, Some(vanity))
	};

	let link = if let Some(id) = id {
		store.get_redirect(id).await?
	} else {
		None
	};

	if let Some(link) = link.clone() {
		if req.method() == Method::GET {
			*res.status_mut() = StatusCode::FOUND;
		} else {
			*res.status_mut() = StatusCode::TEMPORARY_REDIRECT;
		}

		res.headers_mut().insert(
			"Location",
			HeaderValue::from_str(&link.into_string()).unwrap(),
		);

		res.headers_mut().insert(
			"Link-Id",
			HeaderValue::from_str(&id.unwrap().to_string()).unwrap(),
		);
	} else {
		*res.status_mut() = StatusCode::NOT_FOUND;
	}

	let redirect_time = redirect_start.elapsed();

	debug!(?res);
	info!(
		time_ns = %redirect_time.as_nanos(),
		link = %link.map_or_else(|| "[none]".to_string(), |link| link.to_string()),
		id = %id.map_or_else(|| "[none]".to_string(), |id| id.to_string()),
		vanity = %vanity.map_or_else(|| "[none]".to_string(), |vanity| vanity.to_string()),
		status_code = %res.status(),
		"redirect processed in {:.6} seconds",
		redirect_time.as_secs_f64()
	);

	Ok(res)
}
