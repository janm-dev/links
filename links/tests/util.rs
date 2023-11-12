//! Utilities for end-to-end tests of the links redirector server and CLI

use std::{
	env,
	ffi::OsStr,
	io::Write,
	process::{Command, Stdio},
	thread,
	time::Duration,
};

use links::api::LinksClient;
use tonic::{
	codegen::CompressionEncoding,
	transport::{Channel, ClientTlsConfig},
};

/// Run a function automatically on drop. The provided function can only be
/// called once (either with `call()` or automatically on drop).
#[must_use]
pub struct Terminator<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Terminator<F> {
	pub fn new(f: F) -> Self {
		Self(Some(f))
	}

	pub fn call(&mut self) {
		if let Some(f) = self.0.take() {
			f()
		}
	}
}

impl<F: FnOnce()> Drop for Terminator<F> {
	fn drop(&mut self) {
		self.call();
	}
}

/// Convert an absolute file path to a format that the server can understand.
/// This does nothing with the normal links server, but is required e.g. for the
/// Docker container on Windows.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn convert_path(path: impl AsRef<str>) -> String {
	let mode = env::var("LINKS_TEST_EXTERNAL").ok();
	let server_mode = mode.as_deref();

	if server_mode == Some("docker") && cfg!(target_os = "windows") {
		"/".to_string() + path.as_ref().replace(':', "").replace('\\', "/").as_str()
	} else {
		path.as_ref().to_string()
	}
}

/// Start the links redirector server in the background with predetermined
/// arguments. To kill the server process call or drop the returned function.
/// The server will listen on all addresses with default ports (80, 443, 50051,
/// and 530), with the default redirect enabled, the RPC token set to `abc123`,
/// and TLS controlled by the `tls` argument of this function. Panics on any
/// error.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn start_server(tls: bool) -> Terminator<impl FnOnce()> {
	let mut args = vec![
		"--example-redirect",
		"--token",
		"abc123",
		"--watcher-timeout",
		"100",
		"--watcher-debounce",
		"100",
	];

	if tls {
		args.extend([
			"--default-certificate",
			r#"{"source": "files", "cert": "tests/cert.pem", "key": "tests/key.pem"}"#,
		])
	}

	start_server_with_args(args)
}

/// Start the links redirector server in the background with the specified
/// command-line arguments. To kill the server process call or drop the returned
/// function. Panics on any error. The value of the `LINKS_TEST_EXTERNAL`
/// environment variable controls the way that the server is started:
/// - `docker` will use Docker to start (but not build) a container named
///   `links:test` and use that container for testing
/// - any other value (or an unset variable) will start the server that was
///   built as part of `cargo test`
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn start_server_with_args(args: Vec<impl AsRef<OsStr>>) -> Terminator<impl FnOnce()> {
	let var = env::var("LINKS_TEST_EXTERNAL").ok();
	let kill_server: Box<dyn FnOnce()> = match var.as_deref() {
		Some("docker") => {
			let path = convert_path(env!("CARGO_MANIFEST_DIR"))
				+ ":" + &convert_path(env!("CARGO_MANIFEST_DIR"));

			let temp_path = convert_path(env!("CARGO_TARGET_TMPDIR"))
				+ ":" + &convert_path(env!("CARGO_TARGET_TMPDIR"));

			let mut cmd = Command::new("docker");
			cmd.args([
				"run",
				"-d",
				"--pull",
				"never",
				"-p",
				"80:80",
				"-p",
				"443:443",
				"-p",
				"530:530",
				"-p",
				"50051:50051",
				// Used in `server-misc::listener_args`
				"-p",
				"8080:8080",
				// Used in `server-misc::listener_args`
				"-p",
				"8443:8443",
				// Used in `server-files::listeners_reload`
				"-p",
				"81:81",
				// By default grpc only listens on localhost (but setting this
				// via command line arguments messes with tests)
				"-e",
				"LINKS_LISTENERS=[\"http::80\",\"https::443\",\"grpc::50051\",\"grpcs::530\"]",
				"-v",
				path.as_str(),
				"-v",
				temp_path.as_str(),
				"-w",
				convert_path(env!("CARGO_MANIFEST_DIR")).as_str(),
				"links:test",
				"--log-level",
				"info",
			]);
			cmd.args(args);

			let server_id = String::from_utf8(
				dbg!(cmd.output())
					.expect("Couldn't start server using Docker")
					.stdout,
			)
			.expect("Docker printed invalid output");
			let server_id = dbg!(server_id);

			thread::sleep(Duration::from_millis(250));

			Box::new(move || {
				// This uses SIGKILL to kill the server, and therefore doesn't allow coverage
				// collection, regardless of `cfg!(coverage)`
				let mut cmd = Command::new("docker");
				cmd.args(["rm", "-f", "-v", server_id.trim()]);
				dbg!(cmd.output()).expect("could not stop on server process");
				thread::sleep(Duration::from_millis(250));
			})
		}
		_ => {
			let mut cmd = Command::new(env!("CARGO_BIN_EXE_server"));
			cmd.args(args);
			cmd.stdin(Stdio::piped());

			let mut server = cmd.spawn().unwrap();
			thread::sleep(Duration::from_millis(250));

			Box::new(move || {
				// When collecting test coverage, the server listens for "x" on stdin
				// as a stop signal, which (unlike just killing the server process)
				// allows for the coverage data to be collected and saved
				if cfg!(coverage) {
					server
						.stdin
						.take()
						.unwrap()
						.write_all(b"x")
						.expect("could not stop server process");
				} else {
					server.kill().expect("could not kill server process");
				}

				server.wait().expect("could not wait on server process");
			})
		}
	};

	Terminator::new(kill_server)
}

/// Run the links CLI with the provided arguments, returning the output (from
/// stdout). No configuration from environment variables will be used. Panics on
/// any non-cli error.
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub fn run_cli(args: Vec<impl AsRef<OsStr>>) -> String {
	let mut cmd = Command::new(env!("CARGO_BIN_EXE_cli"));
	cmd.args(args);

	let out = cmd.output().unwrap();
	String::from_utf8(out.stdout).unwrap()
}

/// Get a links RPC client for the given host and port, with or without TLS
/// enabled
#[allow(dead_code)] // False positive, this function is used in tests, just not *all* of them
pub async fn get_rpc_client(
	host: impl AsRef<str>,
	port: u16,
	enable_tls: bool,
) -> LinksClient<Channel> {
	if enable_tls {
		let tls_config = ClientTlsConfig::new();

		let channel = Channel::from_shared(format!("grpc://{}:{}", host.as_ref(), port))
			.unwrap()
			.tls_config(tls_config)
			.unwrap()
			.connect()
			.await
			.unwrap();

		LinksClient::new(channel)
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip)
	} else {
		LinksClient::connect(format!("grpc://{}:{}", host.as_ref(), port))
			.await
			.unwrap()
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip)
	}
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
