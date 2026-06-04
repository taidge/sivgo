# Aggressive stroke edge tasks

Updated: 2026-05-31

Goal: finish the remaining Stroke to Path edge slice by making marker and
vector-effect cases explicit, testable, and undoable through the existing core
operation.

- [x] Preserve marker references as conversion metadata when Stroke to Path
  clears live marker rendering attributes.
- [x] Preserve vector-effect source state as conversion metadata and clear the
  live vector-effect from the filled outline.
- [x] Bake transform stroke scale into normal Stroke to Path output.
- [x] Keep `vector-effect: non-scaling-stroke` at the original visible stroke
  width after transform baking.
- [x] Add focused core tests for marker metadata and vector-effect stroke
  widths.
- [x] Run formatting, focused core tests, and desktop check.
