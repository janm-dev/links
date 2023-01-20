//! End to end links CLI tests with TLS. Also tests the RPC API of the links
//! redirector server.

mod util;

use links_id::Id;

/// Test `cli id` with TLS
#[tokio::test]
#[serial_test::serial]
async fn id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "id"];

	let res = util::run_cli(args);

	assert!(Id::is_valid(res.trim()));
}

/// Test `cli new <URL>` with TLS
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

/// Test `cli new <URL> <VANITY>` with TLS
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

/// Test `cli get <ID>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn get_id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "get", "9dDbKpJP"];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" ---> "https://example.com/"$"#, res);
}

/// Test `cli set <ID> <URL>` with TLS
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

/// Test `cli add <VANITY> <ID>` with TLS
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

/// Test `cli rem <ID>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_id() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "rem", "9dDbKpJP"];

	let res = util::run_cli(args);

	assert_re!(r#"^"9dDbKpJP" -X-> "https://example.com/"$"#, res);
}

/// Test `cli rem <VANITY>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn rem_vanity() {
	let _terminator = util::start_server(true);

	let args = vec!["--token", "abc123", "--tls", "rem", "example"];

	let res = util::run_cli(args);

	assert_re!(r#"^"example" -X-> "9dDbKpJP"$"#, res);
}

/// Test `cli stats-get` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-get"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^\[.+"link": ?"test".+\]$"#, res);
}

/// Test `cli stats-get <VANITY>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get_vanity() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-get", "test"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^\[\[.+\]\]$"#, res);
}

/// Test `cli stats-get <VANITY> <TYPE>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_get_vanity_type() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-get", "test", "request"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^\[\]$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(
		r#"^\[\[\{"link":"test","type":"request","data":"","time":"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:00Z"\},1\]\]$"#,
		res
	);
}

/// Test `cli stats-rem` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-rem"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed [1-9][0-9]* statistics$"#, res);
}

/// Test `cli stats-rem <VANITY>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem_vanity() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-rem", "test"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed [1-9][0-9]* statistics$"#, res);
}

/// Test `cli stats-rem <VANITY> <TYPE>` with TLS
#[tokio::test]
#[serial_test::serial]
async fn stats_rem_vanity_type() {
	let _terminator = util::start_server(true);
	let args = vec!["--token", "abc123", "--tls", "stats-rem", "test", "request"];

	let res = util::run_cli(args.clone());
	assert_re!(r#"^Removed 0 statistics$"#, res);

	reqwest::get("https://localhost/test").await.unwrap();

	let res = util::run_cli(args);
	assert_re!(r#"^Removed 1 statistics$"#, res);
}
