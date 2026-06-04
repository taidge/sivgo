# Aggressive plugin system tasks

Goal: replace plugin execution stubs with enforceable manifest validation,
permissions, WASM/Lua execution boundaries, and extension UI metadata.

- [x] Add manifest validation and a normalized permission-set model.
- [x] Make plugin installation read and validate real manifest files instead of
  synthesizing installed plugins from path names.
- [x] Replace the WASM magic-number/mock-result path with wasmparser validation,
  import/memory policy checks, and a bounded sandboxed runtime for exported
  numeric functions.
- [x] Replace Lua hard-coded API stubs with permission-aware host API calls that
  produce auditable host commands.
- [x] Add extension UI registry metadata for menu contributions and plugin
  dialogs with permission validation.
- [x] Add focused plugin tests for manifest rejection, permission denial,
  WASM execution, Lua host API enforcement, registry install, and extension UI.
- [x] Run plugin tests and desktop check, then check the main `_todos.md`
  plugin item.
