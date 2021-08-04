//! # Proxylib
//! A library to make writing proxies easier

use std::convert::Infallible;
use std::future::Future;
use std::net::{SocketAddr, TcpListener};

use hyper::client::HttpConnector;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use thiserror::Error;

/// A collection of common [`RequestHandler`]s and combinators
pub mod handlers;

/// Something that can handle a request and give back a response (or an error)
pub trait RequestHandler {
	/// The error type in [`Output`](Self::Output)
	type Error: std::error::Error + Send + Sync + 'static;
	/// The future returned by [`handle`](Self::handle)
	type Output: Future<Output = Result<Response<Body>, Self::Error>> + Send + 'static;

	/// Handle the request and give back a result of a response
	fn handle(
		&self,
		from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output;
}

/// The config of a proxy
pub struct ProxyConfig<T: RequestHandler + 'static> {
	/// The address where the proxy listens for requests
	pub listen_on: SocketAddr,
	/// The handler that handles the incoming requests
	pub request_handler: &'static T,
}

#[derive(Debug, Error)]
/// An error while running the proxy
pub enum ProxyError {
	#[error("failed to bind TcpListener: {0}")]
	/// Failed to bing the `TcpListener` to the specified address
	BindListener(std::io::Error),
	#[error("failed to start http server: {0}")]
	/// Failed to start the internal http server
	StartServer(hyper::Error),
	#[error("http server stopped with error: {0}")]
	/// The internal http server encountered an error while running
	Serve(hyper::Error),
}

/// Run a proxy with the given configuration
pub async fn run_proxy<T: RequestHandler + Sync + 'static>(
	config: ProxyConfig<T>,
) -> Result<(), ProxyError> {
	let listener = TcpListener::bind(config.listen_on).map_err(ProxyError::BindListener)?;
	let server_builder = Server::from_tcp(listener).map_err(ProxyError::StartServer)?;

	let client: &'static Client<HttpConnector> = Box::leak(Box::new(Client::new()));

	let make_service = make_service_fn(move |conn: &AddrStream| {
		let addr = conn.remote_addr();

		let handler = config.request_handler;

		let handle = move |req: Request<Body>| handler.handle(addr, req, client);

		async move { Ok::<_, Infallible>(service_fn(handle)) }
	});

	let server = server_builder.serve(make_service);

	// This future completes once the server quits
	server.await.map_err(ProxyError::Serve)
}
