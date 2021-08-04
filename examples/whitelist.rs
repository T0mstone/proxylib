use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

use hyper::http::uri::Authority;
use once_cell::sync::Lazy;
use proxylib::handlers::filter::AddrLookupFilter;
use proxylib::handlers::redirect::ChangeAuthority;
use proxylib::handlers::{Filter, Redirect};
use proxylib::ProxyConfig;

static HANDLER: Lazy<Filter<Redirect<ChangeAuthority>, AddrLookupFilter>> = Lazy::new(|| {
	Filter::addr_whitelist(
		Redirect::change_authority(Authority::from_static("example.com")),
		{
			let mut whitelist = HashSet::new();
			for addr in "localhost:8000".to_socket_addrs().unwrap() {
				whitelist.insert(addr);
			}
			whitelist.insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), 8000));
			whitelist
		},
	)
});

#[tokio::main]
async fn main() {
	let config = ProxyConfig {
		listen_on: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
		request_handler: &*HANDLER,
	};

	proxylib::run_proxy(config).await.unwrap();
}
