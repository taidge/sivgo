//! Sivgo web server entry point.
//!
//! Starts a Salvo HTTP server that serves the Sivgo REST API.

use salvo::prelude::*;

use sivgo_web::config::ServerConfig;
use sivgo_web::router::create_router;
use sivgo_web::store::Store;

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = ServerConfig::from_env();
    let store = Store::new();

    log::info!("Starting Sivgo server on {}", config.bind_addr());

    let service = create_router(store, &config);
    let acceptor = TcpListener::new(config.bind_addr()).bind().await;
    Server::new(acceptor).serve(service).await;
}
