//! End to end links redirector HTTP server tests.

use hyper::{header::HeaderValue, StatusCode};
use pico_args::Arguments;
use reqwest::{redirect::Policy, ClientBuilder};
use tracing::Level;

/// HTTPS/1.1 redirect tests
#[tokio::test]
#[serial_test::serial]
async fn https1_redirect() {
	let server = tokio::spawn(async {
		links::server::run(
			Arguments::from_vec(vec![
				"--example-redirect".into(),
				"-t".into(),
				"-c".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem").into(),
				"-k".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem").into(),
			]),
			Level::INFO,
		)
		.await
		.unwrap();
	});

	let client = ClientBuilder::new()
		.http1_only()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let status_nonexistent = client
		.get("http://localhost/nonexistent")
		.send()
		.await
		.unwrap()
		.status();
	assert_eq!(status_nonexistent, StatusCode::NOT_FOUND);

	let redirect_res = client.get("http://localhost/example").send().await.unwrap();
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
	let server = tokio::spawn(async {
		links::server::run(
			Arguments::from_vec(vec![
				"--example-redirect".into(),
				"-t".into(),
				"-c".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem").into(),
				"-k".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem").into(),
			]),
			Level::INFO,
		)
		.await
		.unwrap();
	});

	let client = ClientBuilder::new()
		.http2_prior_knowledge()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let status_nonexistent = client
		.get("http://localhost/nonexistent")
		.send()
		.await
		.unwrap()
		.status();
	assert_eq!(status_nonexistent, StatusCode::NOT_FOUND);

	let redirect_res = client.get("http://localhost/example").send().await.unwrap();
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
