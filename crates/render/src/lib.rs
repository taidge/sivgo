//! `sivgo-render` -- Rendering pipeline for the Sivgo SVG editor.
//!
//! This crate provides:
//!
//! * [`RenderBackend`] -- a trait abstracting 2D rasterization surfaces.
//! * [`TinySkiaBackend`] -- a CPU-based implementation using `tiny-skia`.
//! * [`DocumentRenderer`] -- walks an [`sivgo_dom::SvgDocument`] tree and
//!   dispatches drawing commands to a backend.
//! * [`render_document_to_pixmap`] -- convenience function that renders a
//!   document to an RGBA pixel buffer in one call.
//! * [`DamageTracker`] -- tracks dirty rectangular regions for incremental
//!   rendering.
//! * [`TileGrid`] / [`Tile`] -- tile-based rendering for large documents.
//! * [`RenderCache`] -- generation-based cache to skip full re-renders.

pub mod backend;
pub mod damage;
pub mod mesh_gradient;
pub mod render_cache;
pub mod renderer;
pub mod tiling;
pub mod tiny_skia_backend;
pub mod types;

pub use backend::RenderBackend;
pub use damage::DamageTracker;
pub use render_cache::RenderCache;
pub use renderer::{
    render_document_to_pixmap, render_document_to_pixmap_without_file_access, DocumentRenderer,
};
pub use tiling::{Tile, TileGrid, DEFAULT_TILE_SIZE};
pub use tiny_skia_backend::TinySkiaBackend;
pub use types::{
    FillRule, GradientStop, GradientUnits, LineCap, LineJoin, RenderPaint, SpreadMethod,
    StrokeStyle,
};
