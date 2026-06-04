# Aggressive filter editor parity tasks

Goal: make the Filter Editor operate on real SVG filter definitions attached to
the selected object, with live renderer feedback and menu presets.

- [x] Add core selected-filter primitive state and default parameters.
- [x] Create/rebuild SVG `<filter>` definitions in `<defs>` and attach them to
  the selected object through undoable engine operations.
- [x] Support add, remove, reorder, enable/disable, and parameter edits.
- [x] Expose default editable parameters for every Filter Editor primitive kind.
- [x] Wire the desktop inspector Filter tab, Filter Editor menu action, Blur,
  and Drop Shadow presets.
- [x] Make the renderer skip disabled primitives and execute editor-created
  displacement/convolution primitives.
- [x] Add focused tests and run formatting/checks.
