use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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

/// Normalized permission flags used by WASM, Lua, registry, and UI policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginPermissionSet {
    pub read_document: bool,
    pub modify_document: bool,
    pub add_menu_item: bool,
    pub access_filesystem: bool,
    pub network_access: bool,
}

impl PluginPermissionSet {
    pub fn from_permissions(permissions: &[Permission]) -> Self {
        let mut set = Self::default();
        for permission in permissions {
            match permission {
                Permission::ReadDocument => set.read_document = true,
                Permission::ModifyDocument => {
                    set.read_document = true;
                    set.modify_document = true;
                }
                Permission::AddMenuItem => set.add_menu_item = true,
                Permission::AccessFileSystem => set.access_filesystem = true,
                Permission::NetworkAccess => set.network_access = true,
            }
        }
        set
    }

    pub fn allows(self, permission: &Permission) -> bool {
        match permission {
            Permission::ReadDocument => self.read_document,
            Permission::ModifyDocument => self.modify_document,
            Permission::AddMenuItem => self.add_menu_item,
            Permission::AccessFileSystem => self.access_filesystem,
            Permission::NetworkAccess => self.network_access,
        }
    }
}

impl PluginManifest {
    /// Validate identity, version, description, and requested permissions.
    pub fn validate(&self) -> Result<(), String> {
        validate_id(&self.id)?;
        validate_nonempty("name", &self.name)?;
        validate_semver(&self.version)?;
        validate_nonempty("author", &self.author)?;
        validate_nonempty("description", &self.description)?;

        let mut seen = HashSet::new();
        for permission in &self.permissions {
            if !seen.insert(permission) {
                return Err(format!("duplicate permission requested: {permission:?}"));
            }
        }

        Ok(())
    }

    pub fn permission_set(&self) -> PluginPermissionSet {
        PluginPermissionSet::from_permissions(&self.permissions)
    }
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("manifest field '{field}' must not be empty"));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), String> {
    validate_nonempty("id", id)?;
    if !id.contains('.') {
        return Err("manifest id must contain a namespace separator '.'".to_string());
    }
    if id
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')))
    {
        return Err("manifest id contains unsupported characters".to_string());
    }
    Ok(())
}

fn validate_semver(version: &str) -> Result<(), String> {
    validate_nonempty("version", version)?;
    let core = version
        .split_once('-')
        .map(|(core, _)| core)
        .unwrap_or(version);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.parse::<u64>().is_err()) {
        return Err("manifest version must be a semantic version like 1.2.3".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> PluginManifest {
        PluginManifest {
            id: "ext.valid".to_string(),
            name: "Valid Extension".to_string(),
            version: "1.2.3".to_string(),
            author: "Sivgo".to_string(),
            description: "A valid extension".to_string(),
            permissions: vec![Permission::ModifyDocument, Permission::AddMenuItem],
        }
    }

    #[test]
    fn validates_manifest_identity_and_permissions() {
        let manifest = manifest();
        assert!(manifest.validate().is_ok());
        let perms = manifest.permission_set();
        assert!(perms.read_document);
        assert!(perms.modify_document);
        assert!(perms.add_menu_item);
        assert!(!perms.network_access);
    }

    #[test]
    fn rejects_invalid_manifest_fields() {
        let mut invalid = manifest();
        invalid.id = "bad id".to_string();
        assert!(invalid.validate().unwrap_err().contains("id"));

        invalid = manifest();
        invalid.version = "1".to_string();
        assert!(invalid.validate().unwrap_err().contains("semantic"));

        invalid = manifest();
        invalid.permissions.push(Permission::ModifyDocument);
        assert!(invalid.validate().unwrap_err().contains("duplicate"));
    }
}
