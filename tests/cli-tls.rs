//! End to end links CLI tests with TLS. Also tests the RPC API of the links
//! redirector server.

mod util;

use std::ffi::OsString;

use links::id::Id;

use self::util::start_server;

/// Test `cli id` without TLS
#[tokio::test]
#[serial_test::serial]
async fn id() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"id".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert!(Id::is_valid(res.trim()));

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli new <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn new_url() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"new".into(),
		"https://example.org".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(
		r#"^"\d[6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz]{7}" ---> "https://example.org/"$"#,
		res
	);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli new <URL> <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn new_url_vanity() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"new".into(),
		"https://example.net".into(),
		"example-dot-net".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(
		r#"^"example-dot-net" ---> "\d[6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz]{7}" ---> "https://example.net/"$"#,
		res
	);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli get <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn get_id() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"get".into(),
		"9dDbKpJP".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(r#"^"9dDbKpJP" ---> "https://example.com/"$"#, res);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli set <ID> <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn set() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"set".into(),
		"06666666".into(),
		"https://example.com/other".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(r#"^"06666666" ---> "https://example.com/other"$"#, res);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli add <VANITY> <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn add() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"add".into(),
		"other-example".into(),
		"06666666".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(r#"^"other-example" ---> "06666666"$"#, res);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli rem <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_id() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"rem".into(),
		"9dDbKpJP".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(r#"^"9dDbKpJP" -X-> "https://example.com/"$"#, res);

	server.abort();
	server.await.unwrap_err();
}

/// Test `cli rem <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_vanity() {
	let server = start_server(true);

	let args: Vec<OsString> = vec![
		"links-cli".into(),
		"--host".into(),
		"localhost".into(),
		"--token".into(),
		"abc123".into(),
		"--tls".into(),
		"rem".into(),
		"example".into(),
	];

	let res = links::cli::run(args).await.unwrap();

	assert_re!(r#"^"example" -X-> "9dDbKpJP"$"#, res);

	server.abort();
	server.await.unwrap_err();
}
