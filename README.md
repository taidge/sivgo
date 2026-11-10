# Sivgo

Sivgo is a Rust SVG vector graphics editor prototype inspired by Inkscape. The current workspace provides a native Glory/Apace desktop application with Sapid widgets and a Salvo REST API backend; a Web/WASM client is not present in this worktree.

## Architecture

Sivgo is organized as a Cargo workspace with 15 crates:

| Crate | Description |
|-------|-------------|
| `sivgo-math` | 2D geometry primitives, transforms, bezier curves, hit testing |
| `sivgo-dom` | SVG document model with arena-based tree and all SVG elements |
| `sivgo-style` | CSS cascade, specificity, computed values, inheritance and stylesheet rules for rendering |
| `sivgo-render` | `RenderBackend` trait and the tiny-skia CPU backend |
| `sivgo-path-ops` | Boolean path operations (union, intersect, diff, xor), offset, simplify |
| `sivgo-text` | Text shaping via cosmic-text, SVG text layout, text-to-path |
| `sivgo-filters` | All SVG filter primitives (blur, blend, lighting, etc.) |
| `sivgo-trace` | Bitmap-to-vector tracing (potrace algorithm) |
| `sivgo-io` | Export: SVG, SVGZ, PNG, JPEG, WebP, PDF, EPS, DXF. Import: SVG, SVGZ, bitmaps, and a partial PDF reader |
| `sivgo-core` | Editor engine: tools, commands, undo/redo, snapping, selection |
| `sivgo-plugin` | Plugin manifests and a permission model. Modules are validated with `wasmparser` and run on a small built-in interpreter that covers numeric functions only; the scripting layer is a restricted expression DSL, not a Lua runtime |
| `sivgo-ui` | Glory semantic UI components and Sapid controls used by the desktop app |
| `sivgo-agent` | OpenAI-compatible conversation client and SVG proposal validation |
| `sivgo-desktop` | Desktop application binary (Glory Apace desktop host) |
| `sivgo-web` | Salvo REST API backend for documents, assets, export, preferences, plugins, and AI suggestions |

Each crate depends only on what it actually uses:

```
sivgo-math    (foundation, no internal dependencies)
sivgo-dom     math
sivgo-style   dom
sivgo-path-ops
sivgo-text    math
sivgo-filters
sivgo-trace   math
sivgo-render  math + dom + style + text + filters
sivgo-io      math + dom + render + trace
sivgo-core    math + dom + path-ops + text
sivgo-plugin  math + dom
sivgo-agent   dom
sivgo-ui      math + core + dom + render
sivgo-desktop math + dom + style + render + io + core + plugin + ui + agent
sivgo-web     dom + render + agent
```

## Prerequisites

- **Rust** 1.99+ (install via [rustup](https://rustup.rs))
- **just** command runner (install via `cargo install just`)

The native UI uses [Glory 0.7.0](https://crates.io/crates/glory/0.7.0) and
[Sapid 0.7.1](https://crates.io/crates/sapid/0.7.1) from crates.io. Apace and
Wingy are also resolved from crates.io. Use `cargo test --workspace --locked`
to verify the checked-in dependency set. Cargo inherits the caller's build
environment. Run `cargo metadata --no-deps --format-version 1` in this workspace
to find its effective `target_directory`. The previous Dioxus app is preserved
on `old-dioxus`.

No external database is required for the current development backend; `sivgo-web` uses in-memory stores.

See the [Inkscape comparison and development history](docs/inkscape-parity.md)
for current feature gaps and the order of work.

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

## Desktop file recovery

**File → Open Recent** lists drawings opened or saved in the editor and lets
you clear the list. A missing file is removed when you try to open it.
The recent file limit and autosave interval are under **Preferences → I/O**.

Autosave writes a separate recovery draft for unsaved changes. On the next
launch after an interruption, Sivgo offers the latest available draft.
**File → Recover Draft** opens the same prompt if you chose **Later**.
A recovered drawing is unsaved and asks for a destination when you save.
Saving a drawing or explicitly discarding its edits removes its recovery
draft. Recovery files are limited to 128 MiB each. SVG and SVGZ saves stage a
complete replacement beside the destination before publishing it.

## AI Agent

Open **AI Agent** from the desktop toolbar. In provider settings, enter the API
base URL (including its version prefix, such as `https://api.openai.com/v1`),
a model ID supported by that provider, and its API key if required. A local
provider can use HTTP on a loopback address. The desktop keeps settings and
chat in memory. Set `SIVGO_AI_BASE_URL`, `SIVGO_AI_MODEL`, and
`SIVGO_AI_API_KEY` before launch to prefill the fields.

Send an instruction such as “Add a blue circle in the center.” The provider
receives the current SVG and recent chat. Review the proposed drawing in the
sidebar, then choose **Apply edit** or **Discard**. Applying records one undo
step. You can ask a follow-up or start a new chat. Suggestions apply to the
drawing state used for the request. AI editing accepts SVG source up to 256 KiB
and proposals within the supported static SVG subset.

The same provider integration is available at authenticated
`POST /api/v1/agent/suggest`. Set `SIVGO_AI_ENABLE_API=1` and the provider
environment variables on the server to enable it. Send JSON with `svg`,
`prompt`, optional `history` (entries with `role` and `content`), and optional
`selected_ids` (SVG XML IDs). The response contains `message` and nullable
`svg`. Clients decide how to apply the suggestion. The endpoint shares a
per-process limit of 10 requests per minute and two concurrent provider calls.
See `/api/v1/openapi.json` for the schema and the
[OpenAI Chat Completions reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
for the upstream protocol.

## Project Layout

```
sivgo/
  Cargo.toml              # Workspace root
  justfile                 # Task runner recipes
  crates/                  # All 15 library and binary crates
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
