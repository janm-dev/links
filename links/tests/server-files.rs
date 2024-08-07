//! Tests for the links server's file handling, especially file reloading

mod util;

use std::{path::PathBuf, str::FromStr, time::Duration};

use reqwest::{redirect::Policy, Certificate, Client, ClientBuilder};
use serde_json::json;
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

/// Create a new client that trust all OS-installed root certificates. This
/// function panics on any error. The client will not follow redirects.
fn get_client() -> Client {
	ClientBuilder::new()
		.redirect(Policy::none())
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
	let config_path_str = util::convert_path(config_path.to_str().unwrap());
	fs::write(&config_path, TEST_CONFIG).await.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path_str.as_str(),
		"--watcher-timeout",
		"50",
		"--watcher-debounce",
		"50",
	]);

	let res_before = get_client()
		.get("http://localhost/example")
		.send()
		.await
		.unwrap();

	fs::write(
		&config_path,
		TEST_CONFIG.replace("send_server = true", "send_server = false"),
	)
	.await
	.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after = get_client()
		.get("http://localhost/example")
		.send()
		.await
		.unwrap();

	assert!(dbg!(res_before.headers()).get("Server").is_some());
	assert!(dbg!(res_after.headers()).get("Server").is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn tls_reconfigure_default() {
	let config_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reconfigure_default")
		.with_extension("toml");
	let config_path_str = util::convert_path(config_path.to_str().unwrap());
	fs::write(
		&config_path,
		concat!(
			include_str!("test-config.toml"),
			"\ndefault_certificate = { source = \"files\", cert = \"tests/cert.pem\", key = \
			 \"tests/key.pem\" }\n"
		),
	)
	.await
	.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path_str.as_str(),
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

	fs::write(
		&config_path,
		concat!(
			include_str!("test-config.toml"),
			"\ndefault_certificate = { source = \"files\", cert = \"tests/other-cert.pem\", key = \
			 \"tests/other-key.pem\" }\n"
		),
	)
	.await
	.unwrap();

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

#[tokio::test]
#[serial_test::serial]
async fn tls_reconfigure_domains() {
	let config_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reconfigure_default")
		.with_extension("toml");
	let config_path_str = util::convert_path(config_path.to_str().unwrap());
	fs::write(
		&config_path,
		concat!(
			include_str!("test-config.toml"),
			"\ncertificates = [{ source = \"files\", domains = [\"localhost\"], cert = \
			 \"tests/cert.pem\", key = \"tests/key.pem\" }]\n"
		),
	)
	.await
	.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path_str.as_str(),
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

	fs::write(
		&config_path,
		concat!(
			include_str!("test-config.toml"),
			"\ncertificates = [{ source = \"files\", domains = [\"localhost\"], cert = \
			 \"tests/other-cert.pem\", key = \"tests/other-key.pem\" }]\n"
		),
	)
	.await
	.unwrap();

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

#[tokio::test]
#[serial_test::serial]
async fn tls_remove_default() {
	let config_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_remove_default")
		.with_extension("toml");
	let config_path_str = util::convert_path(config_path.to_str().unwrap());
	fs::write(
		&config_path,
		concat!(
			include_str!("test-config.toml"),
			"\ndefault_certificate = { source = \"files\", cert = \"tests/cert.pem\", key = \
			 \"tests/key.pem\" }\n"
		),
	)
	.await
	.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path_str.as_str(),
		"--watcher-timeout",
		"50",
		"--watcher-debounce",
		"50",
	]);

	let res_before = get_client_with_cert(TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;

	fs::write(&config_path, TEST_CONFIG).await.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after = get_client_with_cert(TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;

	assert!(dbg!(res_before).is_ok());
	assert!(dbg!(res_after).is_err());
}

#[tokio::test]
#[serial_test::serial]
async fn tls_reload_default() {
	let cert_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload_default-cert")
		.with_extension("pem");
	let cert_path_str = util::convert_path(cert_path.to_str().unwrap());
	fs::write(&cert_path, TEST_CERT).await.unwrap();

	let key_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload_default-key")
		.with_extension("pem");
	let key_path_str = util::convert_path(key_path.to_str().unwrap());
	fs::write(&key_path, TEST_KEY).await.unwrap();

	let certificate = json! {
		{
			"source": "files",
			"cert": cert_path_str,
			"key": key_path_str
		}
	}
	.to_string();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		"tests/test-config.toml",
		"--default-certificate",
		certificate.as_str(),
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

#[tokio::test]
#[serial_test::serial]
async fn tls_key_mismatch_detection() {
	let cert_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_key_mismatch_detection-cert")
		.with_extension("pem");
	let cert_path_str = util::convert_path(cert_path.to_str().unwrap());
	fs::write(&cert_path, TEST_CERT).await.unwrap();

	let key_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_key_mismatch_detection-key")
		.with_extension("pem");
	let key_path_str = util::convert_path(key_path.to_str().unwrap());
	fs::write(&key_path, TEST_KEY).await.unwrap();

	let certificate = json! {
		{
			"source": "files",
			"cert": cert_path_str,
			"key": key_path_str
		}
	}
	.to_string();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		"tests/test-config.toml",
		"--default-certificate",
		certificate.as_str(),
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

	time::sleep(Duration::from_millis(500)).await;

	let res_middle = get_client_with_cert(TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;
	let other_res_middle = get_client_with_cert(OTHER_TEST_CERT)
		.get("https://localhost/example")
		.send()
		.await;

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

	// Certificate shouldn't get reloaded because it doesn't match the key
	assert!(dbg!(res_middle).is_ok());
	assert!(dbg!(other_res_middle).is_err());

	// Now both got reloaded
	assert!(dbg!(res_after).is_err());
	assert!(dbg!(other_res_after).is_ok());
}

#[tokio::test]
#[serial_test::serial]
async fn tls_reload_domains() {
	let cert_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload_domains-cert")
		.with_extension("pem");
	let cert_path_str = util::convert_path(cert_path.to_str().unwrap());
	fs::write(&cert_path, TEST_CERT).await.unwrap();

	let key_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-tls_reload_domains-key")
		.with_extension("pem");
	let key_path_str = util::convert_path(key_path.to_str().unwrap());
	fs::write(&key_path, TEST_KEY).await.unwrap();

	let certificates = json! {
		[{
			"source": "files",
			"domains": ["localhost"],
			"cert": cert_path_str,
			"key": key_path_str
		}]
	}
	.to_string();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		"tests/test-config.toml",
		"--certificates",
		certificates.as_str(),
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

#[tokio::test]
#[serial_test::serial]
async fn listeners_reload() {
	let config_path = PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
		.unwrap()
		.join("links_test_file_reload-listeners_reload")
		.with_extension("toml");
	let config_path_str = util::convert_path(config_path.to_str().unwrap());
	fs::write(&config_path, TEST_CONFIG).await.unwrap();

	let _terminator = util::start_server_with_args(vec![
		"-c",
		config_path_str.as_str(),
		"--watcher-timeout",
		"50",
		"--watcher-debounce",
		"50",
	]);

	let res_before_a = get_client()
		.get("http://localhost:80/example")
		.send()
		.await
		.unwrap();

	let res_before_b = get_client().get("http://localhost:81/example").send().await;

	fs::write(&config_path, TEST_CONFIG.replace("http::80", "http::81"))
		.await
		.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after_a = get_client().get("http://localhost:80/example").send().await;
	let res_after_b = get_client()
		.get("http://localhost:81/example")
		.send()
		.await
		.unwrap();

	fs::write(
		&config_path,
		TEST_CONFIG.replace("http::80", "http:198.51.100.5:81"),
	)
	.await
	.unwrap();

	time::sleep(Duration::from_millis(500)).await;

	let res_after_c = get_client().get("http://localhost:81/example").send().await;

	assert!(dbg!(res_before_a.headers()).get("Server").is_some());
	assert!(dbg!(res_before_b).is_err());
	assert!(dbg!(res_after_a).is_err());
	assert!(dbg!(res_after_b.headers()).get("Server").is_some());
	assert!(dbg!(res_after_c).is_err());
}
