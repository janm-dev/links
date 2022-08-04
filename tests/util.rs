//! Utilities for end-to-end tests of the links redirector server and CLI

use std::{process::Command, thread, time::Duration};

/// Run a function automatically on drop.
#[must_use]
pub struct Terminator<F: FnMut()>(F);

impl<F: FnMut()> Terminator<F> {
	pub fn new(f: F) -> Self {
		Self(f)
	}

	pub fn call(&mut self) {
		self.0();
	}
}

impl<F: FnMut()> Drop for Terminator<F> {
	fn drop(&mut self) {
		self.call();
	}
}

/// Start the links redirector server in the background with predetermined
/// arguments. To kill the server process call or drop the returned function.
/// The server will listen on all addresses with default ports (80, 443, and
/// 530), with the default redirect enabled, the RPC token set to `abc123`, and
/// TLS controlled by the `tls` argument of this function. Panics on any error.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn start_server(tls: bool) -> Terminator<impl FnMut()> {
	let mut args = vec!["--example-redirect", "--token", "abc123"];

	if tls {
		args.extend([
			"--tls",
			"on",
			"--tls-cert",
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cert.pem"),
			"--tls-key",
			concat!(env!("CARGO_MANIFEST_DIR"), "/tests/key.pem"),
		])
	}

	start_server_with_args(args)
}

/// Start the links redirector server in the background with the specified
/// command-line arguments. To kill the server process call or drop the returned
/// function. The server will listen on all addresses with default ports (80,
/// 443, and 530). Panics on any error.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn start_server_with_args(args: Vec<&'static str>) -> Terminator<impl FnMut()> {
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_server"));
	cmd.args(args);

	let mut server = cmd.spawn().unwrap();

	thread::sleep(Duration::from_millis(250));

	Terminator::new(move || {
		server.kill().expect("could not kill server process");
		server.wait().expect("could not wait on server process");
	})
}

/// Run the links CLI with the provided arguments, returning the output (from
/// stdout). No configuration from environment variables will be used. Panics on
/// any non-cli error.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn run_cli(args: Vec<&'static str>) -> String {
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_cli"));
	cmd.args(args);

	let out = cmd.output().unwrap();
	String::from_utf8(out.stdout).unwrap()
}

#[macro_export]
macro_rules! assert_re {
	($re:literal, $m:ident) => {
		let re: &'static str = $re;
		let message: &str = $m.as_ref();
		let regex = regex::RegexBuilder::new(re)
			.case_insensitive(false)
			.dot_matches_new_line(false)
			.ignore_whitespace(false)
			.multi_line(false)
			.octal(false)
			.unicode(true)
			.build()
			.unwrap();

		assert!(dbg!(regex).is_match(dbg!(message.trim())));
	};
}
