# Aggressive render backend tasks

Goal: complete the rendering-backend backlog item without waiting for full
native GPU integration by replacing empty Vello/Canvas2D/WebGPU stubs with real
render-command replay paths, deterministic pixels, and explicit capability
metadata.

- [x] Replace the Vello backend stub with a complete `RenderBackend`
  implementation that records Vello-style scene commands and renders every
  operation through a deterministic backend.
- [x] Replace Canvas2D fallback stubs with a complete browser command stream and
  deterministic pixel replay for native tests.
- [x] Add a WebGPU browser backend surface with command recording and the same
  deterministic replay behavior.
- [x] Expose backend capability/status data so callers can distinguish native
  GPU, browser Canvas2D, browser WebGPU, and CPU replay modes.
- [x] Add focused render tests proving Vello, Canvas2D, and WebGPU backends draw
  non-rect geometry, strokes, clips/masks, images, offscreen groups, and command
  streams instead of returning blank/stub output.
- [x] Run render tests and desktop check, then check the main `_todos.md`
  rendering item.
