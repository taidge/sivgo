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
    // A radius wider than the buffer already covers every pixel, so clamping
    // to the buffer size changes no result. It does keep `x + radius` from
    // overflowing `i32` on a hostile value such as `radius="3000000000"`,
    // which saturates the cast to `i32::MAX`.
    let max_radius = f64::from(input.width.max(input.height));
    let rx = radius_x.round().clamp(0.0, max_radius) as i32;
    let ry = radius_y.round().clamp(0.0, max_radius) as i32;

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
///
/// Uses the van Herk / Gil-Werman running-extremum algorithm, which costs a
/// constant number of comparisons per pixel no matter how wide the window is.
/// The straightforward window scan is `O(w·h·radius)`, and because both the
/// radius and the canvas size come from the document, that let a small input
/// ask for an unbounded amount of work.
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

    // Length of the axis being scanned, and how many lines there are of it.
    let (line_len, line_count) = if horizontal { (w, h) } else { (h, w) };
    if line_len <= 0 || line_count <= 0 {
        return output;
    }

    let n = line_len as usize;
    let r = radius as usize;
    let window = 2 * r + 1;

    // The line is padded by `radius` on each side with copies of the edge
    // sample. Padding rather than shortening the window at the edges keeps the
    // block alignment the algorithm depends on, and gives the same answer: a
    // replicated edge value is already inside the clamped window.
    let padded_len = n + 2 * r;

    // Scratch buffers reused across lines.
    let mut padded = vec![[0.0f32; 4]; padded_len];
    let mut forward = vec![[0.0f32; 4]; padded_len];
    let mut backward = vec![[0.0f32; 4]; padded_len];

    for l in 0..line_count {
        for (j, slot) in padded.iter_mut().enumerate() {
            let i = j.saturating_sub(r).min(n - 1);
            *slot = if horizontal {
                input.get_pixel(i as u32, l as u32)
            } else {
                input.get_pixel(l as u32, i as u32)
            };
        }

        // Within each aligned block of `window` samples, `forward[j]` is the
        // extremum from the block start up to j, and `backward[j]` from j to
        // the block end.
        let mut block_start = 0usize;
        while block_start < padded_len {
            let block_end = (block_start + window).min(padded_len);

            forward[block_start] = padded[block_start];
            for j in block_start + 1..block_end {
                for c in 0..4 {
                    forward[j][c] = combine(forward[j - 1][c], padded[j][c]);
                }
            }

            backward[block_end - 1] = padded[block_end - 1];
            for j in (block_start..block_end - 1).rev() {
                for c in 0..4 {
                    backward[j][c] = combine(backward[j + 1][c], padded[j][c]);
                }
            }

            block_start = block_end;
        }

        // Output `i` spans padded positions `i ..= i + window - 1`. Those two
        // ends lie in the same block or in adjacent ones, so the pair of
        // running extrema covers the window exactly.
        for i in 0..n {
            let mut result = [0.0f32; 4];
            for c in 0..4 {
                result[c] = combine(backward[i][c], forward[i + window - 1][c]);
            }
            if horizontal {
                output.set_pixel(i as u32, l as u32, result);
            } else {
                output.set_pixel(l as u32, i as u32, result);
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The straightforward window scan, kept only as a reference to check the
    /// running-extremum implementation against.
    fn naive_pass(
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

    #[test]
    fn running_extremum_matches_naive_window_scan() {
        // Deterministic pseudo-random content.
        let (w, h) = (17u32, 13u32);
        let mut buf = FilterBuffer::new(w, h);
        let mut state = 12345u32;
        for y in 0..h {
            for x in 0..w {
                let mut next = || {
                    state = state.wrapping_mul(1103515245).wrapping_add(12345);
                    ((state >> 16) & 0xff) as f32 / 255.0
                };
                buf.set_pixel(x, y, [next(), next(), next(), next()]);
            }
        }

        for op in [MorphologyOp::Dilate, MorphologyOp::Erode] {
            for radius in [1, 2, 3, 5, 8, 16, 40] {
                for horizontal in [true, false] {
                    let fast = morphology_pass(&buf, op, radius, horizontal);
                    let slow = naive_pass(&buf, op, radius, horizontal);
                    for y in 0..h {
                        for x in 0..w {
                            assert_eq!(
                                fast.get_pixel(x, y),
                                slow.get_pixel(x, y),
                                "mismatch at ({x},{y}) op={op:?} radius={radius} horizontal={horizontal}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn absurd_radius_is_clamped_and_does_not_overflow() {
        let mut buf = FilterBuffer::new(8, 8);
        buf.set_pixel(4, 4, [1.0, 1.0, 1.0, 1.0]);

        // Saturates the cast to i32::MAX; `x + radius` used to overflow.
        let result = morphology(&buf, MorphologyOp::Dilate, 3_000_000_000.0, 3_000_000_000.0);
        assert_eq!(result.get_pixel(0, 0), [1.0, 1.0, 1.0, 1.0]);

        // NaN and negative radii are not fatal either.
        let _ = morphology(&buf, MorphologyOp::Dilate, f64::NAN, f64::NAN);
        let _ = morphology(&buf, MorphologyOp::Erode, -5.0, -5.0);
    }

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
