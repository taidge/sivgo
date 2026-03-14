//! Progressive Web App (PWA) manifest and service worker generation.
//!
//! Provides data models for the PWA manifest and helper methods to
//! generate the JSON manifest and a stub service worker script.

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

    /// Generate a stub service worker script.
    ///
    /// The generated JavaScript implements a basic cache-first strategy
    /// for static assets and a network-first strategy for API calls.
    pub fn generate_service_worker(&self) -> String {
        format!(
            r#"// Sivgo Service Worker (auto-generated stub)
const CACHE_NAME = 'sivgo-cache-v1';
const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/pkg/sivgo_web_bg.wasm',
  '/pkg/sivgo_web.js',
  '/icons/icon-192.png',
  '/icons/icon-512.png',
];

self.addEventListener('install', (event) => {{
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS))
  );
}});

self.addEventListener('activate', (event) => {{
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    )
  );
}});

self.addEventListener('fetch', (event) => {{
  const url = new URL(event.request.url);
  if (url.pathname.startsWith('/api/')) {{
    // Network-first for API calls.
    event.respondWith(
      fetch(event.request).catch(() => caches.match(event.request))
    );
  }} else {{
    // Cache-first for static assets.
    event.respondWith(
      caches.match(event.request).then((cached) => cached || fetch(event.request))
    );
  }}
}});
"#
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
        assert!(sw.contains("CACHE_NAME"));
        assert!(sw.contains("sivgo-cache-v1"));
        assert!(sw.contains("addEventListener"));
        assert!(sw.contains("install"));
        assert!(sw.contains("fetch"));
    }

    #[test]
    fn service_worker_has_api_strategy() {
        let config = PwaConfig::new();
        let sw = config.generate_service_worker();
        assert!(sw.contains("/api/"));
        assert!(sw.contains("Network-first"));
        assert!(sw.contains("Cache-first"));
    }
}
