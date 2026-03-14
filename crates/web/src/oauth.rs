//! OAuth2 login support.
//!
//! Provides configuration and helpers for Google and GitHub OAuth2 flows.
//! The `exchange_code` and `get_user_info` methods are stubbed for development
//! -- a production deployment would perform real HTTP requests to the
//! provider endpoints.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Provider enum
// ---------------------------------------------------------------------------

/// Supported OAuth2 providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OAuthProvider {
    Google,
    GitHub,
}

impl OAuthProvider {
    /// Return the canonical lowercase name used in URL paths.
    pub fn name(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::GitHub => "github",
        }
    }

    /// Parse a provider name string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" => Some(OAuthProvider::Google),
            "github" => Some(OAuthProvider::GitHub),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// OAuth2 credentials and endpoint URLs for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
}

// ---------------------------------------------------------------------------
// Token / user info types
// ---------------------------------------------------------------------------

/// Token response returned after exchanging an authorization code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

/// User profile information retrieved from the OAuth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub provider_user_id: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// An OAuth2 client that holds configurations for multiple providers and
/// exposes methods for the authorization-code flow.
#[derive(Debug, Clone)]
pub struct OAuthClient {
    configs: HashMap<OAuthProvider, OAuthConfig>,
}

impl OAuthClient {
    /// Create a new client with the given provider configurations.
    pub fn new(configs: HashMap<OAuthProvider, OAuthConfig>) -> Self {
        Self { configs }
    }

    /// Create an empty client (no providers configured).
    pub fn empty() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// Add or replace the configuration for a provider.
    pub fn add_provider(&mut self, provider: OAuthProvider, config: OAuthConfig) {
        self.configs.insert(provider, config);
    }

    /// Return `true` if the given provider has been configured.
    pub fn has_provider(&self, provider: OAuthProvider) -> bool {
        self.configs.contains_key(&provider)
    }

    /// Build the authorization URL that the client should redirect the user
    /// to.
    ///
    /// The `state` parameter is an opaque anti-CSRF token that the client
    /// should validate when the user returns.
    pub fn get_auth_url(&self, provider: OAuthProvider) -> Result<String, String> {
        let config = self
            .configs
            .get(&provider)
            .ok_or_else(|| format!("Provider {:?} is not configured", provider))?;
        Ok(format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile",
            config.auth_url, config.client_id, config.redirect_uri,
        ))
    }

    /// Exchange an authorization code for an access token.
    ///
    /// **Stub implementation** -- returns a mock token without making a real
    /// HTTP request.  A production deployment would POST to
    /// `config.token_url` with the code, client_id, client_secret, and
    /// redirect_uri.
    pub fn exchange_code(&self, provider: OAuthProvider, code: &str) -> Result<OAuthToken, String> {
        let config = self
            .configs
            .get(&provider)
            .ok_or_else(|| format!("Provider {:?} is not configured", provider))?;

        if code.is_empty() {
            return Err("authorization code must not be empty".to_string());
        }

        // In production this would be:
        //   POST {config.token_url}
        //     grant_type=authorization_code
        //     code={code}
        //     client_id={config.client_id}
        //     client_secret={config.client_secret}
        //     redirect_uri={config.redirect_uri}
        Ok(OAuthToken {
            access_token: format!(
                "mock_access_token_{}_{}",
                provider.name(),
                &config.client_id
            ),
            refresh_token: Some(format!("mock_refresh_token_{}", provider.name())),
            expires_in: 3600,
        })
    }

    /// Retrieve user profile information from the OAuth provider using an
    /// access token.
    ///
    /// **Stub implementation** -- returns plausible mock data.  A production
    /// deployment would call the provider's userinfo endpoint.
    pub fn get_user_info(
        &self,
        provider: OAuthProvider,
        token: &OAuthToken,
    ) -> Result<OAuthUserInfo, String> {
        if token.access_token.is_empty() {
            return Err("access token must not be empty".to_string());
        }

        let _ = self
            .configs
            .get(&provider)
            .ok_or_else(|| format!("Provider {:?} is not configured", provider))?;

        // In production this would fetch from:
        //   Google: GET https://www.googleapis.com/oauth2/v3/userinfo
        //   GitHub: GET https://api.github.com/user
        Ok(match provider {
            OAuthProvider::Google => OAuthUserInfo {
                email: "user@gmail.com".to_string(),
                name: "Google User".to_string(),
                avatar_url: Some("https://lh3.googleusercontent.com/photo.jpg".to_string()),
                provider_user_id: "google-uid-123456".to_string(),
            },
            OAuthProvider::GitHub => OAuthUserInfo {
                email: "user@github.com".to_string(),
                name: "GitHub User".to_string(),
                avatar_url: Some("https://avatars.githubusercontent.com/u/12345".to_string()),
                provider_user_id: "github-uid-789012".to_string(),
            },
        })
    }
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self::empty()
    }
}

// ---------------------------------------------------------------------------
// Legacy helpers (kept for backward-compat with existing handler code)
// ---------------------------------------------------------------------------

/// Legacy flat configuration struct.
///
/// Used by the existing router/handler code.  New code should prefer
/// [`OAuthClient`] with per-provider [`OAuthConfig`] values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OAuthConfigLegacy {
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub redirect_uri: String,
}

/// Legacy per-provider descriptor.
#[derive(Debug, Clone)]
pub struct OAuthProviderLegacy {
    pub name: String,
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
}

/// Build a Google provider from the legacy config.
pub fn google_provider(config: &OAuthConfigLegacy) -> Option<OAuthProviderLegacy> {
    let client_id = config.google_client_id.as_ref()?;
    let client_secret = config.google_client_secret.as_ref()?;
    Some(OAuthProviderLegacy {
        name: "google".to_string(),
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
    })
}

/// Build a GitHub provider from the legacy config.
pub fn github_provider(config: &OAuthConfigLegacy) -> Option<OAuthProviderLegacy> {
    let client_id = config.github_client_id.as_ref()?;
    let client_secret = config.github_client_secret.as_ref()?;
    Some(OAuthProviderLegacy {
        name: "github".to_string(),
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
    })
}

/// Build the authorization URL for a legacy provider.
pub fn build_auth_url(provider: &OAuthProviderLegacy, state: &str) -> String {
    format!(
        "{}?client_id={}&state={}&response_type=code&scope=openid%20email%20profile",
        provider.auth_url, provider.client_id, state,
    )
}

/// Exchange an authorization code for a token (legacy stub).
pub fn exchange_code(provider: &OAuthProviderLegacy, code: &str) -> Result<String, String> {
    if code.is_empty() {
        return Err("authorization code must not be empty".to_string());
    }
    Ok(format!(
        "mock_access_token_{}_{}_{}",
        provider.name, provider.client_id, code
    ))
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----- New OAuthClient tests ------------------------------------------

    fn make_client() -> OAuthClient {
        let mut configs = HashMap::new();
        configs.insert(
            OAuthProvider::Google,
            OAuthConfig {
                client_id: "google-id".to_string(),
                client_secret: "google-secret".to_string(),
                redirect_uri: "http://localhost:3000/callback".to_string(),
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
            },
        );
        configs.insert(
            OAuthProvider::GitHub,
            OAuthConfig {
                client_id: "github-id".to_string(),
                client_secret: "github-secret".to_string(),
                redirect_uri: "http://localhost:3000/callback".to_string(),
                auth_url: "https://github.com/login/oauth/authorize".to_string(),
                token_url: "https://github.com/login/oauth/access_token".to_string(),
            },
        );
        OAuthClient::new(configs)
    }

    #[test]
    fn provider_from_str() {
        assert_eq!(
            OAuthProvider::from_str("google"),
            Some(OAuthProvider::Google)
        );
        assert_eq!(
            OAuthProvider::from_str("GitHub"),
            Some(OAuthProvider::GitHub)
        );
        assert_eq!(
            OAuthProvider::from_str("GOOGLE"),
            Some(OAuthProvider::Google)
        );
        assert_eq!(OAuthProvider::from_str("twitter"), None);
    }

    #[test]
    fn provider_name() {
        assert_eq!(OAuthProvider::Google.name(), "google");
        assert_eq!(OAuthProvider::GitHub.name(), "github");
    }

    #[test]
    fn get_auth_url_contains_client_id() {
        let client = make_client();
        let url = client.get_auth_url(OAuthProvider::Google).unwrap();
        assert!(url.contains("client_id=google-id"));
        assert!(url.contains("accounts.google.com"));
    }

    #[test]
    fn get_auth_url_unconfigured_provider() {
        let client = OAuthClient::empty();
        let result = client.get_auth_url(OAuthProvider::Google);
        assert!(result.is_err());
    }

    #[test]
    fn exchange_code_returns_token() {
        let client = make_client();
        let token = client
            .exchange_code(OAuthProvider::GitHub, "auth-code-123")
            .unwrap();
        assert!(token.access_token.contains("mock_access_token"));
        assert!(token.access_token.contains("github"));
        assert!(token.refresh_token.is_some());
        assert_eq!(token.expires_in, 3600);
    }

    #[test]
    fn exchange_code_rejects_empty_code() {
        let client = make_client();
        let result = client.exchange_code(OAuthProvider::Google, "");
        assert!(result.is_err());
    }

    #[test]
    fn get_user_info_returns_mock_data() {
        let client = make_client();
        let token = client
            .exchange_code(OAuthProvider::Google, "test-code")
            .unwrap();
        let info = client.get_user_info(OAuthProvider::Google, &token).unwrap();
        assert!(!info.email.is_empty());
        assert!(!info.name.is_empty());
        assert!(!info.provider_user_id.is_empty());
    }

    #[test]
    fn get_user_info_github_mock() {
        let client = make_client();
        let token = OAuthToken {
            access_token: "some-token".to_string(),
            refresh_token: None,
            expires_in: 3600,
        };
        let info = client.get_user_info(OAuthProvider::GitHub, &token).unwrap();
        assert!(info.email.contains("github"));
        assert!(info.avatar_url.is_some());
    }

    #[test]
    fn get_user_info_rejects_empty_token() {
        let client = make_client();
        let token = OAuthToken {
            access_token: "".to_string(),
            refresh_token: None,
            expires_in: 0,
        };
        let result = client.get_user_info(OAuthProvider::Google, &token);
        assert!(result.is_err());
    }

    #[test]
    fn add_provider_dynamically() {
        let mut client = OAuthClient::empty();
        assert!(!client.has_provider(OAuthProvider::Google));
        client.add_provider(
            OAuthProvider::Google,
            OAuthConfig {
                client_id: "new-id".to_string(),
                client_secret: "new-secret".to_string(),
                redirect_uri: "http://localhost/cb".to_string(),
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
            },
        );
        assert!(client.has_provider(OAuthProvider::Google));
    }

    // ----- Legacy helpers tests -------------------------------------------

    fn test_legacy_config() -> OAuthConfigLegacy {
        OAuthConfigLegacy {
            google_client_id: Some("google-id".to_string()),
            google_client_secret: Some("google-secret".to_string()),
            github_client_id: Some("github-id".to_string()),
            github_client_secret: Some("github-secret".to_string()),
            redirect_uri: "http://localhost:3000/callback".to_string(),
        }
    }

    #[test]
    fn legacy_google_provider_returns_some_when_configured() {
        let config = test_legacy_config();
        let provider = google_provider(&config).expect("should build provider");
        assert_eq!(provider.name, "google");
        assert_eq!(provider.client_id, "google-id");
        assert!(provider.auth_url.contains("accounts.google.com"));
    }

    #[test]
    fn legacy_github_provider_returns_none_when_missing() {
        let config = OAuthConfigLegacy {
            github_client_id: None,
            github_client_secret: None,
            ..test_legacy_config()
        };
        assert!(github_provider(&config).is_none());
    }

    #[test]
    fn legacy_build_auth_url_contains_state_and_client_id() {
        let config = test_legacy_config();
        let provider = google_provider(&config).unwrap();
        let url = build_auth_url(&provider, "random-state");
        assert!(url.contains("client_id=google-id"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn legacy_exchange_code_returns_mock_token() {
        let config = test_legacy_config();
        let provider = github_provider(&config).unwrap();
        let token = exchange_code(&provider, "auth-code-123").unwrap();
        assert!(token.contains("mock_access_token"));
        assert!(token.contains("github"));
    }

    #[test]
    fn legacy_exchange_code_rejects_empty_code() {
        let config = test_legacy_config();
        let provider = google_provider(&config).unwrap();
        let result = exchange_code(&provider, "");
        assert!(result.is_err());
    }
}
