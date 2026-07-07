# Sivgo

Sivgo is a Rust SVG vector graphics editor prototype inspired by Inkscape. The current workspace provides a native Dioxus desktop application and a Salvo REST API backend; a Web/WASM client is not present in this worktree.

## Architecture

Sivgo is organized as a Cargo workspace with 14 crates:

| Crate | Description |
|-------|-------------|
| `sivgo-math` | 2D geometry primitives, transforms, bezier curves, hit testing |
| `sivgo-dom` | SVG document model with arena-based tree and all SVG elements |
| `sivgo-style` | CSS cascade, specificity, computed values, inheritance |
| `sivgo-render` | `RenderBackend` trait + backends (tiny-skia, vello, canvas2d) |
| `sivgo-path-ops` | Boolean path operations (union, intersect, diff, xor), offset, simplify |
| `sivgo-text` | Text shaping via cosmic-text, SVG text layout, text-to-path |
| `sivgo-filters` | All SVG filter primitives (blur, blend, lighting, etc.) |
| `sivgo-trace` | Bitmap-to-vector tracing (potrace algorithm) |
| `sivgo-io` | Import/export: SVG, PNG, JPEG, PDF, EPS, DXF |
| `sivgo-core` | Editor engine: tools, commands, undo/redo, snapping, selection |
| `sivgo-plugin` | Plugin system with WASM sandbox and Lua scripting |
| `sivgo-ui` | Dioxus UI components used by the desktop app |
| `sivgo-desktop` | Desktop application binary (Dioxus desktop) |
| `sivgo-web` | Salvo REST API backend for documents, assets, export, preferences, and plugins |

```
sivgo-math (foundation)
  -> sivgo-dom -> sivgo-style -> sivgo-render
  -> sivgo-path-ops
  -> sivgo-text, sivgo-filters, sivgo-trace
  -> sivgo-io (dom + render + style)
  -> sivgo-core (all above)
  -> sivgo-plugin (core + dom)
  -> sivgo-ui (core + dom + render)
    -> sivgo-desktop
  -> sivgo-web (core + dom + render + io)
```

## Prerequisites

- **Rust** 1.89+ (install via [rustup](https://rustup.rs))
- **just** command runner (install via `cargo install just`)

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

## Project Layout

```
sivgo/
  Cargo.toml              # Workspace root
  justfile                 # Task runner recipes
  crates/                  # All 14 library and binary crates
  docs/                    # Architecture docs
  examples/                # Sample SVG files
  tests/visual/            # Visual test helpers and corpus
  crates/dom/benches/      # Workspace performance benchmarks
  fuzz/                    # Fuzz testing targets
  .github/workflows/       # CI pipeline
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
