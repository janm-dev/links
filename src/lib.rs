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
// Allowed due to false positives relating to `wasi`, a transitive dependency
// never used on any target supported by links.
#![allow(clippy::multiple_crate_versions)]

pub mod api;
pub mod cli;
pub mod config;
pub mod id;
pub mod normalized;
pub mod redirector;
pub mod server;
pub mod store;
pub mod util;
