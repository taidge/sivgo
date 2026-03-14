//! Edge-detection-based tracing.
//!
//! Applies a Sobel edge detection filter to the grayscale image,
//! thresholds the edge magnitude, and then traces the resulting
//! binary edge map using the standard contour + smoothing pipeline.

use sivgo_math::BezPath;

use crate::bitmap::GrayBitmap;
use crate::contour::{contour_area, find_contours};
use crate::smooth::smooth_contour;
use crate::threshold::{threshold, BinaryBitmap};

/// Options for edge-detection tracing.
#[derive(Debug, Clone)]
pub struct EdgeTraceOptions {
    /// Edge magnitude threshold (0–255). Pixels with Sobel magnitude
    /// above this become foreground edges.  Default: 50.
    pub edge_threshold: u8,

    /// Smoothing factor for output curves.  Default: 1.0.
    pub smoothing: f64,

    /// Minimum contour area in pixels.  Default: 4.
    pub min_area: u32,
}

impl Default for EdgeTraceOptions {
    fn default() -> Self {
        Self {
            edge_threshold: 50,
            smoothing: 1.0,
            min_area: 4,
        }
    }
}

/// Compute the Sobel edge magnitude image from a grayscale bitmap.
///
/// Returns a new `GrayBitmap` where each pixel is the clamped gradient
/// magnitude (sqrt(gx² + gy²)), scaled to 0–255.
pub fn sobel_edges(bitmap: &GrayBitmap) -> GrayBitmap {
    let w = bitmap.width;
    let h = bitmap.height;
    let mut out = GrayBitmap::new(w, h);

    if w < 3 || h < 3 {
        return out;
    }

    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            // Sobel kernels:
            // Gx:           Gy:
            // -1  0 +1      -1 -2 -1
            // -2  0 +2       0  0  0
            // -1  0 +1      +1 +2 +1

            let p = |dx: i32, dy: i32| -> i32 {
                bitmap.get((x as i32 + dx) as u32, (y as i32 + dy) as u32) as i32
            };

            let gx = -p(-1, -1) + p(1, -1) - 2 * p(-1, 0) + 2 * p(1, 0) - p(-1, 1) + p(1, 1);

            let gy = -p(-1, -1) - 2 * p(0, -1) - p(1, -1) + p(-1, 1) + 2 * p(0, 1) + p(1, 1);

            let mag = ((gx as f64).hypot(gy as f64)).min(255.0) as u8;
            out.set(x, y, mag);
        }
    }

    out
}

/// Trace edges in a grayscale bitmap.
///
/// 1. Apply Sobel edge detection
/// 2. Threshold the edge magnitudes
/// 3. Trace contours of the binary edge map
/// 4. Smooth contours into Bezier curves
pub fn trace_edges(bitmap: &GrayBitmap, options: &EdgeTraceOptions) -> Vec<BezPath> {
    let edge_map = sobel_edges(bitmap);

    // Edges are bright in the edge map, so pixels above threshold are foreground.
    // We need to invert because our threshold() treats < thresh as foreground.
    let mut inverted = edge_map;
    inverted.invert();
    let binary: BinaryBitmap = threshold(&inverted, 255 - options.edge_threshold);

    let contours = find_contours(&binary);

    let min_area = options.min_area as f64;

    contours
        .iter()
        .filter(|c| contour_area(c) >= min_area)
        .map(|c| smooth_contour(c, options.smoothing))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobel_detects_vertical_edge() {
        // Left half black, right half white -> strong vertical edge in the middle
        let mut bmp = GrayBitmap::new(20, 20);
        for y in 0..20 {
            for x in 0..10 {
                bmp.set(x, y, 0);
            }
        }
        let edges = sobel_edges(&bmp);
        // The column at x=10 should have high edge values
        let edge_val = edges.get(10, 10);
        assert!(
            edge_val > 100,
            "Edge at boundary should be strong, got {edge_val}"
        );
        // Interior should have low edge values
        let interior = edges.get(5, 10);
        assert!(
            interior < 10,
            "Interior should have no edges, got {interior}"
        );
    }

    #[test]
    fn sobel_detects_horizontal_edge() {
        // Top half black, bottom half white
        let mut bmp = GrayBitmap::new(20, 20);
        for y in 0..10 {
            for x in 0..20 {
                bmp.set(x, y, 0);
            }
        }
        let edges = sobel_edges(&bmp);
        let edge_val = edges.get(10, 10);
        assert!(
            edge_val > 100,
            "Edge at boundary should be strong, got {edge_val}"
        );
    }

    #[test]
    fn trace_edges_finds_rect_outline() {
        let mut bmp = GrayBitmap::new(40, 40);
        // Draw a black square -> edges should outline it
        for y in 10..30 {
            for x in 10..30 {
                bmp.set(x, y, 0);
            }
        }
        let opts = EdgeTraceOptions {
            edge_threshold: 50,
            smoothing: 0.0,
            min_area: 1,
        };
        let paths = trace_edges(&bmp, &opts);
        assert!(!paths.is_empty(), "Should find edges around the square");
    }

    #[test]
    fn uniform_image_no_edges() {
        let bmp = GrayBitmap::new(20, 20); // all white
        let opts = EdgeTraceOptions::default();
        let paths = trace_edges(&bmp, &opts);
        assert!(
            paths.is_empty(),
            "Uniform image should have no edge contours"
        );
    }
}
