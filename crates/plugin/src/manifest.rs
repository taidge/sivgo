use serde::{Deserialize, Serialize};

/// Metadata describing a plugin: identity, version, author, and required permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique identifier, e.g. `"builtin.hello"`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Semver version string.
    pub version: String,
    /// Author name or organisation.
    pub author: String,
    /// Short description of what the plugin does.
    pub description: String,
    /// Permissions the plugin requires.
    pub permissions: Vec<Permission>,
}

/// Permissions that a plugin may request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Permission {
    /// Read the document tree (non-destructive).
    ReadDocument,
    /// Create, delete, or modify nodes.
    ModifyDocument,
    /// Register a menu item in the editor UI.
    AddMenuItem,
    /// Read/write files on disk.
    AccessFileSystem,
    /// Open network connections.
    NetworkAccess,
}
