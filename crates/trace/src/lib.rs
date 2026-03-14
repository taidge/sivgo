//! sivgo-trace: Bitmap-to-vector tracing for Sivgo.
//!
//! Provides a simplified potrace-style algorithm to convert raster images into SVG vector paths.
//! The pipeline is:
//!
//! 1. Convert input to grayscale ([`GrayBitmap`])
//! 2. Binarize with a threshold ([`threshold`], [`otsu_threshold`])
//! 3. Trace contour boundaries ([`find_contours`])
//! 4. Smooth contours into Bezier curves ([`smooth_contour`])
//!
//! # Example
//!
//! ```rust
//! use sivgo_trace::{GrayBitmap, TraceOptions, trace};
//!
//! let mut bitmap = GrayBitmap::new(100, 100);
//! // ... fill in pixel data ...
//! # for y in 20..80 { for x in 20..80 { bitmap.set(x, y, 0); } }
//!
//! let options = TraceOptions::default();
//! let paths = trace(&bitmap, &options);
//!
//! for path in &paths {
//!     println!("{}", path.to_svg_string());
//! }
//! ```

pub mod bitmap;
pub mod centerline;
pub mod color_quant;
pub mod contour;
pub mod edge;
pub mod smooth;
pub mod threshold;
pub mod trace;

// Re-exports for convenience
pub use bitmap::GrayBitmap;
pub use centerline::{trace_centerline, CenterlineOptions};
pub use color_quant::{trace_colors, ColorLayer, ColorTraceOptions, RgbColor};
pub use contour::{contour_area, find_contours, Contour};
pub use edge::{sobel_edges, trace_edges, EdgeTraceOptions};
pub use smooth::smooth_contour;
pub use threshold::{otsu_threshold, threshold, BinaryBitmap};
pub use trace::{edge_preview, trace, trace_preview, trace_rgba, TraceOptions};
