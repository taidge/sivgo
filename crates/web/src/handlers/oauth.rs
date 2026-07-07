//! OAuth2 authentication handlers.

use salvo::prelude::*;

use crate::auth::{self, create_token, hash_password};
use crate::config::ServerConfig;
use crate::errors::ApiError;
use crate::models::AuthResponse;
use crate::oauth::{OAuthClient, OAuthProvider};
use crate::store::Store;

/// `GET /api/v1/auth/oauth/{provider}`
///
/// Redirects the user to the OAuth2 provider's authorization page.
#[handler]
pub async fn oauth_login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let oauth_client = match depot.obtain::<OAuthClient>() {
        Ok(client) => client,
        Err(_) => {
            ApiError::internal("OAuth client unavailable").write_to(res);
            return;
        }
    };

    let provider_name: String = match req.param("provider") {
        Some(p) => p,
        None => {
            ApiError::bad_request("Missing provider parameter").write_to(res);
            return;
        }
    };

    let provider = match OAuthProvider::parse_name(&provider_name) {
        Some(provider) => provider,
        None => {
            ApiError::bad_request(format!("Unsupported provider: {provider_name}")).write_to(res);
            return;
        }
    };

    let config = match depot.obtain::<ServerConfig>() {
        Ok(c) => c.clone(),
        Err(_) => {
            ApiError::internal("Config unavailable").write_to(res);
            return;
        }
    };

    // Issue a signed, short-lived state so the callback can verify it was
    // server-issued (anti-forgery/CSRF) rather than supplied by an attacker.
    let state = auth::create_oauth_state(provider.name(), &config.jwt_secret);
    let auth_url = match oauth_client.get_auth_url_with_state(provider, &state) {
        Ok(url) => url,
        Err(e) => {
            ApiError::bad_request(e).write_to(res);
            return;
        }
    };

    res.render(Json(serde_json::json!({
        "auth_url": auth_url,
        "state": state,
    })));
}

/// `GET /api/v1/auth/oauth/callback`
///
/// Handles the OAuth2 callback after the user authorizes the app.
#[handler]
pub async fn oauth_callback(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let code: String = match req.query("code") {
        Some(c) => c,
        None => {
            ApiError::bad_request("Missing code parameter").write_to(res);
            return;
        }
    };

    let state: String = match req.query("state") {
        Some(s) => s,
        None => {
            ApiError::bad_request("Missing state parameter").write_to(res);
            return;
        }
    };

    let config = match depot.obtain::<ServerConfig>() {
        Ok(c) => c.clone(),
        Err(_) => {
            ApiError::internal("Config unavailable").write_to(res);
            return;
        }
    };

    // Verify the state's signature/expiry and derive the provider solely from
    // the (server-issued) state. A forged or expired state is rejected, and the
    // `provider` query param is intentionally ignored here.
    let provider_name = match auth::verify_oauth_state(&state, &config.jwt_secret) {
        Some(provider_name) => provider_name,
        None => {
            ApiError::bad_request("Invalid or expired OAuth state").write_to(res);
            return;
        }
    };
    let provider = match OAuthProvider::parse_name(&provider_name) {
        Some(provider) => provider,
        None => {
            ApiError::bad_request(format!("Unsupported provider: {provider_name}")).write_to(res);
            return;
        }
    };

    let oauth_client = match depot.obtain::<OAuthClient>() {
        Ok(client) => client,
        Err(_) => {
            ApiError::internal("OAuth client unavailable").write_to(res);
            return;
        }
    };

    let token = match oauth_client.exchange_code(provider, &code) {
        Ok(token) => token,
        Err(e) => {
            ApiError::bad_request(format!("Token exchange failed: {e}")).write_to(res);
            return;
        }
    };

    let info = match oauth_client.get_user_info(provider, &token) {
        Ok(info) => info,
        Err(e) => {
            ApiError::bad_request(format!("Userinfo request failed: {e}")).write_to(res);
            return;
        }
    };

    // Map the verified provider identity to a local account and issue a local
    // session token. We never return the upstream access/refresh tokens to the
    // browser.
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
            return;
        }
    };

    if info.email.is_empty() {
        ApiError::bad_request("OAuth provider did not supply an email").write_to(res);
        return;
    }

    let user = match store.find_user_by_email(&info.email) {
        Some(user) => user,
        None => {
            // OAuth-only account: store a hash of a random secret as the
            // password so the local password login path can never match.
            let placeholder = hash_password(&uuid::Uuid::new_v4().to_string());
            let username = if info.name.is_empty() {
                info.email.clone()
            } else {
                info.name.clone()
            };
            store.create_user(&info.email, &username, &placeholder)
        }
    };

    let session_token = create_token(user.id, &config.jwt_secret);
    res.render(Json(AuthResponse {
        token: session_token,
        user,
    }));
}
