//! OAuth2 authentication handlers.

use salvo::prelude::*;

use crate::errors::ApiError;
use crate::oauth::{self, OAuthConfigLegacy as OAuthConfig};

/// `GET /api/v1/auth/oauth/{provider}`
///
/// Redirects the user to the OAuth2 provider's authorization page.
#[handler]
pub async fn oauth_login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let config = match depot.obtain::<OAuthConfig>() {
        Ok(c) => c.clone(),
        Err(_) => {
            ApiError::internal("OAuth config unavailable").write_to(res);
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

    let provider = match provider_name.as_str() {
        "google" => oauth::google_provider(&config),
        "github" => oauth::github_provider(&config),
        _ => {
            ApiError::bad_request(format!("Unsupported provider: {provider_name}")).write_to(res);
            return;
        }
    };

    let provider = match provider {
        Some(p) => p,
        None => {
            ApiError::bad_request(format!("Provider {provider_name} is not configured"))
                .write_to(res);
            return;
        }
    };

    let state = uuid::Uuid::new_v4().to_string();
    let auth_url = oauth::build_auth_url(&provider, &state);

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
    let config = match depot.obtain::<OAuthConfig>() {
        Ok(c) => c.clone(),
        Err(_) => {
            ApiError::internal("OAuth config unavailable").write_to(res);
            return;
        }
    };

    let code: String = match req.query("code") {
        Some(c) => c,
        None => {
            ApiError::bad_request("Missing code parameter").write_to(res);
            return;
        }
    };

    let _state: String = match req.query("state") {
        Some(s) => s,
        None => {
            ApiError::bad_request("Missing state parameter").write_to(res);
            return;
        }
    };

    // Try Google first, then GitHub. In production the state parameter
    // would encode which provider was used.
    let provider = oauth::google_provider(&config).or_else(|| oauth::github_provider(&config));

    let provider = match provider {
        Some(p) => p,
        None => {
            ApiError::bad_request("No OAuth provider configured").write_to(res);
            return;
        }
    };

    match oauth::exchange_code(&provider, &code) {
        Ok(token) => {
            res.render(Json(serde_json::json!({
                "access_token": token,
                "provider": provider.name,
            })));
        }
        Err(e) => {
            ApiError::bad_request(format!("Token exchange failed: {e}")).write_to(res);
        }
    }
}
