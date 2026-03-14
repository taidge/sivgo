//! `sivgo-math` -- Foundation math library for the Sivgo editor.
//!
//! Provides 2D geometry types, affine transforms, Bezier path handling,
//! SVG transform parsing, and hit testing. Built on top of `kurbo`.

mod arc;
pub mod color;
pub mod color_profile;
mod convex_hull;
mod error;
mod hit_test;
pub mod icc;
mod intersect;
pub mod palette;
mod path;
mod perspective;
mod point;
mod rect;
mod size;
mod transform;
mod vector;

// Re-export all public types at the crate root for convenience.
pub use arc::{CenterArc, SvgArc};
pub use color::{ColorF64, ColorSpace};
pub use convex_hull::convex_hull;
pub use error::MathError;
pub use hit_test::{bounding_box, hit_test_fill, hit_test_stroke};
pub use intersect::{
    cubic_cubic_intersections, line_cubic_intersections, line_line_intersection,
    line_quad_intersections, Intersection,
};
pub use path::{subdivide_cubic, subdivide_quad, BezPath, PathSegment};
pub use perspective::{PerspectiveTransform, VanishingPoint};
pub use point::Point;
pub use rect::Rect;
pub use size::Size;
pub use transform::{parse_transform, Transform2D};
pub use vector::Vector;

/// A convenience type alias for Results in this crate.
pub type Result<T> = std::result::Result<T, MathError>;
