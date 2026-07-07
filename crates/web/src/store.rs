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
        created_by: Uuid,
        shared_with: Option<Uuid>,
        permission: SharePermission,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> DocumentShare {
        let token = Uuid::new_v4().to_string();
        let share = DocumentShare {
            id: Uuid::new_v4(),
            document_id,
            created_by,
            shared_with,
            permission,
            token,
            created_at: Utc::now(),
            expires_at,
            revoked_at: None,
            last_accessed_at: None,
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

    /// Find an active share by its unique token string and record access time.
    pub fn find_active_share_by_token(&self, token: &str) -> Option<DocumentShare> {
        let now = Utc::now();
        let mut shares = self.shares.write().unwrap();
        for share in shares.values_mut().flatten() {
            if share.token != token || !share.is_active_at(now) {
                continue;
            }
            share.last_accessed_at = Some(now);
            return Some(share.clone());
        }
        None
    }

    /// Revoke a share without deleting its audit record.
    pub fn revoke_share(&self, document_id: Uuid, share_id: Uuid) -> Option<DocumentShare> {
        let mut shares = self.shares.write().unwrap();
        let doc_shares = shares.get_mut(&document_id)?;
        let share = doc_shares.iter_mut().find(|share| share.id == share_id)?;
        if share.revoked_at.is_none() {
            share.revoked_at = Some(Utc::now());
        }
        Some(share.clone())
    }
}

impl DocumentShare {
    fn is_active_at(&self, now: chrono::DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && self.expires_at.is_none_or(|expires_at| expires_at > now)
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn active_share_resolves_and_records_access() {
        let store = Store::new();
        let doc_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let share = store.create_share(
            doc_id,
            owner_id,
            None,
            SharePermission::View,
            Some(Utc::now() + Duration::minutes(5)),
        );

        let resolved = store.find_active_share_by_token(&share.token).unwrap();

        assert_eq!(resolved.id, share.id);
        assert_eq!(resolved.created_by, owner_id);
        assert!(resolved.last_accessed_at.is_some());
    }

    #[test]
    fn expired_share_does_not_resolve() {
        let store = Store::new();
        let share = store.create_share(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            SharePermission::View,
            Some(Utc::now() - Duration::seconds(1)),
        );

        assert!(store.find_active_share_by_token(&share.token).is_none());
    }

    #[test]
    fn revoked_share_does_not_resolve_but_stays_listed() {
        let store = Store::new();
        let doc_id = Uuid::new_v4();
        let share = store.create_share(doc_id, Uuid::new_v4(), None, SharePermission::View, None);

        let revoked = store.revoke_share(doc_id, share.id).unwrap();

        assert!(revoked.revoked_at.is_some());
        assert!(store.find_active_share_by_token(&share.token).is_none());
        assert_eq!(store.get_shares(doc_id).len(), 1);
    }
}
