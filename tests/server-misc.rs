//! Miscellaneous links redirector server tests

mod util;

use std::ffi::OsString;

use hyper::{header::HeaderValue, StatusCode};
use pico_args::Arguments;
use reqwest::{redirect::Policy, ClientBuilder};
use tracing::Level;

/// Test random API secret generation
#[tokio::test]
#[serial_test::serial]
async fn random_secret() {
	let server = tokio::spawn(async {
		links::server::run(
			Arguments::from_vec(vec!["--example-redirect".into()]),
			Level::INFO,
		)
		.await
		.unwrap();
	});

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"id".into(),
	];

	let res = links::cli::run(args).await.unwrap_err();

	assert_re!(r#"auth token is invalid"#, res);

	server.abort();
	server.await.unwrap_err();
}

/// HTTP to HTTPS redirect
#[tokio::test]
#[serial_test::serial]
async fn http_to_https_redirect() {
	let server = tokio::spawn(async {
		links::server::run(
			Arguments::from_vec(vec![
				"--example-redirect".into(),
				"--api-secret".into(),
				"abc123".into(),
				"-t".into(),
				"-c".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem").into(),
				"-k".into(),
				concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem").into(),
				"--redirect-https".into(),
			]),
			Level::INFO,
		)
		.await
		.unwrap();
	});

	let client = ClientBuilder::new()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let nonexistent_res = client
		.get("http://localhost/nonexistent")
		.send()
		.await
		.unwrap();
	let status_redirect = nonexistent_res.status();
	assert_eq!(status_redirect, StatusCode::FOUND);
	let redirect_dest = nonexistent_res.headers().get("Location");
	assert_eq!(
		redirect_dest,
		Some(&HeaderValue::from_static("https://localhost/nonexistent"))
	);
	let redirect_id = nonexistent_res.headers().get("Link-ID");
	assert_eq!(redirect_id, None);

	let redirect_res = client.get("http://localhost/example").send().await.unwrap();
	let status_redirect = redirect_res.status();
	assert_eq!(status_redirect, StatusCode::FOUND);
	let redirect_dest = redirect_res.headers().get("Location");
	assert_eq!(
		redirect_dest,
		Some(&HeaderValue::from_static("https://localhost/example"))
	);
	let redirect_id = redirect_res.headers().get("Link-ID");
	assert_eq!(redirect_id, None);

	server.abort();
	server.await.unwrap_err();
}
