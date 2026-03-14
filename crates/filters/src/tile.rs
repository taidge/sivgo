//! feTile -- tile the input buffer to fill the output area.

use crate::buffer::FilterBuffer;

/// Tile the input buffer to fill an output buffer of the given dimensions.
///
/// The input image is repeated in both x and y directions to cover the entire
/// output area. If the input dimensions are zero, returns a transparent black buffer.
pub fn tile(input: &FilterBuffer, output_width: u32, output_height: u32) -> FilterBuffer {
    let mut output = FilterBuffer::new(output_width, output_height);

    if input.width == 0 || input.height == 0 {
        return output;
    }

    let iw = input.width;
    let ih = input.height;

    for y in 0..output_height {
        for x in 0..output_width {
            let sx = x % iw;
            let sy = y % ih;
            let pixel = input.get_pixel(sx, sy);
            output.set_pixel(x, y, pixel);
        }
    }

    output
}

/// Tile the input buffer to fill a buffer of the same size.
///
/// This is the common SVG filter case where the output region matches
/// the filter region dimensions (same as input dimensions).
pub fn tile_same_size(input: &FilterBuffer) -> FilterBuffer {
    // When tiling to the same dimensions, it is just a clone.
    input.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_same_size_is_clone() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);
        buf.set_pixel(1, 1, [0.0, 1.0, 0.0, 1.0]);

        let result = tile_same_size(&buf);

        assert_eq!(result.get_pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(1, 1), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(2, 2), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_tile_doubles() {
        // A 2x2 input tiled to 4x4
        let mut buf = FilterBuffer::new(2, 2);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]); // red
        buf.set_pixel(1, 0, [0.0, 1.0, 0.0, 1.0]); // green
        buf.set_pixel(0, 1, [0.0, 0.0, 1.0, 1.0]); // blue
        buf.set_pixel(1, 1, [1.0, 1.0, 1.0, 1.0]); // white

        let result = tile(&buf, 4, 4);

        // Check that the pattern repeats
        assert_eq!(result.get_pixel(0, 0), [1.0, 0.0, 0.0, 1.0]); // red
        assert_eq!(result.get_pixel(2, 0), [1.0, 0.0, 0.0, 1.0]); // red (tiled)
        assert_eq!(result.get_pixel(1, 0), [0.0, 1.0, 0.0, 1.0]); // green
        assert_eq!(result.get_pixel(3, 0), [0.0, 1.0, 0.0, 1.0]); // green (tiled)
        assert_eq!(result.get_pixel(0, 2), [1.0, 0.0, 0.0, 1.0]); // red (tiled y wraps to row 0)
        assert_eq!(result.get_pixel(1, 3), [1.0, 1.0, 1.0, 1.0]); // white (tiled y wraps to row 1)
    }

    #[test]
    fn test_tile_non_multiple() {
        // A 2x2 input tiled to 3x3
        let mut buf = FilterBuffer::new(2, 2);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);
        buf.set_pixel(1, 0, [0.0, 1.0, 0.0, 1.0]);
        buf.set_pixel(0, 1, [0.0, 0.0, 1.0, 1.0]);
        buf.set_pixel(1, 1, [1.0, 1.0, 0.0, 1.0]);

        let result = tile(&buf, 3, 3);

        // Row 0: red, green, red
        assert_eq!(result.get_pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(1, 0), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(2, 0), [1.0, 0.0, 0.0, 1.0]);

        // Row 1: blue, yellow, blue
        assert_eq!(result.get_pixel(0, 1), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(result.get_pixel(1, 1), [1.0, 1.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(2, 1), [0.0, 0.0, 1.0, 1.0]);

        // Row 2: red, green, red (wraps y)
        assert_eq!(result.get_pixel(0, 2), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(result.get_pixel(1, 2), [0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn test_tile_single_pixel() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.5, 0.3, 0.7, 1.0]);

        let result = tile(&buf, 4, 3);

        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(result.get_pixel(x, y), [0.5, 0.3, 0.7, 1.0]);
            }
        }
    }

    #[test]
    fn test_tile_larger_output() {
        let mut buf = FilterBuffer::new(3, 2);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);
        buf.set_pixel(1, 0, [0.0, 1.0, 0.0, 1.0]);
        buf.set_pixel(2, 0, [0.0, 0.0, 1.0, 1.0]);
        buf.set_pixel(0, 1, [1.0, 1.0, 0.0, 1.0]);
        buf.set_pixel(1, 1, [0.0, 1.0, 1.0, 1.0]);
        buf.set_pixel(2, 1, [1.0, 0.0, 1.0, 1.0]);

        let result = tile(&buf, 6, 4);

        // Check tiling in x
        assert_eq!(result.get_pixel(3, 0), result.get_pixel(0, 0));
        assert_eq!(result.get_pixel(4, 0), result.get_pixel(1, 0));
        assert_eq!(result.get_pixel(5, 0), result.get_pixel(2, 0));
        // Check tiling in y
        assert_eq!(result.get_pixel(0, 2), result.get_pixel(0, 0));
        assert_eq!(result.get_pixel(0, 3), result.get_pixel(0, 1));
    }
}
