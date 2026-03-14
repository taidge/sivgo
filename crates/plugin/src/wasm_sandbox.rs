//! WASM plugin sandbox abstraction for running untrusted plugin code.
//!
//! This module provides the *interface* for a sandboxed WASM runtime.
//! The actual WASM engine (wasmer, wasmtime, etc.) is injected via the
//! [`WasmRuntime`] trait so that the rest of the plugin system stays
//! engine-agnostic.  A [`StubWasmRuntime`] is included for testing.
//!
//! The [`WasmSandbox`] struct bundles module bytes together with
//! resource limits and permission flags so that the host can enforce
//! security policy before executing any guest code.

// ---------------------------------------------------------------------------
// WasmPermissions
// ---------------------------------------------------------------------------

/// Fine-grained capability flags that control what a WASM plugin module
/// is allowed to do.
#[derive(Debug, Clone)]
pub struct WasmPermissions {
    /// Whether the module may read the SVG document tree.
    pub can_read_document: bool,
    /// Whether the module may modify (add / remove / edit) nodes.
    pub can_modify_document: bool,
    /// Whether the module may open network connections.
    pub can_access_network: bool,
    /// Whether the module may read or write the filesystem.
    pub can_access_filesystem: bool,
}

impl Default for WasmPermissions {
    fn default() -> Self {
        Self {
            can_read_document: true,
            can_modify_document: false,
            can_access_network: false,
            can_access_filesystem: false,
        }
    }
}

// ---------------------------------------------------------------------------
// WasmValue
// ---------------------------------------------------------------------------

/// Dynamically-typed value that can cross the WASM boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Void,
}

// ---------------------------------------------------------------------------
// WasmSandbox
// ---------------------------------------------------------------------------

/// Configuration for the WASM sandbox that limits what a plugin module
/// may do.  Bundles the raw module bytes with resource limits and
/// permissions.
#[derive(Debug, Clone)]
pub struct WasmSandbox {
    /// Raw `.wasm` module bytes.
    pub module_bytes: Vec<u8>,
    /// Maximum linear-memory size the module may allocate (bytes).
    pub memory_limit: usize,
    /// Fuel (instruction) budget per call.
    pub fuel_limit: u64,
    /// Wall-clock execution budget per call (milliseconds).
    pub time_limit_ms: u64,
    /// Host functions the module is allowed to import (e.g.
    /// `"env.log"`, `"sivgo.add_node"`).
    pub allowed_imports: Vec<String>,
    /// Capability flags.
    pub permissions: WasmPermissions,
}

impl WasmSandbox {
    /// Create a new sandbox wrapping the given WASM bytes with default
    /// limits and read-only permissions.
    pub fn new(wasm_bytes: Vec<u8>) -> Self {
        Self {
            module_bytes: wasm_bytes,
            memory_limit: 16 * 1024 * 1024, // 16 MiB
            fuel_limit: 1_000_000,
            time_limit_ms: 5_000,
            allowed_imports: Vec::new(),
            permissions: WasmPermissions::default(),
        }
    }

    /// Replace the permission flags.
    pub fn set_permissions(&mut self, perms: WasmPermissions) {
        self.permissions = perms;
    }

    /// Validate the module bytes.
    ///
    /// In a real implementation this would parse and validate the WASM
    /// binary.  The stub version checks for the WASM magic number
    /// (`\0asm`) and a non-empty payload.
    pub fn validate_module(&self) -> Result<(), String> {
        if self.module_bytes.len() < 8 {
            return Err("module too small to be valid WASM".to_string());
        }
        // WASM magic: 0x00 0x61 0x73 0x6D
        if &self.module_bytes[..4] != b"\0asm" {
            return Err("invalid WASM magic number".to_string());
        }
        Ok(())
    }

    /// Execute an exported function by name.
    ///
    /// This is a **stub** implementation -- it does not run a real WASM
    /// engine.  Instead it validates the module, checks permissions, and
    /// returns a mock result derived from the function name and arguments.
    pub fn execute(&self, fn_name: &str, args: &[WasmValue]) -> Result<Vec<WasmValue>, String> {
        self.validate_module()?;

        if fn_name.is_empty() {
            return Err("function name must not be empty".to_string());
        }

        // Permission gate: if the function name suggests document
        // modification, check permissions.
        if fn_name.starts_with("modify_") || fn_name.starts_with("set_") {
            if !self.permissions.can_modify_document {
                return Err(format!(
                    "permission denied: function '{fn_name}' requires can_modify_document"
                ));
            }
        }

        if fn_name.starts_with("net_") || fn_name.starts_with("fetch_") {
            if !self.permissions.can_access_network {
                return Err(format!(
                    "permission denied: function '{fn_name}' requires can_access_network"
                ));
            }
        }

        if fn_name.starts_with("fs_") || fn_name.starts_with("file_") {
            if !self.permissions.can_access_filesystem {
                return Err(format!(
                    "permission denied: function '{fn_name}' requires can_access_filesystem"
                ));
            }
        }

        // Stub: return a mock result based on the argument types.
        let result = if args.is_empty() {
            vec![WasmValue::I32(0)]
        } else {
            args.iter()
                .map(|a| match a {
                    WasmValue::I32(v) => WasmValue::I32(*v),
                    WasmValue::I64(v) => WasmValue::I64(*v),
                    WasmValue::F32(v) => WasmValue::F32(*v),
                    WasmValue::F64(v) => WasmValue::F64(*v),
                    WasmValue::String(s) => WasmValue::String(s.clone()),
                    WasmValue::Void => WasmValue::Void,
                })
                .collect()
        };

        Ok(result)
    }
}

impl Default for WasmSandbox {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// WasmModule
// ---------------------------------------------------------------------------

/// A compiled (or to-be-compiled) WASM module together with its sandbox
/// configuration.
#[derive(Debug, Clone)]
pub struct WasmModule {
    /// Human-readable name for logging/diagnostics.
    pub name: String,
    /// Raw `.wasm` bytes.
    pub bytes: Vec<u8>,
    /// Sandbox constraints that apply when this module executes.
    pub sandbox: WasmSandbox,
}

// ---------------------------------------------------------------------------
// WasmRuntime trait
// ---------------------------------------------------------------------------

/// Trait abstracting the WASM runtime engine.
///
/// Implementations are responsible for compiling / instantiating the
/// module, enforcing sandbox limits, and marshalling call arguments.
pub trait WasmRuntime {
    /// Load (compile + instantiate) a [`WasmModule`].
    fn load(&mut self, module: &WasmModule) -> Result<(), String>;

    /// Call an exported function by name.
    fn call(&mut self, function: &str, args: &[WasmValue]) -> Result<WasmValue, String>;
}

// ---------------------------------------------------------------------------
// StubWasmRuntime -- no-op implementation for unit tests / feature-gating
// ---------------------------------------------------------------------------

/// A no-op [`WasmRuntime`] that accepts any module and returns
/// [`WasmValue::Void`] for every call.  Useful for tests that need to
/// exercise the plugin pipeline without shipping a real WASM engine.
#[derive(Debug, Default)]
pub struct StubWasmRuntime {
    loaded_module: Option<String>,
}

impl StubWasmRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the name of the currently loaded module, if any.
    pub fn loaded_module_name(&self) -> Option<&str> {
        self.loaded_module.as_deref()
    }
}

impl WasmRuntime for StubWasmRuntime {
    fn load(&mut self, module: &WasmModule) -> Result<(), String> {
        self.loaded_module = Some(module.name.clone());
        Ok(())
    }

    fn call(&mut self, _function: &str, _args: &[WasmValue]) -> Result<WasmValue, String> {
        if self.loaded_module.is_none() {
            return Err("no module loaded".to_string());
        }
        Ok(WasmValue::Void)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid-looking WASM header (magic + version).
    fn wasm_magic() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00]
    }

    // -- StubWasmRuntime tests --

    #[test]
    fn stub_load_and_call() {
        let mut rt = StubWasmRuntime::new();

        let module = WasmModule {
            name: "test_module".to_string(),
            bytes: wasm_magic(),
            sandbox: WasmSandbox::new(wasm_magic()),
        };

        assert!(rt.load(&module).is_ok());
        assert_eq!(rt.loaded_module_name(), Some("test_module"));

        let result = rt.call("add", &[WasmValue::I32(1), WasmValue::I32(2)]);
        assert_eq!(result, Ok(WasmValue::Void));
    }

    #[test]
    fn stub_call_without_load_fails() {
        let mut rt = StubWasmRuntime::new();
        let result = rt.call("anything", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn sandbox_default_values() {
        let sb = WasmSandbox::new(Vec::new());
        assert_eq!(sb.memory_limit, 16 * 1024 * 1024);
        assert_eq!(sb.fuel_limit, 1_000_000);
        assert_eq!(sb.time_limit_ms, 5_000);
        assert!(sb.allowed_imports.is_empty());
        assert!(sb.permissions.can_read_document);
        assert!(!sb.permissions.can_modify_document);
        assert!(!sb.permissions.can_access_network);
        assert!(!sb.permissions.can_access_filesystem);
    }

    // -- WasmSandbox tests --

    #[test]
    fn sandbox_new_and_validate() {
        let sb = WasmSandbox::new(wasm_magic());
        assert!(sb.validate_module().is_ok());
    }

    #[test]
    fn sandbox_validate_rejects_short_bytes() {
        let sb = WasmSandbox::new(vec![0x00, 0x01]);
        assert!(sb.validate_module().is_err());
    }

    #[test]
    fn sandbox_validate_rejects_bad_magic() {
        let sb = WasmSandbox::new(vec![0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00]);
        let err = sb.validate_module().unwrap_err();
        assert!(err.contains("magic"));
    }

    #[test]
    fn sandbox_execute_no_args() {
        let sb = WasmSandbox::new(wasm_magic());
        let result = sb.execute("init", &[]).unwrap();
        assert_eq!(result, vec![WasmValue::I32(0)]);
    }

    #[test]
    fn sandbox_execute_with_args() {
        let sb = WasmSandbox::new(wasm_magic());
        let result = sb
            .execute("compute", &[WasmValue::I32(42), WasmValue::F64(3.14)])
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], WasmValue::I32(42));
        assert_eq!(result[1], WasmValue::F64(3.14));
    }

    #[test]
    fn sandbox_execute_empty_fn_name_fails() {
        let sb = WasmSandbox::new(wasm_magic());
        assert!(sb.execute("", &[]).is_err());
    }

    #[test]
    fn sandbox_execute_invalid_module_fails() {
        let sb = WasmSandbox::new(vec![0x00, 0x01]);
        assert!(sb.execute("init", &[]).is_err());
    }

    #[test]
    fn sandbox_permission_denied_modify() {
        let sb = WasmSandbox::new(wasm_magic());
        // Default perms: can_modify_document = false
        let err = sb.execute("modify_node", &[]).unwrap_err();
        assert!(err.contains("permission denied"));
    }

    #[test]
    fn sandbox_permission_granted_modify() {
        let mut sb = WasmSandbox::new(wasm_magic());
        sb.set_permissions(WasmPermissions {
            can_modify_document: true,
            ..Default::default()
        });
        assert!(sb.execute("modify_node", &[]).is_ok());
    }

    #[test]
    fn sandbox_permission_denied_network() {
        let sb = WasmSandbox::new(wasm_magic());
        let err = sb.execute("net_request", &[]).unwrap_err();
        assert!(err.contains("permission denied"));
    }

    #[test]
    fn sandbox_permission_denied_filesystem() {
        let sb = WasmSandbox::new(wasm_magic());
        let err = sb.execute("fs_read", &[]).unwrap_err();
        assert!(err.contains("permission denied"));
    }

    #[test]
    fn sandbox_set_permissions() {
        let mut sb = WasmSandbox::new(wasm_magic());
        sb.set_permissions(WasmPermissions {
            can_read_document: false,
            can_modify_document: true,
            can_access_network: true,
            can_access_filesystem: true,
        });
        assert!(!sb.permissions.can_read_document);
        assert!(sb.permissions.can_modify_document);
        assert!(sb.permissions.can_access_network);
        assert!(sb.permissions.can_access_filesystem);
    }

    #[test]
    fn sandbox_execute_with_string_value() {
        let sb = WasmSandbox::new(wasm_magic());
        let result = sb
            .execute("greet", &[WasmValue::String("hello".to_string())])
            .unwrap();
        assert_eq!(result, vec![WasmValue::String("hello".to_string())]);
    }

    #[test]
    fn wasm_permissions_default() {
        let perms = WasmPermissions::default();
        assert!(perms.can_read_document);
        assert!(!perms.can_modify_document);
        assert!(!perms.can_access_network);
        assert!(!perms.can_access_filesystem);
    }
}
