//! `sivgo-desktop` -- Desktop application integration modules.
//!
//! These modules provide self-contained features that augment the main
//! Sivgo desktop editor (see `main.rs`):
//!
//! * [`clipboard`] -- System clipboard integration (copy/paste SVG & bitmap).
//! * [`window`]    -- Window title formatting and multi-document tab management.
//! * [`autosave`]  -- Auto-save path generation, recovery file discovery, and cleanup.
//! * [`batch`]     -- Command-line batch processing (SVG -> PNG, etc.).

pub mod autosave;
pub mod batch;
pub mod clipboard;
pub mod window;
