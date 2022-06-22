//! A simple command-line interface for configuring links redirects via the
//! gRPC API built into every redirector.
//!
//! Supports all basic links store operations using the redirectors' gRPC API.

use std::{env, ffi::OsString};

use links::cli;

#[tokio::main]
async fn main() {
	let args: Vec<OsString> = env::args_os().collect();

	let res = cli::run(args).await;

	println!("{}", res.unwrap_or_else(|e| e));
}
