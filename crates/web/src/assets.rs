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

/// Binary asset payload for authenticated downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetDownload {
    pub owner_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
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
        crate::lock::write(&self.assets).insert(asset.id, asset);
        info
    }

    /// Download (retrieve) an asset by id.
    pub fn download_asset(&self, id: Uuid) -> Option<AssetDownload> {
        let assets = crate::lock::read(&self.assets);
        let asset = assets.get(&id)?;
        Some(AssetDownload {
            owner_id: asset.owner_id,
            filename: asset.filename.clone(),
            content_type: asset.content_type.clone(),
            data: asset.data.clone(),
        })
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
        if crate::lock::write(&self.assets).remove(&id).is_some() {
            Ok(())
        } else {
            Err(format!("asset {} not found", id))
        }
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

        let download = store.download_asset(info.id).expect("should exist");
        assert_eq!(download.data, data);
        assert_eq!(download.content_type, "text/plain");
        assert_eq!(download.filename, "test.txt");
        assert_eq!(download.owner_id, owner);
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
}
