//! A simple command-line interface for configuring links redirects via the
//! gRPC API built into every redirector.
//!
//! Supports all basic links store operations using the redirectors' gRPC API.

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use links::api::{
	GetRedirectRequest, GetVanityRequest, LinksClient, RemRedirectRequest, RemVanityRequest,
	SetRedirectRequest, SetVanityRequest,
};
use links::id::{ConversionError, Id};
use links::normalized::{Link, Normalized};
use std::convert::Infallible;
use std::fmt::Debug;
use std::process;
use std::str::FromStr;
use tonic::{
	metadata::AsciiMetadataValue, transport::Channel, transport::Error as TonicError, Request,
	Status,
};

#[derive(Parser, Debug)]
#[clap(name = "links-cli", version, about = "A simple command-line interface for configuring links redirects via the gRPC API built into every redirector.", long_about = None)]
struct Cli {
	#[clap(subcommand)]
	command: Commands,

	/// Redirector server hostaname
	#[clap(short, long, env = "LINKS_RPC_HOST")]
	host: String,

	/// Redirector gRPC port
	#[clap(short, long, env = "LINKS_RPC_PORT", default_value = "530")]
	port: u16,

	/// gRPC API authentication token
	#[clap(short, long, env = "LINKS_RPC_TOKEN")]
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
	fn format_err(self, message: &'static str) -> T;
}

fn format_result<T, E: Debug>(res: Result<T, E>, message: &'static str) -> T {
	match res {
		Ok(ok) => ok,
		Err(err) => {
			println!(
				"{} {}\n\n{} {:?}",
				"error:".red().bold(),
				message,
				"more info:".blue().bold(),
				err
			);

			process::exit(2)
		}
	}
}

impl<T> FormatError<T> for Result<T, Status> {
	fn format_err(self, message: &'static str) -> T {
		match self {
			Self::Ok(ok) => ok,
			Self::Err(err) => {
				println!(
					"{} {} - {}\n\n{} {:?}",
					"error:".red().bold(),
					message,
					err.message(),
					"more info:".blue().bold(),
					err
				);

				process::exit(2)
			}
		}
	}
}

impl<T> FormatError<T> for Result<T, TonicError> {
	fn format_err(self, message: &'static str) -> T {
		format_result(self, message)
	}
}

impl<T> FormatError<T> for Result<T, ConversionError> {
	fn format_err(self, message: &'static str) -> T {
		format_result(self, message)
	}
}

#[tokio::main]
async fn main() -> Result<()> {
	// Get command-line args
	let cli = Cli::parse();

	// Connect to gRPC API
	let client = LinksClient::connect(format!("grpc://{}:{}", cli.host, cli.port))
		.await
		.format_err("Could not connect to gRPC API server")
		.accept_gzip()
		.send_gzip();

	// Do what the user wants
	let res = match cli.command {
		Commands::Id => id(client, cli.token).await,
		Commands::Get { redirect } => get(redirect, client, cli.token).await,
		Commands::New { from, to } => new(from, to, client, cli.token).await,
		Commands::Set { id, link } => set(id, link, client, cli.token).await,
		Commands::Add { id, vanity } => add(id, vanity, client, cli.token).await,
		Commands::Rem { redirect } => rem(redirect, client, cli.token).await,
	};

	// Display the results to the user
	println!("{}", if cli.verbose { res.1 } else { res.0 });

	Ok(())
}

/// Generate random IDs, and return the first unused one, so that the ID is
/// guaranteed to be unique at the time of the function call. If all IDs are
/// taken, this will loop forever (but considering that that would be about
/// 5 TB of IDs alone, that's quite unlikely).
async fn gen_unique_id(mut client: LinksClient<Channel>, token: AsciiMetadataValue) -> Id {
	loop {
		let id = Id::new();
		let mut req = Request::new(GetRedirectRequest { id: id.to_string() });
		req.metadata_mut().append("auth", token.clone());
		let res = client
			.get_redirect(req)
			.await
			.format_err("API call failed")
			.into_inner();

		if res.link.is_none() {
			break id;
		}
	}
}

/// Generate a new random links id, then check if it already exists. The
/// returned id information is guaranteed to contain an id which at the time
/// of this function call is unused.
async fn id(client: LinksClient<Channel>, token: AsciiMetadataValue) -> (String, String) {
	let id = gen_unique_id(client, token).await;

	(
		format!("{id}"),
		format!("A new random, unique ID: \"{id}\""),
	)
}

/// Get information about a redirect by its ID or vanity path.
async fn get(
	redirect: IdOrVanity,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> (String, String) {
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
					.format_err("API call failed")
					.into_inner()
					.id
					.map(|id| Id::try_from(id).format_err("API returned invalid link ID")),
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
			.format_err("API call failed")
			.into_inner()
			.link
	} else {
		None
	};

	match (vanity, id, link) {
		(Some(v), None, None) => (
			format!("\"{v}\" ---> ??? ---> ???"),
			format!("\"{v}\" is a vanity path, but doesn't correspond to an ID, and doesn't redirect anywhere")
		),
		(Some(v), Some(i), None) => (
			format!("\"{v}\" ---> \"{i}\" ---> ???"),
			format!("\"{v}\" is a vanity path corresponding to ID \"{i}\", but doesn't redirect anywhere")
		),
		(Some(v), Some(i), Some(l)) => (
			format!("\"{v}\" ---> \"{i}\" ---> \"{l}\""),
			format!("\"{v}\" is a vanity path corresponding to ID \"{i}\" and redirects to \"{l}\"")
		),
		(None, Some(i), Some(l)) => (
			format!("\"{i}\" ---> \"{l}\""),
			format!("\"{i}\" is an ID and redirects to \"{l}\"")
		),
		(None, Some(i), None) => (
			format!("\"{i}\" ---> ???"),
			format!("\"{i}\" is a valid ID, but doesn't redirect anywhere")
		),
		_ => unreachable!(),
	}
}

/// Set a redirect from a random ID, optionally with a custom vanity path, to
/// the provided path.
async fn new(
	from: Option<Normalized>,
	to: Link,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> (String, String) {
	let id = gen_unique_id(client.clone(), token.clone()).await;

	let mut req = Request::new(SetRedirectRequest {
		id: id.to_string(),
		link: to.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	client.set_redirect(req).await.format_err("API call failed");

	if let Some(vanity) = from {
		let mut req = Request::new(SetVanityRequest {
			vanity: vanity.clone().into_string(),
			id: id.to_string(),
		});
		req.metadata_mut().append("auth", token.clone());
		client.set_vanity(req).await.format_err("API call failed");

		(
			format!("\"{vanity}\" ---> \"{id}\" ---> \"{to}\""), 
			format!("Successfully set new redirect from ID \"{id}\" to \"{to}\" with vanity path \"{vanity}\"")
		)
	} else {
		(
			format!("\"{id}\" ---> \"{to}\""),
			format!("Successfully set new redirect from ID \"{id}\" to \"{to}\""),
		)
	}
}

/// Set a redirect's link with a specified ID.
async fn set(
	id: Id,
	link: Link,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> (String, String) {
	let mut req = Request::new(SetRedirectRequest {
		id: id.to_string(),
		link: link.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	let old = client
		.set_redirect(req)
		.await
		.format_err("API call failed")
		.into_inner()
		.link;

	if let Some(old) = old {
		(
			format!("\"{id}\" ---> \"{link}\" (-X-> \"{old}\")"),
			format!("Successfully modified redirect from ID \"{id}\" to \"{link}\" (used to redirect to \"{old}\")"),
		)
	} else {
		(
			format!("\"{id}\" ---> \"{link}\""),
			format!("Successfully set new redirect from ID \"{id}\" to \"{link}\""),
		)
	}
}

/// Add a new vanity path to an existing redirect
async fn add(
	id: Id,
	vanity: Normalized,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> (String, String) {
	let mut req = Request::new(SetVanityRequest {
		id: id.to_string(),
		vanity: vanity.clone().into_string(),
	});
	req.metadata_mut().append("auth", token.clone());
	client.set_vanity(req).await.format_err("API call failed");

	(
		format!("\"{vanity}\" ---> \"{id}\""),
		format!("Successfully added vanity path \"{vanity}\" to redirect with ID \"{id}\""),
	)
}

/// Get information about a redirect by its ID or vanity path.
async fn rem(
	redirect: IdOrVanity,
	mut client: LinksClient<Channel>,
	token: AsciiMetadataValue,
) -> (String, String) {
	match redirect {
		IdOrVanity::Id(id) => {
			let mut req = Request::new(RemRedirectRequest { id: id.to_string() });
			req.metadata_mut().append("auth", token.clone());
			let old = client
				.rem_redirect(req)
				.await
				.format_err("API call failed")
				.into_inner()
				.link;

			if let Some(old) = old {
				(format!("\"{id}\" -X-> \"{old}\""), format!("Successfully removed redirect with ID \"{id}\" (used to redirect to \"{old}\")"))
			} else {
				(
					format!("\"{id}\" -X-> ???"),
					format!("No redirect with ID \"{id}\" was found"),
				)
			}
		}
		IdOrVanity::Vanity(vanity) => {
			let mut req = Request::new(RemVanityRequest {
				vanity: vanity.clone().to_string(),
			});
			req.metadata_mut().append("auth", token.clone());
			let old = client
				.rem_vanity(req)
				.await
				.format_err("API call failed")
				.into_inner()
				.id;

			if let Some(old) = old {
				(format!("\"{vanity}\" -X-> \"{old}\""), format!("Successfully removed vanity path \"{vanity}\" (used to point to ID \"{old}\")"))
			} else {
				(
					format!("\"{vanity}\" -X-> ???"),
					format!("No redirect with vanity path \"{vanity}\" was found"),
				)
			}
		}
	}
}
