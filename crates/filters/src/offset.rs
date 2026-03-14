//! feOffset -- shift the image by (dx, dy) pixels.

use crate::buffer::FilterBuffer;

/// Shift the image by (dx, dy) pixels.
///
/// Pixels that fall outside the buffer boundaries become transparent black.
pub fn offset(input: &FilterBuffer, dx: f64, dy: f64) -> FilterBuffer {
    let mut output = input.clone_empty();
    let dx = dx.round() as i32;
    let dy = dy.round() as i32;
    let w = input.width as i32;
    let h = input.height as i32;

    for y in 0..h {
        for x in 0..w {
            let sx = x - dx;
            let sy = y - dy;
            if sx >= 0 && sx < w && sy >= 0 && sy < h {
                let pixel = input.get_pixel(sx as u32, sy as u32);
                output.set_pixel(x as u32, y as u32, pixel);
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_moves_pixel() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.set_pixel(1, 1, [1.0, 0.0, 0.0, 1.0]);

        let result = offset(&buf, 2.0, 3.0);

        // Original position should be empty
        assert_eq!(result.get_pixel(1, 1), [0.0, 0.0, 0.0, 0.0]);
        // New position should have the pixel
        assert_eq!(result.get_pixel(3, 4), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_offset_clips_out_of_bounds() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(2, 2, [1.0, 1.0, 1.0, 1.0]);

        let result = offset(&buf, 2.0, 2.0);

        // The pixel at (2,2) would move to (4,4) which is out of bounds.
        // So entire buffer should be transparent.
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(result.get_pixel(x, y), [0.0, 0.0, 0.0, 0.0]);
            }
        }
    }

    #[test]
    fn test_offset_negative() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.set_pixel(3, 3, [0.5, 0.5, 0.5, 1.0]);

        let result = offset(&buf, -1.0, -2.0);

        assert_eq!(result.get_pixel(2, 1), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(result.get_pixel(3, 3), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_offset_zero() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(1, 1, [1.0, 0.0, 0.0, 1.0]);

        let result = offset(&buf, 0.0, 0.0);

        assert_eq!(result.get_pixel(1, 1), [1.0, 0.0, 0.0, 1.0]);
    }
}
