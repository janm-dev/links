#![doc = include_str!("../README.md")]
#![doc(
	html_logo_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/janm-dev/links/main/misc/icon.svg"
)]
#![deny(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod api;
pub mod id;
pub mod normalized;
pub mod redirector;
pub mod store;
pub mod util;
