# Sivgo

Sivgo is a Rust SVG vector graphics editor prototype inspired by Inkscape. The current workspace provides a native Glory/Blitz desktop application with Sapid widgets and a Salvo REST API backend; a Web/WASM client is not present in this worktree.

## Architecture

Sivgo is organized as a Cargo workspace with 14 crates:

| Crate | Description |
|-------|-------------|
| `sivgo-math` | 2D geometry primitives, transforms, bezier curves, hit testing |
| `sivgo-dom` | SVG document model with arena-based tree and all SVG elements |
| `sivgo-style` | CSS cascade, specificity, computed values, inheritance (`<style>` rules are resolved here; the renderer applies inline `style` and inheritance directly) |
| `sivgo-render` | `RenderBackend` trait and the tiny-skia CPU backend. The Vello, Canvas2D and WebGPU types record a command stream and replay it through tiny-skia; there is no GPU or browser execution path |
| `sivgo-path-ops` | Boolean path operations (union, intersect, diff, xor), offset, simplify |
| `sivgo-text` | Text shaping via cosmic-text, SVG text layout, text-to-path |
| `sivgo-filters` | All SVG filter primitives (blur, blend, lighting, etc.) |
| `sivgo-trace` | Bitmap-to-vector tracing (potrace algorithm) |
| `sivgo-io` | Export: SVG, PNG, JPEG, WebP, PDF, EPS, DXF. Import: SVG, bitmaps, and a partial PDF reader |
| `sivgo-core` | Editor engine: tools, commands, undo/redo, snapping, selection |
| `sivgo-plugin` | Plugin manifests and a permission model. Modules are validated with `wasmparser` and run on a small built-in interpreter that covers numeric functions only; the scripting layer is a restricted expression DSL, not a Lua runtime |
| `sivgo-ui` | Glory semantic UI components and Sapid controls used by the desktop app |
| `sivgo-desktop` | Desktop application binary (Glory Blitz desktop host) |
| `sivgo-web` | Salvo REST API backend for documents, assets, export, preferences, and plugins |

Each crate depends only on what it actually uses:

```
sivgo-math    (foundation, no internal dependencies)
sivgo-dom     math
sivgo-style   dom                          (not yet used by other crates)
sivgo-path-ops
sivgo-text    math
sivgo-filters
sivgo-trace   math
sivgo-render  math + dom + text + filters
sivgo-io      math + dom + render + trace
sivgo-core    math + dom + path-ops + text
sivgo-plugin  math + dom
sivgo-ui      math + core + dom + render
sivgo-desktop math + dom + render + io + core + plugin + ui
sivgo-web     dom + render
```

## Prerequisites

- **Rust** 1.94+ (install via [rustup](https://rustup.rs))
- **just** command runner (install via `cargo install just`)

The `glory` branch uses sibling source checkouts:

```text
D:/Works/glory-rs/glory
D:/Works/glory-rs/sapid
D:/Works/taidge/sivgo
```

Workspace dependencies resolve these paths relative to Sivgo's root. Glory is
loaded from `../../glory-rs/glory/crates/glory`, and Sapid from
`../../glory-rs/sapid`. Keep these repositories in that relative layout.
The desktop host renders through Blitz; it does not require a WebView.

On Windows the checked-in Cargo configuration places build output in
`D:/BuildCache/sivgo-glory/target` and compiler temporary files in
`D:/BuildCache/sivgo-glory/tmp`. Create that temporary directory before building.
Use `CARGO_HOME=D:/BuildCache/cargo` for downloaded dependency caches.
`./scripts/cargo.ps1 test --workspace` sets these paths, creates the directories,
and verifies Cargo metadata before executing the requested Cargo command. On other
systems override `CARGO_TARGET_DIR` and update the temporary paths in
`.cargo/config.toml`; for
WSL use a verified D-drive mount under `/mnt/d/BuildCache/sivgo-glory`.
After setting the environment, `cargo metadata --no-deps --format-version 1`
reports the effective `target_directory`.

No external database is required for the current development backend; `sivgo-web` uses in-memory stores.

## Quick Start

```sh
# Build everything
just build

# Run all tests
just test

# Run the desktop app
just desktop

# Run the web server
just serve

# Render an SVG to PNG from CLI
just render examples/basic_shapes.svg output.png
```

For local development, `sivgo-web` uses an in-memory store and permissive
CORS defaults. Production runs must set a non-default `JWT_SECRET`, set
`SIVGO_ENV=production`, and provide exact frontend origins through
`CORS_ALLOWED_ORIGINS`, for example:

```sh
SIVGO_ENV=production \
JWT_SECRET=<strong-random-secret-at-least-32-bytes> \
CORS_ALLOWED_ORIGINS=https://app.example.com \
just serve
```

When the server is reachable only through a trusted reverse proxy that
sanitizes client-IP headers, set `TRUST_PROXY_HEADERS=true`. Leave it unset for
direct deployments so forged `X-Forwarded-For` values cannot bypass rate
limiting.

## Project Layout

```
sivgo/
  Cargo.toml              # Workspace root
  justfile                 # Task runner recipes
  crates/                  # All 14 library and binary crates
  docs/                    # Architecture docs
  examples/                # Sample SVG files
  tests/visual/            # Comparison helpers and SVG corpus for the render smoke tests
                           # (no committed reference images yet, so these check
                           #  that rendering is non-empty and deterministic)
  crates/dom/benches/      # Workspace performance benchmarks
  fuzz/                    # Fuzz testing targets
  .github/workflows/       # CI pipeline
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
