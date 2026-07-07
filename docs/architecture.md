# Sivgo Architecture

Sivgo is an SVG editor written in Rust, designed as a modern alternative to
Inkscape. It uses a 14-crate workspace to enforce clean separation of concerns,
with a Dioxus-based desktop UI and a Salvo-based REST API backend. The current
worktree does not include a Web/WASM frontend.

## Crate Dependency Graph

```
sivgo-desktop (binary: sivgo)
  ├── sivgo-ui          ── Dioxus UI components and panels
  │     ├── sivgo-core   ── Editor engine, tools, commands, undo/redo
  │     │     ├── sivgo-dom    ── SVG document object model (parse/serialize)
  │     │     │     └── sivgo-math  ── Geometry primitives (wraps kurbo)
  │     │     ├── sivgo-render ── Rendering pipeline
  │     │     │     ├── sivgo-dom
  │     │     │     ├── sivgo-text     ── Text shaping (cosmic-text, swash)
  │     │     │     ├── sivgo-filters  ── SVG filter effects
  │     │     │     └── sivgo-math
  │     │     └── sivgo-style  ── CSS/presentation attribute resolution
  │     │           ├── sivgo-dom
  │     │           └── sivgo-math
  │     ├── sivgo-dom
  │     ├── sivgo-render
  │     └── sivgo-math
  ├── sivgo-io          ── Import/export (PNG, JPEG, SVG, SVGZ)
  │     ├── sivgo-dom
  │     ├── sivgo-render
  │     └── sivgo-math
  ├── sivgo-dom
  ├── sivgo-render
  └── sivgo-math

sivgo-web (binary: sivgo-server)
  ├── sivgo-core
  ├── sivgo-dom
  ├── sivgo-render
  ├── sivgo-io
  └── sivgo-math

sivgo-plugin              ── Plugin system
  ├── sivgo-core
  ├── sivgo-dom
  └── sivgo-math

sivgo-path-ops            ── Boolean path operations (wraps i_overlay)
  ├── sivgo-dom
  └── sivgo-math

sivgo-trace               ── Bitmap-to-vector tracing
  └── sivgo-math
```

## Crate Summaries

| Crate | Purpose |
|-------|---------|
| **sivgo-math** | Geometry primitives: points, rects, transforms, bezier paths. Wraps `kurbo`. |
| **sivgo-dom** | SVG document tree with typed elements and presentation attributes. Parses with `roxmltree`, serializes with `xmlwriter`. Includes a DOM event/observer system. |
| **sivgo-style** | CSS cascade and presentation attribute resolution. |
| **sivgo-render** | Renders the DOM tree to pixels via the `RenderBackend` trait. Includes `TinySkiaBackend` (CPU), with stubs for Vello (GPU) and Canvas2D (WASM). Handles gradients, clip paths, masks, filters, markers, and image decoding. |
| **sivgo-text** | Text shaping and layout using `cosmic-text` and `swash`. |
| **sivgo-filters** | SVG filter effect primitives (blur, color matrix, morphology, lighting, etc.). |
| **sivgo-trace** | Bitmap-to-vector tracing (potrace-style). |
| **sivgo-path-ops** | Boolean operations on paths (union, intersection, difference, XOR) via `i_overlay`. |
| **sivgo-io** | File I/O: SVG/SVGZ read/write, PNG/JPEG/WebP export, base64 image embedding. |
| **sivgo-core** | Editor engine: tools (select, pen, pencil, shapes, text, etc.), the command pattern for undo/redo, live path effects (LPEs), snap system, viewport management, and keyboard shortcuts. |
| **sivgo-plugin** | Plugin system allowing extensions to register tools and commands. |
| **sivgo-ui** | Dioxus UI layer for the desktop app: canvas widget, toolbox, property panels, layer panel. |
| **sivgo-desktop** | Desktop application entry point. Integrates Dioxus Desktop, file dialogs (`rfd`), clipboard (`arboard`), and CLI argument parsing (`clap`). |
| **sivgo-web** | Web backend using Salvo. Exposes REST APIs for server-side SVG editing, rendering, export, assets, preferences, and plugins. Current storage is in-memory. |

## Key Design Decisions

### Arena-based DOM

The SVG document (`SvgDocument`) stores nodes in a flat `HashMap<NodeId, SvgNode>`
rather than a recursive `Box`-tree. This gives O(1) node lookup by ID, avoids
borrow-checker issues with recursive mutable references, and makes
serialization straightforward. Parent/child relationships are tracked via
`NodeId` references stored alongside each node.

### Command Pattern for Undo/Redo

Every editing operation implements the `Command` trait with `execute` and `undo`
methods. The `CommandHistory` maintains undo and redo stacks with a configurable
maximum depth. Commands operate on `&mut SvgDocument`, making them
backend-agnostic. Compound commands group multiple atomic operations into a
single undo step.

### RenderBackend Trait

Rendering is abstracted behind the `RenderBackend` trait, which defines
operations for filling/stroking shapes, clipping, masking, offscreen group
compositing, and image drawing. The primary implementation is `TinySkiaBackend`
(CPU rasterization via `tiny-skia`). This trait allows future GPU backends
(Vello) or browser Canvas2D backends to be swapped in without changing the
rendering pipeline logic.

### DOM Observer / Event Bus

The DOM includes a lightweight event bus (`DomEvent`). Mutations to the document
push events that consumers (e.g., the render cache, UI layer) can drain each
frame. This decouples the DOM from its consumers and enables incremental
re-rendering via the damage/tiling system in `sivgo-render`.

### Live Path Effects (LPEs)

Following Inkscape's design, LPEs are non-destructive path transformations
(fillet, power stroke, bend, pattern-along-path, etc.) stored as metadata on
the original path. They are re-applied whenever the source path changes,
keeping the original geometry intact for further editing.
