//! The main part of links. This module contains code relating to actually
//! redirecting requests.

use hyper::{
	header::HeaderValue, http::uri::PathAndQuery, Body, Method, Request, Response, StatusCode, Uri,
};
use tokio::time::Instant;
use tracing::{debug, field::Empty, instrument, trace};

use crate::{
	config::{Hsts, Redirector as Config},
	id::Id,
	normalized::Normalized,
	store::Store,
	util::{csp_hashes, include_html, SERVER_NAME},
};

/// Redirects the `req`uest to the appropriate target URL (if one is found in
/// the `store`) or returns a `404 Not Found` response. When redirecting, the
/// status code is `302 Found` when the method is GET, and `307 Temporary
/// Redirect` otherwise.
#[instrument(level = "debug", name = "redirect-external", skip_all, fields(http.version = ?req.version(), http.host = %req.uri().host().unwrap_or_else(|| req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]"))), http.path = ?req.uri().path(), http.method = %req.method(), store = %store.backend_name(), time_ns = Empty, link = Empty, id = Empty, vanity = Empty, status_code = Empty))]
pub async fn redirector(
	req: Request<Body>,
	store: Store,
	config: Config,
) -> Result<Response<String>, anyhow::Error> {
	let redirect_start = Instant::now();
	trace!(?req);

	let path = req.uri().path();
	let mut res = Response::builder();

	// Set default response headers
	res = res.header("Referrer-Policy", "unsafe-url");
	if config.send_server {
		res = res.header("Server", SERVER_NAME);
	}

	if config.send_csp {
		res = res.header(
			"Content-Security-Policy",
			concat!(
				"default-src 'none'; style-src ",
				csp_hashes!("style"),
				"; sandbox allow-top-navigation"
			),
		);
	}

	if config.send_alt_svc {
		res = res.header("Alt-Svc", "h2=\":443\"; ma=31536000");
	}

	res = match config.hsts {
		Hsts::Disable => res,
		Hsts::Enable(max_age) => {
			res.header("Strict-Transport-Security", &format!("max-age={max_age}"))
		}
		Hsts::IncludeSubDomains(max_age) => res.header(
			"Strict-Transport-Security",
			&format!("max-age={max_age}; includeSubDomains"),
		),
		Hsts::Preload(max_age) => res.header(
			"Strict-Transport-Security",
			&format!("max-age={max_age}; includeSubDomains; preload"),
		),
	};

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

	trace!(?res);
	let span = tracing::Span::current();
	span.record("time_ns", redirect_time.as_nanos());
	span.record(
		"link",
		link.map_or_else(|| "[none]".to_string(), |link| link.to_string()),
	);
	span.record(
		"id",
		id.map_or_else(|| "[none]".to_string(), |id| id.to_string()),
	);
	span.record(
		"vanity",
		vanity.map_or_else(|| "[none]".to_string(), |vanity| vanity.to_string()),
	);
	span.record("status_code", res.status().as_u16());

	debug!(
		"External redirect processed in {:.6} seconds",
		redirect_time.as_secs_f64()
	);

	Ok(res)
}

/// Redirects an incoming request to the same host and path, but with the
/// `https` scheme.
#[instrument(level = "debug", name = "redirect-https", skip_all, fields(http.version = ?req.version(), http.host = %req.uri().host().unwrap_or_else(|| req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]"))), http.path = ?req.uri().path(), http.method = %req.method(), time_ns = Empty, link = Empty, status_code = Empty))]
pub async fn https_redirector(
	req: Request<Body>,
	config: Config,
) -> Result<Response<String>, anyhow::Error> {
	let redirect_start = Instant::now();
	trace!(?req);

	// Set default response headers
	let mut res = Response::builder();
	res = res.header("Referrer-Policy", "no-referrer");
	if config.send_server {
		res = res.header("Server", SERVER_NAME);
	}
	if config.send_csp {
		res = res.header(
			"Content-Security-Policy",
			concat!(
				"default-src 'none'; style-src ",
				csp_hashes!("style"),
				"; sandbox allow-top-navigation"
			),
		);
	}
	if config.send_alt_svc {
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

	trace!(?res);
	let span = tracing::Span::current();
	span.record("time_ns", redirect_time.as_nanos());
	span.record("link", link.unwrap_or_else(|| "[none]".to_string()));
	span.record("status_code", res.status().as_u16());

	debug!(
		"HTTP-to-HTTPS redirect processed in {:.6} seconds",
		redirect_time.as_secs_f64()
	);

	Ok(res)
}
