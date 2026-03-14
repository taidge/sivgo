//! Lightweight authentication helpers.
//!
//! For development purposes we use a simplified HMAC-SHA256 "JWT" and a
//! basic password hash.  In production these should be replaced with
//! proper `jsonwebtoken` + `argon2` implementations.

use std::collections::HashMap;
use std::fmt::Write as _;

use salvo::prelude::*;
use uuid::Uuid;

use crate::config::ServerConfig;

/// Minimal JWT-style token: base64(json(header)) . base64(json(payload)) . signature
///
/// For the dev server we skip real crypto and just encode the user id in the
/// payload so that handlers can extract it. This MUST NOT be used in production.
pub fn create_token(user_id: Uuid, secret: &str) -> String {
    // Simple token: "userId:{uuid}:secret_hash"
    let payload = format!("{}:{}", user_id, simple_hmac(&user_id.to_string(), secret));
    base64_encode(&payload)
}

/// Verify a token and return the user id if valid.
pub fn verify_token(token: &str, secret: &str) -> Option<Uuid> {
    let decoded = base64_decode(token)?;
    let parts: Vec<&str> = decoded.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let user_id: Uuid = parts[0].parse().ok()?;
    let expected_hmac = simple_hmac(&user_id.to_string(), secret);
    if parts[1] == expected_hmac {
        Some(user_id)
    } else {
        None
    }
}

/// Hash a password (simplified for dev -- NOT secure for production).
pub fn hash_password(password: &str) -> String {
    // Use a simple hash so the dev server works without heavy crypto deps.
    simple_hmac(password, "sivgo-password-salt")
}

/// Verify a password against a stored hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    hash_password(password) == hash
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

// ---------------------------------------------------------------------------
// Trivial helpers (dev-only, not cryptographically secure)
// ---------------------------------------------------------------------------

/// A very simple HMAC-like hash using std only.  NOT for production use.
fn simple_hmac(data: &str, key: &str) -> String {
    // XOR-fold the data+key bytes into a 32-byte digest.
    let mut buf = [0u8; 32];
    for (i, b) in data.bytes().chain(key.bytes()).enumerate() {
        buf[i % 32] ^= b;
        // mix bits around a little
        buf[(i + 7) % 32] = buf[(i + 7) % 32].wrapping_add(b);
    }
    let mut hex = String::with_capacity(64);
    for b in &buf {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

fn base64_encode(input: &str) -> String {
    // Manual base64 encoding (no extra deps).
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

fn base64_decode(input: &str) -> Option<String> {
    let mut table = HashMap::new();
    for (i, &c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        .iter()
        .enumerate()
    {
        table.insert(c, i as u32);
    }
    let bytes: Vec<u32> = input
        .bytes()
        .filter_map(|b| table.get(&b).copied())
        .collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let len = chunk.len();
        let b0 = chunk[0];
        let b1 = if len > 1 { chunk[1] } else { 0 };
        let b2 = if len > 2 { chunk[2] } else { 0 };
        let b3 = if len > 3 { chunk[3] } else { 0 };
        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;
        out.push(((triple >> 16) & 0xFF) as u8);
        if len > 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if len > 3 {
            out.push((triple & 0xFF) as u8);
        }
    }
    String::from_utf8(out).ok()
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
