//! feMorphology -- erode or dilate an image.

use crate::buffer::FilterBuffer;

/// Morphology operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphologyOp {
    Erode,
    Dilate,
}

/// Apply morphology (erode or dilate) with the given radii.
///
/// A rectangular structuring element is separable, so this runs a horizontal
/// pass followed by a vertical pass — O(w·h·(rx+ry)) instead of the
/// O(w·h·rx·ry) of a full 2D window scan.
/// - Erode: take the minimum of each channel in the kernel.
/// - Dilate: take the maximum of each channel in the kernel.
pub fn morphology(
    input: &FilterBuffer,
    op: MorphologyOp,
    radius_x: f64,
    radius_y: f64,
) -> FilterBuffer {
    let rx = radius_x.round().max(0.0) as i32;
    let ry = radius_y.round().max(0.0) as i32;

    if rx == 0 && ry == 0 {
        return input.clone();
    }

    let horizontal = if rx > 0 {
        morphology_pass(input, op, rx, true)
    } else {
        input.clone()
    };
    if ry > 0 {
        morphology_pass(&horizontal, op, ry, false)
    } else {
        horizontal
    }
}

/// One separable morphology pass. When `horizontal` is true the window spans
/// `[x-radius, x+radius]` along the row; otherwise it spans the column.
fn morphology_pass(
    input: &FilterBuffer,
    op: MorphologyOp,
    radius: i32,
    horizontal: bool,
) -> FilterBuffer {
    let w = input.width as i32;
    let h = input.height as i32;
    let mut output = input.clone_empty();

    let combine = |acc: f32, v: f32| match op {
        MorphologyOp::Erode => acc.min(v),
        MorphologyOp::Dilate => acc.max(v),
    };
    let init = match op {
        MorphologyOp::Erode => f32::MAX,
        MorphologyOp::Dilate => f32::MIN,
    };

    for y in 0..h {
        for x in 0..w {
            let mut result = [init; 4];
            let (lo, hi) = if horizontal {
                ((x - radius).max(0), (x + radius).min(w - 1))
            } else {
                ((y - radius).max(0), (y + radius).min(h - 1))
            };
            for k in lo..=hi {
                let px = if horizontal {
                    input.get_pixel(k as u32, y as u32)
                } else {
                    input.get_pixel(x as u32, k as u32)
                };
                for c in 0..4 {
                    result[c] = combine(result[c], px[c]);
                }
            }
            output.set_pixel(x as u32, y as u32, result);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dilate_grows_bright_areas() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.set_pixel(2, 2, [1.0, 1.0, 1.0, 1.0]);

        let result = morphology(&buf, MorphologyOp::Dilate, 1.0, 1.0);

        // The center pixel should still be bright
        assert_eq!(result.get_pixel(2, 2), [1.0, 1.0, 1.0, 1.0]);

        // Neighbors within radius should also be bright
        assert_eq!(result.get_pixel(1, 2), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(result.get_pixel(3, 2), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(result.get_pixel(2, 1), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(result.get_pixel(2, 3), [1.0, 1.0, 1.0, 1.0]);

        // Pixels beyond radius should still be dark
        assert_eq!(result.get_pixel(0, 0), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(result.get_pixel(4, 4), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_erode_shrinks_bright_areas() {
        // Fill a 5x5 buffer with white
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(1.0, 1.0, 1.0, 1.0);
        // Set corners to dark
        buf.set_pixel(0, 0, [0.0, 0.0, 0.0, 0.0]);

        let result = morphology(&buf, MorphologyOp::Erode, 1.0, 1.0);

        // The pixel at (1,1) should be dark because (0,0) is in its kernel
        assert_eq!(result.get_pixel(1, 1), [0.0, 0.0, 0.0, 0.0]);

        // Center should still be white (no dark pixel in its kernel)
        assert_eq!(result.get_pixel(2, 2), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_zero_radius_is_identity() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(1, 1, [0.5, 0.5, 0.5, 1.0]);

        let result = morphology(&buf, MorphologyOp::Dilate, 0.0, 0.0);
        assert_eq!(result.get_pixel(1, 1), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(result.get_pixel(0, 0), [0.0, 0.0, 0.0, 0.0]);
    }
}
