//! OAuth2 login support.
//!
//! Provides configuration and helpers for Google and GitHub OAuth2 flows with
//! injectable HTTP transport for token exchange and userinfo retrieval.

use salvo::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

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
    pub fn parse_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" => Some(OAuthProvider::Google),
            "github" => Some(OAuthProvider::GitHub),
            _ => None,
        }
    }

    fn auth_url(self) -> &'static str {
        match self {
            OAuthProvider::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            OAuthProvider::GitHub => "https://github.com/login/oauth/authorize",
        }
    }

    fn token_url(self) -> &'static str {
        match self {
            OAuthProvider::Google => "https://oauth2.googleapis.com/token",
            OAuthProvider::GitHub => "https://github.com/login/oauth/access_token",
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// OAuth2 credentials and endpoint URLs for a single provider.
///
/// Neither `Debug` nor `Serialize` is derived: both would print the client
/// secret, and a config struct ends up in log lines and error reports very
/// easily. `Debug` is implemented by hand below with the secret masked.
#[derive(Clone, Deserialize)]
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

impl std::fmt::Debug for OAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_uri", &self.redirect_uri)
            .field("auth_url", &self.auth_url)
            .field("token_url", &self.token_url)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// An OAuth2 client that holds configurations for multiple providers and
/// exposes methods for the authorization-code flow.
/// Transport used to talk to an OAuth provider.
///
/// The methods are async: they are called from request handlers, and a
/// blocking HTTP client would tie up a runtime worker for the whole round trip
/// (and panics outright if its own runtime is dropped inside an async context).
#[async_trait]
pub trait OAuthHttpClient: Send + Sync {
    async fn post_form(&self, url: &str, fields: &[(&str, String)]) -> Result<String, String>;
    async fn get_bearer(&self, url: &str, access_token: &str) -> Result<String, String>;
}

/// How long to wait on a provider before giving up.
///
/// Without a bound, a slow or unresponsive provider holds the request open for
/// as long as it likes; the callback route is public, so that is worth
/// capping.
const OAUTH_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct ReqwestOAuthHttpClient {
    client: reqwest::Client,
}

impl ReqwestOAuthHttpClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Sivgo/0.1 OAuth client")
            .timeout(OAUTH_HTTP_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for ReqwestOAuthHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OAuthHttpClient for ReqwestOAuthHttpClient {
    async fn post_form(&self, url: &str, fields: &[(&str, String)]) -> Result<String, String> {
        let form = fields
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        self.client
            .post(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|err| format!("OAuth token request failed: {err}"))?
            .error_for_status()
            .map_err(|err| format!("OAuth token request was rejected: {err}"))?
            .text()
            .await
            .map_err(|err| format!("OAuth token response read failed: {err}"))
    }

    async fn get_bearer(&self, url: &str, access_token: &str) -> Result<String, String> {
        self.client
            .get(url)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|err| format!("OAuth userinfo request failed: {err}"))?
            .error_for_status()
            .map_err(|err| format!("OAuth userinfo request was rejected: {err}"))?
            .text()
            .await
            .map_err(|err| format!("OAuth userinfo response read failed: {err}"))
    }
}

#[derive(Clone)]
pub struct OAuthClient {
    configs: HashMap<OAuthProvider, OAuthConfig>,
    http_client: Option<Arc<dyn OAuthHttpClient>>,
}

impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthClient")
            .field("configs", &self.configs.keys().collect::<Vec<_>>())
            .field("http_client_configured", &self.http_client.is_some())
            .finish()
    }
}

impl OAuthClient {
    /// Create a new client with the given provider configurations.
    pub fn new(configs: HashMap<OAuthProvider, OAuthConfig>) -> Self {
        Self {
            configs,
            http_client: None,
        }
    }

    /// Create an empty client (no providers configured).
    pub fn empty() -> Self {
        Self {
            configs: HashMap::new(),
            http_client: None,
        }
    }

    pub fn from_env(http_client: Arc<dyn OAuthHttpClient>) -> Self {
        let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:3000/api/v1/auth/oauth/callback".to_string());
        let mut configs = HashMap::new();

        for provider in [OAuthProvider::Google, OAuthProvider::GitHub] {
            if let Some(config) = provider_config_from_env(provider, &redirect_uri) {
                configs.insert(provider, config);
            }
        }

        Self::new(configs).with_http_client(http_client)
    }

    pub fn with_http_client(mut self, http_client: Arc<dyn OAuthHttpClient>) -> Self {
        self.http_client = Some(http_client);
        self
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
        self.get_auth_url_with_state(provider, "")
    }

    pub fn get_auth_url_with_state(
        &self,
        provider: OAuthProvider,
        state: &str,
    ) -> Result<String, String> {
        let config = self
            .configs
            .get(&provider)
            .ok_or_else(|| format!("Provider {:?} is not configured", provider))?;
        Ok(format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}",
            config.auth_url,
            url_encode(&config.client_id),
            url_encode(&config.redirect_uri),
            url_encode(state),
        ))
    }

    pub async fn exchange_code(
        &self,
        provider: OAuthProvider,
        code: &str,
    ) -> Result<OAuthToken, String> {
        let config = self
            .configs
            .get(&provider)
            .ok_or_else(|| format!("Provider {:?} is not configured", provider))?;

        if code.is_empty() {
            return Err("authorization code must not be empty".to_string());
        }

        let http = self
            .http_client
            .as_ref()
            .ok_or_else(|| "OAuth HTTP client is not configured".to_string())?;
        let body = http
            .post_form(
                &config.token_url,
                &[
                    ("grant_type", "authorization_code".to_string()),
                    ("code", code.to_string()),
                    ("client_id", config.client_id.clone()),
                    ("client_secret", config.client_secret.clone()),
                    ("redirect_uri", config.redirect_uri.clone()),
                ],
            )
            .await?;
        parse_token_response(&body)
    }

    pub async fn get_user_info(
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

        let http = self
            .http_client
            .as_ref()
            .ok_or_else(|| "OAuth HTTP client is not configured".to_string())?;
        let body = http
            .get_bearer(userinfo_url(provider), &token.access_token)
            .await?;
        parse_user_info(provider, &body)
    }
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self::empty()
    }
}

// ---------------------------------------------------------------------------
// State helpers
// ---------------------------------------------------------------------------

pub fn build_state(provider_name: &str, nonce: &str) -> String {
    format!("{}:{}", provider_name, nonce)
}

pub fn provider_name_from_state(state: &str) -> Option<String> {
    let (provider_name, nonce) = state.split_once(':')?;
    if nonce.is_empty() || OAuthProvider::parse_name(provider_name).is_none() {
        return None;
    }
    Some(provider_name.to_string())
}

fn parse_token_response(body: &str) -> Result<OAuthToken, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("invalid token JSON: {err}"))?;
    let access_token = json
        .get("access_token")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "token response missing access_token".to_string())?;
    Ok(OAuthToken {
        access_token: access_token.to_string(),
        refresh_token: json
            .get("refresh_token")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        expires_in: json
            .get("expires_in")
            .and_then(|value| value.as_u64())
            .unwrap_or(3600),
    })
}

fn parse_user_info(provider: OAuthProvider, body: &str) -> Result<OAuthUserInfo, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("invalid userinfo JSON: {err}"))?;
    match provider {
        OAuthProvider::Google => Ok(OAuthUserInfo {
            email: json_string(&json, "email")?,
            name: json_string(&json, "name")?,
            avatar_url: json
                .get("picture")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            provider_user_id: json_string(&json, "sub")?,
        }),
        OAuthProvider::GitHub => {
            let login = json_string(&json, "login")?;
            let id = json
                .get("id")
                .map(|value| value.to_string())
                .ok_or_else(|| "github userinfo missing id".to_string())?;
            Ok(OAuthUserInfo {
                email: json
                    .get("email")
                    .and_then(|value| value.as_str())
                    .filter(|email| !email.is_empty())
                    .map(|email| email.to_string())
                    .unwrap_or_else(|| format!("{login}@users.noreply.github.com")),
                name: json
                    .get("name")
                    .and_then(|value| value.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&login)
                    .to_string(),
                avatar_url: json
                    .get("avatar_url")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string()),
                provider_user_id: id,
            })
        }
    }
}

fn json_string(json: &serde_json::Value, key: &str) -> Result<String, String> {
    json.get(key)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("userinfo missing {key}"))
}

fn userinfo_url(provider: OAuthProvider) -> &'static str {
    match provider {
        OAuthProvider::Google => "https://www.googleapis.com/oauth2/v3/userinfo",
        OAuthProvider::GitHub => "https://api.github.com/user",
    }
}

fn provider_config_from_env(provider: OAuthProvider, redirect_uri: &str) -> Option<OAuthConfig> {
    let prefix = provider.name().to_ascii_uppercase();
    let client_id = env_value(&format!("{prefix}_OAUTH_CLIENT_ID"))?;
    let client_secret = env_value(&format!("{prefix}_OAUTH_CLIENT_SECRET"))?;
    Some(OAuthConfig {
        client_id,
        client_secret,
        redirect_uri: redirect_uri.to_string(),
        auth_url: provider.auth_url().to_string(),
        token_url: provider.token_url().to_string(),
    })
}

fn url_encode(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----- New OAuthClient tests ------------------------------------------

    #[derive(Debug)]
    struct FakeHttpClient;

    #[async_trait]
    impl OAuthHttpClient for FakeHttpClient {
        async fn post_form(&self, _url: &str, fields: &[(&str, String)]) -> Result<String, String> {
            assert!(fields
                .iter()
                .any(|(key, value)| *key == "code" && value == "auth-code-123"));
            Ok(
                r#"{"access_token":"access-123","refresh_token":"refresh-123","expires_in":7200}"#
                    .to_string(),
            )
        }

        async fn get_bearer(&self, url: &str, access_token: &str) -> Result<String, String> {
            assert_eq!(access_token, "access-123");
            if url.contains("googleapis") {
                Ok(r#"{"sub":"google-1","email":"user@gmail.com","name":"Google User","picture":"https://img.test/g.png"}"#.to_string())
            } else {
                Ok(r#"{"id":42,"login":"octo","email":null,"name":"Octo Cat","avatar_url":"https://img.test/gh.png"}"#.to_string())
            }
        }
    }

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
        OAuthClient::new(configs).with_http_client(Arc::new(FakeHttpClient))
    }

    #[test]
    fn provider_from_str() {
        assert_eq!(
            OAuthProvider::parse_name("google"),
            Some(OAuthProvider::Google)
        );
        assert_eq!(
            OAuthProvider::parse_name("GitHub"),
            Some(OAuthProvider::GitHub)
        );
        assert_eq!(
            OAuthProvider::parse_name("GOOGLE"),
            Some(OAuthProvider::Google)
        );
        assert_eq!(OAuthProvider::parse_name("twitter"), None);
    }

    #[test]
    fn provider_name() {
        assert_eq!(OAuthProvider::Google.name(), "google");
        assert_eq!(OAuthProvider::GitHub.name(), "github");
    }

    #[test]
    fn get_auth_url_contains_client_id() {
        let client = make_client();
        let url = client
            .get_auth_url_with_state(OAuthProvider::Google, "state 1")
            .unwrap();
        assert!(url.contains("client_id=google-id"));
        assert!(url.contains("accounts.google.com"));
        assert!(url.contains("state=state%201"));
    }

    #[test]
    fn get_auth_url_unconfigured_provider() {
        let client = OAuthClient::empty();
        let result = client.get_auth_url(OAuthProvider::Google);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn exchange_code_returns_token() {
        let client = make_client();
        let token = client
            .exchange_code(OAuthProvider::GitHub, "auth-code-123")
            .await
            .unwrap();
        assert_eq!(token.access_token, "access-123");
        assert!(token.refresh_token.is_some());
        assert_eq!(token.expires_in, 7200);
    }

    #[tokio::test]
    async fn exchange_code_rejects_empty_code() {
        let client = make_client();
        let result = client.exchange_code(OAuthProvider::Google, "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn exchange_code_requires_http_client() {
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
        let client = OAuthClient::new(configs);
        assert!(client
            .exchange_code(OAuthProvider::Google, "auth-code-123")
            .await
            .unwrap_err()
            .contains("HTTP client"));
    }

    #[tokio::test]
    async fn get_user_info_returns_provider_data() {
        let client = make_client();
        let token = client
            .exchange_code(OAuthProvider::Google, "auth-code-123")
            .await
            .unwrap();
        let info = client
            .get_user_info(OAuthProvider::Google, &token)
            .await
            .unwrap();
        assert_eq!(info.email, "user@gmail.com");
        assert_eq!(info.name, "Google User");
        assert_eq!(info.provider_user_id, "google-1");
    }

    #[tokio::test]
    async fn get_user_info_github_falls_back_to_noreply_email() {
        let client = make_client();
        let token = OAuthToken {
            access_token: "access-123".to_string(),
            refresh_token: None,
            expires_in: 3600,
        };
        let info = client
            .get_user_info(OAuthProvider::GitHub, &token)
            .await
            .unwrap();
        assert_eq!(info.email, "octo@users.noreply.github.com");
        assert!(info.avatar_url.is_some());
    }

    #[tokio::test]
    async fn get_user_info_rejects_empty_token() {
        let client = make_client();
        let token = OAuthToken {
            access_token: "".to_string(),
            refresh_token: None,
            expires_in: 0,
        };
        let result = client.get_user_info(OAuthProvider::Google, &token).await;
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
}
