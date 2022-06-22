//! Utilities for end-to-end tests of the links redirector server and CLI

use pico_args::Arguments;
use tokio::task::JoinHandle;
use tracing::Level;

/// Start the links redirector server in the background and return a
/// `JoinHandle` to it. To stop the server, run `abort()` and
/// `await.unwrap_err()` on the returned handle. The server will listen on all
/// addresses with default ports (80, 443, and 530), with the default redirect
/// enabled, the RPC token set to `abc123`, and TLS controlled by the `tls`
/// argument of this function.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn start_server(tls: bool) -> JoinHandle<()> {
	let mut args = vec![
		"--example-redirect".into(),
		"--api-secret".into(),
		"abc123".into(),
	];

	if tls {
		args.extend([
			"-t".into(),
			"-c".into(),
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem").into(),
			"-k".into(),
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem").into(),
		])
	}

	tokio::spawn(async {
		links::server::run(Arguments::from_vec(args), Level::INFO)
			.await
			.unwrap();
	})
}

#[macro_export]
macro_rules! assert_re {
	($re:literal, $m:ident) => {
		let re: &'static str = $re;
		let m: &str = $m.as_ref();
		let re = regex::RegexBuilder::new(re)
			.case_insensitive(false)
			.dot_matches_new_line(false)
			.ignore_whitespace(false)
			.multi_line(false)
			.octal(false)
			.unicode(true)
			.build()
			.unwrap();

		assert!(re.is_match(m));
	};
}
