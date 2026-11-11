# Frontend E2E integration test strategy

Updated: 2026-06-01

## Goal

Provide CI-friendly coverage for the desktop editor's main user workflows
without depending on fragile manual screenshots. Native desktop visual testing
can be added later with a driver, but this phase locks down the contracts that
caused the reported regressions.

## Test Layers

- [x] Shell contract tests: the app must render one menu source only, with the
  native menu handling editor actions and the in-window topbar limited to brand
  and command buttons.
- [x] Toolbar contract tests: every exposed tool must have an SVG icon
  descriptor and no toolbar icon should be a plain letter fallback.
- [x] Canvas interaction tests: simulate drag flows through `EditorEngine` using
  the same coordinate normalization rules as the frontend and assert real SVG
  nodes are created.
- [x] Inspector contract tests: selecting tools such as Text, Pages, LPE,
  Filters, and Resources must be reachable without layout-only placeholders.
- [x] Render smoke tests: after a draw operation, render the document to a PNG
  data URI and assert the image payload is non-empty.

## Future Browser/Desktop Driver

2026-11-12 native regression additions:

- [x] Mock provider roundtrip verifies bearer credentials, selection context,
  three conversation turns, preview, explicit apply, retained object IDs and
  selection, and undo/redo across full SVG and object attribute proposals.
- [x] Stop action cancels an in-flight HTTP connection, restores the prompt,
  leaves the drawing clean and ignores background completion.
- [x] Native panel mounting and compact tree interactions cover vector
  disclosure, visibility and lock icons with unchanged accessible action names.
- [x] Inspect an actual Apace raster of the proposed drawing and attribute
  before/after review. Set `SIVGO_TEST_SNAPSHOT` to an output PNG path when running
  the desktop agent preview test to capture it again.

Future expansion: add a Playwright or Windows UI Automation layer once the
desktop runtime exposes stable selectors or an embedded browser debug port. This
layer should click the real toolbar, drag the real canvas, and compare
screenshots at 1280x800 and the minimum 1120x720 viewport.
