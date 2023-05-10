//! This module contains the gRPC-based low-level links API, responsible for
//! allowing outside services access to the links store.

use links_id::Id;
use links_normalized::{Link, Normalized};
use rpc::links_server::Links;
pub use rpc::{
	links_client::LinksClient, links_server::LinksServer, GetRedirectRequest, GetRedirectResponse,
	GetStatisticsRequest, GetVanityRequest, GetVanityResponse, RemRedirectRequest,
	RemRedirectResponse, RemStatisticsRequest, RemVanityRequest, RemVanityResponse,
	SetRedirectRequest, SetRedirectResponse, SetVanityRequest, SetVanityResponse,
};
use rpc_wrapper::rpc;
use tokio::time::Instant;
pub use tonic::{Code, Request, Response, Status};
use tracing::{info, instrument, trace};

use crate::{
	config::Config,
	stats::StatisticDescription,
	store::{Current, Store},
};
/// A wrapper around the generated tonic code. Contains the `rpc` module with
/// all of the actual functionality. This is necessary to allow
/// `clippy::pedantic` on the generated code.
mod rpc_wrapper {
	tonic::include_proto!("links");
}

/// Get a function that checks authentication/authorization of an incoming grpc
/// API call. The incoming request is checked for the `auth` metadata value,
/// which should be a shared secret string value, that is simply compared to
/// the one configured. **It is critical that this value is kept secret and
/// never exposed publicly!**
///
/// # Errors
/// Returns the `UNAUTHENTICATED` status code if the token is not provided or
/// is invalid.
pub fn get_auth_checker(
	config: &'static Config,
) -> impl Fn(Request<()>) -> Result<Request<()>, Status> + Clone {
	#[allow(clippy::cognitive_complexity)] // Caused by macro expansion
	move |req: Request<()>| -> Result<Request<()>, Status> {
		let token = if let Some(token) = req.metadata().get("auth") {
			token.as_encoded_bytes()
		} else {
			trace!("no auth token to check");
			return Err(Status::new(Code::Unauthenticated, "no auth token provided"));
		};

		let secret = config.token();

		trace!("checking auth token {token:?}, secret is {secret:?}");

		if secret.as_bytes() == token {
			trace!("auth token is valid");
			Ok(req)
		} else {
			trace!("auth token is not valid");
			Err(Status::new(Code::Unauthenticated, "auth token is invalid"))
		}
	}
}

/// The grpc API implementation. Contains a reference to the store on which all
/// operations are performed. Implements all RPC calls from `links.proto`.
#[derive(Debug)]
pub struct Api {
	store: &'static Current,
}

impl Api {
	/// Create a new API instance. This instance will operate on the `store`
	/// provided, and provide access to that store via gRPC.
	#[instrument(level = "info", skip_all, fields(store = store.backend_name()))]
	pub fn new(store: &'static Current) -> Self {
		Self { store }
	}

	/// Get a reference to this API's store.
	#[instrument(level = "trace", skip_all)]
	pub fn store(&self) -> Store {
		self.store.get()
	}
}

#[tonic::async_trait]
impl Links for Api {
	#[instrument(level = "info", name = "rpc_get_redirect", skip_all, fields(store = %self.store.backend_name()))]
	async fn get_redirect(
		&self,
		req: Request<rpc::GetRedirectRequest>,
	) -> Result<Response<rpc::GetRedirectResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let Ok(id) = Id::try_from(req.into_inner().id) else {
			return Err(Status::new(Code::InvalidArgument, "id is invalid"))
		};

		let Ok(link) = store.get_redirect(id).await else {
			return Err(Status::new(Code::Internal, "store operation failed"))
		};

		let res = Ok(Response::new(rpc::GetRedirectResponse {
			link: link.map(Link::into_string),
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

	#[instrument(level = "info", name = "rpc_set_redirect", skip_all, fields(store = %self.store.backend_name()))]
	async fn set_redirect(
		&self,
		req: Request<rpc::SetRedirectRequest>,
	) -> Result<Response<rpc::SetRedirectResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let rpc::SetRedirectRequest { id, link } = req.into_inner();

		let Ok(id) = Id::try_from(id) else {
			return Err(Status::new(Code::InvalidArgument, "id is invalid"));
		};

		let Ok(link) = Link::new(&link) else {
			return Err(Status::new(Code::InvalidArgument, "link is invalid"));
		};

		let Ok(link) = store.set_redirect(id, link).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let res = Ok(Response::new(rpc::SetRedirectResponse {
			link: link.map(Link::into_string),
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
		req: Request<rpc::RemRedirectRequest>,
	) -> Result<Response<rpc::RemRedirectResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let Ok(id) = Id::try_from(req.into_inner().id) else {
			return Err(Status::new(Code::InvalidArgument, "id is invalid"));
		};

		let Ok(link) = store.rem_redirect(id).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let res = Ok(Response::new(rpc::RemRedirectResponse {
			link: link.map(Link::into_string),
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
	async fn get_vanity(
		&self,
		req: Request<rpc::GetVanityRequest>,
	) -> Result<Response<rpc::GetVanityResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let vanity = Normalized::new(&req.into_inner().vanity);

		let Ok(id) = store.get_vanity(vanity).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let res = Ok(Response::new(rpc::GetVanityResponse {
			id: id.map(|id| id.to_string()),
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

	#[instrument(level = "info", name = "rpc_set_vanity", skip_all, fields(store = %self.store.backend_name()))]
	async fn set_vanity(
		&self,
		req: Request<rpc::SetVanityRequest>,
	) -> Result<Response<rpc::SetVanityResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let rpc::SetVanityRequest { vanity, id } = req.into_inner();

		let vanity = Normalized::new(&vanity);

		let Ok(id) = Id::try_from(id) else {
			return Err(Status::new(Code::InvalidArgument, "id is invalid"));
		};

		let Ok(id) = store.set_vanity(vanity, id).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let res = Ok(Response::new(rpc::SetVanityResponse {
			id: id.map(|id| id.to_string()),
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
		req: Request<rpc::RemVanityRequest>,
	) -> Result<Response<rpc::RemVanityResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let vanity = Normalized::new(&req.into_inner().vanity);

		let Ok(id) = store.rem_vanity(vanity).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let res = Ok(Response::new(rpc::RemVanityResponse {
			id: id.map(|id| id.to_string()),
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

	#[instrument(level = "info", name = "rpc_get_statistics", skip_all, fields(store = %self.store.backend_name()))]
	async fn get_statistics(
		&self,
		req: Request<rpc::GetStatisticsRequest>,
	) -> Result<Response<rpc::GetStatisticsResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let rpc::GetStatisticsRequest {
			link,
			r#type: stat_type,
			data,
			time: stat_time,
		} = req.into_inner();

		let stat_desc = match (
			link.map(TryInto::try_into).transpose(),
			stat_type.map(|s| s.as_str().try_into()).transpose(),
			data.map(TryInto::try_into).transpose(),
			stat_time.map(|t| t.as_str().try_into()).transpose(),
		) {
			(Ok(link), Ok(stat_type), Ok(data), Ok(time)) => StatisticDescription {
				link,
				stat_type,
				data,
				time,
			},
			_ => {
				return Err(Status::new(
					Code::InvalidArgument,
					"one of the supplied arguments is invalid",
				))
			}
		};

		let Ok(stats) = store.get_statistics(stat_desc).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let statistics = stats
			.map(|(s, v)| rpc::StatisticWithValue {
				link: s.link.to_string(),
				r#type: s.stat_type.to_string(),
				data: s.data.to_string(),
				time: s.time.to_string(),
				value: v.get(),
			})
			.collect();

		let res = Ok(Response::new(rpc::GetStatisticsResponse { statistics }));

		let time = time.elapsed();
		info!(
			time_ns = %time.as_nanos(),
			success = %res.is_ok(),
			"rpc processed in {:.6} seconds",
			time.as_secs_f64()
		);

		res
	}

	#[instrument(level = "info", name = "rpc_rem_statistics", skip_all, fields(store = %self.store.backend_name()))]
	async fn rem_statistics(
		&self,
		req: Request<rpc::RemStatisticsRequest>,
	) -> Result<Response<rpc::RemStatisticsResponse>, Status> {
		let time = Instant::now();
		let store = self.store();

		let rpc::RemStatisticsRequest {
			link,
			r#type: stat_type,
			data,
			time: stat_time,
		} = req.into_inner();

		let stat_desc = match (
			link.map(TryInto::try_into).transpose(),
			stat_type.map(|s| s.as_str().try_into()).transpose(),
			data.map(TryInto::try_into).transpose(),
			stat_time.map(|t| t.as_str().try_into()).transpose(),
		) {
			(Ok(link), Ok(stat_type), Ok(data), Ok(time)) => StatisticDescription {
				link,
				stat_type,
				data,
				time,
			},
			_ => {
				return Err(Status::new(
					Code::InvalidArgument,
					"one of the supplied arguments is invalid",
				))
			}
		};

		let Ok(stats) = store.rem_statistics(stat_desc).await else {
			return Err(Status::new(Code::Internal, "store operation failed"));
		};

		let statistics = stats
			.map(|(s, v)| rpc::StatisticWithValue {
				link: s.link.to_string(),
				r#type: s.stat_type.to_string(),
				data: s.data.to_string(),
				time: s.time.to_string(),
				value: v.get(),
			})
			.collect();

		let res = Ok(Response::new(rpc::RemStatisticsResponse { statistics }));

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
