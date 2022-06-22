//! End to end links redirector HTTP server tests.

mod util;

use hyper::{header::HeaderValue, StatusCode};
use reqwest::{redirect::Policy, ClientBuilder};

use self::util::start_server;

/// HTTPS/1.1 redirect tests
#[tokio::test]
#[serial_test::serial]
async fn https1_redirect() {
	let server = start_server(true);

	let client = ClientBuilder::new()
		.http1_only()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let status_nonexistent = client
		.get("https://localhost/nonexistent")
		.send()
		.await
		.unwrap()
		.status();
	assert_eq!(status_nonexistent, StatusCode::NOT_FOUND);

	let redirect_res = client
		.get("https://localhost/example")
		.send()
		.await
		.unwrap();
	let status_redirect = redirect_res.status();
	assert_eq!(status_redirect, StatusCode::FOUND);
	let redirect_dest = redirect_res.headers().get("Location");
	assert_eq!(
		redirect_dest,
		Some(&HeaderValue::from_static("https://example.com/"))
	);
	let redirect_id = redirect_res.headers().get("Link-ID");
	assert_eq!(redirect_id, Some(&HeaderValue::from_static("9dDbKpJP")));

	server.abort();
	server.await.unwrap_err();
}

/// HTTPS/2.0 redirect tests
#[tokio::test]
#[serial_test::serial]
async fn https2_redirect() {
	let server = start_server(true);

	let client = ClientBuilder::new()
		.http2_prior_knowledge()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let status_nonexistent = client
		.get("https://localhost/nonexistent")
		.send()
		.await
		.unwrap()
		.status();
	assert_eq!(status_nonexistent, StatusCode::NOT_FOUND);

	let redirect_res = client
		.get("https://localhost/example")
		.send()
		.await
		.unwrap();
	let status_redirect = redirect_res.status();
	assert_eq!(status_redirect, StatusCode::FOUND);
	let redirect_dest = redirect_res.headers().get("Location");
	assert_eq!(
		redirect_dest,
		Some(&HeaderValue::from_static("https://example.com/"))
	);
	let redirect_id = redirect_res.headers().get("Link-ID");
	assert_eq!(redirect_id, Some(&HeaderValue::from_static("9dDbKpJP")));

	server.abort();
	server.await.unwrap_err();
}
