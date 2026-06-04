//! Progressive Web App (PWA) manifest and service worker generation.
//!
//! Provides data models for the PWA manifest and helper methods to generate
//! the JSON manifest and production service worker script.

use serde::{Deserialize, Serialize};

/// An icon entry for the PWA manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwaIcon {
    pub src: String,
    pub sizes: String,
    /// MIME type of the icon image (e.g. `"image/png"`).
    #[serde(rename = "type")]
    pub icon_type: String,
}

/// Configuration for the Sivgo Progressive Web App.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwaConfig {
    /// Full application name.
    pub app_name: String,
    /// Short name for the home screen.
    pub short_name: String,
    /// Application description.
    pub description: String,
    /// Theme colour for the browser chrome.
    pub theme_color: String,
    /// Background colour for the splash screen.
    pub background_color: String,
    /// Display mode (`"standalone"`, `"fullscreen"`, etc.).
    pub display: String,
    /// Start URL when launched from the home screen.
    pub start_url: String,
    pub scope: String,
    pub cache_version: String,
    pub offline_url: String,
    /// Icons at various resolutions.
    pub icons: Vec<PwaIcon>,
}

impl Default for PwaConfig {
    fn default() -> Self {
        Self {
            app_name: "Sivgo".to_string(),
            short_name: "Sivgo".to_string(),
            description: "A collaborative SVG editor for the web".to_string(),
            theme_color: "#1a1a2e".to_string(),
            background_color: "#ffffff".to_string(),
            display: "standalone".to_string(),
            start_url: "/".to_string(),
            scope: "/".to_string(),
            cache_version: "v1".to_string(),
            offline_url: "/offline.html".to_string(),
            icons: vec![
                PwaIcon {
                    src: "/icons/icon-192.png".to_string(),
                    sizes: "192x192".to_string(),
                    icon_type: "image/png".to_string(),
                },
                PwaIcon {
                    src: "/icons/icon-512.png".to_string(),
                    sizes: "512x512".to_string(),
                    icon_type: "image/png".to_string(),
                },
            ],
        }
    }
}

impl PwaConfig {
    /// Create a new config with Sivgo defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an icon to the manifest.
    pub fn add_icon(&mut self, src: &str, sizes: &str, icon_type: &str) {
        self.icons.push(PwaIcon {
            src: src.to_string(),
            sizes: sizes.to_string(),
            icon_type: icon_type.to_string(),
        });
    }

    /// Generate the `manifest.json` content as a JSON string.
    pub fn generate_manifest(&self) -> String {
        let manifest = serde_json::json!({
            "name": self.app_name,
            "short_name": self.short_name,
            "description": self.description,
            "theme_color": self.theme_color,
            "background_color": self.background_color,
            "display": self.display,
            "start_url": self.start_url,
            "scope": self.scope,
            "icons": self.icons.iter().map(|icon| {
                serde_json::json!({
                    "src": icon.src,
                    "sizes": icon.sizes,
                    "type": icon.icon_type,
                })
            }).collect::<Vec<_>>(),
        });
        serde_json::to_string_pretty(&manifest).unwrap_or_default()
    }

    pub fn generate_service_worker(&self) -> String {
        let static_assets = self
            .icons
            .iter()
            .map(|icon| format!("  '{}'", icon.src))
            .chain([
                "  '/'".to_string(),
                "  '/index.html'".to_string(),
                format!("  '{}'", self.offline_url),
                "  '/pkg/sivgo_web_bg.wasm'".to_string(),
                "  '/pkg/sivgo_web.js'".to_string(),
            ])
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            r#"// Sivgo Service Worker (auto-generated)
const CACHE_VERSION = '{}';
const STATIC_CACHE = `sivgo-static-${{CACHE_VERSION}}`;
const RUNTIME_CACHE = `sivgo-runtime-${{CACHE_VERSION}}`;
const OFFLINE_URL = '{}';
const STATIC_ASSETS = [
{}
];

self.addEventListener('install', (event) => {{
  event.waitUntil(
    caches.open(STATIC_CACHE)
      .then((cache) => cache.addAll(STATIC_ASSETS))
      .then(() => self.skipWaiting())
  );
}});

self.addEventListener('activate', (event) => {{
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((key) => ![STATIC_CACHE, RUNTIME_CACHE].includes(key))
          .map((key) => caches.delete(key))
      )
    ).then(() => self.clients.claim())
  );
}});

self.addEventListener('fetch', (event) => {{
  const url = new URL(event.request.url);
  if (url.pathname.startsWith('/api/')) {{
    event.respondWith(
      fetch(event.request)
        .then((response) => {{
          const copy = response.clone();
          caches.open(RUNTIME_CACHE).then((cache) => cache.put(event.request, copy));
          return response;
        }})
        .catch(() => caches.match(event.request))
    );
    return;
  }}

  if (event.request.mode === 'navigate') {{
    event.respondWith(
      fetch(event.request).catch(() => caches.match(OFFLINE_URL))
    );
    return;
  }}

  event.respondWith(
    caches.match(event.request).then((cached) => {{
      if (cached) return cached;
      return fetch(event.request).then((response) => {{
        const copy = response.clone();
        caches.open(RUNTIME_CACHE).then((cache) => cache.put(event.request, copy));
        return response;
      }});
    }})
  );
}});

self.addEventListener('message', (event) => {{
  if (event.data === 'SKIP_WAITING') {{
    self.skipWaiting();
  }}
}});
"#,
            self.cache_version, self.offline_url, static_assets
        )
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = PwaConfig::new();
        assert_eq!(config.app_name, "Sivgo");
        assert_eq!(config.short_name, "Sivgo");
        assert_eq!(config.theme_color, "#1a1a2e");
        assert_eq!(config.background_color, "#ffffff");
        assert_eq!(config.display, "standalone");
        assert_eq!(config.start_url, "/");
        assert_eq!(config.scope, "/");
        assert_eq!(config.offline_url, "/offline.html");
        assert_eq!(config.icons.len(), 2);
    }

    #[test]
    fn add_icon() {
        let mut config = PwaConfig::new();
        config.add_icon("/icons/icon-128.png", "128x128", "image/png");
        assert_eq!(config.icons.len(), 3);
        assert_eq!(config.icons[2].sizes, "128x128");
    }

    #[test]
    fn generate_manifest_is_valid_json() {
        let config = PwaConfig::new();
        let json = config.generate_manifest();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["name"], "Sivgo");
        assert_eq!(parsed["short_name"], "Sivgo");
        assert_eq!(parsed["theme_color"], "#1a1a2e");
        assert_eq!(parsed["display"], "standalone");
        assert_eq!(parsed["scope"], "/");
        assert!(parsed["icons"].is_array());
        assert_eq!(parsed["icons"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn manifest_contains_icon_details() {
        let config = PwaConfig::new();
        let json = config.generate_manifest();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let icon = &parsed["icons"][0];
        assert_eq!(icon["src"], "/icons/icon-192.png");
        assert_eq!(icon["sizes"], "192x192");
        assert_eq!(icon["type"], "image/png");
    }

    #[test]
    fn generate_service_worker_contains_cache() {
        let config = PwaConfig::new();
        let sw = config.generate_service_worker();
        assert!(sw.contains("STATIC_CACHE"));
        assert!(sw.contains("RUNTIME_CACHE"));
        assert!(sw.contains("addEventListener"));
        assert!(sw.contains("install"));
        assert!(sw.contains("fetch"));
    }

    #[test]
    fn service_worker_has_api_strategy() {
        let config = PwaConfig::new();
        let sw = config.generate_service_worker();
        assert!(sw.contains("/api/"));
        assert!(sw.contains("OFFLINE_URL"));
        assert!(sw.contains("SKIP_WAITING"));
    }
}
