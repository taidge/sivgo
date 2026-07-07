//! Server configuration.
//!
//! Reads settings from environment variables with sensible defaults for
//! local development.

/// The built-in development secret. Using this value in production is a
/// complete authentication bypass (see `auth.rs`), so [`ServerConfig::from_env`]
/// refuses to start with it when `SIVGO_ENV=production`.
pub const DEV_JWT_SECRET: &str = "sivgo-dev-secret-change-in-production";

/// Configuration for the Sivgo web server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address (e.g. `"127.0.0.1"`).
    pub host: String,
    /// Bind port.
    pub port: u16,
    /// Secret used to sign/verify JWT tokens.
    pub jwt_secret: String,
    /// Explicit list of allowed CORS origins. When empty the server falls back
    /// to a permissive (any-origin) policy intended only for local development.
    pub cors_allowed_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            jwt_secret: DEV_JWT_SECRET.to_string(),
            cors_allowed_origins: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// Build a config from environment variables, falling back to defaults.
    ///
    /// | Variable     | Default                                   |
    /// |-------------|-------------------------------------------|
    /// | `HOST`      | `127.0.0.1`                               |
    /// | `PORT`      | `3000`                                    |
    /// | `JWT_SECRET`| `sivgo-dev-secret-change-in-production`    |
    pub fn from_env() -> Self {
        let defaults = Self::default();
        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or(defaults.jwt_secret);
        let is_production = std::env::var("SIVGO_ENV")
            .map(|v| v.eq_ignore_ascii_case("production"))
            .unwrap_or(false);
        if jwt_secret == DEV_JWT_SECRET {
            if is_production {
                panic!(
                    "Refusing to start in production with the default JWT secret. \
                     Set the JWT_SECRET environment variable to a strong random value."
                );
            }
            log::warn!(
                "Using the built-in development JWT secret — tokens are forgeable. \
                 Set JWT_SECRET before exposing this server."
            );
        }
        let cors_allowed_origins = std::env::var("CORS_ALLOWED_ORIGINS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if cors_allowed_origins.is_empty() && is_production {
            panic!(
                "Refusing to start in production without CORS_ALLOWED_ORIGINS. \
                 Set it to the exact frontend origin(s) that may call this API."
            );
        }
        Self {
            host: std::env::var("HOST").unwrap_or(defaults.host),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(defaults.port),
            jwt_secret,
            cors_allowed_origins,
        }
    }

    /// Return the `host:port` bind address.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvSnapshot {
        values: Vec<(&'static str, Option<String>)>,
    }

    impl EnvSnapshot {
        fn new(keys: &[&'static str]) -> Self {
            let values = keys
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect();
            Self { values }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                match value {
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }

    fn set_env(key: &str, value: &str) {
        unsafe { std::env::set_var(key, value) };
    }

    fn clear_env(key: &str) {
        unsafe { std::env::remove_var(key) };
    }

    fn with_config_env(test: impl FnOnce()) {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap();
        let _snapshot = EnvSnapshot::new(&[
            "HOST",
            "PORT",
            "JWT_SECRET",
            "SIVGO_ENV",
            "CORS_ALLOWED_ORIGINS",
        ]);
        clear_env("HOST");
        clear_env("PORT");
        clear_env("JWT_SECRET");
        clear_env("SIVGO_ENV");
        clear_env("CORS_ALLOWED_ORIGINS");
        test();
    }

    #[test]
    fn development_allows_permissive_cors_defaults() {
        with_config_env(|| {
            let config = ServerConfig::from_env();

            assert_eq!(config.jwt_secret, DEV_JWT_SECRET);
            assert!(config.cors_allowed_origins.is_empty());
            assert_eq!(config.bind_addr(), "127.0.0.1:3000");
        });
    }

    #[test]
    fn production_rejects_default_jwt_secret() {
        with_config_env(|| {
            set_env("SIVGO_ENV", "production");
            set_env("CORS_ALLOWED_ORIGINS", "https://app.example.com");

            let result = panic::catch_unwind(ServerConfig::from_env);

            assert!(result.is_err());
        });
    }

    #[test]
    fn production_rejects_missing_cors_allowlist() {
        with_config_env(|| {
            set_env("SIVGO_ENV", "production");
            set_env("JWT_SECRET", "strong-secret-used-in-tests");

            let result = panic::catch_unwind(ServerConfig::from_env);

            assert!(result.is_err());
        });
    }

    #[test]
    fn production_accepts_explicit_cors_allowlist() {
        with_config_env(|| {
            set_env("SIVGO_ENV", "production");
            set_env("JWT_SECRET", "strong-secret-used-in-tests");
            set_env(
                "CORS_ALLOWED_ORIGINS",
                "https://app.example.com, https://admin.example.com",
            );

            let config = ServerConfig::from_env();

            assert_eq!(
                config.cors_allowed_origins,
                vec!["https://app.example.com", "https://admin.example.com"]
            );
        });
    }
}
