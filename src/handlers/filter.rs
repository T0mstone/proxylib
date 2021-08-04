use std::collections::HashSet;
use std::future::{ready, Ready};
use std::net::SocketAddr;

use futures::future::{Either, FutureExt, Map};
use hyper::client::HttpConnector;
use hyper::{Body, Client, Request, Response};
use thiserror::Error;

use crate::RequestHandler;

/// The exchangable part of a [`Filter`]
pub trait FilterLogic {
	/// Return whether the request should be let through
	fn filter(&self, from_addr: SocketAddr, request: &Request<Body>) -> bool;
}

/// Obtain a [`FilterLogic`] from a function/closure
pub fn filter_fn<F: Fn(SocketAddr, &Request<Body>) -> bool>(f: F) -> impl FilterLogic {
	struct FilterFn<F: Fn(SocketAddr, &Request<Body>) -> bool>(F);

	impl<F: Fn(SocketAddr, &Request<Body>) -> bool> FilterLogic for FilterFn<F> {
		fn filter(&self, from_addr: SocketAddr, request: &Request<Body>) -> bool {
			(self.0)(from_addr, request)
		}
	}

	FilterFn(f)
}

/// A request handler combinator that filters requests before giving those that passed to
/// another request handler
pub struct Filter<H: RequestHandler, F: FilterLogic> {
	/// The inner request handler to give requests to
	pub inner: H,
	/// The [`FilterLogic`] providing the filtering functionality
	pub logic: F,
}

/// The error type for `<`[`Filter`]` as `[`RequestHandler`]`>`
#[derive(Debug, Error)]
pub enum FilterError<E: std::error::Error> {
	#[error("{0}")]
	/// The inner request handler returned an error
	Inner(E),
	#[error("request from {0} was filtered out")]
	/// The request was filtered out
	FilteredOut(SocketAddr, Box<Request<Body>>),
}

type FilterResult<E> = Result<Response<Body>, FilterError<E>>;
#[allow(type_alias_bounds)]
type FilterPassedFuture<H: RequestHandler> =
	Map<H::Output, fn(Result<Response<Body>, H::Error>) -> FilterResult<H::Error>>;
#[allow(type_alias_bounds)]
type FilterBlockedFuture<H: RequestHandler> = Ready<FilterResult<H::Error>>;
#[allow(type_alias_bounds)]
type FilterFuture<H: RequestHandler> = Either<FilterPassedFuture<H>, FilterBlockedFuture<H>>;

impl<H: RequestHandler, F: FilterLogic> RequestHandler for Filter<H, F> {
	type Error = FilterError<H::Error>;
	type Output = FilterFuture<H>;

	fn handle(
		&self,
		from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output {
		if self.logic.filter(from_addr, &request) {
			Either::Left(
				self.inner
					.handle(from_addr, request, client)
					.map(|res: Result<_, _>| res.map_err(FilterError::Inner)),
			)
		} else {
			Either::Right(ready(Err(FilterError::FilteredOut(
				from_addr,
				Box::new(request),
			))))
		}
	}
}

/// A [`FilterLogic`] which just looks the source address up in a list of known addresses
/// and blocks based on if it is included or not
pub struct AddrLookupFilter {
	/// The list of known addresses
	pub list: HashSet<SocketAddr>,
	/// Whether the filter acts as a blacklist (`true`) or a whitelist (`false`)
	///
	/// If it is `true`, all requests from any address in the list will be blocked
	/// and all others will be let through.
	///
	/// If it is `false`, all requests from any address **not** in the list will be blocked
	/// and all others will be let through.
	pub is_blacklist: bool,
}

impl FilterLogic for AddrLookupFilter {
	fn filter(&self, from_addr: SocketAddr, _: &Request<Body>) -> bool {
		self.is_blacklist != self.list.contains(&from_addr)
	}
}

impl<H: RequestHandler> Filter<H, AddrLookupFilter> {
	/// A shortcut to get a [`Filter`]`<_, `[`AddrLookupFilter`]`>`
	pub fn addr_whitelist(inner: H, whitelist: HashSet<SocketAddr>) -> Self {
		Self {
			inner,
			logic: AddrLookupFilter {
				list: whitelist,
				is_blacklist: false,
			},
		}
	}

	/// A shortcut to get a [`Filter`]`<_, `[`AddrLookupFilter`]`>`
	pub fn addr_blacklist(inner: H, whitelist: HashSet<SocketAddr>) -> Self {
		Self {
			inner,
			logic: AddrLookupFilter {
				list: whitelist,
				is_blacklist: true,
			},
		}
	}
}
