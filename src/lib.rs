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

pub trait RequestHandler {
	type Error: std::error::Error + Send + Sync + 'static;
	type Output: Future<Output = Result<Response<Body>, Self::Error>> + Send + 'static;

	fn handle(
		&self,
		from_addr: SocketAddr,
		request: Request<Body>,
		client: &Client<HttpConnector>,
	) -> Self::Output;
}

pub struct ProxyConfig<T: RequestHandler + 'static> {
	pub listen_on: SocketAddr,
	pub request_handler: &'static T,
}

#[derive(Debug, Error)]
pub enum ProxyError {
	#[error("failed to bind TcpListener: {0}")]
	BindListener(std::io::Error),
	#[error("failed to start http server: {0}")]
	StartServer(hyper::Error),
	#[error("http server stopped with error: {0}")]
	Serve(hyper::Error),
}

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
