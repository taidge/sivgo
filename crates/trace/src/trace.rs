use sivgo_math::BezPath;

use crate::bitmap::GrayBitmap;
use crate::contour::{contour_area, find_contours};
use crate::smooth::smooth_contour;
use crate::threshold::{otsu_threshold, threshold};

/// Options controlling bitmap-to-vector tracing.
#[derive(Debug, Clone)]
pub struct TraceOptions {
    /// Brightness threshold for binarization.
    /// Pixels darker than this become foreground.
    /// `None` = automatic (Otsu's method).
    pub threshold: Option<u8>,

    /// Smoothing factor for output curves (0.0 = straight lines, 1.0 = full smoothing).
    /// Default: 1.0.
    pub smoothing: f64,

    /// Minimum contour area in pixels. Contours smaller than this are discarded (speckle
    /// removal). Default: 4.
    pub min_area: u32,

    /// If true, invert the bitmap before tracing (trace white-on-black instead of
    /// black-on-white). Default: false.
    pub invert: bool,
}

impl Default for TraceOptions {
    fn default() -> Self {
        TraceOptions {
            threshold: None,
            smoothing: 1.0,
            min_area: 4,
            invert: false,
        }
    }
}

/// Trace a grayscale bitmap to produce vector paths.
///
/// Returns one `BezPath` per detected contour (both outer boundaries and holes).
pub fn trace(bitmap: &GrayBitmap, options: &TraceOptions) -> Vec<BezPath> {
    let mut bmp = bitmap.clone();
    if options.invert {
        bmp.invert();
    }

    let thresh = options.threshold.unwrap_or_else(|| otsu_threshold(&bmp));

    log::debug!(
        "Tracing with threshold={}, smoothing={}, min_area={}",
        thresh,
        options.smoothing,
        options.min_area
    );

    let binary = threshold(&bmp, thresh);
    let contours = find_contours(&binary);

    log::debug!("Found {} raw contours", contours.len());

    let min_area = options.min_area as f64;

    contours
        .iter()
        .filter(|c| contour_area(c) >= min_area)
        .map(|c| smooth_contour(c, options.smoothing))
        .collect()
}

/// Trace RGBA pixel data directly to produce vector paths.
///
/// `data` must contain exactly `width * height * 4` bytes (R, G, B, A per pixel).
/// Returns one `BezPath` per detected contour.
pub fn trace_rgba(data: &[u8], width: u32, height: u32, options: &TraceOptions) -> Vec<BezPath> {
    let gray = GrayBitmap::from_rgba(data, width, height);
    trace(&gray, options)
}

/// Generate a preview bitmap showing what the trace result would look like.
///
/// Returns a `GrayBitmap` where foreground (traced) areas are black and
/// background is white.  This is much faster than a full trace + render
/// because it skips curve fitting and only computes the binary threshold.
///
/// Useful for interactive dialogs where the user adjusts trace parameters
/// and needs immediate visual feedback.
pub fn trace_preview(bitmap: &GrayBitmap, options: &TraceOptions) -> GrayBitmap {
    let mut bmp = bitmap.clone();
    if options.invert {
        bmp.invert();
    }

    let thresh = options.threshold.unwrap_or_else(|| otsu_threshold(&bmp));

    let binary = threshold(&bmp, thresh);

    // Convert binary back to a grayscale preview
    let mut preview = GrayBitmap::new(bmp.width, bmp.height);
    for y in 0..bmp.height {
        for x in 0..bmp.width {
            if binary.get(x as i32, y as i32) {
                // Check minimum area would be applied at contour level,
                // but for preview we just show the thresholded result
                preview.set(x, y, 0); // black = foreground
            }
            // background stays white (255, the default)
        }
    }

    preview
}

/// Generate a preview of edge-detection tracing.
///
/// Returns a `GrayBitmap` showing the Sobel edge magnitude.
pub fn edge_preview(bitmap: &GrayBitmap) -> GrayBitmap {
    crate::edge::sobel_edges(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::PathSegment;

    /// Helper: create a GrayBitmap with a black filled rectangle on a white background.
    fn make_gray_rect(width: u32, height: u32, rx: u32, ry: u32, rw: u32, rh: u32) -> GrayBitmap {
        let mut bmp = GrayBitmap::new(width, height);
        for y in ry..(ry + rh).min(height) {
            for x in rx..(rx + rw).min(width) {
                bmp.set(x, y, 0); // black
            }
        }
        bmp
    }

    #[test]
    fn test_trace_black_square() {
        // A 10x10 black square in a 20x20 white image
        let bmp = make_gray_rect(20, 20, 5, 5, 10, 10);
        let options = TraceOptions {
            threshold: Some(128),
            smoothing: 0.0, // straight lines for easier verification
            min_area: 1,
            invert: false,
        };

        let paths = trace(&bmp, &options);
        assert!(
            !paths.is_empty(),
            "Should trace at least one path from a black square"
        );

        // The path should start with MoveTo and end with ClosePath
        let elements = paths[0].elements();
        assert!(matches!(elements[0], PathSegment::MoveTo(_)));
        assert!(matches!(elements.last(), Some(PathSegment::ClosePath)));
    }

    #[test]
    fn test_trace_all_white() {
        // All white image — no contours
        let bmp = GrayBitmap::new(20, 20);
        let options = TraceOptions {
            threshold: Some(128),
            ..Default::default()
        };

        let paths = trace(&bmp, &options);
        assert!(paths.is_empty(), "All-white image should produce no paths");
    }

    #[test]
    fn test_trace_with_min_area_filter() {
        let mut bmp = GrayBitmap::new(30, 30);

        // Draw a large 10x10 square
        for y in 5..15 {
            for x in 5..15 {
                bmp.set(x, y, 0);
            }
        }
        // Draw a tiny 2x1 speckle
        bmp.set(25, 25, 0);
        bmp.set(26, 25, 0);

        let options = TraceOptions {
            threshold: Some(128),
            smoothing: 0.0,
            min_area: 10, // should filter out the speckle
            invert: false,
        };

        let paths = trace(&bmp, &options);
        // Should have the large square but not the speckle
        assert!(!paths.is_empty(), "Should have at least the large square");
        // All returned paths should have substantial area
        for path in &paths {
            let elems = path.elements();
            assert!(
                elems.len() >= 4,
                "Returned path should have at least a few segments"
            );
        }
    }

    #[test]
    fn test_trace_invert() {
        // A white square on a black background — use invert to trace
        let mut bmp = GrayBitmap::new(20, 20);
        // Fill everything black
        for y in 0..20 {
            for x in 0..20 {
                bmp.set(x, y, 0);
            }
        }
        // White square in the center
        for y in 5..15 {
            for x in 5..15 {
                bmp.set(x, y, 255);
            }
        }

        // Without invert, should find contours of the black region (the border)
        // With invert, the white square becomes black and the border becomes white
        let options = TraceOptions {
            threshold: Some(128),
            smoothing: 0.0,
            min_area: 1,
            invert: true,
        };

        let paths = trace(&bmp, &options);
        assert!(
            !paths.is_empty(),
            "Inverted tracing should find the white square"
        );
    }

    #[test]
    fn test_trace_auto_threshold() {
        // Bimodal image: dark left half, light right half
        let mut bmp = GrayBitmap::new(20, 20);
        for y in 0..20 {
            for x in 0..10 {
                bmp.set(x, y, 30); // dark
            }
            for x in 10..20 {
                bmp.set(x, y, 220); // light
            }
        }

        let options = TraceOptions {
            threshold: None, // auto
            smoothing: 0.0,
            min_area: 1,
            invert: false,
        };

        let paths = trace(&bmp, &options);
        assert!(
            !paths.is_empty(),
            "Auto-threshold should detect the dark region"
        );
    }

    #[test]
    fn test_trace_rgba() {
        // Create a 4x4 RGBA image: top half black, bottom half white
        let mut data = vec![255u8; 4 * 4 * 4]; // all white
        for y in 0..2 {
            for x in 0..4 {
                let idx = (y * 4 + x) * 4;
                data[idx] = 0; // R
                data[idx + 1] = 0; // G
                data[idx + 2] = 0; // B
                data[idx + 3] = 255; // A
            }
        }

        let options = TraceOptions {
            threshold: Some(128),
            smoothing: 0.0,
            min_area: 1,
            invert: false,
        };

        let paths = trace_rgba(&data, 4, 4, &options);
        assert!(
            !paths.is_empty(),
            "RGBA tracing should detect the black region"
        );
    }
}
