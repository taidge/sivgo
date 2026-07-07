//! Authentication helpers: HS256 JWT session tokens and Argon2 password hashing.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::ServerConfig;

/// Token lifetime in seconds (24 hours).
const TOKEN_TTL_SECONDS: i64 = 24 * 60 * 60;

/// JWT claims. `sub` holds the user id; `exp` is the Unix expiry timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

/// Create a signed HS256 session token for `user_id` that expires in 24h.
pub fn create_token(user_id: Uuid, secret: &str) -> String {
    let exp = (chrono::Utc::now().timestamp() + TOKEN_TTL_SECONDS).max(0) as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    // Signing only fails on serialization errors, which cannot happen for this
    // fixed claims shape; fall back to an empty (always-invalid) token.
    .unwrap_or_default()
}

/// Verify a token's signature and expiry, returning the user id if valid.
pub fn verify_token(token: &str, secret: &str) -> Option<Uuid> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    data.claims.sub.parse().ok()
}

/// OAuth `state` lifetime in seconds (10 minutes — long enough to complete the
/// provider round-trip, short enough to limit replay).
const OAUTH_STATE_TTL_SECONDS: i64 = 10 * 60;

/// Signed OAuth `state` claims: the provider and a random nonce, plus expiry.
#[derive(Debug, Serialize, Deserialize)]
struct StateClaims {
    provider: String,
    nonce: String,
    exp: usize,
}

/// Build a signed, short-lived OAuth `state` value. Because it is an HS256 JWT
/// over the server secret, the callback can prove the state was issued by this
/// server (anti-forgery/CSRF) and has not expired (anti-replay).
pub fn create_oauth_state(provider: &str, secret: &str) -> String {
    let exp = (chrono::Utc::now().timestamp() + OAUTH_STATE_TTL_SECONDS).max(0) as usize;
    let claims = StateClaims {
        provider: provider.to_string(),
        nonce: Uuid::new_v4().to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap_or_default()
}

/// Verify a signed OAuth `state` and return the provider name if valid.
pub fn verify_oauth_state(state: &str, secret: &str) -> Option<String> {
    let data = decode::<StateClaims>(
        state,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    Some(data.claims.provider)
}

/// Hash a password with Argon2id using a fresh random per-password salt.
/// Returns a self-describing PHC string (algorithm, params and salt embedded).
pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        // A hashing failure here would be an environment/OOM problem; return an
        // empty string, which `verify_password` treats as never-matching.
        .unwrap_or_default()
}

/// Verify a password against a stored Argon2 PHC hash. The comparison is
/// constant-time inside Argon2's verifier.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Extract the authenticated user id from the request's `Authorization`
/// header.  Returns `None` if the header is absent or the token is invalid.
pub fn extract_user_id(req: &Request, depot: &Depot) -> Option<Uuid> {
    let config = depot.obtain::<ServerConfig>().ok()?;
    let header = req.headers().get("authorization")?.to_str().ok()?;
    let token = header.strip_prefix("Bearer ")?;
    verify_token(token, &config.jwt_secret)
}

/// Middleware that validates the `Authorization: Bearer <token>` header and
/// injects the authenticated `UserId` into the depot.
///
/// If the token is missing or invalid, responds with `401 Unauthorized`.
#[handler]
pub async fn auth_guard(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    match extract_user_id(req, depot) {
        Some(user_id) => {
            depot.insert("user_id", user_id);
        }
        None => {
            res.status_code(StatusCode::UNAUTHORIZED);
            res.render(Json(serde_json::json!({"error": "Unauthorized"})));
            ctrl.skip_rest();
        }
    }
}

/// Helper to get the authenticated user id that was injected by [`auth_guard`].
pub fn get_user_id(depot: &Depot) -> Option<Uuid> {
    depot.get::<Uuid>("user_id").ok().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip() {
        let uid = Uuid::new_v4();
        let secret = "test-secret";
        let token = create_token(uid, secret);
        let result = verify_token(&token, secret);
        assert_eq!(result, Some(uid));
    }

    #[test]
    fn token_reject_bad_secret() {
        let uid = Uuid::new_v4();
        let token = create_token(uid, "secret-a");
        let result = verify_token(&token, "secret-b");
        assert_eq!(result, None);
    }

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("my-password");
        assert!(verify_password("my-password", &hash));
        assert!(!verify_password("wrong", &hash));
    }
}
