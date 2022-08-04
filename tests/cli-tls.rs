//! End to end links CLI tests with TLS. Also tests the RPC API of the links
//! redirector server.

mod util;

use links::id::Id;

/// Test `cli id` without TLS
#[tokio::test]
#[serial_test::serial]
async fn id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "id"];

	let res = util::run_cli(args);

	assert!(Id::is_valid(res.trim()));
}

/// Test `cli new <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn new_url() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "new", "https://example.org"];

	let res = util::run_cli(args);

	assert_re!(
		r#"^"\d[6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz]{7}" ---> "https://example.org/"$"#,
		res
	);
}

/// Test `cli new <URL> <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn new_url_vanity() {
	let _terminator = util::start_server(true);

	let args = vec![
		"--token",
		"abc123",
		"--tls",
		"new",
		"https://example.net",
		"example-dot-net",
	];

	let res = util::run_cli(args);

	assert_re!(
		r#"^"example-dot-net" ---> "\d[6789BCDFGHJKLMNPQRTWXbcdfghjkmnpqrtwxz]{7}" ---> "https://example.net/"$"#,
		res
	);
}

/// Test `cli get <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn get_id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "get", "9dDbKpJP"];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" ---> "https://example.com/"$"#, res);
}

/// Test `cli set <ID> <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn set() {
	let _terminator = util::start_server(true);

	let args = vec![
		"--token",
		"abc123",
		"--tls",
		"set",
		"06666666",
		"https://example.com/other",
	];

	let res = util::run_cli(args);

	assert_re!(r#"^"06666666" ---> "https://example.com/other"$"#, res);
}

/// Test `cli add <VANITY> <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn add() {
	let _terminator = util::start_server(true);

	let args = vec![
		"--token",
		"abc123",
		"--tls",
		"add",
		"other-example",
		"06666666",
	];

	let res = util::run_cli(args);

	assert_re!(r#"^"other-example" ---> "06666666"$"#, res);
}

/// Test `cli rem <ID>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "rem", "9dDbKpJP"];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" -X-> "https://example.com/"$"#, res);
}

/// Test `cli rem <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_vanity() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "rem", "example"];

	let res = util::run_cli(args);

	assert_re!(r#"^"example" -X-> "9dDbKpJP"$"#, res);
}
