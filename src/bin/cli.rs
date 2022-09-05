//! A simple command-line interface for configuring links redirects via the RPC
//! API built into every redirector server.
//!
//! Supports all basic links store operations using the redirectors' RPC API.

use std::{convert::Infallible, env, ffi::OsString, fmt::Debug, str::FromStr};

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use hyper::http::uri::InvalidUri;
use links::{
	api::{
		GetRedirectRequest, GetVanityRequest, LinksClient, RemRedirectRequest, RemVanityRequest,
		SetRedirectRequest, SetVanityRequest,
	},
	id::{ConversionError, Id},
	normalized::{Link, Normalized},
};
use tonic::{
	codec::CompressionEncoding,
	metadata::AsciiMetadataValue,
	transport::{Channel, ClientTlsConfig, Error as TonicError},
	Request, Status,
};

#[tokio::main]
async fn main() {
	let args: Vec<OsString> = env::args_os().collect();

	let res = run(args).await;

	println!("{}", res.unwrap_or_else(|e| e));
}

#[derive(Parser, Debug)]
#[clap(name = "links-cli", version, about = "A simple command-line interface for configuring links redirects via the gRPC API built into every redirector.", long_about = None)]
struct Cli {
	#[clap(subcommand)]
	command: Commands,

	/// Whether to use TLS when connecting to the gRPC API
	#[clap(short = 't', long, env = "LINKS_RPC_TLS")]
	tls: bool,

	/// Redirector server hostaname
	#[clap(short = 'H', long, env = "LINKS_RPC_HOST", default_value = "localhost")]
	host: String,

	/// Redirector gRPC port
	#[clap(short = 'P', long, env = "LINKS_RPC_PORT", default_value = "530")]
	port: u16,

	/// gRPC API authentication token
	#[clap(short = 'T', long, env = "LINKS_RPC_TOKEN")]
	token: AsciiMetadataValue,

	/// Show more verbose results
	#[clap(short, long)]
	verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Generate a random, unique links id
	Id,

	/// Get the destination of a redirect by its ID or vanity path
	Get { redirect: IdOrVanity },

	/// Create a new redirect with a random ID
	New { to: Link, from: Option<Normalized> },

	/// Create or modify a redirect with a specified ID and destination link
	Set { id: Id, link: Link },

	/// Add a vanity path to an existing redirect
	Add { vanity: Normalized, id: Id },

	/// Remove a vanity path from a redirect, or a redirect by its ID
	Rem { redirect: IdOrVanity },
}

#[derive(Debug, Clone)]
enum IdOrVanity {
	Id(Id),
	Vanity(Normalized),
}

impl FromStr for IdOrVanity {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Id::try_from(s).map_or_else(|_| Self::Vanity(Normalized::from(s)), Self::Id))
	}
}

trait FormatError<T> {
	fn format_err(self, message: &'static str) -> Result<T, String>;
}

fn format_result<T, E: Debug>(res: Result<T, E>, message: &'static str) -> Result<T, String> {
	res.map_err(|err| {
		format!(
			"{} {}\n\n{} {:?}",
			"error:".red().bold(),
			message,
			"more info:".blue().bold(),
			err
		)
	})
}

impl<T> FormatError<T> for Result<T, Status> {
	fn format_err(self, message: &'static str) -> Result<T, String> {
		self.map_err(|err| {
			format!(
				"{} {} - {}\n\n{} {:?}",
				"error:".red().bold(),
				message,
				err.message(),
				"more info:".blue().bold(),
				err
			)
		})
	}
}

impl<T> FormatError<T> for Result<T, TonicError> {
	fn format_err(self, message: &'static str) -> Result<T, String> {
		format_result(self, message)
	}
}

impl<T> FormatError<T> for Result<T, ConversionError> {
	fn format_err(self, message: &'static str) -> Result<T, String> {
		format_result(self, message)
	}
}

impl<T> FormatError<T> for Result<T, InvalidUri> {
	fn format_err(self, message: &'static str) -> Result<T, String> {
		format_result(self, message)
	}
}

/// Run the links CLI using configuration from the provided command line
/// arguments. This is essentially the entire CLI binary, but exposed via
/// `lib.rs` to aid in integration tests.
///
/// # What this function *doesn't* do
/// - Print output to the console. Instead, console-destined output is returned
///   from this function as a string, ready to be printed. This is done to aid
///   in integration tests.
///
/// # Errors
/// Any errors are formatted as text in the returned string.
async fn run<I, T>(args: I) -> Result<String, String>
where
	I: IntoIterator<Item = T> + Send,
	T: Into<OsString> + Clone,
{
	// Get command-line args
	let cli = Cli::parse_from(args);

	// Connect to gRPC API with native CA certs
	let client = if cli.tls {
		let tls_config = ClientTlsConfig::new();

		let channel = Channel::from_shared(format!("grpc://{}:{}", cli.host, cli.port))
			.format_err("The host or port is invalid")?
			.tls_config(tls_config)
			.expect("Invalid TLS config")
			.connect()
			.await
			.format_err("Could not connect to gRPC API server")?;

		LinksClient::new(channel)
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip)
	} else {
		LinksClient::connect(format!("grpc://{}:{}", cli.host, cli.port))
			.await
			.format_err("Could not connect to gRPC API server")?
			.send_compressed(CompressionEncoding::Gzip)
			.accept_compressed(CompressionEncoding::Gzip)
	};

	// Do what the user wants
	let res = match cli.command {
		Commands::Id => id(client, cli.token).await,
		Commands::Get { redirect } => get(redirect, client, cli.token).await,
		Commands::New { from, to } => new(from, to, client, cli.token).await,
		Commands::Set { id, link } => set(id, link, client, cli.token).await,
		Commands::Add { id, vanity } => add(id, vanity, client, cli.token).await,
		Commands::Rem { redirect } => rem(redirect, client, cli.token).await,
	}?;

	Ok(if cli.verbose { res.1 } else { res.0 })
}

/// Generate random IDs, and return the first unused one, so that the ID is
/// guaranteed to be unique at the time of the function call. If all IDs are
/// taken, this will loop forever (but considering that that would be about
/// 5 TB of IDs alone, that's quite unlikely).
#[allow(clippy::similar_names)] // Caused by `res` and `req`
async fn gen_unique_id(
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<Id, String> {
	loop {
		let id = Id::new();
		let mut req = Request::new(GetRedirectRequest { id: id.to_string() });
		req.metadata_mut().append("auth", token.clone());
		let res = client
			.get_redirect(req)
			.await
			.format_err("API call failed")?
			.into_inner();

		if res.link.is_none() {
			break Ok(id);
		}
	}
}

/// Generate a new random links id, then check if it already exists. The
/// returned id information is guaranteed to contain an id which at the time
/// of this function call is unused.
async fn id(
	client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	let id = gen_unique_id(client, token).await?;

	Ok((
		format!("{id}"),
		format!("A new random, unique ID: \"{id}\""),
	))
}

/// Get information about a redirect by its ID or vanity path.
async fn get(
	redirect: IdOrVanity,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	let (id, vanity) = match redirect {
		IdOrVanity::Vanity(vanity) => {
			let mut req = Request::new(GetVanityRequest {
				vanity: vanity.clone().into_string(),
			});
			req.metadata_mut().append("auth", token.clone());
			(
				client
					.get_vanity(req)
					.await
					.format_err("API call failed")?
					.into_inner()
					.id
					.map(|id| Id::try_from(id).format_err("API returned invalid link ID"))
					.transpose()?,
				Some(vanity),
			)
		}

		IdOrVanity::Id(id) => (Some(id), None),
	};

	let link = if id.is_some() {
		let mut req = Request::new(GetRedirectRequest {
			id: id.unwrap().to_string(),
		});
		req.metadata_mut().append("auth", token.clone());
		client
			.get_redirect(req)
			.await
			.format_err("API call failed")?
			.into_inner()
			.link
	} else {
		None
	};

	Ok(match (vanity, id, link) {
		(Some(v), None, None) => (
			format!("\"{v}\" ---> ??? ---> ???"),
			format!(
				"\"{v}\" is a vanity path, but doesn't correspond to an ID, and doesn't redirect \
				 anywhere"
			),
		),
		(Some(v), Some(i), None) => (
			format!("\"{v}\" ---> \"{i}\" ---> ???"),
			format!(
				"\"{v}\" is a vanity path corresponding to ID \"{i}\", but doesn't redirect \
				 anywhere"
			),
		),
		(Some(v), Some(i), Some(l)) => (
			format!("\"{v}\" ---> \"{i}\" ---> \"{l}\""),
			format!(
				"\"{v}\" is a vanity path corresponding to ID \"{i}\" and redirects to \"{l}\""
			),
		),
		(None, Some(i), Some(l)) => (
			format!("\"{i}\" ---> \"{l}\""),
			format!("\"{i}\" is an ID and redirects to \"{l}\""),
		),
		(None, Some(i), None) => (
			format!("\"{i}\" ---> ???"),
			format!("\"{i}\" is a valid ID, but doesn't redirect anywhere"),
		),
		_ => unreachable!(),
	})
}

/// Set a redirect from a random ID, optionally with a custom vanity path, to
/// the provided path.
async fn new(
	from: Option<Normalized>,
	to: Link,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	let id = gen_unique_id(client.clone(), token.clone()).await?;

	let mut req = Request::new(SetRedirectRequest {
		id: id.to_string(),
		link: to.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	client
		.set_redirect(req)
		.await
		.format_err("API call failed")?;

	if let Some(vanity) = from {
		let mut req = Request::new(SetVanityRequest {
			vanity: vanity.clone().into_string(),
			id: id.to_string(),
		});
		req.metadata_mut().append("auth", token.clone());
		client.set_vanity(req).await.format_err("API call failed")?;

		Ok((
			format!("\"{vanity}\" ---> \"{id}\" ---> \"{to}\""),
			format!(
				"Successfully set new redirect from ID \"{id}\" to \"{to}\" with vanity path \
				 \"{vanity}\""
			),
		))
	} else {
		Ok((
			format!("\"{id}\" ---> \"{to}\""),
			format!("Successfully set new redirect from ID \"{id}\" to \"{to}\""),
		))
	}
}

/// Set a redirect's link with a specified ID.
async fn set(
	id: Id,
	link: Link,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	let mut req = Request::new(SetRedirectRequest {
		id: id.to_string(),
		link: link.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	let old = client
		.set_redirect(req)
		.await
		.format_err("API call failed")?
		.into_inner()
		.link;

	Ok(old.map_or_else(
		|| {
			(
				format!("\"{id}\" ---> \"{link}\""),
				format!("Successfully set new redirect from ID \"{id}\" to \"{link}\""),
			)
		},
		|old| {
			(
				format!("\"{id}\" ---> \"{link}\" (-X-> \"{old}\")"),
				format!(
					"Successfully modified redirect from ID \"{id}\" to \"{link}\" (used to \
					 redirect to \"{old}\")"
				),
			)
		},
	))
}

/// Add a new vanity path to an existing redirect
async fn add(
	id: Id,
	vanity: Normalized,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	let mut req = Request::new(SetVanityRequest {
		id: id.to_string(),
		vanity: vanity.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	client.set_vanity(req).await.format_err("API call failed")?;

	Ok((
		format!("\"{vanity}\" ---> \"{id}\""),
		format!("Successfully added vanity path \"{vanity}\" to redirect with ID \"{id}\""),
	))
}

/// Get information about a redirect by its ID or vanity path.
async fn rem(
	redirect: IdOrVanity,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> Result<(String, String), String> {
	match redirect {
		IdOrVanity::Id(id) => {
			let mut req = Request::new(RemRedirectRequest { id: id.to_string() });
			req.metadata_mut().append("auth", token.clone());
			let old = client
				.rem_redirect(req)
				.await
				.format_err("API call failed")?
				.into_inner()
				.link;

			Ok(old.map_or_else(
				|| {
					(
						format!("\"{id}\" -X-> ???"),
						format!("No redirect with ID \"{id}\" was found"),
					)
				},
				|old| {
					(
						format!("\"{id}\" -X-> \"{old}\""),
						format!(
							"Successfully removed redirect with ID \"{id}\" (used to redirect to \
							 \"{old}\")"
						),
					)
				},
			))
		}

		IdOrVanity::Vanity(vanity) => {
			let mut req = Request::new(RemVanityRequest {
				vanity: vanity.clone().to_string(),
			});
			req.metadata_mut().append("auth", token.clone());
			let old = client
				.rem_vanity(req)
				.await
				.format_err("API call failed")?
				.into_inner()
				.id;

			Ok(old.map_or_else(
				|| {
					(
						format!("\"{vanity}\" -X-> ???"),
						format!("No redirect with vanity path \"{vanity}\" was found"),
					)
				},
				|old| {
					(
						format!("\"{vanity}\" -X-> \"{old}\""),
						format!(
							"Successfully removed vanity path \"{vanity}\" (used to point to ID \
							 \"{old}\")"
						),
					)
				},
			))
		}
	}
}
