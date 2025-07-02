pub mod cli;
pub mod config;
pub mod database;
pub mod error;
pub mod formatter;
pub mod log_rotation;
pub mod process;

#[cfg(feature = "http-api")]
pub mod api {
    pub mod auth;
    pub mod docs;
    pub mod handlers;
    pub mod server;

    pub use auth::AuthManager;
    pub use server::ApiServer;
}

pub use error::{Error, Result};
