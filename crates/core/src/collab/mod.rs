//! Real-time collaboration subsystem.
//!
//! This module groups the building blocks for multi-user editing:
//!
//! - [`crdt`]: a CRDT operation model with version vectors.
//! - [`ot`]: operational-transform helpers over CRDT ops.
//! - [`transport`]: an in-process transport hub for broadcasting ops.
//! - [`presence`] / [`awareness`]: peer cursor/selection state.
//! - [`comments`]: anchored document comments and threads.
//! - [`version_history`]: snapshot/branch history over CRDT ops.
//! - [`conflict_ui`]: conflict-resolution descriptors for the UI.
//!
//! The pieces are self-contained and transport-agnostic so they can be driven
//! by any networking layer.

pub mod awareness;
pub mod comments;
pub mod conflict_ui;
pub mod crdt;
pub mod ot;
pub mod presence;
pub mod transport;
pub mod version_history;
