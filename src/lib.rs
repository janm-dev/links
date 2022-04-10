#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod api;
pub mod id;
pub mod normalized;
pub mod redirector;
pub mod store;
pub mod util;
