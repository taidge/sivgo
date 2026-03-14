//! Database schema definitions for the Sivgo web backend.
//!
//! These structs model the database tables that a production deployment would
//! use with Diesel + PostgreSQL.  For now they serve as canonical data models
//! that the rest of the codebase can reference.  When Diesel is wired up,
//! the `#[derive(Queryable, Insertable, ...)]` macros can be added directly
//! to these types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

/// A registered user account.
///
/// Maps to the `users` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    /// Argon2 (or bcrypt) hash -- never serialised to API responses.
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// An SVG document owned by a user.
///
/// Maps to the `documents` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Document {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    /// The raw SVG markup.
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A versioned snapshot of a document.
///
/// Maps to the `document_versions` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version_number: i32,
    /// The SVG content at this version.
    pub content: String,
    pub created_at: DateTime<Utc>,
    /// Optional human-readable commit message.
    pub message: Option<String>,
}

/// Permission level for a document share.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SharePermission {
    Read,
    Write,
}

/// A share record that grants access to a document.
///
/// Maps to the `document_shares` table.
///
/// A share can be either:
/// - **Direct**: `shared_with_user_id` is `Some(user_id)` -- the document is
///   shared with a specific user.
/// - **Link-based**: `link_token` is `Some(token)` -- anyone with the token
///   can access the document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentShare {
    pub id: Uuid,
    pub document_id: Uuid,
    /// The user this document is shared with, if this is a direct share.
    pub shared_with_user_id: Option<Uuid>,
    /// A unique token for link-based sharing.
    pub link_token: Option<String>,
    pub permission: SharePermission,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// A binary asset (image, font, etc.) uploaded by a user.
///
/// Maps to the `assets` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    /// The raw binary data.  Stored as a `bytea` column in PostgreSQL.
    #[serde(skip_serializing)]
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

/// A plugin available in the registry.
///
/// Maps to the `plugins` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plugin {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

/// A record of a user installing a plugin.
///
/// Maps to the `plugin_installations` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginInstallation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub plugin_id: Uuid,
    pub installed_at: DateTime<Utc>,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// User preferences
// ---------------------------------------------------------------------------

/// A single user preference key-value pair.
///
/// Maps to the `user_preferences` table.  The composite primary key is
/// `(user_id, key)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserPreference {
    pub user_id: Uuid,
    pub key: String,
    pub value: String,
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_password_hash_is_not_serialised() {
        let user = User {
            id: Uuid::new_v4(),
            username: "alice".into(),
            email: "alice@example.com".into(),
            password_hash: "secret-hash".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(!json.contains("secret-hash"));
        assert!(json.contains("alice"));
    }

    #[test]
    fn document_roundtrip() {
        let now = Utc::now();
        let doc = Document {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            title: "My Drawing".into(),
            content: "<svg></svg>".into(),
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "My Drawing");
        assert_eq!(parsed.content, "<svg></svg>");
    }

    #[test]
    fn document_version_with_message() {
        let ver = DocumentVersion {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            version_number: 3,
            content: "<svg><rect/></svg>".into(),
            created_at: Utc::now(),
            message: Some("Added rectangle".into()),
        };
        let json = serde_json::to_string(&ver).unwrap();
        assert!(json.contains("Added rectangle"));

        let ver_no_msg = DocumentVersion {
            message: None,
            ..ver.clone()
        };
        let json2 = serde_json::to_string(&ver_no_msg).unwrap();
        assert!(json2.contains("\"message\":null"));
    }

    #[test]
    fn share_permission_variants() {
        let read = SharePermission::Read;
        let write = SharePermission::Write;
        assert_ne!(read, write);

        let json = serde_json::to_string(&read).unwrap();
        assert_eq!(json, "\"Read\"");
    }

    #[test]
    fn document_share_direct_vs_link() {
        let direct = DocumentShare {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            shared_with_user_id: Some(Uuid::new_v4()),
            link_token: None,
            permission: SharePermission::Write,
            created_at: Utc::now(),
        };
        assert!(direct.shared_with_user_id.is_some());
        assert!(direct.link_token.is_none());

        let link = DocumentShare {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            shared_with_user_id: None,
            link_token: Some("abc123".into()),
            permission: SharePermission::Read,
            created_at: Utc::now(),
        };
        assert!(link.shared_with_user_id.is_none());
        assert!(link.link_token.is_some());
    }

    #[test]
    fn asset_data_is_not_serialised() {
        let asset = Asset {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            filename: "logo.png".into(),
            content_type: "image/png".into(),
            size_bytes: 1024,
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&asset).unwrap();
        assert!(json.contains("logo.png"));
        // Binary data should not appear in JSON output.
        assert!(!json.contains("DEAD"));
    }

    #[test]
    fn plugin_and_installation() {
        let plugin = Plugin {
            id: Uuid::new_v4(),
            name: "Shape Library".into(),
            version: "1.2.0".into(),
            description: "Extra shape templates".into(),
            author: "Sivgo Team".into(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        assert!(json.contains("Shape Library"));

        let installation = PluginInstallation {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            plugin_id: plugin.id,
            installed_at: Utc::now(),
            enabled: true,
        };
        assert!(installation.enabled);
    }

    #[test]
    fn user_preference_key_value() {
        let pref = UserPreference {
            user_id: Uuid::new_v4(),
            key: "theme".into(),
            value: "dark".into(),
        };
        let json = serde_json::to_string(&pref).unwrap();
        let parsed: UserPreference = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "theme");
        assert_eq!(parsed.value, "dark");
    }
}
