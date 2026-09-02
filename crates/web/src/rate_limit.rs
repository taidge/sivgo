//! Lightweight in-memory rate limiting for public API routes.
//!
//! This protects unauthenticated endpoints in development and single-process
//! deployments. A production deployment should replace or augment it with a
//! distributed limiter at the edge or database/cache layer.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use salvo::prelude::*;

use crate::config::ServerConfig;
use crate::errors::ApiError;

#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_requests: u32,
    window: Duration,
    buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
}

#[derive(Debug, Clone)]
struct RateLimitBucket {
    window_start: Instant,
    count: u32,
}

/// Largest number of distinct clients tracked at once.
///
/// Each unique key costs an entry, and an IPv6 client can present a fresh
/// address per request, so the table needs a ceiling of its own. When it is
/// reached, stale buckets are dropped first and the whole table only as a last
/// resort; briefly forgetting counts is preferable to unbounded growth.
const MAX_TRACKED_CLIENTS: usize = 100_000;

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests: max_requests.max(1),
            window,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut buckets = crate::lock::write(&self.buckets);

        // Sweeping the whole table on every request is O(n) under the write
        // lock, so only do it when the table has actually grown.
        if buckets.len() > MAX_TRACKED_CLIENTS {
            buckets.retain(|_, bucket| now.duration_since(bucket.window_start) <= self.window * 2);
            if buckets.len() > MAX_TRACKED_CLIENTS {
                buckets.clear();
            }
        }

        let bucket = buckets.entry(key.to_string()).or_insert(RateLimitBucket {
            window_start: now,
            count: 0,
        });

        if now.duration_since(bucket.window_start) > self.window {
            bucket.window_start = now;
            bucket.count = 0;
        }

        if bucket.count >= self.max_requests {
            return false;
        }

        bucket.count += 1;
        true
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(120, Duration::from_secs(60))
    }
}

/// Rate limiter applied to authenticated routes, keyed by user rather than
/// address.
///
/// Registration is open, so "authenticated" is not a meaningful barrier on its
/// own: one account can otherwise issue unlimited writes.
#[derive(Debug, Clone)]
pub struct UserRateLimiter(pub RateLimiter);

impl Default for UserRateLimiter {
    fn default() -> Self {
        UserRateLimiter(RateLimiter::new(600, Duration::from_secs(60)))
    }
}

/// Rate-limit authenticated routes per user id.
#[handler]
pub async fn user_rate_limit(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    let Ok(limiter) = depot.obtain::<UserRateLimiter>() else {
        // Nothing configured: leave the request alone rather than failing it.
        return;
    };

    // Falls back to the address when the token has not been read yet, so the
    // hoop works wherever it is placed in the chain.
    let key = match crate::auth::extract_user_id(req, depot) {
        Some(user_id) => format!("user:{user_id}"),
        None => client_key(req, false),
    };

    if limiter.0.allow(&key) {
        return;
    }

    ApiError::rate_limited("Rate limit exceeded").write_to(res);
    ctrl.skip_rest();
}

/// Rate-limit public unauthenticated routes by socket IP. Forwarded client IP
/// headers are only considered when explicitly enabled for a trusted proxy.
#[handler]
pub async fn public_rate_limit(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    let limiter = match depot.obtain::<RateLimiter>() {
        Ok(limiter) => limiter,
        Err(_) => {
            ApiError::internal("Rate limiter unavailable").write_to(res);
            ctrl.skip_rest();
            return;
        }
    };

    let trust_proxy_headers = depot
        .obtain::<ServerConfig>()
        .map(|config| config.trust_proxy_headers)
        .unwrap_or(false);
    let key = client_key(req, trust_proxy_headers);
    if limiter.allow(&key) {
        return;
    }

    ApiError::rate_limited("Rate limit exceeded").write_to(res);
    ctrl.skip_rest();
}

fn client_key(req: &Request, trust_proxy_headers: bool) -> String {
    let forwarded = trust_proxy_headers.then(|| {
        header_ip(req, "x-forwarded-for", true).or_else(|| header_ip(req, "x-real-ip", false))
    });
    let ip = forwarded.flatten().or_else(|| req.remote_addr().ip());

    format!(
        "public:{}",
        ip.map(|ip| ip.to_string())
            .unwrap_or_else(|| "local".to_string())
    )
}

/// Read a client address from a proxy header.
///
/// `X-Forwarded-For` accumulates left to right, each hop appending the address
/// it saw. The value the trusted proxy in front of us added is therefore the
/// **last** entry; everything to its left was supplied by the client and can
/// say anything. Taking the leftmost entry let a caller mint a fresh identity
/// per request and bypass the limit entirely.
///
/// `take_last` is false only for single-valued headers such as `X-Real-IP`.
fn header_ip(req: &Request, name: &str, take_last: bool) -> Option<IpAddr> {
    let value = req.headers().get(name)?.to_str().ok()?;
    let candidate = if take_last {
        value.rsplit(',').next()?
    } else {
        value
    };
    candidate.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_blocks_after_window_quota() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));

        assert!(limiter.allow("client"));
        assert!(limiter.allow("client"));
        assert!(!limiter.allow("client"));
    }

    #[test]
    fn limiter_tracks_clients_independently() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));

        assert!(limiter.allow("client-a"));
        assert!(limiter.allow("client-b"));
        assert!(!limiter.allow("client-a"));
    }

    fn request_with_remote_ip(ip: &str) -> Request {
        let mut request = Request::default();
        let socket = format!("{ip}:8080")
            .parse::<std::net::SocketAddr>()
            .expect("test socket should parse");
        *request.remote_addr_mut() = socket.into();
        request
    }

    #[test]
    fn client_key_ignores_untrusted_forwarding_headers() {
        let mut request = request_with_remote_ip("203.0.113.10");
        request
            .headers_mut()
            .insert("x-forwarded-for", "198.51.100.77".parse().unwrap());

        assert_eq!(client_key(&request, false), "public:203.0.113.10");
    }

    #[test]
    fn client_key_accepts_valid_headers_from_trusted_proxy() {
        let mut request = request_with_remote_ip("203.0.113.10");
        // The proxy appends the address it saw, so the rightmost entry is the
        // one that can be trusted.
        request.headers_mut().insert(
            "x-forwarded-for",
            "198.51.100.77, 203.0.113.10".parse().unwrap(),
        );

        assert_eq!(client_key(&request, true), "public:203.0.113.10");
    }

    #[test]
    fn client_cannot_forge_an_identity_by_prepending_to_forwarded_for() {
        // Two requests from the same client that each claim a different
        // upstream address must still land on the same bucket, otherwise the
        // limit can be bypassed one forged address at a time.
        let mut first = request_with_remote_ip("203.0.113.10");
        first
            .headers_mut()
            .insert("x-forwarded-for", "1.2.3.4, 203.0.113.10".parse().unwrap());

        let mut second = request_with_remote_ip("203.0.113.10");
        second
            .headers_mut()
            .insert("x-forwarded-for", "5.6.7.8, 203.0.113.10".parse().unwrap());

        assert_eq!(client_key(&first, true), client_key(&second, true));
    }

    #[test]
    fn a_single_forwarded_value_is_still_honoured() {
        let mut request = request_with_remote_ip("203.0.113.10");
        request
            .headers_mut()
            .insert("x-forwarded-for", "198.51.100.77".parse().unwrap());

        assert_eq!(client_key(&request, true), "public:198.51.100.77");
    }

    #[test]
    fn invalid_trusted_proxy_header_falls_back_to_socket_ip() {
        let mut request = request_with_remote_ip("203.0.113.10");
        request
            .headers_mut()
            .insert("x-forwarded-for", "not-an-ip".parse().unwrap());

        assert_eq!(client_key(&request, true), "public:203.0.113.10");
    }
}
