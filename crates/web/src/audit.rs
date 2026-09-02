//! Structured audit events for security-sensitive API actions.
//!
//! The current implementation is an in-memory ring buffer intended for
//! development and tests. Production deployments should forward these events
//! to durable storage or a centralized logging pipeline.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use salvo::prelude::Depot;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A security-relevant action that should be visible in audit history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditAction {
    UserRegistered,
    UserLoggedIn,
    OAuthUserAuthenticated,
    DocumentCreated,
    DocumentUpdated,
    DocumentDeleted,
    ShareCreated,
    ShareRevoked,
    SharedDocumentAccessed,
    AssetUploaded,
    AssetDeleted,
    PreferencesUpdated,
    PluginInstalled,
    PluginUninstalled,
    ExportRequested,
}

/// One structured audit event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEvent {
    pub id: Uuid,
    pub action: AuditAction,
    pub actor_id: Option<Uuid>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub metadata: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

/// Bounded, thread-safe in-memory audit log.
#[derive(Debug, Clone)]
pub struct AuditLog {
    events: Arc<RwLock<VecDeque<AuditEvent>>>,
    max_events: usize,
}

impl AuditLog {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::new())),
            max_events: max_events.max(1),
        }
    }

    pub fn record(
        &self,
        action: AuditAction,
        actor_id: Option<Uuid>,
        target_type: Option<&str>,
        target_id: Option<String>,
        metadata: serde_json::Value,
    ) -> AuditEvent {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            action,
            actor_id,
            target_type: target_type.map(str::to_owned),
            target_id,
            metadata,
            occurred_at: Utc::now(),
        };

        let mut events = crate::lock::write(&self.events);
        while events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(event.clone());
        if let Ok(serialized) = serde_json::to_string(&event) {
            log::info!("audit_event={serialized}");
        }
        event
    }

    pub fn list(&self) -> Vec<AuditEvent> {
        crate::lock::read(&self.events).iter().cloned().collect()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// Best-effort helper for request handlers. Missing audit state should not
/// break request handling in tests or custom router setups.
pub fn record_if_available(
    depot: &Depot,
    action: AuditAction,
    actor_id: Option<Uuid>,
    target_type: Option<&str>,
    target_id: Option<String>,
    metadata: serde_json::Value,
) {
    if let Ok(audit_log) = depot.obtain::<AuditLog>() {
        audit_log.record(action, actor_id, target_type, target_id, metadata);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_keeps_recent_events_only() {
        let audit = AuditLog::new(2);

        audit.record(
            AuditAction::UserRegistered,
            None,
            Some("user"),
            Some("first".to_string()),
            serde_json::json!({}),
        );
        audit.record(
            AuditAction::UserLoggedIn,
            None,
            Some("user"),
            Some("second".to_string()),
            serde_json::json!({}),
        );
        audit.record(
            AuditAction::DocumentCreated,
            None,
            Some("document"),
            Some("third".to_string()),
            serde_json::json!({}),
        );

        let events = audit.list();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].target_id.as_deref(), Some("second"));
        assert_eq!(events[1].target_id.as_deref(), Some("third"));
    }
}
