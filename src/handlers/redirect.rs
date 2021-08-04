use std::net::SocketAddr;

use hyper::client::{HttpConnector, ResponseFuture};
use hyper::http::uri::Authority;
use hyper::{Body, Client, Request, Uri};

use crate::RequestHandler;

/// The exchangable part of a [`Redirect`]
pub trait RedirectLogic {
	/// modify the URI
	fn change_uri(&self, uri: &mut Uri);
}

/// Get a [`RedirectLogic`] from a function/closure
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
/// A request handler that works by changing the request URI and forwarding that request to the client
pub struct Redirect<L: RedirectLogic> {
	/// The [`RedirectLogic`] providing the redirect functionality
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

/// A [`RedirectLogic`] which justs sets the authority to a specified value
///
/// For example, if configured with `to: Authority::from_static("example.com")`,
/// it would redirect a request to `<own addr>/a/b/c` to `example.com/a/b/c`.
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
	/// A convenience method to get a [`Redirect`]`<`[`ChangeAuthority`]`>`
	pub fn change_authority(to: Authority) -> Self {
		Self {
			logic: ChangeAuthority { to },
		}
	}
}
