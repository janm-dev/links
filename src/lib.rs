#![doc = include_str!("../README.md")]
#![doc(
	html_logo_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg"
)]
#![deny(unsafe_code)]
#![warn(
	clippy::pedantic,
	clippy::cargo,
	clippy::nursery,
	missing_docs,
	rustdoc::all
)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::tabs_in_doc_comments)]
#![allow(clippy::use_self)] // False-positives in #[derive(Serialize)] generated code

pub mod api;
pub mod certs;
pub mod config;
pub mod id;
pub mod normalized;
pub mod redirector;
pub mod server;
pub mod stats;
pub mod store;
pub mod util;
