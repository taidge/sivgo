//! WebSocket transport layer for real-time collaboration.
//!
//! Provides message types and a stub transport that queues messages locally.
//! In production this would wrap an actual WebSocket connection; here we use
//! pure-Rust data structures so the crate compiles without external
//! networking dependencies.

use std::collections::HashMap;

use super::crdt::CrdtOp;

// ---------------------------------------------------------------------------
// SyncMessage
// ---------------------------------------------------------------------------

/// Messages exchanged between collaborating peers over the sync transport.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessage {
    /// Request to join a collaboration room.
    Join {
        room_id: String,
        user_id: String,
    },
    /// Notify that a user has left.
    Leave {
        user_id: String,
    },
    /// Broadcast one or more CRDT operations.
    Op {
        operations: Vec<CrdtOp>,
    },
    /// Full state synchronisation: version vector + operations the receiver
    /// may be missing.
    Sync {
        version_vector: HashMap<u64, u64>,
        operations: Vec<CrdtOp>,
    },
    /// Acknowledge receipt of operations up to the given clock.
    Ack {
        clock: u64,
    },
    /// User presence update (cursor position, selection).
    Presence {
        user_id: String,
        cursor: Option<(f64, f64)>,
        selection: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// SyncTransport (stub)
// ---------------------------------------------------------------------------

/// Stub WebSocket transport for collaboration messages.
///
/// This is a local-only implementation: `send` pushes to `message_log` and
/// `receive` pops from `pending_ops` / internally queued messages so that
/// unit tests can exercise the protocol without a real network.
#[derive(Debug)]
pub struct SyncTransport {
    /// Whether this transport is "connected" to a room.
    pub connected: bool,
    /// The room this transport is connected to, if any.
    pub room_id: Option<String>,
    /// Operations waiting to be sent.
    pub pending_ops: Vec<CrdtOp>,
    /// Log of all messages sent through this transport.
    pub message_log: Vec<SyncMessage>,
    /// Inbound message queue (for stub/testing purposes).
    inbound: Vec<SyncMessage>,
}

impl SyncTransport {
    /// Create a new disconnected transport.
    pub fn new() -> Self {
        Self {
            connected: false,
            room_id: None,
            pending_ops: Vec::new(),
            message_log: Vec::new(),
            inbound: Vec::new(),
        }
    }

    /// Connect to a collaboration room.  In this stub this simply records the
    /// room id and flips the `connected` flag.
    pub fn connect(&mut self, room_id: &str) {
        self.room_id = Some(room_id.to_string());
        self.connected = true;
    }

    /// Disconnect from the current room.
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.room_id = None;
    }

    /// Send a message.  The message is appended to the `message_log`.
    pub fn send(&mut self, msg: SyncMessage) {
        self.message_log.push(msg);
    }

    /// Receive the next inbound message, if any.
    pub fn receive(&mut self) -> Option<SyncMessage> {
        if self.inbound.is_empty() {
            None
        } else {
            Some(self.inbound.remove(0))
        }
    }

    /// Whether the transport is currently connected to a room.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Enqueue an inbound message (testing helper).
    pub fn enqueue_inbound(&mut self, msg: SyncMessage) {
        self.inbound.push(msg);
    }
}

impl Default for SyncTransport {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_transport_disconnected() {
        let t = SyncTransport::new();
        assert!(!t.is_connected());
        assert!(t.room_id.is_none());
        assert!(t.pending_ops.is_empty());
        assert!(t.message_log.is_empty());
    }

    #[test]
    fn test_connect_disconnect() {
        let mut t = SyncTransport::new();
        t.connect("room-1");
        assert!(t.is_connected());
        assert_eq!(t.room_id.as_deref(), Some("room-1"));

        t.disconnect();
        assert!(!t.is_connected());
        assert!(t.room_id.is_none());
    }

    #[test]
    fn test_send_logs_message() {
        let mut t = SyncTransport::new();
        t.connect("room-1");
        t.send(SyncMessage::Join {
            room_id: "room-1".into(),
            user_id: "alice".into(),
        });
        assert_eq!(t.message_log.len(), 1);
        match &t.message_log[0] {
            SyncMessage::Join { room_id, user_id } => {
                assert_eq!(room_id, "room-1");
                assert_eq!(user_id, "alice");
            }
            _ => panic!("Expected Join message"),
        }
    }

    #[test]
    fn test_receive_empty() {
        let mut t = SyncTransport::new();
        assert!(t.receive().is_none());
    }

    #[test]
    fn test_receive_enqueued() {
        let mut t = SyncTransport::new();
        t.enqueue_inbound(SyncMessage::Ack { clock: 5 });
        t.enqueue_inbound(SyncMessage::Ack { clock: 6 });

        let msg1 = t.receive().unwrap();
        assert_eq!(msg1, SyncMessage::Ack { clock: 5 });
        let msg2 = t.receive().unwrap();
        assert_eq!(msg2, SyncMessage::Ack { clock: 6 });
        assert!(t.receive().is_none());
    }

    #[test]
    fn test_send_presence() {
        let mut t = SyncTransport::new();
        t.send(SyncMessage::Presence {
            user_id: "bob".into(),
            cursor: Some((100.0, 200.0)),
            selection: vec!["node-1".into()],
        });
        assert_eq!(t.message_log.len(), 1);
    }

    #[test]
    fn test_send_op() {
        let mut t = SyncTransport::new();
        let ops = vec![CrdtOp::Insert {
            site: 1,
            clock: 1,
            parent: None,
            node_type: "rect".into(),
            attrs: HashMap::new(),
        }];
        t.send(SyncMessage::Op { operations: ops });
        assert_eq!(t.message_log.len(), 1);
    }

    #[test]
    fn test_default_transport() {
        let t = SyncTransport::default();
        assert!(!t.is_connected());
    }

    #[test]
    fn test_sync_message() {
        let mut t = SyncTransport::new();
        let mut vv = HashMap::new();
        vv.insert(1u64, 5u64);
        t.send(SyncMessage::Sync {
            version_vector: vv.clone(),
            operations: vec![],
        });
        match &t.message_log[0] {
            SyncMessage::Sync {
                version_vector,
                operations,
            } => {
                assert_eq!(version_vector, &vv);
                assert!(operations.is_empty());
            }
            _ => panic!("Expected Sync message"),
        }
    }

    #[test]
    fn test_leave_message() {
        let msg = SyncMessage::Leave {
            user_id: "charlie".into(),
        };
        if let SyncMessage::Leave { user_id } = &msg {
            assert_eq!(user_id, "charlie");
        } else {
            panic!("Expected Leave");
        }
    }
}
