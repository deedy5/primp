pub use self::body::Body;
pub use self::client::{Client, ClientBuilder};
pub use self::request::{Request, RequestBuilder};
pub use self::response::Response;
pub use self::upgrade::Upgraded;

/// Type-erased response body used by connection implementations.
pub(crate) type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, crate::Error>;

pub mod body;
pub mod client;
pub(crate) mod h1_client;
pub(crate) mod h2_client;
pub mod h3_client;
#[cfg(feature = "multipart")]
pub mod multipart;
pub(crate) mod negotiate;
pub(crate) mod range_guard;
pub(crate) mod request;
mod response;
mod upgrade;
