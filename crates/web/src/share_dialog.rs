//! Share dialog state and permission models.
//!
//! Manages the state of the document-sharing dialog, including the list
//! of users with access, their permission levels, and link generation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// One user's access to a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecipient {
    pub user_email: String,
    pub level: ShareLevel,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLinkState {
    pub url: String,
    pub token: String,
    pub level: ShareLevel,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

/// State of the share dialog.
#[derive(Debug, Clone)]
pub struct ShareDialogState {
    pub document_id: Uuid,
    pub link_state: Option<ShareLinkState>,
    pub recipients: Vec<ShareRecipient>,
    pub draft_email: String,
}

impl ShareDialogState {
    /// Create a new share dialog state for the given document.
    pub fn new(document_id: Uuid) -> Self {
        Self {
            document_id,
            link_state: None,
            recipients: Vec::new(),
            draft_email: String::new(),
        }
    }

    pub fn generate_link(&mut self) -> String {
        self.generate_link_with_options("https://sivgo.app", ShareLevel::View, None)
    }

    pub fn generate_link_with_options(
        &mut self,
        base_url: &str,
        level: ShareLevel,
        expires_at: Option<DateTime<Utc>>,
    ) -> String {
        let token = Uuid::new_v4().to_string();
        let base = base_url.trim_end_matches('/');
        let url = format!("{base}/share/{token}");
        self.link_state = Some(ShareLinkState {
            url: url.clone(),
            token,
            level,
            expires_at,
            revoked: false,
        });
        url
    }

    pub fn revoke_link(&mut self) -> bool {
        if let Some(link) = &mut self.link_state {
            link.revoked = true;
            true
        } else {
            false
        }
    }

    pub fn active_link_url(&self) -> Option<&str> {
        self.link_state
            .as_ref()
            .filter(|link| !link.revoked)
            .map(|link| link.url.as_str())
    }

    pub fn can_link_edit(&self) -> bool {
        self.link_state
            .as_ref()
            .map(|link| !link.revoked && matches!(link.level, ShareLevel::Edit | ShareLevel::Admin))
            .unwrap_or(false)
    }

    /// Add a user with the given permission level.
    ///
    /// If the user already has a recipient entry, their level is updated.
    pub fn add_user(&mut self, email: &str, level: ShareLevel, granted_at: DateTime<Utc>) {
        if let Some(existing) = self
            .recipients
            .iter_mut()
            .find(|recipient| recipient.user_email == email)
        {
            existing.level = level;
            existing.granted_at = granted_at;
        } else {
            self.recipients.push(ShareRecipient {
                user_email: email.to_string(),
                level,
                granted_at,
            });
        }
    }

    /// Remove a user's access entry.
    pub fn remove_user(&mut self, email: &str) -> bool {
        let before = self.recipients.len();
        self.recipients
            .retain(|recipient| recipient.user_email != email);
        self.recipients.len() < before
    }

    /// Update an existing user's permission level.
    pub fn update_level(&mut self, email: &str, level: ShareLevel) -> bool {
        if let Some(recipient) = self
            .recipients
            .iter_mut()
            .find(|recipient| recipient.user_email == email)
        {
            recipient.level = level;
            true
        } else {
            false
        }
    }

    /// Return the number of users the document is shared with.
    pub fn shared_count(&self) -> usize {
        self.recipients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document_id() -> Uuid {
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    fn timestamp() -> DateTime<Utc> {
        "2025-12-01T00:00:00Z".parse().unwrap()
    }

    #[test]
    fn new_dialog_is_empty() {
        let id = document_id();
        let state = ShareDialogState::new(id);
        assert_eq!(state.document_id, id);
        assert!(state.link_state.is_none());
        assert!(state.recipients.is_empty());
        assert!(state.draft_email.is_empty());
    }

    #[test]
    fn generate_link() {
        let mut state = ShareDialogState::new(document_id());
        let link = state.generate_link();
        assert!(link.starts_with("https://sivgo.app/share/"));
        assert_eq!(state.active_link_url(), Some(link.as_str()));
        assert_eq!(state.link_state.as_ref().unwrap().level, ShareLevel::View);
        assert!(!state.link_state.as_ref().unwrap().token.is_empty());
    }

    #[test]
    fn generate_link_with_options_and_revoke() {
        let mut state = ShareDialogState::new(document_id());
        let expires_at: DateTime<Utc> = "2026-12-01T00:00:00Z".parse().unwrap();
        let link = state.generate_link_with_options(
            "https://example.test/app/",
            ShareLevel::Edit,
            Some(expires_at),
        );
        assert!(link.starts_with("https://example.test/app/share/"));
        assert!(state.can_link_edit());
        assert_eq!(
            state.link_state.as_ref().unwrap().expires_at,
            Some(expires_at)
        );

        assert!(state.revoke_link());
        assert!(state.active_link_url().is_none());
        assert!(state.link_state.as_ref().unwrap().revoked);
        assert!(!state.can_link_edit());
    }

    #[test]
    fn add_user() {
        let mut state = ShareDialogState::new(document_id());
        state.add_user("alice@example.com", ShareLevel::Edit, timestamp());
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.recipients[0].user_email, "alice@example.com");
        assert_eq!(state.recipients[0].level, ShareLevel::Edit);
    }

    #[test]
    fn add_user_updates_existing() {
        let mut state = ShareDialogState::new(document_id());
        state.add_user("alice@example.com", ShareLevel::View, timestamp());
        state.add_user("alice@example.com", ShareLevel::Admin, Utc::now());
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.recipients[0].level, ShareLevel::Admin);
    }

    #[test]
    fn remove_user() {
        let mut state = ShareDialogState::new(document_id());
        state.add_user("alice@example.com", ShareLevel::Edit, timestamp());
        state.add_user("bob@example.com", ShareLevel::View, timestamp());
        assert!(state.remove_user("alice@example.com"));
        assert_eq!(state.shared_count(), 1);
        assert_eq!(state.recipients[0].user_email, "bob@example.com");
    }

    #[test]
    fn remove_nonexistent_user_returns_false() {
        let mut state = ShareDialogState::new(document_id());
        assert!(!state.remove_user("nobody@example.com"));
    }

    #[test]
    fn update_level() {
        let mut state = ShareDialogState::new(document_id());
        state.add_user("alice@example.com", ShareLevel::View, timestamp());
        assert!(state.update_level("alice@example.com", ShareLevel::Edit));
        assert_eq!(state.recipients[0].level, ShareLevel::Edit);
    }

    #[test]
    fn update_level_nonexistent_returns_false() {
        let mut state = ShareDialogState::new(document_id());
        assert!(!state.update_level("nobody@example.com", ShareLevel::Edit));
    }

    #[test]
    fn share_level_display() {
        assert_eq!(ShareLevel::View.to_string(), "View");
        assert_eq!(ShareLevel::Edit.to_string(), "Edit");
        assert_eq!(ShareLevel::Admin.to_string(), "Admin");
    }
}
