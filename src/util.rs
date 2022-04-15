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

	/// The name of the http(s) server implemented by this crate. Used in e.g.
	/// the `Server` http header. Currently this is `hyperlinks/[version]`,
	/// where `hyper` refers to the http library used, `links` is this crate's
	/// name, and the version is `util::VERSION`.
	pub static ref SERVER_NAME: String = format!("hyperlinks/{}", &*VERSION);
}

/// One year in seconds
pub const A_YEAR: u32 = 365 * 24 * 60 * 60;
