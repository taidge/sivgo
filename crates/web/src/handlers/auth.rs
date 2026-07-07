//! Authentication handlers: register and login.

use salvo::prelude::*;

use crate::auth::{create_token, hash_password, verify_password};
use crate::config::ServerConfig;
use crate::errors::ApiError;
use crate::models::*;
use crate::store::Store;

/// `POST /api/v1/auth/register`
///
/// Creates a new user account and returns a JWT token.
#[handler]
pub async fn register(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let body: RegisterRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    // Validate fields.
    if body.email.is_empty() || body.username.is_empty() || body.password.is_empty() {
        ApiError::bad_request("email, username, and password are required").write_to(res);
        return;
    }

    // Check for duplicate email.
    if store.find_user_by_email(&body.email).is_some() {
        ApiError::conflict("Email already registered").write_to(res);
        return;
    }

    let password_hash = hash_password(&body.password);
    let user = store.create_user(&body.email, &body.username, &password_hash);
    let token = create_token(user.id, &config.jwt_secret);

    res.status_code(StatusCode::CREATED);
    res.render(Json(AuthResponse { token, user }));
}

/// `POST /api/v1/auth/login`
///
/// Authenticates with email + password and returns a JWT token.
#[handler]
pub async fn login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let store = match depot.obtain::<Store>() {
        Ok(s) => s.clone(),
        Err(_) => {
            ApiError::internal("Store unavailable").write_to(res);
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

    let body: LoginRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    // Always run a password verification — even when the email is unknown —
    // to avoid leaking account existence through response timing.
    let user = store.find_user_by_email(&body.email);
    let stored_hash = user
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| hash_password("\0invalid-account\0"));
    let password_ok = verify_password(&body.password, &stored_hash);

    let user = match (user, password_ok) {
        (Some(u), true) => u,
        _ => {
            ApiError::unauthorized("Invalid credentials").write_to(res);
            return;
        }
    };

    let token = create_token(user.id, &config.jwt_secret);
    res.render(Json(AuthResponse { token, user }));
}
