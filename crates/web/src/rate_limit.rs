//! Lightweight in-memory rate limiting for public API routes.
//!
//! This protects unauthenticated endpoints in development and single-process
//! deployments. A production deployment should replace or augment it with a
//! distributed limiter at the edge or database/cache layer.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use salvo::prelude::*;

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
        let mut buckets = self.buckets.write().unwrap();
        buckets.retain(|_, bucket| now.duration_since(bucket.window_start) <= self.window * 2);

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

/// Rate-limit public unauthenticated routes by forwarded client IP when
/// available. Local requests without forwarding headers share a fallback key.
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

    let key = client_key(req);
    if limiter.allow(&key) {
        return;
    }

    ApiError::rate_limited("Rate limit exceeded").write_to(res);
    ctrl.skip_rest();
}

fn client_key(req: &Request) -> String {
    let headers = req.headers();
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let real_ip = headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    format!("public:{}", forwarded.or(real_ip).unwrap_or("local"))
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
}
