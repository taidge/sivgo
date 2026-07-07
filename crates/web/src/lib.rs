//! `sivgo-web` -- Salvo web backend for the Sivgo online SVG editor.
//!
//! This crate provides a REST API server that:
//!
//! - Manages user accounts (register / login with JWT tokens)
//! - Stores SVG documents with version history (in-memory for now)
//! - Exports SVG to PNG using the `sivgo-render` pipeline
//!
//! # Quick start
//!
//! ```no_run
//! use sivgo_web::{config::ServerConfig, store::Store, router::create_router};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = ServerConfig::from_env();
//!     let store = Store::new();
//!     let service = create_router(store, &config);
//!     // ...bind and serve...
//! }
//! ```

#![recursion_limit = "512"]

pub mod assets;
pub mod auth;
pub mod config;
pub mod errors;
pub mod handlers;
pub mod models;
pub mod oauth;
pub mod openapi;
pub mod plugin_registry;
pub mod preferences;
pub mod rate_limit;
pub mod router;
pub mod store;
