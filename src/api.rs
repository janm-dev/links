//! This module contains the gRPC-based low-level links API, responsible for
//! allowing outside services access to the links store.

pub use rpc::links_server::LinksServer;

use crate::normalized::Normalized;
use crate::store::Store;
use crate::{id::Id, normalized::Link};
use rpc::links_server::Links;
use tokio::time::Instant;
use tonic::{Code, Request, Response, Status};
use tracing::{info, instrument};

// Do some weird stuff to allow `clippy::pedantic` on generated code.
use rpc_wrapper::rpc;
/// A wrapper around the generated tonic code. Contains the `rpc` module with
/// all of the actual functionality. This is necessary to allow
/// `clippy::pedantic` on the generated code.
mod rpc_wrapper {
	tonic::include_proto!("links");
}

#[derive(Debug)]
pub struct Api<T: Store + 'static> {
	store: &'static T,
}

impl<T: Store + 'static> Api<T> {
	#[instrument(level = "info", skip_all, fields(store = store.backend_name()))]
	pub fn new(store: &'static T) -> Self {
		Api { store }
	}
}

#[tonic::async_trait]
impl<T: Store + 'static> Links for Api<T> {
	#[instrument(level = "info", name = "rpc_get_redirect", skip_all, fields(store = %self.store.backend_name()))]
	async fn get_redirect(&self, req: Request<rpc::Id>) -> Result<Response<rpc::Link>, Status> {
		let time = Instant::now();

		let id = match Id::try_from(req.into_inner().id) {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::InvalidArgument, "id is invalid")),
		};

		let link = match self.store.get_redirect(id).await {
			Ok(link) => link,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = match link {
			Some(link) => Ok(Response::new(rpc::Link {
				url: link.into_string(),
			})),
			None => Err(Status::new(Code::NotFound, "id does not exist")),
		};

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_set_redirect", skip_all, fields(store = %self.store.backend_name()))]
	async fn set_redirect(
		&self,
		req: Request<rpc::IdRedirect>,
	) -> Result<Response<rpc::MaybeLink>, Status> {
		let time = Instant::now();

		let rpc::IdRedirect { id, link } = req.into_inner();

		let id = if let Some(id) = id {
			id.id
		} else {
			return Err(Status::new(Code::InvalidArgument, "id not specified"));
		};

		let link = if let Some(link) = link {
			link.url
		} else {
			return Err(Status::new(Code::InvalidArgument, "link not specified"));
		};

		let id = match Id::try_from(id) {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::InvalidArgument, "id is invalid")),
		};

		let link = match Link::new(&link) {
			Ok(link) => link,
			Err(_) => return Err(Status::new(Code::InvalidArgument, "link is invalid")),
		};

		let link = match self.store.set_redirect(id, link).await {
			Ok(link) => link,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = Ok(Response::new(rpc::MaybeLink {
			link: link.map(|l| rpc::Link {
				url: l.into_string(),
			}),
		}));

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_rem_redirect", skip_all, fields(store = %self.store.backend_name()))]
	async fn rem_redirect(
		&self,
		req: Request<rpc::Id>,
	) -> Result<Response<rpc::MaybeLink>, Status> {
		let time = Instant::now();

		let id = match Id::try_from(req.into_inner().id) {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::InvalidArgument, "id is invalid")),
		};

		let link = match self.store.rem_redirect(id).await {
			Ok(link) => link,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = Ok(Response::new(rpc::MaybeLink {
			link: link.map(|l| rpc::Link {
				url: l.into_string(),
			}),
		}));

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_get_vanity", skip_all, fields(store = %self.store.backend_name()))]
	async fn get_vanity(&self, req: Request<rpc::Vanity>) -> Result<Response<rpc::Id>, Status> {
		let time = Instant::now();

		let vanity = Normalized::new(&req.into_inner().vanity);

		let id = match self.store.get_vanity(vanity).await {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = match id {
			Some(id) => Ok(Response::new(rpc::Id { id: id.to_u64() })),
			None => Err(Status::new(Code::NotFound, "vanity does not exist")),
		};

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_set_vanity", skip_all, fields(store = %self.store.backend_name()))]
	async fn set_vanity(
		&self,
		req: Request<rpc::VanityRedirect>,
	) -> Result<Response<rpc::MaybeId>, Status> {
		let time = Instant::now();

		let rpc::VanityRedirect { vanity, id } = req.into_inner();

		let vanity = if let Some(vanity) = vanity {
			vanity.vanity
		} else {
			return Err(Status::new(Code::InvalidArgument, "vanity not specified"));
		};

		let id = if let Some(id) = id {
			id.id
		} else {
			return Err(Status::new(Code::InvalidArgument, "id not specified"));
		};

		let vanity = Normalized::new(&vanity);

		let id = match Id::try_from(id) {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::InvalidArgument, "id is invalid")),
		};

		let id = match self.store.set_vanity(vanity, id).await {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = Ok(Response::new(rpc::MaybeId {
			id: id.map(|i| rpc::Id { id: i.to_u64() }),
		}));

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_rem_vanity", skip_all, fields(store = %self.store.backend_name()))]
	async fn rem_vanity(
		&self,
		req: Request<rpc::Vanity>,
	) -> Result<Response<rpc::MaybeId>, Status> {
		let time = Instant::now();

		let vanity = Normalized::new(&req.into_inner().vanity);

		let id = match self.store.rem_vanity(vanity).await {
			Ok(id) => id,
			Err(_) => return Err(Status::new(Code::Unavailable, "store operation failed")),
		};

		let res = Ok(Response::new(rpc::MaybeId {
			id: id.map(|i| rpc::Id { id: i.to_u64() }),
		}));

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}
}
