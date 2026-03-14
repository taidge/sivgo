//! feFlood -- fill the entire buffer with a solid color.

use crate::buffer::FilterBuffer;

/// Fill the entire buffer with a solid color.
pub fn flood(width: u32, height: u32, r: f32, g: f32, b: f32, a: f32) -> FilterBuffer {
    let mut buf = FilterBuffer::new(width, height);
    buf.fill(r, g, b, a);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flood_solid_color() {
        let buf = flood(3, 3, 1.0, 0.0, 0.0, 1.0);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(buf.get_pixel(x, y), [1.0, 0.0, 0.0, 1.0]);
            }
        }
    }

    #[test]
    fn test_flood_transparent() {
        let buf = flood(2, 2, 0.0, 0.0, 0.0, 0.0);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(buf.get_pixel(x, y), [0.0, 0.0, 0.0, 0.0]);
            }
        }
    }

    #[test]
    fn test_flood_semi_transparent() {
        let buf = flood(1, 1, 0.5, 0.5, 0.5, 0.5);
        assert_eq!(buf.get_pixel(0, 0), [0.5, 0.5, 0.5, 0.5]);
    }
}
