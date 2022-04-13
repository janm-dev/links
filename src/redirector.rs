//! The main part of links. This module contains code relating to actually
//! redirecting requests.

use crate::id::Id;
use crate::normalized::Normalized;
use crate::store::Store;
use crate::util::SERVER_NAME;
use hyper::{
	header::HeaderValue,
	http::uri::Scheme,
	Method, StatusCode, Uri, {Body, Request, Response},
};
use tokio::time::Instant;
use tracing::{debug, info, instrument, trace};

#[instrument(level = "trace", name = "request-details")]
#[instrument(level = "info", name = "request", skip_all, fields(http.version = ?req.version(), http.host = %req.headers().get("host").map_or_else(|| "[unknown]", |h| h.to_str().unwrap_or("[unknown]")), http.path = ?req.uri().path(), http.method = %req.method(), store = %store.backend_name()))]
pub async fn redirector(
	req: Request<Body>,
	store: &impl Store,
) -> Result<Response<Body>, anyhow::Error> {
	let redirect_start = Instant::now();
	debug!(?req);

	let path = req.uri().path();
	let mut res = Response::new(Body::empty());

	// Set default response headers
	res.headers_mut()
		.insert("Server", HeaderValue::from_str(&SERVER_NAME).unwrap());
	res.headers_mut().insert(
		"Content-Security-Policy",
		HeaderValue::from_str("default-src 'none'; sandbox allow-top-navigation").unwrap(),
	);
	res.headers_mut().insert(
		"Referrer-Policy",
		HeaderValue::from_str("unsafe-url").unwrap(),
	);
	//TODO: make this configurable to allow mixed links/other server deployments and to allow preloading (also set to 63072000)
	res.headers_mut().insert(
		"Strict-Transport-Security",
		HeaderValue::from_str("max-age=300").unwrap(),
	);
	//TODO: make this configurable (especially the port, also set to 31536000)
	res.headers_mut().insert(
		"Alt-Svc",
		HeaderValue::from_str("h2=\":443\"; ma=300").unwrap(),
	);
	//TODO: make this configurable (maybe add this info to `Link`)
	res.headers_mut().insert(
		"Cache-Control",
		HeaderValue::from_str("max-age=600").unwrap(),
	);

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
