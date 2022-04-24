//! Miscellaneous statics, utilities, and macros used throughout links.

use lazy_static::lazy_static;

lazy_static! {
	/// A string representation of this crate's version. In debug builds, this
	/// is in the form of `[full semver crate version]+debug`. In release
	/// builds this gets shortened to `MAJOR.MINOR`.
	pub static ref VERSION: String = if cfg!(debug_assertions) {
		env!("CARGO_PKG_VERSION").to_string() + "+debug"
	} else {
		env!("CARGO_PKG_VERSION_MAJOR").to_string() + "." + env!("CARGO_PKG_VERSION_MINOR")
	};

	/// The name of the HTTP(S) server implemented by this crate. Used in e.g.
	/// the `Server` HTTP header. Currently this is `hyperlinks/[version]`,
	/// where `hyper` refers to the HTTP library used, `links` is this crate's
	/// name, and the version is `util::VERSION`.
	pub static ref SERVER_NAME: String = format!("hyperlinks/{}", &*VERSION);
}

/// One year in seconds
pub const A_YEAR: u32 = 365 * 24 * 60 * 60;

/// Help string for server CLI
pub const SERVER_HELP: &str = r#"links server

USAGE:
    server [FLAGS] [OPTIONS] [STORE CONFIG]

FLAGS (all default off):
 -h --help                   Print this and exit
    --disable-hsts           Disable the Strict-Transport-Security header
    --preload-hsts           Enable HSTS preloading and include subdomains (WARNING: Be very careful about enabling this, see https://hstspreload.org/. Requires hsts-age of at least 1 year.)
    --enable-alt-svc         Enable the Alt-Svc header advertising HTTP/2 support on port 443
    --disable-server         Disable the Server HTTP header
    --disable-csp            Disable the Content-Security-Policy header

OPTIONS:
 -s --store STORE            Store type to use ("memory" *)
 -l --log LEVEL              Log level ("trace" / "debug" / "info" * / "warning")
 -a --api-secret SECRET      Authentication secret for use by the gRPC API (long random ascii string, will generate one if not present)
    --hsts-age SECONDS       HSTS header max-age (default 2 years)

STORE CONFIG:
    --store-[CONFIG] VALUE   Store-specific configuration, see the store docs.

* Default value for this option
"#;
