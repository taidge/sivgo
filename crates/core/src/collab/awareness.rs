//! Awareness protocol for collaboration sessions.
//!
//! Tracks high-level state for each connected user: which page they are
//! viewing, their viewport bounds, and idle status.  This complements the
//! fine-grained [`presence`](super::presence) module with broader context.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// AwarenessState
// ---------------------------------------------------------------------------

/// High-level state of a single user for the awareness protocol.
#[derive(Debug, Clone, PartialEq)]
pub struct AwarenessState {
    /// Unique user identifier.
    pub user_id: String,
    /// Index of the page the user is currently viewing.
    pub viewing_page: usize,
    /// Viewport bounds in document coordinates: (x, y, width, height).
    pub viewport_bounds: (f64, f64, f64, f64),
    /// Unix timestamp (seconds) since the user went idle, or `None` if active.
    pub idle_since: Option<u64>,
}

impl AwarenessState {
    /// Create a new awareness state with sensible defaults.
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            viewing_page: 0,
            viewport_bounds: (0.0, 0.0, 800.0, 600.0),
            idle_since: None,
        }
    }
}

// ---------------------------------------------------------------------------
// AwarenessProtocol
// ---------------------------------------------------------------------------

/// Manages awareness state for all users in a collaboration session.
#[derive(Debug)]
pub struct AwarenessProtocol {
    /// Per-user awareness state.
    pub states: HashMap<String, AwarenessState>,
    /// How often (in milliseconds) the local state should be broadcast.
    pub broadcast_interval_ms: u64,
    /// The local user's identifier.
    local_user_id: String,
}

impl AwarenessProtocol {
    /// Create a new awareness protocol instance for the given local user.
    pub fn new(local_user_id: &str) -> Self {
        let mut states = HashMap::new();
        states.insert(
            local_user_id.to_string(),
            AwarenessState::new(local_user_id),
        );
        Self {
            states,
            broadcast_interval_ms: 1000,
            local_user_id: local_user_id.to_string(),
        }
    }

    /// Update the local user's awareness state.
    pub fn update_local(&mut self, state: AwarenessState) {
        self.states.insert(self.local_user_id.clone(), state);
    }

    /// Apply a remotely received awareness state.
    pub fn apply_remote(&mut self, user_id: &str, state: AwarenessState) {
        self.states.insert(user_id.to_string(), state);
    }

    /// Return the user ids of all users that are currently active (i.e. not
    /// idle and present in the states map).
    pub fn get_active_users(&self) -> Vec<String> {
        self.states
            .values()
            .filter(|s| s.idle_since.is_none())
            .map(|s| s.user_id.clone())
            .collect()
    }

    /// Return the user ids of everyone viewing the given page.
    pub fn who_is_viewing_page(&self, page: usize) -> Vec<String> {
        self.states
            .values()
            .filter(|s| s.viewing_page == page)
            .map(|s| s.user_id.clone())
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
    fn test_new_protocol() {
        let ap = AwarenessProtocol::new("alice");
        assert_eq!(ap.broadcast_interval_ms, 1000);
        assert!(ap.states.contains_key("alice"));
        assert_eq!(ap.states.len(), 1);
    }

    #[test]
    fn test_update_local() {
        let mut ap = AwarenessProtocol::new("alice");
        let state = AwarenessState {
            user_id: "alice".into(),
            viewing_page: 2,
            viewport_bounds: (10.0, 20.0, 400.0, 300.0),
            idle_since: None,
        };
        ap.update_local(state.clone());
        let stored = ap.states.get("alice").unwrap();
        assert_eq!(stored.viewing_page, 2);
        assert_eq!(stored.viewport_bounds, (10.0, 20.0, 400.0, 300.0));
    }

    #[test]
    fn test_apply_remote() {
        let mut ap = AwarenessProtocol::new("alice");
        let bob_state = AwarenessState {
            user_id: "bob".into(),
            viewing_page: 1,
            viewport_bounds: (0.0, 0.0, 1024.0, 768.0),
            idle_since: None,
        };
        ap.apply_remote("bob", bob_state);
        assert_eq!(ap.states.len(), 2);
        assert!(ap.states.contains_key("bob"));
    }

    #[test]
    fn test_get_active_users() {
        let mut ap = AwarenessProtocol::new("alice");
        ap.apply_remote(
            "bob",
            AwarenessState {
                user_id: "bob".into(),
                viewing_page: 0,
                viewport_bounds: (0.0, 0.0, 800.0, 600.0),
                idle_since: None,
            },
        );
        ap.apply_remote(
            "charlie",
            AwarenessState {
                user_id: "charlie".into(),
                viewing_page: 0,
                viewport_bounds: (0.0, 0.0, 800.0, 600.0),
                idle_since: Some(1000),
            },
        );

        let active = ap.get_active_users();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&"alice".to_string()));
        assert!(active.contains(&"bob".to_string()));
        assert!(!active.contains(&"charlie".to_string()));
    }

    #[test]
    fn test_who_is_viewing_page() {
        let mut ap = AwarenessProtocol::new("alice");
        // alice is on page 0 by default
        ap.apply_remote(
            "bob",
            AwarenessState {
                user_id: "bob".into(),
                viewing_page: 0,
                viewport_bounds: (0.0, 0.0, 800.0, 600.0),
                idle_since: None,
            },
        );
        ap.apply_remote(
            "charlie",
            AwarenessState {
                user_id: "charlie".into(),
                viewing_page: 1,
                viewport_bounds: (0.0, 0.0, 800.0, 600.0),
                idle_since: None,
            },
        );

        let page0 = ap.who_is_viewing_page(0);
        assert_eq!(page0.len(), 2);
        assert!(page0.contains(&"alice".to_string()));
        assert!(page0.contains(&"bob".to_string()));

        let page1 = ap.who_is_viewing_page(1);
        assert_eq!(page1.len(), 1);
        assert!(page1.contains(&"charlie".to_string()));

        let page2 = ap.who_is_viewing_page(2);
        assert!(page2.is_empty());
    }

    #[test]
    fn test_awareness_state_new() {
        let state = AwarenessState::new("test");
        assert_eq!(state.user_id, "test");
        assert_eq!(state.viewing_page, 0);
        assert_eq!(state.viewport_bounds, (0.0, 0.0, 800.0, 600.0));
        assert_eq!(state.idle_since, None);
    }

    #[test]
    fn test_broadcast_interval_default() {
        let ap = AwarenessProtocol::new("user1");
        assert_eq!(ap.broadcast_interval_ms, 1000);
    }

    #[test]
    fn test_update_remote_overwrites() {
        let mut ap = AwarenessProtocol::new("alice");
        ap.apply_remote(
            "bob",
            AwarenessState {
                user_id: "bob".into(),
                viewing_page: 0,
                viewport_bounds: (0.0, 0.0, 800.0, 600.0),
                idle_since: None,
            },
        );
        ap.apply_remote(
            "bob",
            AwarenessState {
                user_id: "bob".into(),
                viewing_page: 3,
                viewport_bounds: (50.0, 50.0, 400.0, 300.0),
                idle_since: Some(999),
            },
        );
        let bob = ap.states.get("bob").unwrap();
        assert_eq!(bob.viewing_page, 3);
        assert_eq!(bob.idle_since, Some(999));
    }
}
