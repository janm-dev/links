//! The main part of links. This module contains code relating to actually
//! redirecting requests.

use hyper::{
	header::HeaderValue, http::uri::PathAndQuery, Body, Method, Request, Response, StatusCode, Uri,
};
use tokio::time::Instant;
use tracing::{debug, info, instrument, trace};

use crate::{
	id::Id,
	normalized::Normalized,
	store::Store,
	util::{csp_hashes, include_html, A_YEAR, SERVER_NAME},
};

/// Configuration for how the redirector should behave.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Config {
	/// Whether to enable HTTP Strict Transport Security
	pub enable_hsts: bool,
	/// Whether to preload HTTP Strict Transport Security (also sets
	/// `includeSubDomains`)
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
#[instrument(level = "info", name = "request", skip_all, fields(http.version = ?req.version(), http.host = %req.uri().host().unwrap_or_else(|| req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]"))), http.path = ?req.uri().path(), http.method = %req.method(), store = %store.backend_name()))]
pub async fn redirector(
	req: Request<Body>,
	store: &Store,
	config: Config,
) -> Result<Response<String>, anyhow::Error> {
	let redirect_start = Instant::now();
	debug!(?req);

	let path = req.uri().path();
	let mut res = Response::builder();

	// Set default response headers
	res = res.header("Referrer-Policy", "unsafe-url");
	if config.enable_server {
		res = res.header("Server", &*SERVER_NAME);
	}
	if config.enable_csp {
		res = res.header(
			"Content-Security-Policy",
			concat!(
				"default-src 'none'; style-src ",
				csp_hashes!("style"),
				"; sandbox allow-top-navigation"
			),
		);
	}
	if config.enable_hsts {
		res = res.header(
			"Strict-Transport-Security",
			&format!(
				"max-age={}{}",
				config.hsts_age,
				if config.preload_hsts {
					"; includeSubDomains; preload"
				} else {
					""
				}
			),
		);
	}
	if config.enable_alt_svc {
		res = res.header("Alt-Svc", "h2=\":443\"; ma=31536000");
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

	let res = if let Some(link) = link.clone() {
		let link = link.into_string();

		res = res.header("Location", &link);
		res = res.header("Link-Id", &id.unwrap().to_string());

		if req.method() == Method::GET {
			res = res.status(StatusCode::FOUND);
		} else {
			res = res.status(StatusCode::TEMPORARY_REDIRECT);
		}

		res = res.header("Content-Type", "text/html; charset=UTF-8");
		res.body(
			include_html!("redirect")
				.to_string()
				.replace("{{LINK_URL}}", &link),
		)?
	} else {
		res = res.status(StatusCode::NOT_FOUND);
		res = res.header("Content-Type", "text/html; charset=UTF-8");
		res.body(include_html!("not-found").to_string())?
	};

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

/// Redirects an incoming request to the same host and path, but with the
/// `https` scheme.
#[instrument(level = "trace", name = "request-https-details")]
#[instrument(level = "info", name = "request-https", skip_all, fields(http.version = ?req.version(), http.host = %req.uri().host().unwrap_or_else(|| req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]"))), http.path = ?req.uri().path(), http.method = %req.method()))]
pub async fn https_redirector(
	req: Request<Body>,
	config: Config,
) -> Result<Response<String>, anyhow::Error> {
	let redirect_start = Instant::now();
	debug!(?req);

	// Set default response headers
	let mut res = Response::builder();
	res = res.header("Referrer-Policy", "no-referrer");
	if config.enable_server {
		res = res.header("Server", &*SERVER_NAME);
	}
	if config.enable_csp {
		res = res.header(
			"Content-Security-Policy",
			concat!(
				"default-src 'none'; style-src ",
				csp_hashes!("style"),
				"; sandbox allow-top-navigation"
			),
		);
	}
	if config.enable_alt_svc {
		res = res.header("Alt-Svc", "h2=\":443\"; ma=31536000");
	}

	let p_and_q = req.uri().path_and_query().map_or("/", PathAndQuery::as_str);
	let (res, link) = if let Some(Ok(host)) = req.headers().get("host").map(HeaderValue::to_str) {
		let link = Uri::builder()
			.scheme("https")
			.authority(host)
			.path_and_query(p_and_q)
			.build()?
			.to_string();

		res = res.header("Location", &link);

		if req.method() == Method::GET {
			res = res.status(StatusCode::FOUND);
		} else {
			res = res.status(StatusCode::TEMPORARY_REDIRECT);
		}

		res = res.header("Content-Type", "text/html; charset=UTF-8");
		(
			res.body(include_html!("https-redirect").to_string())?,
			Some(link),
		)
	} else {
		res = res.status(StatusCode::BAD_REQUEST);
		res = res.header("Content-Type", "text/html; charset=UTF-8");
		(res.body(include_html!("bad-request").to_string())?, None)
	};

	let redirect_time = redirect_start.elapsed();

	debug!(?res);
	info!(
		time_ns = %redirect_time.as_nanos(),
		link = %link.map_or_else(|| "[none]".to_string(), |link| link),
		status_code = %res.status(),
		"redirect processed in {:.6} seconds",
		redirect_time.as_secs_f64()
	);

	Ok(res)
}
