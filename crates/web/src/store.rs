//! In-memory document store.
//!
//! This is a simple, thread-safe store backed by `RwLock<HashMap>`.  It is
//! intended for development and testing.  A production deployment would
//! replace this with a Diesel + PostgreSQL backend.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::Utc;
use uuid::Uuid;

use crate::models::*;

/// Thread-safe, in-memory store for users, documents, versions, and shares.
///
/// All methods that read or write use interior mutability via `RwLock`, so
/// the store can be shared across async tasks via `Arc` or injected into
/// Salvo's `Depot`.
#[derive(Debug, Clone)]
pub struct Store {
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    documents: Arc<RwLock<HashMap<Uuid, Document>>>,
    versions: Arc<RwLock<HashMap<Uuid, Vec<DocumentVersion>>>>,
    shares: Arc<RwLock<HashMap<Uuid, Vec<DocumentShare>>>>,
}

impl Store {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
            versions: Arc::new(RwLock::new(HashMap::new())),
            shares: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ----- User operations ------------------------------------------------

    /// Insert a new user and return it.
    pub fn create_user(&self, email: &str, username: &str, password_hash: &str) -> User {
        let user = User {
            id: Uuid::new_v4(),
            email: email.to_owned(),
            username: username.to_owned(),
            password_hash: password_hash.to_owned(),
            created_at: Utc::now(),
        };
        self.users.write().unwrap().insert(user.id, user.clone());
        user
    }

    /// Look up a user by email.
    pub fn find_user_by_email(&self, email: &str) -> Option<User> {
        self.users
            .read()
            .unwrap()
            .values()
            .find(|u| u.email == email)
            .cloned()
    }

    /// Look up a user by id.
    pub fn find_user_by_id(&self, id: Uuid) -> Option<User> {
        self.users.read().unwrap().get(&id).cloned()
    }

    // ----- Document operations --------------------------------------------

    /// Create a new document owned by `owner_id`.
    pub fn create_document(&self, owner_id: Uuid, title: &str, svg_content: &str) -> Document {
        let now = Utc::now();
        let doc = Document {
            id: Uuid::new_v4(),
            owner_id,
            title: title.to_owned(),
            svg_content: svg_content.to_owned(),
            created_at: now,
            updated_at: now,
        };
        self.documents.write().unwrap().insert(doc.id, doc.clone());
        doc
    }

    /// Get a document by id.
    pub fn get_document(&self, id: Uuid) -> Option<Document> {
        self.documents.read().unwrap().get(&id).cloned()
    }

    /// Update a document.  Only the fields that are `Some` are changed.
    pub fn update_document(
        &self,
        id: Uuid,
        title: Option<&str>,
        svg_content: Option<&str>,
    ) -> Option<Document> {
        let mut docs = self.documents.write().unwrap();
        let doc = docs.get_mut(&id)?;
        if let Some(t) = title {
            doc.title = t.to_owned();
        }
        if let Some(c) = svg_content {
            doc.svg_content = c.to_owned();
        }
        doc.updated_at = Utc::now();
        Some(doc.clone())
    }

    /// Delete a document.  Returns `true` if it existed.
    pub fn delete_document(&self, id: Uuid) -> bool {
        self.documents.write().unwrap().remove(&id).is_some()
    }

    /// List all documents owned by a specific user.
    pub fn list_documents(&self, owner_id: Uuid) -> Vec<Document> {
        self.documents
            .read()
            .unwrap()
            .values()
            .filter(|d| d.owner_id == owner_id)
            .cloned()
            .collect()
    }

    // ----- Version operations ---------------------------------------------

    /// Save a version snapshot for a document.
    pub fn save_version(&self, document_id: Uuid, svg_content: &str) -> DocumentVersion {
        let mut versions = self.versions.write().unwrap();
        let list = versions.entry(document_id).or_default();
        let next_ver = list.len() as i32 + 1;
        let version = DocumentVersion {
            id: Uuid::new_v4(),
            document_id,
            version: next_ver,
            svg_content: svg_content.to_owned(),
            created_at: Utc::now(),
        };
        list.push(version.clone());
        version
    }

    /// Get all versions for a document, ordered by version number.
    pub fn get_versions(&self, document_id: Uuid) -> Vec<DocumentVersion> {
        self.versions
            .read()
            .unwrap()
            .get(&document_id)
            .cloned()
            .unwrap_or_default()
    }

    // ----- Share operations -----------------------------------------------

    /// Create a share record for a document.
    pub fn create_share(
        &self,
        document_id: Uuid,
        shared_with: Option<Uuid>,
        permission: SharePermission,
    ) -> DocumentShare {
        let token = Uuid::new_v4().to_string();
        let share = DocumentShare {
            id: Uuid::new_v4(),
            document_id,
            shared_with,
            permission,
            token,
            created_at: Utc::now(),
        };
        let mut shares = self.shares.write().unwrap();
        shares.entry(document_id).or_default().push(share.clone());
        share
    }

    /// Get all shares for a document.
    pub fn get_shares(&self, document_id: Uuid) -> Vec<DocumentShare> {
        self.shares
            .read()
            .unwrap()
            .get(&document_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Find a share by its unique token string.
    pub fn find_share_by_token(&self, token: &str) -> Option<DocumentShare> {
        self.shares
            .read()
            .unwrap()
            .values()
            .flatten()
            .find(|s| s.token == token)
            .cloned()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
