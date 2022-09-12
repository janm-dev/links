//! Tests for the links server's file handling, especially file reloading

mod util;

use std::{path::PathBuf, str::FromStr, time::Duration};

use reqwest::{redirect::Policy, Certificate, Client, ClientBuilder};
use tokio::{fs, time};

const TEST_CONFIG: &str = include_str!("test-config.toml");

const TEST_KEY: &[u8] = include_bytes!("key.pem");
const OTHER_TEST_KEY: &[u8] = include_bytes!("other-key.pem");

const TEST_CERT: &[u8] = include_bytes!("cert.pem");
const OTHER_TEST_CERT: &[u8] = include_bytes!("other-cert.pem");

/// Create a new client with the provided PEM certificate as its only trusted
/// root cert. This function panics on any error. The client will not follow
/// redirects.
fn get_client_with_cert(cert: &[u8]) -> Client {
	ClientBuilder::new()
		.redirect(Policy::none())
		.tls_built_in_root_certs(false)
		.add_root_certificate(Certificate::from_pem(cert).unwrap())
		.build()
		.unwrap()
}

#[tokio::test]
#[serial_test::serial]
async fn config_reload() {
	let config_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-config_reload")
		.with_extension("toml");
	fs::write(&config_path, TEST_CONFIG).await.unwrap();

	let client = ClientBuilder::new()
		.redirect(Policy::none())
		.build()
		.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path.to_str().unwrap(),
		"--watcher-timeout",
		"50",
		"--watcher-debounce",
		"50",
	]);

	let res_before = client.get("http://localhost/example").send().await.unwrap();

	fs::write(
		&config_path,
		TEST_CONFIG.replace("send_server = true", "send_server = false"),
	)
	.await
	.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after = client.get("http://localhost/example").send().await.unwrap();

	assert!(dbg!(res_before.headers()).get("Server").is_some());
	assert!(dbg!(res_after.headers()).get("Server").is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn tls_reload() {
	let cert_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload-cert")
		.with_extension("pem");
	fs::write(&cert_path, TEST_CERT).await.unwrap();

	let key_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload-key")
		.with_extension("pem");
	fs::write(&key_path, TEST_KEY).await.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		"tests/test-config.toml",
		"--tls-enable",
		"true",
		"--tls-key",
		key_path.to_str().unwrap(),
		"--tls-cert",
		cert_path.to_str().unwrap(),
		"--watcher-timeout",
		"50",
		"--watcher-debounce",
		"50",
	]);

	// Can't reuse the client, because the connection would be kept alive, and
	// the certificate reloading wouldn't be noticed
	let res_before = get_client_with_cert(TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;
	let other_res_before = get_client_with_cert(OTHER_TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;

	fs::write(&cert_path, OTHER_TEST_CERT).await.unwrap();
	fs::write(&key_path, OTHER_TEST_KEY).await.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after = get_client_with_cert(TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;
	let other_res_after = get_client_with_cert(OTHER_TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;

	assert!(dbg!(res_before).is_ok());
	assert!(dbg!(other_res_before).is_err());

	assert!(dbg!(res_after).is_err());
	assert!(dbg!(other_res_after).is_ok());
}
