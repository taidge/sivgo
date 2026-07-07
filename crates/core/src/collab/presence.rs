//! User presence tracking for real-time collaboration.
//!
//! Tracks cursor positions, selections, and online status of every user in a
//! collaboration session so that the UI can render cursor overlays and show
//! who is working on what.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// UserStatus
// ---------------------------------------------------------------------------

/// Online status of a collaborating user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Online,
    Away,
    Offline,
}

// ---------------------------------------------------------------------------
// UserPresence
// ---------------------------------------------------------------------------

/// Full presence state for a single collaborating user.
#[derive(Debug, Clone, PartialEq)]
pub struct UserPresence {
    /// Unique user identifier.
    pub user_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// User's assigned colour (hex string, e.g. `"#e91e63"`).
    pub color: String,
    /// Current cursor position in document coordinates, if known.
    pub cursor_position: Option<(f64, f64)>,
    /// Node ids the user currently has selected.
    pub selected_nodes: Vec<String>,
    /// Unix-epoch timestamp (seconds) of the last activity from this user.
    pub last_seen: u64,
    /// Current online status.
    pub status: UserStatus,
}

impl UserPresence {
    /// Create a new presence entry with defaults.
    pub fn new(user_id: &str, display_name: &str, color: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            display_name: display_name.to_string(),
            color: color.to_string(),
            cursor_position: None,
            selected_nodes: Vec::new(),
            last_seen: 0,
            status: UserStatus::Online,
        }
    }
}

// ---------------------------------------------------------------------------
// CursorOverlay
// ---------------------------------------------------------------------------

/// Describes a cursor overlay to be rendered in the editor canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorOverlay {
    /// Position in document coordinates.
    pub position: (f64, f64),
    /// Colour of the cursor marker.
    pub color: String,
    /// Label to display next to the cursor (usually the user's display name).
    pub label: String,
}

// ---------------------------------------------------------------------------
// PresenceManager
// ---------------------------------------------------------------------------

/// Manages presence state for all users in a collaboration session.
#[derive(Debug)]
pub struct PresenceManager {
    /// Map of user_id to their presence state.
    users: HashMap<String, UserPresence>,
    /// The local user's identifier.
    self_id: String,
}

impl PresenceManager {
    /// Create a new presence manager for the given local user.
    pub fn new(self_id: &str, display_name: &str, color: &str) -> Self {
        let mut users = HashMap::new();
        users.insert(
            self_id.to_string(),
            UserPresence::new(self_id, display_name, color),
        );
        Self {
            users,
            self_id: self_id.to_string(),
        }
    }

    /// Update the local user's cursor position and selection.
    pub fn update_self(&mut self, cursor: Option<(f64, f64)>, selection: Vec<String>) {
        if let Some(presence) = self.users.get_mut(&self.self_id) {
            presence.cursor_position = cursor;
            presence.selected_nodes = selection;
        }
    }

    /// Update (or insert) presence data received from a remote peer.
    pub fn update_peer(&mut self, user_id: &str, presence: UserPresence) {
        self.users.insert(user_id.to_string(), presence);
    }

    /// Remove a peer from the presence map (e.g. they disconnected).
    pub fn remove_peer(&mut self, user_id: &str) {
        if user_id != self.self_id {
            self.users.remove(user_id);
        }
    }

    /// Return references to all peers (excluding the local user).
    pub fn get_all_peers(&self) -> Vec<&UserPresence> {
        self.users
            .values()
            .filter(|p| p.user_id != self.self_id)
            .collect()
    }

    /// Build cursor overlays for all peers that have a known cursor position.
    pub fn get_cursor_overlays(&self) -> Vec<CursorOverlay> {
        self.users
            .values()
            .filter(|p| p.user_id != self.self_id)
            .filter_map(|p| {
                p.cursor_position.map(|pos| CursorOverlay {
                    position: pos,
                    color: p.color.clone(),
                    label: p.display_name.clone(),
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_presence_manager() {
        let pm = PresenceManager::new("alice", "Alice", "#e91e63");
        assert_eq!(pm.self_id, "alice");
        assert!(pm.users.contains_key("alice"));
        assert!(pm.get_all_peers().is_empty());
    }

    #[test]
    fn test_update_self() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        pm.update_self(Some((100.0, 200.0)), vec!["node-1".into()]);
        let self_p = pm.users.get("alice").unwrap();
        assert_eq!(self_p.cursor_position, Some((100.0, 200.0)));
        assert_eq!(self_p.selected_nodes, vec!["node-1".to_string()]);
    }

    #[test]
    fn test_update_peer() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        let bob = UserPresence {
            user_id: "bob".into(),
            display_name: "Bob".into(),
            color: "#2196f3".into(),
            cursor_position: Some((50.0, 75.0)),
            selected_nodes: vec![],
            last_seen: 1000,
            status: UserStatus::Online,
        };
        pm.update_peer("bob", bob);
        assert_eq!(pm.get_all_peers().len(), 1);
        assert_eq!(pm.get_all_peers()[0].user_id, "bob");
    }

    #[test]
    fn test_remove_peer() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        pm.update_peer("bob", UserPresence::new("bob", "Bob", "#2196f3"));
        assert_eq!(pm.get_all_peers().len(), 1);
        pm.remove_peer("bob");
        assert!(pm.get_all_peers().is_empty());
    }

    #[test]
    fn test_remove_self_ignored() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        pm.remove_peer("alice");
        // Self should not be removed.
        assert!(pm.users.contains_key("alice"));
    }

    #[test]
    fn test_get_cursor_overlays() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        let mut bob = UserPresence::new("bob", "Bob", "#2196f3");
        bob.cursor_position = Some((10.0, 20.0));
        pm.update_peer("bob", bob);

        let mut charlie = UserPresence::new("charlie", "Charlie", "#4caf50");
        charlie.cursor_position = None; // no cursor
        pm.update_peer("charlie", charlie);

        let overlays = pm.get_cursor_overlays();
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].label, "Bob");
        assert_eq!(overlays[0].position, (10.0, 20.0));
        assert_eq!(overlays[0].color, "#2196f3");
    }

    #[test]
    fn test_get_cursor_overlays_excludes_self() {
        let mut pm = PresenceManager::new("alice", "Alice", "#e91e63");
        pm.update_self(Some((100.0, 200.0)), vec![]);
        let overlays = pm.get_cursor_overlays();
        assert!(overlays.is_empty());
    }

    #[test]
    fn test_user_presence_new() {
        let p = UserPresence::new("test", "Test User", "#ff0000");
        assert_eq!(p.user_id, "test");
        assert_eq!(p.display_name, "Test User");
        assert_eq!(p.color, "#ff0000");
        assert_eq!(p.cursor_position, None);
        assert!(p.selected_nodes.is_empty());
        assert_eq!(p.status, UserStatus::Online);
    }

    #[test]
    fn test_user_status_variants() {
        assert_ne!(UserStatus::Online, UserStatus::Away);
        assert_ne!(UserStatus::Away, UserStatus::Offline);
        assert_ne!(UserStatus::Online, UserStatus::Offline);
    }

    #[test]
    fn test_cursor_overlay_clone() {
        let overlay = CursorOverlay {
            position: (1.0, 2.0),
            color: "#abc".into(),
            label: "Test".into(),
        };
        let cloned = overlay.clone();
        assert_eq!(overlay, cloned);
    }
}
