//! Asset upload and download management.
//!
//! Provides an in-memory asset store for binary file uploads (images, fonts,
//! etc.) that are associated with a user account.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A stored binary asset (image, font, etc.).
#[derive(Debug, Clone)]
pub struct Asset {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    /// The raw binary data.
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

/// Lightweight metadata for an asset, suitable for API responses.
///
/// Does *not* contain the binary data itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetInfo {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    /// Download URL for the asset (relative to the API root).
    pub url: String,
}

impl AssetInfo {
    /// Build an `AssetInfo` from a full `Asset`, generating the download URL.
    pub fn from_asset(asset: &Asset) -> Self {
        Self {
            id: asset.id,
            filename: asset.filename.clone(),
            content_type: asset.content_type.clone(),
            size_bytes: asset.size_bytes,
            created_at: asset.created_at,
            url: format!("/api/v1/assets/{}", asset.id),
        }
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Thread-safe, in-memory asset store.
///
/// Uses `Arc<RwLock<..>>` so it can be cheaply cloned and shared across
/// async tasks and injected into Salvo's `Depot`.
#[derive(Debug, Clone)]
pub struct AssetStore {
    assets: Arc<RwLock<HashMap<Uuid, Asset>>>,
}

impl AssetStore {
    /// Create a new, empty asset store.
    pub fn new() -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Upload a new asset and return its metadata.
    pub fn upload_asset(
        &self,
        owner_id: Uuid,
        filename: &str,
        content_type: &str,
        data: Vec<u8>,
    ) -> AssetInfo {
        let size_bytes = data.len() as u64;
        let asset = Asset {
            id: Uuid::new_v4(),
            owner_id,
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            size_bytes,
            data,
            created_at: Utc::now(),
        };
        let info = AssetInfo::from_asset(&asset);
        self.assets.write().unwrap().insert(asset.id, asset);
        info
    }

    /// Download (retrieve) an asset by id.
    ///
    /// Returns the binary data and the content type, or `None` if the asset
    /// does not exist.
    pub fn download_asset(&self, id: Uuid) -> Option<(Vec<u8>, String)> {
        let assets = self.assets.read().unwrap();
        let asset = assets.get(&id)?;
        Some((asset.data.clone(), asset.content_type.clone()))
    }

    /// List all assets owned by a specific user.
    pub fn list_assets(&self, owner_id: Uuid) -> Vec<AssetInfo> {
        self.assets
            .read()
            .unwrap()
            .values()
            .filter(|a| a.owner_id == owner_id)
            .map(AssetInfo::from_asset)
            .collect()
    }

    /// Delete an asset by id. Returns `Ok(())` if it existed, or an error
    /// message if it was not found.
    pub fn delete_asset(&self, id: Uuid) -> Result<(), String> {
        if self.assets.write().unwrap().remove(&id).is_some() {
            Ok(())
        } else {
            Err(format!("asset {} not found", id))
        }
    }

    // ------------------------------------------------------------------
    // Legacy compat helpers (string-based IDs)
    // ------------------------------------------------------------------

    /// Upload using string-based owner id (legacy compatibility).
    pub fn upload(
        &self,
        owner_id: &str,
        filename: &str,
        content_type: &str,
        data: Vec<u8>,
    ) -> String {
        let uid = Uuid::parse_str(owner_id).unwrap_or_else(|_| Uuid::new_v4());
        let info = self.upload_asset(uid, filename, content_type, data);
        info.id.to_string()
    }

    /// Download using string-based id (legacy compatibility).
    pub fn download(&self, id: &str) -> Option<Asset> {
        let uid = Uuid::parse_str(id).ok()?;
        self.assets.read().unwrap().get(&uid).cloned()
    }

    /// Delete using string-based id (legacy compatibility).
    pub fn delete(&self, id: &str) -> bool {
        let uid = match Uuid::parse_str(id) {
            Ok(u) => u,
            Err(_) => return false,
        };
        self.assets.write().unwrap().remove(&uid).is_some()
    }

    /// List using string-based owner id (legacy compatibility).
    pub fn list(&self, owner_id: &str) -> Vec<AssetInfo> {
        let uid = Uuid::parse_str(owner_id).unwrap_or_else(|_| Uuid::nil());
        self.list_assets(uid)
    }
}

impl Default for AssetStore {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_and_download_asset() {
        let store = AssetStore::new();
        let owner = Uuid::new_v4();
        let data = b"hello world".to_vec();
        let info = store.upload_asset(owner, "test.txt", "text/plain", data.clone());

        assert_eq!(info.filename, "test.txt");
        assert_eq!(info.content_type, "text/plain");
        assert_eq!(info.size_bytes, 11);
        assert!(info.url.contains(&info.id.to_string()));

        let (downloaded_data, ct) = store.download_asset(info.id).expect("should exist");
        assert_eq!(downloaded_data, data);
        assert_eq!(ct, "text/plain");
    }

    #[test]
    fn download_nonexistent_returns_none() {
        let store = AssetStore::new();
        assert!(store.download_asset(Uuid::new_v4()).is_none());
    }

    #[test]
    fn delete_asset_ok_and_err() {
        let store = AssetStore::new();
        let owner = Uuid::new_v4();
        let info = store.upload_asset(owner, "a.png", "image/png", vec![1, 2, 3]);

        assert!(store.delete_asset(info.id).is_ok());
        assert!(store.download_asset(info.id).is_none());

        // Deleting again returns error.
        assert!(store.delete_asset(info.id).is_err());
    }

    #[test]
    fn list_assets_filters_by_owner() {
        let store = AssetStore::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        store.upload_asset(user1, "a.png", "image/png", vec![1]);
        store.upload_asset(user2, "b.png", "image/png", vec![2]);
        store.upload_asset(user1, "c.png", "image/png", vec![3]);

        let user1_assets = store.list_assets(user1);
        assert_eq!(user1_assets.len(), 2);
        for a in &user1_assets {
            assert!(a.url.starts_with("/api/v1/assets/"));
        }

        let user2_assets = store.list_assets(user2);
        assert_eq!(user2_assets.len(), 1);
        assert_eq!(user2_assets[0].filename, "b.png");
    }

    #[test]
    fn asset_info_has_correct_url() {
        let asset = Asset {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            owner_id: Uuid::new_v4(),
            filename: "test.svg".into(),
            content_type: "image/svg+xml".into(),
            size_bytes: 256,
            data: vec![0; 256],
            created_at: Utc::now(),
        };
        let info = AssetInfo::from_asset(&asset);
        assert_eq!(
            info.url,
            "/api/v1/assets/550e8400-e29b-41d4-a716-446655440000"
        );
    }

    // Legacy compat tests

    #[test]
    fn legacy_upload_and_download() {
        let store = AssetStore::new();
        let uid = Uuid::new_v4();
        let data = b"legacy data".to_vec();
        let id_str = store.upload(&uid.to_string(), "test.txt", "text/plain", data.clone());

        let asset = store.download(&id_str).expect("should exist");
        assert_eq!(asset.filename, "test.txt");
        assert_eq!(asset.data, data);
    }

    #[test]
    fn legacy_delete_removes_asset() {
        let store = AssetStore::new();
        let uid = Uuid::new_v4();
        let id = store.upload(&uid.to_string(), "a.png", "image/png", vec![1, 2, 3]);
        assert!(store.delete(&id));
        assert!(store.download(&id).is_none());
        assert!(!store.delete(&id));
    }

    #[test]
    fn legacy_list_filters_by_owner() {
        let store = AssetStore::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        store.upload(&user1.to_string(), "a.png", "image/png", vec![1]);
        store.upload(&user2.to_string(), "b.png", "image/png", vec![2]);
        store.upload(&user1.to_string(), "c.png", "image/png", vec![3]);

        let user1_assets = store.list(&user1.to_string());
        assert_eq!(user1_assets.len(), 2);

        let user2_assets = store.list(&user2.to_string());
        assert_eq!(user2_assets.len(), 1);
    }
}
