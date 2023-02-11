//! Miscellaneous statics, utilities, and macros used throughout links.

use std::collections::HashMap;

/// A string representation of this crate's version. In debug builds, this
/// is in the form of `[full semver crate version]+debug`. In release
/// builds this gets shortened to `MAJOR.MINOR`.
pub const VERSION: &str = if cfg!(debug_assertions) {
	concat!(env!("CARGO_PKG_VERSION"), "+debug")
} else {
	concat!(
		env!("CARGO_PKG_VERSION_MAJOR"),
		".",
		env!("CARGO_PKG_VERSION_MINOR")
	)
};

/// The name of the HTTP(S) server implemented by this crate. Used in e.g.
/// the `Server` HTTP header. Currently this is `hyperlinks/[version]`,
/// where `hyper` refers to the HTTP library used, `links` is this crate's
/// name, and the version is `util::VERSION`.
pub const SERVER_NAME: &str = if cfg!(debug_assertions) {
	concat!("hyperlinks/", env!("CARGO_PKG_VERSION"), "+debug")
} else {
	concat!(
		"hyperlinks/",
		env!("CARGO_PKG_VERSION_MAJOR"),
		".",
		env!("CARGO_PKG_VERSION_MINOR")
	)
};

/// Make a decent-looking and readable string out of a string -> string map
pub fn stringify_map<K, V, H>(map: &HashMap<K, V, H>) -> String
where
	K: AsRef<str>,
	V: AsRef<str>,
{
	let mut buf = String::with_capacity(map.len() * 16);
	buf += "{ ";

	for (i, (k, v)) in map.iter().enumerate() {
		buf += k.as_ref();
		buf += " = ";
		buf += v.as_ref();
		if i < map.len() - 1 {
			buf += ", ";
		}
	}

	buf + " }"
}

/// One year in seconds
pub const A_YEAR: u32 = 365 * 24 * 60 * 60;

/// Help string for server CLI
pub const SERVER_HELP: &str = r#"links server

USAGE:
    server [FLAGS] [OPTIONS] [CONFIGURATION]

EXAMPLE:
    server -c ./config.toml --log-level warn

FLAGS:
 -h --help                   Print this and exit
    --example-redirect       Set an example redirect on server start ("example" -> "9dDbKpJP" -> "https://example.com/")

OPTIONS:
 -c --config PATH            Configuration file path. Supported formats: toml (*.toml), yaml/json (*.yaml, *.yml, *.json)
    --watcher-timeout MS     File watcher timeout in milliseconds, default 10000
    --watcher-debounce MS    File watcher debounce time in milliseconds, default 1000

CONFIGURATION:
    --[OPTION] VALUE         Configuration option (in "kebab-case"), see documentation for possible options and values

The FLAGS and OPTIONS above are separate from configuration options, because they influence server behaviour on startup only, and can only be specified on the command-line.
Configuration options are parsed first from environment variables ("LINKS_[CONFIG_OPTION]"), then from the configuration file, then from command-line arguments ("--[config-option]"), later ones overwriting earlier ones.
This means that command-line options overwrite everything, config file options overwrite default values and environment variables, environment variable overwrite only defaults, and the default value is used only when an option is not specified anywhere.
"#;

pub use crate::include_html;
/// Include a generated minified html file as a `&'static str`. The file must
/// be generated by the build script and located in the `OUT_DIR` directory.
#[macro_export]
macro_rules! include_html {
	($name:literal) => {
		include_str!(concat!(env!("OUT_DIR"), concat!("/", $name, ".html")))
	};
}

pub use crate::csp_hashes;
/// Include a list of allowed style hashes for use in the CSP header as a
/// `&'static str` generated at compile time by the build script. The hashes
/// are in the CSP header format (`sha256-HASH_ONE sha256-HASH_TWO ...`), and
/// are generated per HTML file.
#[macro_export]
macro_rules! csp_hashes {
	($file_name:literal, $tag_name:literal) => {
		include_str!(concat!(
			env!("OUT_DIR"),
			concat!("/", $file_name, ".", $tag_name, ".hash")
		))
	};
}