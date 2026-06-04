//! OAuth2 authentication handlers.

use salvo::prelude::*;

use crate::errors::ApiError;
use crate::oauth::{self, OAuthClient, OAuthProvider};

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

    let provider = match OAuthProvider::from_str(&provider_name) {
        Some(provider) => provider,
        None => {
            ApiError::bad_request(format!("Unsupported provider: {provider_name}")).write_to(res);
            return;
        }
    };

    let state = oauth::build_state(provider.name(), &uuid::Uuid::new_v4().to_string());
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

    let provider_name = match req
        .query::<String>("provider")
        .or_else(|| oauth::provider_name_from_state(&state))
    {
        Some(provider_name) => provider_name,
        None => {
            ApiError::bad_request("Missing OAuth provider in state").write_to(res);
            return;
        }
    };
    let provider = match OAuthProvider::from_str(&provider_name) {
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

    match oauth_client.get_user_info(provider, &token) {
        Ok(user) => res.render(Json(serde_json::json!({
            "access_token": token.access_token,
            "refresh_token": token.refresh_token,
            "expires_in": token.expires_in,
            "provider": provider.name(),
            "user": user,
        }))),
        Err(e) => ApiError::bad_request(format!("Userinfo request failed: {e}")).write_to(res),
    }
}
