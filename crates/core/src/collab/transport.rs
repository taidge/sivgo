//! Transport layer for real-time collaboration.
//!
//! Provides protocol messages and an in-memory room hub that behaves like a
//! deterministic WebSocket broker in tests and local collaboration sessions.

use std::collections::HashMap;

use super::crdt::CrdtOp;

// ---------------------------------------------------------------------------
// SyncMessage
// ---------------------------------------------------------------------------

/// Messages exchanged between collaborating peers over the sync transport.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncMessage {
    /// Request to join a collaboration room.
    Join { room_id: String, user_id: String },
    /// Notify that a user has left.
    Leave { user_id: String },
    /// Broadcast one or more CRDT operations.
    Op { operations: Vec<CrdtOp> },
    /// Full state synchronisation: version vector + operations the receiver
    /// may be missing.
    Sync {
        version_vector: HashMap<u64, u64>,
        operations: Vec<CrdtOp>,
    },
    /// Acknowledge receipt of operations up to the given clock.
    Ack { clock: u64 },
    /// User presence update (cursor position, selection).
    Presence {
        user_id: String,
        cursor: Option<(f64, f64)>,
        selection: Vec<String>,
    },
    /// Comment or comment-thread update.
    Comment {
        thread_id: String,
        comment_id: String,
        author_id: String,
        text: String,
        resolved: bool,
    },
}

/// Endpoint connected to a collaboration room.
#[derive(Debug)]
pub struct SyncTransport {
    /// Whether this transport is "connected" to a room.
    pub connected: bool,
    /// The room this transport is connected to, if any.
    pub room_id: Option<String>,
    /// User id for hub fan-out.
    pub user_id: Option<String>,
    /// Operations waiting to be sent.
    pub pending_ops: Vec<CrdtOp>,
    /// Log of all messages sent through this transport.
    pub message_log: Vec<SyncMessage>,
    /// Inbound message queue.
    inbound: Vec<SyncMessage>,
}

impl SyncTransport {
    /// Create a new disconnected transport.
    pub fn new() -> Self {
        Self {
            connected: false,
            room_id: None,
            user_id: None,
            pending_ops: Vec::new(),
            message_log: Vec::new(),
            inbound: Vec::new(),
        }
    }

    pub fn connect(&mut self, room_id: &str) {
        self.connect_as(room_id, "");
    }

    pub fn connect_as(&mut self, room_id: &str, user_id: &str) {
        self.room_id = Some(room_id.to_string());
        self.user_id = (!user_id.is_empty()).then(|| user_id.to_string());
        self.connected = true;
    }

    /// Disconnect from the current room.
    pub fn disconnect(&mut self) {
        self.connected = false;
        self.room_id = None;
        self.user_id = None;
    }

    /// Send a message.  The message is appended to the `message_log`.
    pub fn send(&mut self, msg: SyncMessage) {
        self.message_log.push(msg);
    }

    pub fn send_to_hub(
        &mut self,
        hub: &mut InMemorySyncHub,
        msg: SyncMessage,
    ) -> Result<(), String> {
        if !self.connected {
            return Err("transport is not connected".to_string());
        }
        let room_id = self
            .room_id
            .as_deref()
            .ok_or_else(|| "transport has no room".to_string())?;
        let user_id = self
            .user_id
            .as_deref()
            .ok_or_else(|| "transport has no user id".to_string())?;
        hub.broadcast(room_id, user_id, msg.clone())?;
        self.send(msg);
        Ok(())
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

    pub fn poll_hub(&mut self, hub: &mut InMemorySyncHub) -> Result<usize, String> {
        let room_id = self
            .room_id
            .as_deref()
            .ok_or_else(|| "transport has no room".to_string())?;
        let user_id = self
            .user_id
            .as_deref()
            .ok_or_else(|| "transport has no user id".to_string())?;
        let messages = hub.drain_user(room_id, user_id)?;
        let count = messages.len();
        self.inbound.extend(messages);
        Ok(count)
    }
}

impl Default for SyncTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic room broker for collaboration transport tests and local
/// embedded sessions.
#[derive(Debug, Default)]
pub struct InMemorySyncHub {
    rooms: HashMap<String, HashMap<String, Vec<SyncMessage>>>,
}

impl InMemorySyncHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn join(&mut self, room_id: &str, user_id: &str) {
        self.rooms
            .entry(room_id.to_string())
            .or_default()
            .entry(user_id.to_string())
            .or_default();
    }

    pub fn leave(&mut self, room_id: &str, user_id: &str) {
        if let Some(room) = self.rooms.get_mut(room_id) {
            room.remove(user_id);
        }
    }

    pub fn broadcast(
        &mut self,
        room_id: &str,
        sender_id: &str,
        msg: SyncMessage,
    ) -> Result<usize, String> {
        let Some(room) = self.rooms.get_mut(room_id) else {
            return Err(format!("room not found: {room_id}"));
        };
        if !room.contains_key(sender_id) {
            return Err(format!("sender is not in room: {sender_id}"));
        }
        let mut delivered = 0;
        for (user_id, queue) in room.iter_mut() {
            if user_id != sender_id {
                queue.push(msg.clone());
                delivered += 1;
            }
        }
        Ok(delivered)
    }

    pub fn drain_user(&mut self, room_id: &str, user_id: &str) -> Result<Vec<SyncMessage>, String> {
        let Some(room) = self.rooms.get_mut(room_id) else {
            return Err(format!("room not found: {room_id}"));
        };
        let Some(queue) = room.get_mut(user_id) else {
            return Err(format!("user is not in room: {user_id}"));
        };
        Ok(std::mem::take(queue))
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
        assert!(t.user_id.is_none());
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
        assert!(t.user_id.is_none());
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

    #[test]
    fn test_in_memory_hub_fans_out_messages_by_room() {
        let mut hub = InMemorySyncHub::new();
        hub.join("room-1", "alice");
        hub.join("room-1", "bob");
        hub.join("room-2", "mallory");

        let mut alice = SyncTransport::new();
        let mut bob = SyncTransport::new();
        let mut mallory = SyncTransport::new();
        alice.connect_as("room-1", "alice");
        bob.connect_as("room-1", "bob");
        mallory.connect_as("room-2", "mallory");

        alice
            .send_to_hub(
                &mut hub,
                SyncMessage::Presence {
                    user_id: "alice".into(),
                    cursor: Some((1.0, 2.0)),
                    selection: vec!["n1".into()],
                },
            )
            .unwrap();

        assert_eq!(bob.poll_hub(&mut hub).unwrap(), 1);
        assert_eq!(mallory.poll_hub(&mut hub).unwrap(), 0);
        assert!(matches!(bob.receive(), Some(SyncMessage::Presence { .. })));
    }

    #[test]
    fn test_comment_message_roundtrip_through_hub() {
        let mut hub = InMemorySyncHub::new();
        hub.join("room", "alice");
        hub.join("room", "bob");

        let mut alice = SyncTransport::new();
        let mut bob = SyncTransport::new();
        alice.connect_as("room", "alice");
        bob.connect_as("room", "bob");

        alice
            .send_to_hub(
                &mut hub,
                SyncMessage::Comment {
                    thread_id: "thread-1".into(),
                    comment_id: "comment-1".into(),
                    author_id: "alice".into(),
                    text: "Please check this".into(),
                    resolved: false,
                },
            )
            .unwrap();
        bob.poll_hub(&mut hub).unwrap();
        assert!(matches!(
            bob.receive(),
            Some(SyncMessage::Comment {
                comment_id,
                resolved: false,
                ..
            }) if comment_id == "comment-1"
        ));
    }
}
