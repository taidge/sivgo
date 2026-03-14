//! Server configuration.
//!
//! Reads settings from environment variables with sensible defaults for
//! local development.

/// Configuration for the Sivgo web server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address (e.g. `"127.0.0.1"`).
    pub host: String,
    /// Bind port.
    pub port: u16,
    /// Secret used to sign/verify JWT tokens.
    pub jwt_secret: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            jwt_secret: "sivgo-dev-secret-change-in-production".to_string(),
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
        Self {
            host: std::env::var("HOST").unwrap_or(defaults.host),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(defaults.port),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or(defaults.jwt_secret),
        }
    }

    /// Return the `host:port` bind address.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
