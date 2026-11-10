//! Route definitions for the Sivgo web API.

use std::sync::Arc;
use std::time::Duration;

use salvo::catch_panic::CatchPanic;
use salvo::cors::Cors;
use salvo::http::request::SecureMaxSize;
use salvo::http::Method;
use salvo::logging::Logger;
use salvo::prelude::*;
use salvo::size_limiter::max_size;
use salvo::timeout::Timeout;

use crate::assets::AssetStore;
use crate::audit::AuditLog;
use crate::auth;
use crate::config::ServerConfig;
use crate::handlers;
use crate::oauth::{OAuthClient, ReqwestOAuthHttpClient};
use crate::plugin_registry::PluginRegistry;
use crate::preferences::PreferencesStore;
use crate::rate_limit::{public_rate_limit, user_rate_limit, RateLimiter, UserRateLimiter};
use crate::store::Store;

/// Build the full Salvo [`Router`] tree for the API.
///
/// Layout:
///
/// ```text
/// /api/v1/
///   health                    GET   (public)
///   openapi.json              GET   (public)
///   auth/
///     register                POST  (public)
///     login                   POST  (public)
///     oauth/{provider}        GET   (public)
///     oauth/callback          GET   (public)
///   documents/                GET   (auth)
///                             POST  (auth)
///     {id}                    GET   (auth)
///                             PUT   (auth)
///                             DELETE(auth)
///       versions              GET   (auth)
///   export                    POST  (public)
///   agent/suggest             POST  (auth, opt-in)
///   shared/{token}            GET   (public, rate-limited)
///   assets/                   GET   (auth)
///                             POST  (auth)
///     {id}                    GET   (auth)
///                             DELETE(auth)
///   preferences               GET   (auth)
///                             PUT   (auth)
///                             PATCH (auth)
///   plugins/                  GET   (public)
///     installed               GET   (auth)
///     {pluginId}/install      POST  (auth)
///     {pluginId}/uninstall    POST  (auth)
/// ```
pub fn create_router(store: Store, config: &ServerConfig) -> Service {
    // Restrict CORS to the configured origins; fall back to permissive only
    // when no allowlist is provided (local-dev convenience — see ServerConfig).
    let cors = if config.cors_allowed_origins.is_empty() {
        Cors::permissive().into_handler()
    } else {
        Cors::new()
            .allow_origin(&config.cors_allowed_origins)
            .allow_methods(vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(vec!["authorization", "content-type"])
            .into_handler()
    };

    // Create additional stores.
    let asset_store = AssetStore::new();
    let prefs_store = PreferencesStore::new();
    let plugin_registry = PluginRegistry::new();
    let audit_log = AuditLog::default();
    let oauth_client = OAuthClient::from_env(Arc::new(ReqwestOAuthHttpClient::new()));
    let public_rate_limiter = RateLimiter::new(120, Duration::from_secs(60));

    // Public routes (no auth required).
    let public = Router::with_path("api/v1")
        .hoop(public_rate_limit)
        .push(Router::with_path("health").get(handlers::health::health_check))
        .push(Router::with_path("openapi.json").get(handlers::openapi::openapi_spec))
        .push(
            Router::with_path("auth")
                .push(Router::with_path("register").post(handlers::auth::register))
                .push(Router::with_path("login").post(handlers::auth::login))
                .push(
                    Router::with_path("oauth")
                        .push(Router::with_path("callback").get(handlers::oauth::oauth_callback))
                        .push(Router::with_path("{provider}").get(handlers::oauth::oauth_login)),
                ),
        )
        .push(Router::with_path("export").post(handlers::export::export_document))
        .push(Router::with_path("shared/{token}").get(handlers::shares::get_shared_document))
        .push(Router::with_path("plugins").get(handlers::plugins::list_plugins));

    // Authenticated routes.
    let authenticated = Router::with_path("api/v1")
        .hoop(auth::auth_guard)
        // Registration is open, so authenticated traffic needs a ceiling too.
        .hoop(user_rate_limit)
        .push(
            Router::with_path("agent/suggest")
                .hoop(max_size(2 * 1024 * 1024))
                .hoop(SecureMaxSize(2 * 1024 * 1024))
                .post(handlers::agent::suggest),
        )
        .push(
            Router::with_path("documents")
                .get(handlers::documents::list_documents)
                .post(handlers::documents::create_document)
                .push(
                    Router::with_path("{id}")
                        .get(handlers::documents::get_document)
                        .put(handlers::documents::update_document)
                        .delete(handlers::documents::delete_document)
                        .push(Router::with_path("versions").get(handlers::documents::list_versions))
                        .push(
                            Router::with_path("shares")
                                .get(handlers::shares::list_shares)
                                .post(handlers::shares::create_share)
                                .push(
                                    Router::with_path("{share_id}")
                                        .delete(handlers::shares::revoke_share),
                                ),
                        ),
                ),
        )
        .push(
            Router::with_path("assets")
                .get(handlers::assets::list_assets)
                .post(handlers::assets::upload_asset)
                .push(
                    Router::with_path("{id}")
                        .get(handlers::assets::download_asset)
                        .delete(handlers::assets::delete_asset),
                ),
        )
        .push(
            Router::with_path("preferences")
                .get(handlers::preferences::get_preferences)
                .put(handlers::preferences::set_preferences)
                .patch(handlers::preferences::merge_preferences),
        )
        .push(
            Router::with_path("plugins")
                .push(Router::with_path("installed").get(handlers::plugins::list_installed_plugins))
                .push(
                    Router::with_path("{pluginId}")
                        .push(Router::with_path("install").post(handlers::plugins::install_plugin))
                        .push(
                            Router::with_path("uninstall")
                                .post(handlers::plugins::uninstall_plugin),
                        ),
                ),
        );

    let router = Router::new()
        // Salvo's default body cap is 64 KiB, far below the per-endpoint limits
        // the handlers enforce, so every larger document or asset was rejected
        // with a parse error before reaching them. This raises the transport
        // limit to just above the largest handler limit; the handlers still
        // apply their own, more specific caps.
        .hoop(max_size(MAX_REQUEST_BODY_BYTES))
        // `max_size` only rejects oversized bodies; the limit that `parse_json`
        // itself applies is separate and defaults to 64 KiB.
        .hoop(SecureMaxSize(MAX_REQUEST_BODY_BYTES as usize))
        .hoop(
            affix_state::inject(store)
                .inject(config.clone())
                .inject(asset_store)
                .inject(prefs_store)
                .inject(plugin_registry)
                .inject(audit_log)
                .inject(handlers::agent::AgentService::from_env())
                .inject(public_rate_limiter)
                .inject(UserRateLimiter::default()),
        )
        .hoop(affix_state::inject(oauth_client))
        .push(public)
        .push(authenticated);

    Service::new(router)
        .hoop(cors)
        .hoop(Logger::new())
        // A panic in rendering or parsing must fail one request, not drop the
        // connection with no response; a slow request must not hold a worker
        // open indefinitely.
        .hoop(CatchPanic::new())
        .hoop(Timeout::new(REQUEST_TIMEOUT))
}

/// Largest request body accepted at the transport layer.
///
/// Sits just above the biggest per-endpoint limit (24 MiB for a base64 asset
/// upload) so that handlers, not the transport, produce the error message.
const MAX_REQUEST_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// Wall-clock limit for a single request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
