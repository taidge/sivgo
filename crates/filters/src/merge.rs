//! feMerge -- merge multiple buffers by compositing them in order (source-over).

use crate::buffer::FilterBuffer;

/// Merge multiple buffers by compositing them in order using source-over.
///
/// The first buffer is the bottom layer; subsequent buffers are composited on top.
/// All buffers must have the same dimensions.
/// Returns a transparent buffer if the input slice is empty.
pub fn merge(inputs: &[&FilterBuffer]) -> FilterBuffer {
    if inputs.is_empty() {
        return FilterBuffer::new(0, 0);
    }

    let width = inputs[0].width;
    let height = inputs[0].height;
    let mut result = inputs[0].clone();

    for &layer in &inputs[1..] {
        assert_eq!(layer.width, width);
        assert_eq!(layer.height, height);

        let pixel_count = (width as usize) * (height as usize);
        for i in 0..pixel_count {
            let idx = i * 4;

            // Copy destination pixel values to avoid borrow conflict.
            let dr = result.data[idx];
            let dg = result.data[idx + 1];
            let db = result.data[idx + 2];
            let da = result.data[idx + 3];

            let sr = layer.data[idx];
            let sg = layer.data[idx + 1];
            let sb = layer.data[idx + 2];
            let sa = layer.data[idx + 3];

            // Porter-Duff source-over
            let ra = sa + da * (1.0 - sa);
            if ra == 0.0 {
                result.data[idx] = 0.0;
                result.data[idx + 1] = 0.0;
                result.data[idx + 2] = 0.0;
                result.data[idx + 3] = 0.0;
            } else {
                result.data[idx] = (sr * sa + dr * da * (1.0 - sa)) / ra;
                result.data[idx + 1] = (sg * sa + dg * da * (1.0 - sa)) / ra;
                result.data[idx + 2] = (sb * sa + db * da * (1.0 - sa)) / ra;
                result.data[idx + 3] = ra;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_single_buffer() {
        let mut buf = FilterBuffer::new(2, 2);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);

        let result = merge(&[&buf]);
        assert_eq!(result.get_pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_merge_two_opaque_buffers() {
        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]); // blue

        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]); // red

        let result = merge(&[&bg, &fg]);
        let px = result.get_pixel(0, 0);
        // Opaque red over opaque blue = red
        assert!((px[0] - 1.0).abs() < 0.01);
        assert!(px[2].abs() < 0.01);
    }

    #[test]
    fn test_merge_semi_transparent() {
        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]); // opaque blue

        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 0.5]); // semi-transparent red

        let result = merge(&[&bg, &fg]);
        let px = result.get_pixel(0, 0);
        // alpha = 0.5 + 1.0 * 0.5 = 1.0
        assert!((px[3] - 1.0).abs() < 0.01);
        assert!((px[0] - 0.5).abs() < 0.01);
        assert!((px[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_merge_empty() {
        let result = merge(&[]);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }
}
