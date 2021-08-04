use std::net::SocketAddr;

use hyper::client::{HttpConnector, ResponseFuture};
use hyper::http::uri::Authority;
use hyper::{Body, Client, Request, Uri};

use crate::RequestHandler;

pub trait RedirectLogic {
	fn change_uri(&self, uri: &mut Uri);
}

pub fn redirect_fn<F: Fn(&mut Uri)>(f: F) -> impl RedirectLogic {
	struct RedirectFn<F: Fn(&mut Uri)>(F);

	impl<F: Fn(&mut Uri)> RedirectLogic for RedirectFn<F> {
		fn change_uri(&self, uri: &mut Uri) {
			(self.0)(uri)
		}
	}

	RedirectFn(f)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Redirect<L: RedirectLogic> {
	pub logic: L,
}

impl<L: RedirectLogic> RequestHandler for Redirect<L> {
	type Error = hyper::Error;
	type Output = ResponseFuture;

	fn handle(
		&self,
		_from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output {
		let (mut parts, body) = request.into_parts();

		self.logic.change_uri(&mut parts.uri);

		client.request(Request::from_parts(parts, body))
	}
}

pub struct ChangeAuthority {
	pub to: Authority,
}

impl RedirectLogic for ChangeAuthority {
	fn change_uri(&self, uri: &mut Uri) {
		let mut uri_parts = uri.clone().into_parts();
		uri_parts.authority = Some(self.to.clone());
		*uri = Uri::from_parts(uri_parts).unwrap();
	}
}

impl Redirect<ChangeAuthority> {
	pub fn change_authority(to: Authority) -> Self {
		Self {
			logic: ChangeAuthority { to },
		}
	}
}
