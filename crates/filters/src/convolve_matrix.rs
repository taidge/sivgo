//! feConvolveMatrix -- apply a convolution kernel to an image.

use crate::buffer::FilterBuffer;

/// Edge handling mode for out-of-bounds pixel lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    /// Clamp to the nearest edge pixel.
    Duplicate,
    /// Wrap around to the opposite edge.
    Wrap,
    /// Out-of-bounds pixels are transparent black.
    None,
}

/// Apply a convolution matrix to the input buffer.
///
/// # Parameters
/// - `kernel`: flat row-major kernel values of size `order_x * order_y`.
/// - `order_x`, `order_y`: kernel dimensions.
/// - `divisor`: sum divisor applied after accumulation. If 0, it is computed as
///   the sum of all kernel values (falling back to 1.0 if that sum is 0).
/// - `bias`: value added after division.
/// - `target_x`, `target_y`: the "hot spot" in the kernel (origin pixel).
/// - `edge_mode`: how to handle out-of-bounds lookups.
/// - `preserve_alpha`: if true, the alpha channel is copied from the input unchanged.
#[allow(clippy::too_many_arguments)]
pub fn convolve_matrix(
    input: &FilterBuffer,
    kernel: &[f32],
    order_x: u32,
    order_y: u32,
    divisor: f32,
    bias: f32,
    target_x: u32,
    target_y: u32,
    edge_mode: EdgeMode,
    preserve_alpha: bool,
) -> FilterBuffer {
    assert_eq!(
        kernel.len(),
        (order_x as usize) * (order_y as usize),
        "kernel length must equal order_x * order_y"
    );

    let w = input.width as i32;
    let h = input.height as i32;
    let ox = order_x as i32;
    let oy = order_y as i32;
    let tx = target_x as i32;
    let ty = target_y as i32;

    // Compute effective divisor.
    let effective_divisor = if divisor == 0.0 {
        let sum: f32 = kernel.iter().sum();
        if sum.abs() < 1e-10 {
            1.0
        } else {
            sum
        }
    } else {
        divisor
    };

    let mut output = input.clone_empty();

    for y in 0..h {
        for x in 0..w {
            let mut accum = [0.0f32; 4];
            let channel_count = if preserve_alpha { 3 } else { 4 };

            for ky in 0..oy {
                for kx in 0..ox {
                    let sx = x + kx - tx;
                    let sy = y + ky - ty;

                    let pixel = sample_pixel(input, sx, sy, w, h, edge_mode);
                    let k = kernel[(ky as usize) * (ox as usize) + (kx as usize)];

                    for c in 0..channel_count {
                        accum[c] += pixel[c] * k;
                    }
                }
            }

            let mut out_pixel = [0.0f32; 4];
            for c in 0..channel_count {
                out_pixel[c] = (accum[c] / effective_divisor + bias).clamp(0.0, 1.0);
            }

            if preserve_alpha {
                out_pixel[3] = input.get_pixel(x as u32, y as u32)[3];
            }

            output.set_pixel(x as u32, y as u32, out_pixel);
        }
    }

    output
}

/// Sample a pixel with edge mode handling.
fn sample_pixel(
    input: &FilterBuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    edge_mode: EdgeMode,
) -> [f32; 4] {
    match edge_mode {
        EdgeMode::Duplicate => {
            let cx = x.clamp(0, w - 1) as u32;
            let cy = y.clamp(0, h - 1) as u32;
            input.get_pixel(cx, cy)
        }
        EdgeMode::Wrap => {
            let cx = ((x % w) + w) % w;
            let cy = ((y % h) + h) % h;
            input.get_pixel(cx as u32, cy as u32)
        }
        EdgeMode::None => {
            if x >= 0 && x < w && y >= 0 && y < h {
                input.get_pixel(x as u32, y as u32)
            } else {
                [0.0, 0.0, 0.0, 0.0]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_kernel() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(1, 1, [0.5, 0.3, 0.7, 1.0]);

        // 3x3 identity kernel (only center is 1)
        let kernel = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let result = convolve_matrix(
            &buf,
            &kernel,
            3,
            3,
            1.0,
            0.0,
            1,
            1,
            EdgeMode::Duplicate,
            false,
        );

        let px = result.get_pixel(1, 1);
        assert!((px[0] - 0.5).abs() < 1e-5);
        assert!((px[1] - 0.3).abs() < 1e-5);
        assert!((px[2] - 0.7).abs() < 1e-5);
        assert!((px[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_box_blur_kernel() {
        // 3x3 buffer filled with different values
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.0, 0.0, 0.0, 1.0);
        buf.set_pixel(1, 1, [0.9, 0.9, 0.9, 1.0]);

        // 3x3 box blur kernel (all ones), divisor=9
        let kernel = vec![1.0; 9];
        let result = convolve_matrix(
            &buf,
            &kernel,
            3,
            3,
            9.0,
            0.0,
            1,
            1,
            EdgeMode::Duplicate,
            false,
        );

        // Center pixel should have an average influenced by the single bright pixel
        let px = result.get_pixel(1, 1);
        assert!(px[0] > 0.0, "box blur should spread some color");
        assert!(px[0] < 0.9, "box blur should reduce the peak");
    }

    #[test]
    fn test_edge_mode_none() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(1.0, 1.0, 1.0, 1.0);

        // 3x3 all-ones kernel, target at center
        let kernel = vec![1.0; 9];
        // EdgeMode::None: out-of-bounds pixels are transparent black
        let result = convolve_matrix(&buf, &kernel, 3, 3, 9.0, 0.0, 1, 1, EdgeMode::None, false);

        // Corner pixel (0,0): kernel extends to (-1,-1)..(1,1)
        // Only pixels (0,0),(1,0),(0,1),(1,1) are in bounds = 4 pixels
        // Sum = 4.0 / 9.0 ≈ 0.444
        let corner = result.get_pixel(0, 0);
        assert!((corner[0] - 4.0 / 9.0).abs() < 1e-4);

        // Center pixel (1,1): all 9 neighbors are in bounds
        // Sum = 9.0 / 9.0 = 1.0
        let center = result.get_pixel(1, 1);
        assert!((center[0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_edge_mode_wrap() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(1.0, 1.0, 1.0, 1.0);
        buf.set_pixel(0, 0, [0.0, 0.0, 0.0, 1.0]);

        // With wrap, the pixel at (-1,-1) wraps to (2,2) which is white
        let kernel = vec![1.0; 9];
        let result = convolve_matrix(&buf, &kernel, 3, 3, 9.0, 0.0, 1, 1, EdgeMode::Wrap, false);

        // Every pixel should see the same 9-pixel neighborhood (just shifted/wrapped).
        // Corner (0,0): all 9 pixels visible via wrap, 1 is black = 8/9
        let corner = result.get_pixel(0, 0);
        assert!((corner[0] - 8.0 / 9.0).abs() < 1e-4);
    }

    #[test]
    fn test_preserve_alpha() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(1, 1, [1.0, 1.0, 1.0, 0.5]);

        // Sharpen kernel that might modify alpha
        let kernel = vec![0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0];
        let result = convolve_matrix(
            &buf,
            &kernel,
            3,
            3,
            1.0,
            0.0,
            1,
            1,
            EdgeMode::Duplicate,
            true,
        );

        // Alpha should be preserved from input
        let px = result.get_pixel(1, 1);
        assert!((px[3] - 0.5).abs() < 1e-5, "alpha should be preserved");
    }

    #[test]
    fn test_bias() {
        let buf = FilterBuffer::new(1, 1);
        // Buffer is all zeros.

        let kernel = vec![1.0];
        let result = convolve_matrix(
            &buf,
            &kernel,
            1,
            1,
            1.0,
            0.3,
            0,
            0,
            EdgeMode::Duplicate,
            false,
        );

        let px = result.get_pixel(0, 0);
        // 0.0 / 1.0 + 0.3 = 0.3
        assert!((px[0] - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_auto_divisor() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        // divisor=0 should compute from kernel sum (3.0)
        let kernel = vec![1.0, 1.0, 1.0];
        let result = convolve_matrix(
            &buf,
            &kernel,
            3,
            1,
            0.0,
            0.0,
            1,
            0,
            EdgeMode::Duplicate,
            false,
        );

        let px = result.get_pixel(0, 0);
        // All three samples hit the same pixel (edge duplicate), sum = 1.5, / 3.0 = 0.5
        assert!((px[0] - 0.5).abs() < 1e-5);
    }
}
