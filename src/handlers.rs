/// Functionality relating to [`Filter`]
pub mod filter;
/// Functionality relating to [`Redirect`]
pub mod redirect;

/// All functionality from this module, easy to import
///
/// Just write the following:
/// ```
/// use proxylib::handlers::prelude::*;
/// ```
/// and you have imported everything
pub mod prelude {
	pub use super::filter::*;
	pub use super::redirect::*;
}

pub use filter::Filter;
pub use redirect::Redirect;
