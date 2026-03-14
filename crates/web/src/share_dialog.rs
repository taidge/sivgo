//! Share dialog state and permission models.
//!
//! Manages the state of the document-sharing dialog, including the list
//! of users with access, their permission levels, and link generation.

use serde::{Deserialize, Serialize};

/// Permission level for shared access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareLevel {
    View,
    Edit,
    Admin,
}

impl std::fmt::Display for ShareLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::View => write!(f, "View"),
            Self::Edit => write!(f, "Edit"),
            Self::Admin => write!(f, "Admin"),
        }
    }
}

/// A single share permission entry (one user's access to a document).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePermission {
    pub user_email: String,
    pub level: ShareLevel,
    /// ISO 8601 timestamp of when the permission was granted.
    pub added_at: String,
}

/// State of the share dialog.
#[derive(Debug, Clone)]
pub struct ShareDialogState {
    /// The id of the document being shared.
    pub document_id: String,
    /// A generated share link, if one has been created.
    pub share_link: Option<String>,
    /// Current permissions for the document.
    pub permissions: Vec<SharePermission>,
    /// Email address being typed into the "add user" field.
    pub new_email: String,
}

impl ShareDialogState {
    /// Create a new share dialog state for the given document.
    pub fn new(document_id: impl Into<String>) -> Self {
        Self {
            document_id: document_id.into(),
            share_link: None,
            permissions: Vec::new(),
            new_email: String::new(),
        }
    }

    /// Generate a shareable link for the document.
    ///
    /// In a real implementation this would call the server to create a
    /// token-based link.  The stub generates a deterministic URL.
    pub fn generate_link(&mut self) -> String {
        let link = format!("https://sivgo.app/share/{}", self.document_id);
        self.share_link = Some(link.clone());
        link
    }

    /// Add a user with the given permission level.
    ///
    /// If the user already has a permission entry, their level is updated.
    pub fn add_user(&mut self, email: &str, level: ShareLevel, timestamp: &str) {
        if let Some(existing) = self
            .permissions
            .iter_mut()
            .find(|p| p.user_email == email)
        {
            existing.level = level;
        } else {
            self.permissions.push(SharePermission {
                user_email: email.to_string(),
                level,
                added_at: timestamp.to_string(),
            });
        }
    }

    /// Remove a user's permission entry.
    ///
    /// Returns `true` if the user was found and removed.
    pub fn remove_user(&mut self, email: &str) -> bool {
        let before = self.permissions.len();
        self.permissions.retain(|p| p.user_email != email);
        self.permissions.len() < before
    }

    /// Update an existing user's permission level.
    ///
    /// Returns `true` if the user was found and updated.
    pub fn update_level(&mut self, email: &str, level: ShareLevel) -> bool {
        if let Some(perm) = self
            .permissions
            .iter_mut()
            .find(|p| p.user_email == email)
        {
            perm.level = level;
            true
        } else {
            false
        }
    }

    /// Return the number of users the document is shared with.
    pub fn shared_count(&self) -> usize {
        self.permissions.len()
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_dialog_is_empty() {
        let state = ShareDialogState::new("doc-1");
        assert_eq!(state.document_id, "doc-1");
        assert!(state.share_link.is_none());
        assert!(state.permissions.is_empty());
        assert!(state.new_email.is_empty());
    }

    #[test]
    fn generate_link() {
        let mut state = ShareDialogState::new("doc-42");
        let link = state.generate_link();
        assert_eq!(link, "https://sivgo.app/share/doc-42");
        assert_eq!(state.share_link.as_deref(), Some(link.as_str()));
    }

    #[test]
    fn add_user() {
        let mut state = ShareDialogState::new("doc-1");
        state.add_user("alice@example.com", ShareLevel::Edit, "2025-12-01T00:00:00Z");
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.permissions[0].user_email, "alice@example.com");
        assert_eq!(state.permissions[0].level, ShareLevel::Edit);
    }

    #[test]
    fn add_user_updates_existing() {
        let mut state = ShareDialogState::new("doc-1");
        state.add_user("alice@example.com", ShareLevel::View, "2025-12-01T00:00:00Z");
        state.add_user("alice@example.com", ShareLevel::Admin, "2025-12-02T00:00:00Z");
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.permissions[0].level, ShareLevel::Admin);
    }

    #[test]
    fn remove_user() {
        let mut state = ShareDialogState::new("doc-1");
        state.add_user("alice@example.com", ShareLevel::Edit, "2025-12-01T00:00:00Z");
        state.add_user("bob@example.com", ShareLevel::View, "2025-12-01T00:00:00Z");
        assert!(state.remove_user("alice@example.com"));
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.permissions[0].user_email, "bob@example.com");
    }

    #[test]
    fn remove_nonexistent_user_returns_false() {
        let mut state = ShareDialogState::new("doc-1");
        assert!(!state.remove_user("nobody@example.com"));
    }

    #[test]
    fn update_level() {
        let mut state = ShareDialogState::new("doc-1");
        state.add_user("alice@example.com", ShareLevel::View, "2025-12-01T00:00:00Z");
        assert!(state.update_level("alice@example.com", ShareLevel::Edit));
        assert_eq!(state.permissions[0].level, ShareLevel::Edit);
    }

    #[test]
    fn update_level_nonexistent_returns_false() {
        let mut state = ShareDialogState::new("doc-1");
        assert!(!state.update_level("nobody@example.com", ShareLevel::Edit));
    }

    #[test]
    fn share_level_display() {
        assert_eq!(ShareLevel::View.to_string(), "View");
        assert_eq!(ShareLevel::Edit.to_string(), "Edit");
        assert_eq!(ShareLevel::Admin.to_string(), "Admin");
    }
}
