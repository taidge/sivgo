//! Data models for the Sivgo web API.
//!
//! These are the API-level types used in request/response bodies and in
//! the in-memory document store. When a database backend is added later,
//! these will map to Diesel models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Domain models
// ---------------------------------------------------------------------------

/// A registered user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    /// Never serialized to JSON responses.
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

/// A stored SVG document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub svg_content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A snapshot of a document at a particular point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version: i32,
    pub svg_content: String,
    pub created_at: DateTime<Utc>,
}

/// A share link or direct share for a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentShare {
    pub id: Uuid,
    pub document_id: Uuid,
    /// `None` means the share is a public link (anyone with the token).
    pub shared_with: Option<Uuid>,
    pub permission: SharePermission,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

/// Permission level for a document share.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SharePermission {
    View,
    Edit,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Body for `POST /api/v1/auth/register`.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

/// Body for `POST /api/v1/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Body for `POST /api/v1/documents`.
#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub title: String,
    pub svg_content: Option<String>,
}

/// Body for `PUT /api/v1/documents/:id`.
#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub svg_content: Option<String>,
}

/// Body for `POST /api/v1/export`.
#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub svg_content: String,
    pub format: ExportFormat,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Supported export formats.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Svg,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response for successful authentication.
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

/// Generic API error response body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

impl ErrorBody {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { error: msg.into() }
    }
}
