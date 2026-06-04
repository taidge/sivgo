# Aggressive LPE parity tasks

Goal: replace the disconnected Live Path Effects panel with a non-destructive
selected-path LPE stack that persists on SVG path metadata and can be edited
from the desktop inspector.

- [x] Add a core LPE stack metadata model for selected paths.
- [x] Reapply enabled LPE entries from `inkscape:path-effect` against
  `inkscape:original-d` so edits are non-destructive.
- [x] Support add, remove, reorder, enable/disable, and parameter update
  operations through undoable engine commands.
- [x] Expose default editable parameters for every LPE kind shown in the UI
  panel.
- [x] Wire the desktop inspector LPE tab to selected path state and engine
  actions.
- [x] Add focused tests and run formatting/checks.
