//! feDropShadow -- convenience filter combining offset, blur, flood, and composite.

use crate::buffer::FilterBuffer;
use crate::composite::{composite, CompositeOp};
use crate::gaussian_blur::gaussian_blur;
use crate::offset::offset;

/// Apply a drop shadow effect.
///
/// 1. Extract the alpha channel from the input.
/// 2. Offset by (dx, dy).
/// 3. Blur with `std_dev`.
/// 4. Colorize with the given shadow color.
/// 5. Composite the original input over the shadow.
pub fn drop_shadow(
    input: &FilterBuffer,
    dx: f64,
    dy: f64,
    std_dev: f64,
    color: [f32; 4],
) -> FilterBuffer {
    let w = input.width;
    let h = input.height;

    // Step 1: Extract alpha channel into a grayscale-alpha buffer.
    // We create a buffer where RGB = shadow color and A = input alpha.
    let mut alpha_buf = FilterBuffer::new(w, h);
    let pixel_count = (w as usize) * (h as usize);
    for i in 0..pixel_count {
        let idx = i * 4;
        let a = input.data[idx + 3];
        alpha_buf.data[idx] = color[0];
        alpha_buf.data[idx + 1] = color[1];
        alpha_buf.data[idx + 2] = color[2];
        alpha_buf.data[idx + 3] = a * color[3];
    }

    // Step 2: Offset
    let offset_buf = offset(&alpha_buf, dx, dy);

    // Step 3: Blur
    let blurred = gaussian_blur(&offset_buf, std_dev, std_dev);

    // Step 4: Composite input over shadow
    composite(input, &blurred, CompositeOp::Over)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_shadow_creates_shadow() {
        // Create a small image with a white pixel in the center
        let mut buf = FilterBuffer::new(11, 11);
        buf.set_pixel(5, 5, [1.0, 1.0, 1.0, 1.0]);

        let result = drop_shadow(&buf, 2.0, 2.0, 1.0, [0.0, 0.0, 0.0, 1.0]);

        // The original pixel should still be present
        let center = result.get_pixel(5, 5);
        assert!(center[3] > 0.5, "original pixel should remain");

        // The shadow area (offset by 2,2) should have some opacity
        let shadow = result.get_pixel(7, 7);
        assert!(
            shadow[3] > 0.0,
            "shadow region should have some opacity: got {}",
            shadow[3]
        );

        // Pixels far from both original and shadow should be transparent
        let far = result.get_pixel(0, 0);
        assert!(far[3] < 0.01, "far pixel should be nearly transparent");
    }

    #[test]
    fn test_drop_shadow_preserves_original() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.5, 0.5, 0.5, 1.0);

        let result = drop_shadow(&buf, 1.0, 1.0, 0.0, [0.0, 0.0, 0.0, 0.5]);

        // The original content should be preserved on top
        // Since original is opaque, compositing over shadow gives original colors
        let px = result.get_pixel(2, 2);
        assert!((px[0] - 0.5).abs() < 0.05);
        assert!((px[3] - 1.0).abs() < 0.01);
    }
}
