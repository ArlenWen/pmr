#[cfg(feature = "http-api")]
pub mod server;

#[cfg(feature = "http-api")]
pub mod auth;

#[cfg(feature = "http-api")]
pub mod handlers;

#[cfg(feature = "http-api")]
pub mod docs;

#[cfg(feature = "http-api")]
pub use server::ApiServer;

#[cfg(feature = "http-api")]
pub use auth::{AuthManager, ApiToken};
