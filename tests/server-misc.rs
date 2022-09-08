//! Miscellaneous links redirector server tests

mod util;

use hyper::{header::HeaderValue, StatusCode};
use reqwest::{redirect::Policy, ClientBuilder};

/// Test random API secret generation
#[tokio::test]
#[serial_test::serial]
async fn random_secret() {
	let _terminator = util::start_server_with_args(vec!["--example-redirect"]);

	let args = vec!["--host", "localhost", "--token", "abc123", "id"];

	let res = util::run_cli(args);

	assert_re!(r#"auth token is invalid"#, res);
}

/// HTTP to HTTPS redirect
#[tokio::test]
#[serial_test::serial]
async fn http_to_https_redirect() {
	let _terminator = util::start_server_with_args(vec![
		"--example-redirect",
		"--token",
		"abc123",
		"--tls-enable",
		"true",
		"--tls-cert",
		concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem"),
		"--tls-key",
		concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem"),
		"--https-redirect",
		"true",
	]);

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
}
