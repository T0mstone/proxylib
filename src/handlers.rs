use std::collections::HashSet;
use std::future::{ready, Ready};
use std::net::SocketAddr;

use futures::future::{Either, FutureExt, Map};
use hyper::client::{HttpConnector, ResponseFuture};
use hyper::http::uri::Authority;
use hyper::{Body, Client, Request, Response, Uri};
use thiserror::Error;

use crate::RequestHandler;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Redirect {
	pub to: Authority,
}

impl RequestHandler for Redirect {
	type Error = hyper::Error;
	type Output = ResponseFuture;

	fn handle(
		&self,
		_from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output {
		let (mut parts, body) = request.into_parts();

		let mut uri_parts = parts.uri.clone().into_parts();
		uri_parts.authority = Some(self.to.clone());
		parts.uri = Uri::from_parts(uri_parts).unwrap();

		client.request(Request::from_parts(parts, body))
	}
}

pub trait FilterImpl {
	fn filter(&self, from_addr: SocketAddr, request: &Request<Body>) -> bool;
}

pub fn filter_fn<F: Fn(SocketAddr, &Request<Body>) -> bool>(f: F) -> impl FilterImpl {
	struct FilterFn<F: Fn(SocketAddr, &Request<Body>) -> bool>(F);

	impl<F: Fn(SocketAddr, &Request<Body>) -> bool> FilterImpl for FilterFn<F> {
		fn filter(&self, from_addr: SocketAddr, request: &Request<Body>) -> bool {
			(self.0)(from_addr, request)
		}
	}

	FilterFn(f)
}

pub struct Filter<
	H: RequestHandler,
	F: FilterImpl,
	G: Fn(SocketAddr, Request<Body>) = fn(SocketAddr, Request<Body>),
> {
	pub inner: H,
	pub request_filter: F,
	pub handle_blocked: Option<G>,
}

pub struct AddrLookupFilter {
	pub list: HashSet<SocketAddr>,
	pub is_blacklist: bool,
}

impl FilterImpl for AddrLookupFilter {
	fn filter(&self, from_addr: SocketAddr, _: &Request<Body>) -> bool {
		self.is_blacklist != self.list.contains(&from_addr)
	}
}

impl<H: RequestHandler> Filter<H, AddrLookupFilter> {
	pub fn addr_whitelist(inner: H, whitelist: HashSet<SocketAddr>) -> Self {
		Self {
			inner,
			request_filter: AddrLookupFilter {
				list: whitelist,
				is_blacklist: false,
			},
			handle_blocked: None,
		}
	}

	pub fn addr_blacklist(inner: H, whitelist: HashSet<SocketAddr>) -> Self {
		Self {
			inner,
			request_filter: AddrLookupFilter {
				list: whitelist,
				is_blacklist: true,
			},
			handle_blocked: None,
		}
	}
}

#[derive(Debug, Error)]
pub enum FilterError<E: std::error::Error> {
	#[error("{0}")]
	Inner(E),
	#[error("request from {0} was filtered out")]
	FilteredOut(SocketAddr),
}

type FilterResult<E> = Result<Response<Body>, FilterError<E>>;
#[allow(type_alias_bounds)]
type FilterOkFuture<H: RequestHandler> =
	Map<H::Output, fn(Result<Response<Body>, H::Error>) -> FilterResult<H::Error>>;
#[allow(type_alias_bounds)]
type FilterNotOkFuture<H: RequestHandler> = Ready<FilterResult<H::Error>>;
#[allow(type_alias_bounds)]
type FilterFuture<H: RequestHandler> = Either<FilterOkFuture<H>, FilterNotOkFuture<H>>;

impl<H: RequestHandler, F: FilterImpl, G: Fn(SocketAddr, Request<Body>)> RequestHandler
	for Filter<H, F, G>
{
	type Error = FilterError<H::Error>;
	type Output = FilterFuture<H>;

	fn handle(
		&self,
		from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output {
		if self.request_filter.filter(from_addr, &request) {
			Either::Left(
				self.inner
					.handle(from_addr, request, client)
					.map(|res: Result<_, _>| res.map_err(FilterError::Inner)),
			)
		} else {
			if let Some(f) = &self.handle_blocked {
				f(from_addr, request);
			}
			Either::Right(ready(Err(FilterError::FilteredOut(from_addr))))
		}
	}
}
