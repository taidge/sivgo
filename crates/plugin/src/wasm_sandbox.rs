//! WASM plugin sandbox for untrusted extension modules.
//!
//! The sandbox validates modules with `wasmparser`, enforces import and memory
//! policy before load, and includes a bounded interpreter for exported numeric
//! functions.  Host integrations can swap in another `WasmRuntime`, but the
//! default runtime is no longer a mock: invalid modules, disallowed imports,
//! and unsupported instructions fail closed.

use std::collections::HashMap;

use wasmparser::{
    BinaryReader, ExternalKind, FunctionBody, Imports, Operator, Parser, Payload, TypeRef,
    Validator,
};

use crate::manifest::{Permission, PluginManifest, PluginPermissionSet};

const WASM_PAGE_SIZE: usize = 64 * 1024;

/// Fine-grained capability flags that control what a WASM plugin module is
/// allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmPermissions {
    pub can_read_document: bool,
    pub can_modify_document: bool,
    pub can_access_network: bool,
    pub can_access_filesystem: bool,
}

impl WasmPermissions {
    pub fn from_manifest(manifest: &PluginManifest) -> Self {
        Self::from_permission_set(manifest.permission_set())
    }

    pub fn from_permission_set(set: PluginPermissionSet) -> Self {
        Self {
            can_read_document: set.read_document,
            can_modify_document: set.modify_document,
            can_access_network: set.network_access,
            can_access_filesystem: set.access_filesystem,
        }
    }

    fn allows(&self, permission: Permission) -> bool {
        match permission {
            Permission::ReadDocument => self.can_read_document,
            Permission::ModifyDocument => self.can_modify_document,
            Permission::NetworkAccess => self.can_access_network,
            Permission::AccessFileSystem => self.can_access_filesystem,
            Permission::AddMenuItem => true,
        }
    }
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

/// Dynamically typed values that can cross the WASM boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Void,
}

/// Import kind exposed by module inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmImportKind {
    Function,
    Memory,
    Table,
    Global,
    Tag,
}

/// Export kind exposed by module inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmExportKind {
    Function,
    Memory,
    Table,
    Global,
    Tag,
}

/// One imported host item requested by a WASM module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmImport {
    pub module: String,
    pub name: String,
    pub kind: WasmImportKind,
}

impl WasmImport {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// One exported item from a WASM module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExport {
    pub name: String,
    pub kind: WasmExportKind,
    pub index: u32,
}

/// Parsed module summary used for policy decisions and extension UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WasmModuleInspection {
    pub imports: Vec<WasmImport>,
    pub exports: Vec<WasmExport>,
    pub imported_functions: u32,
    pub defined_functions: u32,
    pub declared_memory_pages: u64,
}

/// Configuration for a WASM module execution boundary.
#[derive(Debug, Clone)]
pub struct WasmSandbox {
    pub module_bytes: Vec<u8>,
    pub memory_limit: usize,
    pub fuel_limit: u64,
    pub time_limit_ms: u64,
    pub allowed_imports: Vec<String>,
    pub permissions: WasmPermissions,
}

impl WasmSandbox {
    pub fn new(wasm_bytes: Vec<u8>) -> Self {
        Self {
            module_bytes: wasm_bytes,
            memory_limit: 16 * 1024 * 1024,
            fuel_limit: 1_000_000,
            time_limit_ms: 5_000,
            allowed_imports: Vec::new(),
            permissions: WasmPermissions::default(),
        }
    }

    pub fn from_manifest(wasm_bytes: Vec<u8>, manifest: &PluginManifest) -> Result<Self, String> {
        manifest.validate()?;
        let mut sandbox = Self::new(wasm_bytes);
        sandbox.permissions = WasmPermissions::from_manifest(manifest);
        Ok(sandbox)
    }

    pub fn set_permissions(&mut self, perms: WasmPermissions) {
        self.permissions = perms;
    }

    pub fn allow_import(&mut self, import: impl Into<String>) {
        let import = import.into();
        if !self.allowed_imports.contains(&import) {
            self.allowed_imports.push(import);
        }
    }

    /// Validate module bytes, memory limits, and import policy.
    pub fn validate_module(&self) -> Result<(), String> {
        if self.module_bytes.len() < 8 {
            return Err("module too small to be valid WASM".to_string());
        }

        Validator::new()
            .validate_all(&self.module_bytes)
            .map_err(|err| format!("WASM validation failed: {err}"))?;

        let inspection = self.inspect_module_unvalidated()?;
        let memory_bytes = inspection.declared_memory_pages as usize * WASM_PAGE_SIZE;
        if memory_bytes > self.memory_limit {
            return Err(format!(
                "memory limit exceeded: module declares {memory_bytes} bytes, limit is {}",
                self.memory_limit
            ));
        }

        for import in &inspection.imports {
            self.enforce_import_policy(import)?;
        }

        Ok(())
    }

    pub fn inspect_module(&self) -> Result<WasmModuleInspection, String> {
        self.validate_module()?;
        self.inspect_module_unvalidated()
    }

    /// Execute a bounded subset of exported WASM functions.
    pub fn execute(&self, fn_name: &str, args: &[WasmValue]) -> Result<Vec<WasmValue>, String> {
        self.validate_module()?;
        self.enforce_function_name_policy(fn_name)?;
        let module = ParsedExecutableModule::parse(&self.module_bytes)?;
        let result = module.execute_export(fn_name, args, self.fuel_limit)?;
        Ok(match result {
            WasmValue::Void => Vec::new(),
            value => vec![value],
        })
    }

    fn enforce_import_policy(&self, import: &WasmImport) -> Result<(), String> {
        let full = import.full_name();
        if self.allowed_imports.iter().any(|allowed| allowed == &full) {
            return Ok(());
        }

        let Some(permission) = permission_required_for_host_name(&full) else {
            return Err(format!("import denied by default: {full}"));
        };
        if self.permissions.allows(permission.clone()) {
            Ok(())
        } else {
            Err(format!(
                "permission denied: import '{full}' requires {permission:?}"
            ))
        }
    }

    fn enforce_function_name_policy(&self, fn_name: &str) -> Result<(), String> {
        if fn_name.trim().is_empty() {
            return Err("function name must not be empty".to_string());
        }
        if let Some(permission) = permission_required_for_host_name(fn_name) {
            if !self.permissions.allows(permission.clone()) {
                return Err(format!(
                    "permission denied: function '{fn_name}' requires {permission:?}"
                ));
            }
        }
        Ok(())
    }

    fn inspect_module_unvalidated(&self) -> Result<WasmModuleInspection, String> {
        inspect_wasm_module(&self.module_bytes)
    }
}

impl Default for WasmSandbox {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// A WASM module together with its sandbox configuration.
#[derive(Debug, Clone)]
pub struct WasmModule {
    pub name: String,
    pub bytes: Vec<u8>,
    pub sandbox: WasmSandbox,
}

/// Trait abstracting a WASM runtime engine.
pub trait WasmRuntime {
    fn load(&mut self, module: &WasmModule) -> Result<(), String>;
    fn call(&mut self, function: &str, args: &[WasmValue]) -> Result<WasmValue, String>;
}

#[derive(Debug, Clone)]
struct LoadedWasmModule {
    name: String,
    sandbox: WasmSandbox,
}

/// Default sandboxed runtime used by Sivgo.
#[derive(Debug, Default)]
pub struct SandboxedWasmRuntime {
    loaded_module: Option<LoadedWasmModule>,
}

impl SandboxedWasmRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn loaded_module_name(&self) -> Option<&str> {
        self.loaded_module
            .as_ref()
            .map(|module| module.name.as_str())
    }
}

impl WasmRuntime for SandboxedWasmRuntime {
    fn load(&mut self, module: &WasmModule) -> Result<(), String> {
        let mut sandbox = module.sandbox.clone();
        if sandbox.module_bytes.is_empty() {
            sandbox.module_bytes = module.bytes.clone();
        }
        sandbox.validate_module()?;
        self.loaded_module = Some(LoadedWasmModule {
            name: module.name.clone(),
            sandbox,
        });
        Ok(())
    }

    fn call(&mut self, function: &str, args: &[WasmValue]) -> Result<WasmValue, String> {
        let Some(module) = &self.loaded_module else {
            return Err("no module loaded".to_string());
        };
        let values = module.sandbox.execute(function, args)?;
        Ok(values.into_iter().next().unwrap_or(WasmValue::Void))
    }
}

#[derive(Debug, Clone)]
struct ParsedExecutableModule {
    import_func_count: u32,
    exports: HashMap<String, u32>,
    function_bodies: Vec<Vec<u8>>,
}

impl ParsedExecutableModule {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut import_func_count = 0u32;
        let mut exports = HashMap::new();
        let mut function_bodies = Vec::new();

        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(|err| err.to_string())? {
                Payload::ImportSection(reader) => {
                    for import in flatten_imports(reader)? {
                        if import.kind == WasmImportKind::Function {
                            import_func_count += 1;
                        }
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(|err| err.to_string())?;
                        if export.kind == ExternalKind::Func {
                            exports.insert(export.name.to_string(), export.index);
                        }
                    }
                }
                Payload::CodeSectionEntry(body) => {
                    function_bodies.push(body.as_bytes().to_vec());
                }
                _ => {}
            }
        }

        Ok(Self {
            import_func_count,
            exports,
            function_bodies,
        })
    }

    fn execute_export(
        &self,
        function: &str,
        args: &[WasmValue],
        fuel_limit: u64,
    ) -> Result<WasmValue, String> {
        let Some(function_index) = self.exports.get(function).copied() else {
            return Err(format!("exported function not found: {function}"));
        };
        if function_index < self.import_func_count {
            return Err(format!(
                "exported function '{function}' resolves to an imported host function"
            ));
        }
        let body_index = (function_index - self.import_func_count) as usize;
        let Some(body) = self.function_bodies.get(body_index) else {
            return Err(format!(
                "missing code body for exported function '{function}'"
            ));
        };
        execute_supported_wasm_body(body, args, fuel_limit)
    }
}

fn execute_supported_wasm_body(
    body_bytes: &[u8],
    args: &[WasmValue],
    fuel_limit: u64,
) -> Result<WasmValue, String> {
    let body = FunctionBody::new(BinaryReader::new(body_bytes, 0));
    let mut reader = body
        .get_operators_reader()
        .map_err(|err| format!("failed to read operators: {err}"))?;
    let mut stack: Vec<WasmValue> = Vec::new();
    let mut fuel = fuel_limit;

    while !reader.eof() {
        if fuel == 0 {
            return Err("fuel limit exceeded".to_string());
        }
        fuel -= 1;

        match reader.read().map_err(|err| err.to_string())? {
            Operator::LocalGet { local_index } => {
                let Some(value) = args.get(local_index as usize) else {
                    return Err(format!("local.get out of bounds: {local_index}"));
                };
                stack.push(value.clone());
            }
            Operator::I32Const { value } => stack.push(WasmValue::I32(value)),
            Operator::I64Const { value } => stack.push(WasmValue::I64(value)),
            Operator::F32Const { value } => stack.push(WasmValue::F32(f32::from(value))),
            Operator::F64Const { value } => stack.push(WasmValue::F64(f64::from(value))),
            Operator::Drop => {
                stack
                    .pop()
                    .ok_or_else(|| "drop on empty stack".to_string())?;
            }
            Operator::I32Add => binary_i32(&mut stack, i32::wrapping_add)?,
            Operator::I32Sub => binary_i32(&mut stack, i32::wrapping_sub)?,
            Operator::I32Mul => binary_i32(&mut stack, i32::wrapping_mul)?,
            Operator::I64Add => binary_i64(&mut stack, i64::wrapping_add)?,
            Operator::I64Sub => binary_i64(&mut stack, i64::wrapping_sub)?,
            Operator::I64Mul => binary_i64(&mut stack, i64::wrapping_mul)?,
            Operator::F32Add => binary_f32(&mut stack, |a, b| a + b)?,
            Operator::F32Sub => binary_f32(&mut stack, |a, b| a - b)?,
            Operator::F32Mul => binary_f32(&mut stack, |a, b| a * b)?,
            Operator::F32Div => binary_f32(&mut stack, |a, b| a / b)?,
            Operator::F64Add => binary_f64(&mut stack, |a, b| a + b)?,
            Operator::F64Sub => binary_f64(&mut stack, |a, b| a - b)?,
            Operator::F64Mul => binary_f64(&mut stack, |a, b| a * b)?,
            Operator::F64Div => binary_f64(&mut stack, |a, b| a / b)?,
            Operator::Return | Operator::End => break,
            other => {
                return Err(format!(
                    "unsupported WASM instruction in sandboxed runtime: {other:?}"
                ));
            }
        }
    }

    Ok(stack.pop().unwrap_or(WasmValue::Void))
}

fn binary_i32(stack: &mut Vec<WasmValue>, op: fn(i32, i32) -> i32) -> Result<(), String> {
    let b = pop_i32(stack)?;
    let a = pop_i32(stack)?;
    stack.push(WasmValue::I32(op(a, b)));
    Ok(())
}

fn binary_i64(stack: &mut Vec<WasmValue>, op: fn(i64, i64) -> i64) -> Result<(), String> {
    let b = pop_i64(stack)?;
    let a = pop_i64(stack)?;
    stack.push(WasmValue::I64(op(a, b)));
    Ok(())
}

fn binary_f32(stack: &mut Vec<WasmValue>, op: impl Fn(f32, f32) -> f32) -> Result<(), String> {
    let b = pop_f32(stack)?;
    let a = pop_f32(stack)?;
    stack.push(WasmValue::F32(op(a, b)));
    Ok(())
}

fn binary_f64(stack: &mut Vec<WasmValue>, op: impl Fn(f64, f64) -> f64) -> Result<(), String> {
    let b = pop_f64(stack)?;
    let a = pop_f64(stack)?;
    stack.push(WasmValue::F64(op(a, b)));
    Ok(())
}

fn pop_i32(stack: &mut Vec<WasmValue>) -> Result<i32, String> {
    match stack.pop() {
        Some(WasmValue::I32(value)) => Ok(value),
        other => Err(format!("expected i32 on stack, got {other:?}")),
    }
}

fn pop_i64(stack: &mut Vec<WasmValue>) -> Result<i64, String> {
    match stack.pop() {
        Some(WasmValue::I64(value)) => Ok(value),
        other => Err(format!("expected i64 on stack, got {other:?}")),
    }
}

fn pop_f32(stack: &mut Vec<WasmValue>) -> Result<f32, String> {
    match stack.pop() {
        Some(WasmValue::F32(value)) => Ok(value),
        other => Err(format!("expected f32 on stack, got {other:?}")),
    }
}

fn pop_f64(stack: &mut Vec<WasmValue>) -> Result<f64, String> {
    match stack.pop() {
        Some(WasmValue::F64(value)) => Ok(value),
        other => Err(format!("expected f64 on stack, got {other:?}")),
    }
}

fn inspect_wasm_module(bytes: &[u8]) -> Result<WasmModuleInspection, String> {
    let mut inspection = WasmModuleInspection::default();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|err| err.to_string())? {
            Payload::ImportSection(reader) => {
                for import in flatten_imports(reader)? {
                    if import.kind == WasmImportKind::Function {
                        inspection.imported_functions += 1;
                    }
                    inspection.imports.push(import);
                }
            }
            Payload::FunctionSection(reader) => {
                inspection.defined_functions += reader.count();
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|err| err.to_string())?;
                    inspection.declared_memory_pages = inspection
                        .declared_memory_pages
                        .saturating_add(memory.initial);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|err| err.to_string())?;
                    if let Some(kind) = export_kind(export.kind) {
                        inspection.exports.push(WasmExport {
                            name: export.name.to_string(),
                            kind,
                            index: export.index,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(inspection)
}

fn flatten_imports(reader: wasmparser::ImportSectionReader<'_>) -> Result<Vec<WasmImport>, String> {
    let mut out = Vec::new();
    for group in reader {
        match group.map_err(|err| err.to_string())? {
            Imports::Single(_, import) => {
                push_import(&mut out, import.module, import.name, import.ty);
            }
            Imports::Compact1 { module, items } => {
                for item in items {
                    let item = item.map_err(|err| err.to_string())?;
                    push_import(&mut out, module, item.name, item.ty);
                }
            }
            Imports::Compact2 { module, ty, names } => {
                for name in names {
                    push_import(&mut out, module, name.map_err(|err| err.to_string())?, ty);
                }
            }
        }
    }
    Ok(out)
}

fn push_import(out: &mut Vec<WasmImport>, module: &str, name: &str, ty: TypeRef) {
    let kind = match ty {
        TypeRef::Func(_) | TypeRef::FuncExact(_) => WasmImportKind::Function,
        TypeRef::Memory(_) => WasmImportKind::Memory,
        TypeRef::Table(_) => WasmImportKind::Table,
        TypeRef::Global(_) => WasmImportKind::Global,
        TypeRef::Tag(_) => WasmImportKind::Tag,
    };
    out.push(WasmImport {
        module: module.to_string(),
        name: name.to_string(),
        kind,
    });
}

fn export_kind(kind: ExternalKind) -> Option<WasmExportKind> {
    match kind {
        ExternalKind::Func => Some(WasmExportKind::Function),
        ExternalKind::Memory => Some(WasmExportKind::Memory),
        ExternalKind::Table => Some(WasmExportKind::Table),
        ExternalKind::Global => Some(WasmExportKind::Global),
        ExternalKind::Tag => Some(WasmExportKind::Tag),
        ExternalKind::FuncExact => None,
    }
}

fn permission_required_for_host_name(name: &str) -> Option<Permission> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("modify")
        || lower.contains("set_")
        || lower.contains("add_node")
        || lower.contains("create_")
        || lower.contains("delete")
    {
        Some(Permission::ModifyDocument)
    } else if lower.contains("read")
        || lower.contains("get_selection")
        || lower.contains("document")
    {
        Some(Permission::ReadDocument)
    } else if lower.contains("network") || lower.contains("fetch") || lower.contains("http") {
        Some(Permission::NetworkAccess)
    } else if lower.contains("filesystem") || lower.contains("file_") || lower.contains("fs_") {
        Some(Permission::AccessFileSystem)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasm_answer() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // type
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // func
            0x03, 0x02, 0x01, 0x00, // export "answer"
            0x07, 0x0a, 0x01, 0x06, b'a', b'n', b's', b'w', b'e', b'r', 0x00, 0x00, // code
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b,
        ]
    }

    fn wasm_add() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // type
            0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // func
            0x03, 0x02, 0x01, 0x00, // export "add"
            0x07, 0x07, 0x01, 0x03, b'a', b'd', b'd', 0x00, 0x00, // code
            0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
        ]
    }

    fn wasm_import_modify() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // type: [] -> []
            0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // import sivgo.modify_document
            0x02, 0x19, 0x01, 0x05, b's', b'i', b'v', b'g', b'o', 0x0f, b'm', b'o', b'd', b'i',
            b'f', b'y', b'_', b'd', b'o', b'c', b'u', b'm', b'e', b'n', b't', 0x00, 0x00,
        ]
    }

    #[test]
    fn sandbox_validates_and_inspects_real_wasm() {
        let sb = WasmSandbox::new(wasm_answer());
        assert!(sb.validate_module().is_ok());
        let inspection = sb.inspect_module().unwrap();
        assert_eq!(inspection.defined_functions, 1);
        assert_eq!(inspection.exports[0].name, "answer");
    }

    #[test]
    fn sandbox_executes_exported_numeric_functions() {
        let sb = WasmSandbox::new(wasm_answer());
        assert_eq!(sb.execute("answer", &[]).unwrap(), vec![WasmValue::I32(42)]);

        let sb = WasmSandbox::new(wasm_add());
        assert_eq!(
            sb.execute("add", &[WasmValue::I32(20), WasmValue::I32(22)])
                .unwrap(),
            vec![WasmValue::I32(42)]
        );
    }

    #[test]
    fn sandbox_denies_imports_without_permission() {
        let sb = WasmSandbox::new(wasm_import_modify());
        let err = sb.validate_module().unwrap_err();
        assert!(err.contains("permission denied"));

        let mut sb = WasmSandbox::new(wasm_import_modify());
        sb.set_permissions(WasmPermissions {
            can_modify_document: true,
            ..Default::default()
        });
        assert!(sb.validate_module().is_ok());
    }

    #[test]
    fn sandbox_allows_explicit_import_allowlist() {
        let mut sb = WasmSandbox::new(wasm_import_modify());
        sb.allow_import("sivgo.modify_document");
        assert!(sb.validate_module().is_ok());
    }

    #[test]
    fn runtime_load_and_call_uses_sandbox() {
        let mut rt = SandboxedWasmRuntime::new();
        let module = WasmModule {
            name: "answer".to_string(),
            bytes: wasm_answer(),
            sandbox: WasmSandbox::new(wasm_answer()),
        };

        rt.load(&module).unwrap();
        assert_eq!(rt.loaded_module_name(), Some("answer"));
        assert_eq!(rt.call("answer", &[]), Ok(WasmValue::I32(42)));
    }

    #[test]
    fn runtime_call_without_load_fails() {
        let mut rt = SandboxedWasmRuntime::new();
        assert!(rt.call("anything", &[]).is_err());
    }

    #[test]
    fn sandbox_rejects_invalid_and_unsupported_modules() {
        let sb = WasmSandbox::new(vec![0x00, 0x01]);
        assert!(sb.validate_module().is_err());

        let sb = WasmSandbox::new(wasm_answer());
        assert!(sb.execute("", &[]).is_err());
        assert!(sb.execute("missing", &[]).is_err());
    }

    #[test]
    fn permissions_from_manifest() {
        let manifest = PluginManifest {
            id: "ext.wasm".to_string(),
            name: "WASM".to_string(),
            version: "1.0.0".to_string(),
            author: "Tester".to_string(),
            description: "WASM extension".to_string(),
            permissions: vec![Permission::ModifyDocument, Permission::NetworkAccess],
        };
        let sb = WasmSandbox::from_manifest(wasm_answer(), &manifest).unwrap();
        assert!(sb.permissions.can_read_document);
        assert!(sb.permissions.can_modify_document);
        assert!(sb.permissions.can_access_network);
        assert!(!sb.permissions.can_access_filesystem);
    }
}
