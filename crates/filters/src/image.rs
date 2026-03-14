//! feImage -- reference an external image or render a solid rectangle as filter input.
//!
//! In a full SVG renderer, feImage can reference an external image file or an internal
//! SVG element via `href`. Since this crate operates purely on pixel buffers and has no
//! access to the DOM or file system, this module provides:
//!
//! 1. `image_from_buffer` -- use an already-decoded image buffer, scaling/placing it
//!    with `preserveAspectRatio`-like behaviour into the filter region.
//! 2. `image_solid` -- produce a solid-color rectangle (useful as a fallback).
//!
//! The higher-level rendering code is responsible for resolving the `href`, decoding
//! the image, and passing the pixel data here.

use crate::buffer::FilterBuffer;

/// How to fit the source image into the target region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFit {
    /// Stretch to fill the target region exactly (no aspect ratio preservation).
    Fill,
    /// Scale uniformly so the entire image fits inside the target, centering the result.
    /// Equivalent to `xMidYMid meet`.
    Meet,
    /// Scale uniformly so the image covers the entire target, centering and clipping.
    /// Equivalent to `xMidYMid slice`.
    Slice,
}

/// Place a source image into a filter-region-sized buffer.
///
/// The source image is scaled according to `fit` and placed in the center of the
/// output buffer of dimensions `(target_width, target_height)`.
///
/// Uses nearest-neighbour sampling for simplicity.
pub fn image_from_buffer(
    source: &FilterBuffer,
    target_width: u32,
    target_height: u32,
    fit: ImageFit,
) -> FilterBuffer {
    let mut output = FilterBuffer::new(target_width, target_height);

    if source.width == 0 || source.height == 0 || target_width == 0 || target_height == 0 {
        return output;
    }

    let sw = source.width as f64;
    let sh = source.height as f64;
    let tw = target_width as f64;
    let th = target_height as f64;

    // Compute the scale and offset to map target coords to source coords.
    let (scale_x, scale_y, offset_x, offset_y) = match fit {
        ImageFit::Fill => {
            // Direct stretch.
            (sw / tw, sh / th, 0.0, 0.0)
        }
        ImageFit::Meet => {
            let scale = (tw / sw).min(th / sh);
            let rendered_w = sw * scale;
            let rendered_h = sh * scale;
            let ox = (tw - rendered_w) * 0.5;
            let oy = (th - rendered_h) * 0.5;
            // Inverse: source_coord = (target_coord - offset) / scale
            // We store inv_scale = 1/scale and the offset in target space.
            (1.0 / scale, 1.0 / scale, ox, oy)
        }
        ImageFit::Slice => {
            let scale = (tw / sw).max(th / sh);
            let rendered_w = sw * scale;
            let rendered_h = sh * scale;
            let ox = (tw - rendered_w) * 0.5;
            let oy = (th - rendered_h) * 0.5;
            (1.0 / scale, 1.0 / scale, ox, oy)
        }
    };

    for ty in 0..target_height {
        for tx in 0..target_width {
            let (sx, sy) = match fit {
                ImageFit::Fill => {
                    let sx = (tx as f64 + 0.5) * scale_x;
                    let sy = (ty as f64 + 0.5) * scale_y;
                    (sx, sy)
                }
                ImageFit::Meet | ImageFit::Slice => {
                    let sx = (tx as f64 + 0.5 - offset_x) * scale_x;
                    let sy = (ty as f64 + 0.5 - offset_y) * scale_y;
                    (sx, sy)
                }
            };

            let ix = sx.floor() as i32;
            let iy = sy.floor() as i32;

            if ix >= 0 && ix < source.width as i32 && iy >= 0 && iy < source.height as i32 {
                let pixel = source.get_pixel(ix as u32, iy as u32);
                output.set_pixel(tx, ty, pixel);
            }
            // else: transparent black (default)
        }
    }

    output
}

/// Produce a solid-color image of the given dimensions.
///
/// This is a convenience helper used as a fallback when the feImage href cannot
/// be resolved.
pub fn image_solid(width: u32, height: u32, color: [f32; 4]) -> FilterBuffer {
    let mut buf = FilterBuffer::new(width, height);
    buf.fill(color[0], color[1], color[2], color[3]);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_solid() {
        let buf = image_solid(3, 3, [1.0, 0.0, 0.0, 1.0]);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(buf.get_pixel(x, y), [1.0, 0.0, 0.0, 1.0]);
            }
        }
    }

    #[test]
    fn test_image_fill_stretch() {
        // 2x2 source stretched to 4x4
        let mut src = FilterBuffer::new(2, 2);
        src.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]); // red
        src.set_pixel(1, 0, [0.0, 1.0, 0.0, 1.0]); // green
        src.set_pixel(0, 1, [0.0, 0.0, 1.0, 1.0]); // blue
        src.set_pixel(1, 1, [1.0, 1.0, 0.0, 1.0]); // yellow

        let result = image_from_buffer(&src, 4, 4, ImageFit::Fill);

        // Top-left quadrant should be red
        assert_eq!(result.get_pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
        // Top-right quadrant should be green
        assert_eq!(result.get_pixel(2, 0), [0.0, 1.0, 0.0, 1.0]);
        // Bottom-left quadrant should be blue
        assert_eq!(result.get_pixel(0, 2), [0.0, 0.0, 1.0, 1.0]);
        // Bottom-right quadrant should be yellow
        assert_eq!(result.get_pixel(2, 2), [1.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn test_image_meet_preserves_aspect() {
        // 4x2 source into 4x4 target with Meet: source scaled to 4x2, centered vertically.
        let mut src = FilterBuffer::new(4, 2);
        src.fill(1.0, 0.0, 0.0, 1.0); // all red

        let result = image_from_buffer(&src, 4, 4, ImageFit::Meet);

        // With Meet for a 4x2 into 4x4: scale = min(4/4, 4/2) = min(1, 2) = 1
        // rendered size = 4x2, offset_y = (4-2)/2 = 1
        // So rows 0 and 3 should be transparent, rows 1 and 2 should be red.
        assert_eq!(result.get_pixel(0, 0), [0.0, 0.0, 0.0, 0.0],);
        assert_eq!(result.get_pixel(0, 1), [1.0, 0.0, 0.0, 1.0],);
        assert_eq!(result.get_pixel(0, 2), [1.0, 0.0, 0.0, 1.0],);
        assert_eq!(result.get_pixel(0, 3), [0.0, 0.0, 0.0, 0.0],);
    }

    #[test]
    fn test_image_slice_covers_target() {
        // 2x4 source into 4x4 target with Slice: scale = max(4/2, 4/4) = max(2, 1) = 2
        // rendered size = 4x8, offset_y = (4-8)/2 = -2
        // The entire 4x4 target should be filled (no transparent pixels).
        let mut src = FilterBuffer::new(2, 4);
        src.fill(0.0, 1.0, 0.0, 1.0); // all green

        let result = image_from_buffer(&src, 4, 4, ImageFit::Slice);

        for y in 0..4 {
            for x in 0..4 {
                let px = result.get_pixel(x, y);
                assert!(
                    px[3] > 0.5,
                    "Slice should cover all pixels, but ({},{}) has alpha {}",
                    x,
                    y,
                    px[3]
                );
            }
        }
    }

    #[test]
    fn test_image_empty_source() {
        let src = FilterBuffer::new(0, 0);
        let result = image_from_buffer(&src, 4, 4, ImageFit::Fill);
        // Should be all transparent black.
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(result.get_pixel(x, y), [0.0, 0.0, 0.0, 0.0]);
            }
        }
    }

    #[test]
    fn test_image_same_size_fill() {
        // Same-size fill should be essentially a copy.
        let mut src = FilterBuffer::new(3, 3);
        src.set_pixel(1, 1, [0.5, 0.5, 0.5, 1.0]);

        let result = image_from_buffer(&src, 3, 3, ImageFit::Fill);
        assert_eq!(result.get_pixel(1, 1), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(result.get_pixel(0, 0), [0.0, 0.0, 0.0, 0.0]);
    }
}
