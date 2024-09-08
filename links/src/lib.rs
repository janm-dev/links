#![doc = include_str!("../../README.md")]
#![doc(
	html_logo_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg"
)]
#![forbid(unsafe_code)]
#![warn(
	clippy::pedantic,
	clippy::cargo,
	clippy::nursery,
	missing_docs,
	rustdoc::missing_crate_level_docs
)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::tabs_in_doc_comments)]
#![allow(clippy::module_name_repetitions)]
#![expect(
	clippy::use_self,
	reason = "false-positives in `#[derive(Serialize)]`-generated code"
)]

pub mod api;
pub mod certs;
pub mod config;
pub mod redirector;
pub mod server;
pub mod stats;
pub mod store;
pub mod util;
