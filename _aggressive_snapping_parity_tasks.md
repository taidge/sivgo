# Aggressive snapping parity tasks

Updated: 2026-05-31

Goal: finish the remaining snapping parity item by adding explicit snap-source
controls in addition to existing snap-target controls.

- [x] Add snap source kinds and preferences for cursor, object nodes, bbox
  corners, bbox centers, and bbox midpoints.
- [x] Add source-aware snapping that prioritizes object nodes over bbox
  sources and cursor fallback when snap distances tie.
- [x] Expose advanced snap source toggles in Preferences state and dialog.
- [x] Apply persisted snap source preferences to the core snapping engine in
  the desktop shell.
- [x] Add focused core/UI tests for source prioritization and preference
  defaults.
- [x] Run formatting, focused tests, and desktop check.
