//! Real-time collaboration infrastructure for Sivgo.
//!
//! Provides session management, peer tracking, collaborative editing
//! operations, and a simple last-write-wins merge strategy.

use std::collections::HashMap;

/// Information about a connected peer in a collaboration session.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub user_id: String,
    pub display_name: String,
    pub color: [u8; 3],
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub active_tool: String,
}

/// A single collaborative editing operation.
#[derive(Debug, Clone)]
pub enum CollabOperation {
    InsertNode {
        parent_id: String,
        element_json: String,
        position: usize,
    },
    RemoveNode {
        node_id: String,
    },
    ModifyAttribute {
        node_id: String,
        attr: String,
        old_value: String,
        new_value: String,
    },
    Transform {
        node_id: String,
        transform_json: String,
    },
    Reorder {
        node_id: String,
        new_position: usize,
    },
}

/// A timestamped edit made by a specific user.
#[derive(Debug, Clone)]
pub struct CollabEdit {
    pub user_id: String,
    pub timestamp: u64,
    pub operation: CollabOperation,
}

/// Ordered history of all edits applied to a document.
#[derive(Debug)]
pub struct CollabHistory {
    edits: Vec<CollabEdit>,
    pub document_id: String,
}

impl CollabHistory {
    fn new(document_id: String) -> Self {
        Self {
            edits: Vec::new(),
            document_id,
        }
    }

    /// Append an edit to the history.
    pub fn push(&mut self, edit: CollabEdit) {
        self.edits.push(edit);
    }

    /// Return all edits in chronological order.
    pub fn edits(&self) -> &[CollabEdit] {
        &self.edits
    }
}

/// A live collaboration session for a single document.
#[derive(Debug)]
pub struct CollabSession {
    pub session_id: String,
    pub document_id: String,
    pub user_id: String,
    pub peers: Vec<PeerInfo>,
    pub connected: bool,
    history: CollabHistory,
    /// Monotonic counter used to generate session-scoped unique IDs.
    #[allow(dead_code)]
    id_counter: u64,
    /// Index for fast peer lookup by `user_id`.
    peer_index: HashMap<String, usize>,
}

impl CollabSession {
    /// Create a new collaboration session for the given document and user.
    ///
    /// The session starts in the *connected* state with an empty peer list.
    pub fn new(document_id: &str, user_id: &str) -> Self {
        let mut counter: u64 = 0;
        let session_id = Self::next_id(&mut counter, "session");
        Self {
            session_id,
            document_id: document_id.to_string(),
            user_id: user_id.to_string(),
            peers: Vec::new(),
            connected: true,
            history: CollabHistory::new(document_id.to_string()),
            id_counter: counter,
            peer_index: HashMap::new(),
        }
    }

    /// Add a peer to the session.
    ///
    /// If a peer with the same `user_id` already exists it is replaced.
    pub fn add_peer(&mut self, peer: PeerInfo) {
        if let Some(&idx) = self.peer_index.get(&peer.user_id) {
            self.peers[idx] = peer;
        } else {
            let idx = self.peers.len();
            self.peer_index.insert(peer.user_id.clone(), idx);
            self.peers.push(peer);
        }
    }

    /// Remove a peer by `user_id`. Returns `true` if the peer was present.
    pub fn remove_peer(&mut self, user_id: &str) -> bool {
        if let Some(&idx) = self.peer_index.get(user_id) {
            self.peers.swap_remove(idx);
            self.peer_index.remove(user_id);
            // Fix the index of the element that was swapped into `idx`.
            if idx < self.peers.len() {
                let swapped_id = self.peers[idx].user_id.clone();
                self.peer_index.insert(swapped_id, idx);
            }
            true
        } else {
            false
        }
    }

    /// Apply a collaborative edit to the session history.
    pub fn apply_edit(&mut self, edit: CollabEdit) {
        self.history.push(edit);
    }

    /// Return the full edit history for this session.
    pub fn get_history(&self) -> &[CollabEdit] {
        self.history.edits()
    }

    /// Merge two edit streams using a **last-write-wins** strategy.
    ///
    /// Both slices are assumed to be sorted by timestamp. The merge produces a
    /// single timeline ordered by timestamp; when two edits share the same
    /// timestamp the `remote` edit wins (appears later).
    pub fn merge_edits(local: &[CollabEdit], remote: &[CollabEdit]) -> Vec<CollabEdit> {
        let mut merged: Vec<CollabEdit> = Vec::with_capacity(local.len() + remote.len());
        let (mut li, mut ri) = (0, 0);

        while li < local.len() && ri < remote.len() {
            if local[li].timestamp < remote[ri].timestamp {
                merged.push(local[li].clone());
                li += 1;
            } else if local[li].timestamp > remote[ri].timestamp {
                merged.push(remote[ri].clone());
                ri += 1;
            } else {
                // Same timestamp -- last-write-wins: keep remote after local.
                merged.push(local[li].clone());
                li += 1;
                merged.push(remote[ri].clone());
                ri += 1;
            }
        }

        while li < local.len() {
            merged.push(local[li].clone());
            li += 1;
        }
        while ri < remote.len() {
            merged.push(remote[ri].clone());
            ri += 1;
        }

        merged
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn next_id(counter: &mut u64, prefix: &str) -> String {
        *counter += 1;
        format!("id-{prefix}-{counter}")
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(id: &str) -> PeerInfo {
        PeerInfo {
            user_id: id.to_string(),
            display_name: format!("User {id}"),
            color: [255, 0, 0],
            cursor_x: 0.0,
            cursor_y: 0.0,
            active_tool: "select".to_string(),
        }
    }

    fn make_edit(user: &str, ts: u64) -> CollabEdit {
        CollabEdit {
            user_id: user.to_string(),
            timestamp: ts,
            operation: CollabOperation::ModifyAttribute {
                node_id: "n1".to_string(),
                attr: "fill".to_string(),
                old_value: "red".to_string(),
                new_value: "blue".to_string(),
            },
        }
    }

    #[test]
    fn new_session_is_connected_with_empty_peers() {
        let session = CollabSession::new("doc-1", "user-a");
        assert!(session.connected);
        assert!(session.peers.is_empty());
        assert_eq!(session.document_id, "doc-1");
        assert_eq!(session.user_id, "user-a");
    }

    #[test]
    fn add_and_remove_peers() {
        let mut session = CollabSession::new("doc-1", "user-a");
        session.add_peer(make_peer("p1"));
        session.add_peer(make_peer("p2"));
        session.add_peer(make_peer("p3"));
        assert_eq!(session.peers.len(), 3);

        // Remove middle peer.
        assert!(session.remove_peer("p2"));
        assert_eq!(session.peers.len(), 2);
        // p2 should no longer be present.
        assert!(!session.remove_peer("p2"));

        // The remaining peers should still be accessible.
        let ids: Vec<&str> = session.peers.iter().map(|p| p.user_id.as_str()).collect();
        assert!(ids.contains(&"p1"));
        assert!(ids.contains(&"p3"));
    }

    #[test]
    fn add_peer_replaces_existing() {
        let mut session = CollabSession::new("doc-1", "user-a");
        let mut peer = make_peer("p1");
        peer.cursor_x = 10.0;
        session.add_peer(peer);

        let mut updated = make_peer("p1");
        updated.cursor_x = 42.0;
        session.add_peer(updated);

        assert_eq!(session.peers.len(), 1);
        assert!((session.peers[0].cursor_x - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_edit_records_history() {
        let mut session = CollabSession::new("doc-1", "user-a");
        session.apply_edit(make_edit("user-a", 1));
        session.apply_edit(make_edit("user-b", 2));
        assert_eq!(session.get_history().len(), 2);
        assert_eq!(session.get_history()[0].timestamp, 1);
        assert_eq!(session.get_history()[1].timestamp, 2);
    }

    #[test]
    fn merge_edits_interleaves_by_timestamp() {
        let local = vec![make_edit("a", 1), make_edit("a", 3), make_edit("a", 5)];
        let remote = vec![make_edit("b", 2), make_edit("b", 4)];
        let merged = CollabSession::merge_edits(&local, &remote);

        let timestamps: Vec<u64> = merged.iter().map(|e| e.timestamp).collect();
        assert_eq!(timestamps, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn merge_edits_remote_wins_on_tie() {
        let local = vec![make_edit("local", 10)];
        let remote = vec![make_edit("remote", 10)];
        let merged = CollabSession::merge_edits(&local, &remote);

        assert_eq!(merged.len(), 2);
        // Remote appears *after* local (last-write-wins).
        assert_eq!(merged[0].user_id, "local");
        assert_eq!(merged[1].user_id, "remote");
    }

    #[test]
    fn collab_operation_variants_are_constructible() {
        let ops = vec![
            CollabOperation::InsertNode {
                parent_id: "root".into(),
                element_json: "{}".into(),
                position: 0,
            },
            CollabOperation::RemoveNode {
                node_id: "n1".into(),
            },
            CollabOperation::ModifyAttribute {
                node_id: "n2".into(),
                attr: "stroke".into(),
                old_value: "none".into(),
                new_value: "#000".into(),
            },
            CollabOperation::Transform {
                node_id: "n3".into(),
                transform_json: r#"{"translate":[10,20]}"#.into(),
            },
            CollabOperation::Reorder {
                node_id: "n4".into(),
                new_position: 2,
            },
        ];
        assert_eq!(ops.len(), 5);
    }
}
