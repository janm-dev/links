//! Miscellaneous statics, utilities, and macros used throughout links.

use lazy_static::lazy_static;

lazy_static! {
	pub static ref VERSION: String = if cfg!(debug_assertions) {
		env!("CARGO_PKG_VERSION").to_string() + "+debug"
	} else {
		env!("CARGO_PKG_VERSION_MAJOR").to_string() + "." + env!("CARGO_PKG_VERSION_MINOR")
	};
	pub static ref SERVER_NAME: String = format!("hyperlinks/{}", &*VERSION);
}
