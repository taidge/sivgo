# Aggressive collaboration tasks

Goal: replace collaboration stubs with deterministic CRDT/OT reconciliation,
transport fan-out, presence/comment-compatible messages, conflict handling, and
version rollback.

- [x] Add materialized CRDT node state, tombstones, duplicate detection, and
  buffered gap reconciliation.
- [x] Replace OT placeholder behavior with deterministic no-op transforms and
  batch transformation.
- [x] Replace local-only transport behavior with an in-memory room hub that
  fans messages out to connected peers.
- [x] Store exact version operations and inverse operations for rollback instead
  of generating placeholder inverse changes from summaries.
- [x] Add focused tests for convergence, gap buffering, conflict transforms,
  transport fan-out, comments/presence messages, and exact rollback.
- [x] Run core collaboration tests and desktop check, then check the main
  `_todos.md` collaboration item.
