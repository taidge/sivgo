//! `sivgo-desktop` -- Desktop application integration modules.
//!
//! These modules provide self-contained features that augment the main
//! Sivgo desktop editor (see `main.rs`):
//!
//! * [`clipboard`] -- System clipboard integration (copy/paste SVG & bitmap).
//! * [`plugins`] -- Built-in plugin/extension system wiring.

pub mod clipboard;
pub mod plugins;
