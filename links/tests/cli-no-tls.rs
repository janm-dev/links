//! End to end links CLI tests without TLS. Also tests the RPC API of the links
//! redirector server.

mod util;

use links_id::Id;

/// Test `cli id` without TLS
#[tokio::test]
#[serial_test::serial]
async fn id() {
	let _terminator = util::start_server(false);

	let args = vec!["--host", "localhost", "--token", "abc123", "id"];

	let res = util::run_cli(args);

	assert!(Id::is_valid(res.trim()));
}

/// Test `cli new <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn new_url() {
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
		"new",
		"https://example.org",
	];

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
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
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
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
		"get",
		"9dDbKpJP",
	];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" ---> "https://example.com/"$"#, res);
}

/// Test `cli set <ID> <URL>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn set() {
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
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
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
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
	let _terminator = util::start_server(false);

	let args = vec![
		"--host",
		"localhost",
		"--token",
		"abc123",
		"rem",
		"9dDbKpJP",
	];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" -X-> "https://example.com/"$"#, res);
}

/// Test `cli rem <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_vanity() {
	let _terminator = util::start_server(false);

	let args = vec!["--host", "localhost", "--token", "abc123", "rem", "example"];

	let res = util::run_cli(args);

	assert_re!(r#"^"example" -X-> "9dDbKpJP"$"#, res);
}

/// Test `cli stats-get` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-get"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^\[.+"link": ?"test".+\]$"#, res);
}

/// Test `cli stats-get <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get_vanity() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-get", "test"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^\[\[.+\]\]$"#, res);
}

/// Test `cli stats-get <VANITY> <TYPE>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get_vanity_type() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-get", "test", "request"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(
		r#"^\[\[\{"link":"test","type":"request","data":"","time":"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:00Z"\},1\]\]$"#,
		res
	);
}

/// Test `cli stats-rem` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-rem"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed [1-9][0-9]* statistics$"#, res);
}

/// Test `cli stats-rem <VANITY>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem_vanity() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-rem", "test"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed [1-9][0-9]* statistics$"#, res);
}

/// Test `cli stats-rem <VANITY> <TYPE>` without TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem_vanity_type() {
	let _terminator = util::start_server(false);
	let args = vec!["--token", "abc123", "stats-rem", "test", "request"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("http://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed 1 statistics$"#, res);
}
