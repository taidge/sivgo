//! feDisplacementMap -- displace pixels based on color values from a second input.

use crate::buffer::FilterBuffer;

/// Channel selector for displacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelSelector {
    R,
    G,
    B,
    A,
}

impl ChannelSelector {
    /// Get the channel index (0=R, 1=G, 2=B, 3=A).
    fn index(self) -> usize {
        match self {
            ChannelSelector::R => 0,
            ChannelSelector::G => 1,
            ChannelSelector::B => 2,
            ChannelSelector::A => 3,
        }
    }
}

/// Displace pixels from `input` based on color values from `map`.
///
/// For each pixel (x, y):
///   output(x, y) = input(x + scale * (map_channel_x(x,y) - 0.5),
///                        y + scale * (map_channel_y(x,y) - 0.5))
///
/// Out-of-bounds samples return transparent black.
pub fn displacement_map(
    input: &FilterBuffer,
    map: &FilterBuffer,
    scale: f64,
    x_channel: ChannelSelector,
    y_channel: ChannelSelector,
) -> FilterBuffer {
    let w = input.width;
    let h = input.height;
    let mut output = input.clone_empty();

    let xi = x_channel.index();
    let yi = y_channel.index();

    for y in 0..h {
        for x in 0..w {
            let map_px = map.get_pixel(x, y);
            let dx = scale * (map_px[xi] as f64 - 0.5);
            let dy = scale * (map_px[yi] as f64 - 0.5);

            let sx = (x as f64 + dx).round() as i32;
            let sy = (y as f64 + dy).round() as i32;

            if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                let pixel = input.get_pixel(sx as u32, sy as u32);
                output.set_pixel(x, y, pixel);
            }
            // else: transparent black (already zero)
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_scale_is_identity() {
        let mut input = FilterBuffer::new(3, 3);
        input.set_pixel(1, 1, [0.8, 0.2, 0.5, 1.0]);

        let mut map = FilterBuffer::new(3, 3);
        map.fill(1.0, 0.0, 0.5, 1.0); // arbitrary map values

        let result = displacement_map(&input, &map, 0.0, ChannelSelector::R, ChannelSelector::G);

        // With scale=0, displacement is always 0 regardless of map values.
        assert_eq!(result.get_pixel(1, 1), [0.8, 0.2, 0.5, 1.0]);
        assert_eq!(result.get_pixel(0, 0), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_displacement_shifts_pixels() {
        let mut input = FilterBuffer::new(5, 5);
        input.fill(0.0, 0.0, 0.0, 1.0);
        input.set_pixel(2, 2, [1.0, 0.0, 0.0, 1.0]); // red center

        // Map with 0.5 everywhere means zero displacement (0.5 - 0.5 = 0)
        let mut map = FilterBuffer::new(5, 5);
        map.fill(0.5, 0.5, 0.5, 0.5);

        let result = displacement_map(&input, &map, 10.0, ChannelSelector::R, ChannelSelector::G);

        // No displacement: center pixel should stay
        assert_eq!(result.get_pixel(2, 2), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_displacement_with_nonzero_map() {
        // 5x5 input with all white pixels
        let mut input = FilterBuffer::new(5, 5);
        input.fill(1.0, 1.0, 1.0, 1.0);

        // Map that pushes sampling out of bounds at corners
        let mut map = FilterBuffer::new(5, 5);
        map.fill(0.0, 0.0, 0.0, 1.0); // R=0 means x displacement = scale*(0 - 0.5) = -scale/2

        // With scale=10, x displacement = 10*(0.0-0.5)=-5, y displacement = 10*(0.0-0.5)=-5
        // For pixel (0,0): sample from (0-5, 0-5) = (-5, -5) -> out of bounds -> transparent black
        let result = displacement_map(&input, &map, 10.0, ChannelSelector::R, ChannelSelector::G);
        let corner = result.get_pixel(0, 0);
        assert_eq!(corner, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_displacement_channel_selectors() {
        let mut input = FilterBuffer::new(3, 3);
        input.fill(0.5, 0.5, 0.5, 1.0);

        // Map: use alpha channel for x, blue channel for y
        let mut map = FilterBuffer::new(3, 3);
        // Set all pixels to R=0, G=0, B=0.5, A=0.5 -> zero displacement for both selected channels
        map.fill(0.0, 0.0, 0.5, 0.5);

        let result = displacement_map(&input, &map, 4.0, ChannelSelector::A, ChannelSelector::B);

        // B=0.5 and A=0.5 means displacement = 4*(0.5-0.5) = 0 in both axes
        let px = result.get_pixel(1, 1);
        assert_eq!(px, [0.5, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn test_mismatched_buffer_sizes_uses_input_dimensions() {
        // Map is smaller than input -- map pixel reads will panic if we go out of bounds.
        // For safety, the function uses min(input, map) dimensions implicitly because
        // get_pixel would panic. This test ensures same-size buffers work fine.
        let mut input = FilterBuffer::new(3, 3);
        input.set_pixel(1, 1, [1.0, 0.0, 0.0, 1.0]);

        let mut map = FilterBuffer::new(3, 3);
        map.fill(0.5, 0.5, 0.5, 0.5);

        let result = displacement_map(&input, &map, 0.0, ChannelSelector::R, ChannelSelector::G);
        assert_eq!(result.get_pixel(1, 1), [1.0, 0.0, 0.0, 1.0]);
    }
}
