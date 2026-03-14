//! `sivgo-path-ops` -- Path boolean operations and utilities for the Sivgo editor.
//!
//! Provides boolean operations (union, intersection, difference, xor), path
//! offset (inset/outset), simplification (Ramer-Douglas-Peucker), cut/slice,
//! stroke-to-path, and shape-to-path / combine / break-apart utilities.
//!
//! Boolean operations are powered by the [`i_overlay`] crate. Paths are
//! flattened to polygons for processing and converted back afterward.

pub mod boolean;
pub mod convert;
pub mod cut;
pub mod dynamic_offset;
pub mod flatten;
pub mod linked_offset;
pub mod offset;
pub mod refit;
pub mod simplify;
pub mod slice;
pub mod stroke;

// Re-export the most commonly used items at the crate root.
pub use boolean::{boolean_op, difference, intersection, union, xor, BooleanOp};
pub use convert::{
    break_apart, circle_to_path, combine_paths, element_to_path, ellipse_to_path, line_to_path,
    polygon_to_path_element, polyline_to_path, rect_to_path, reverse_path,
};
pub use cut::cut;
pub use dynamic_offset::DynamicOffset;
pub use flatten::flatten_path;
pub use linked_offset::LinkedOffset;
pub use offset::offset_path;
pub use refit::{polygon_to_path, shapes_to_path};
pub use simplify::simplify_path;
pub use slice::slice;
pub use stroke::{stroke_to_path, stroke_to_path_simple, LineCap, LineJoin, StrokeParams};
