//! feGaussianBlur -- separable Gaussian blur filter.

use crate::buffer::FilterBuffer;

/// Apply Gaussian blur with the given standard deviations.
///
/// Uses a separable two-pass approach (horizontal then vertical).
/// Edge pixels are handled by clamping coordinates.
pub fn gaussian_blur(input: &FilterBuffer, std_dev_x: f64, std_dev_y: f64) -> FilterBuffer {
    if input.width == 0 || input.height == 0 {
        return input.clone();
    }

    // If both deviations are effectively zero, return a copy.
    if std_dev_x < 0.5 && std_dev_y < 0.5 {
        return input.clone();
    }

    // Horizontal pass
    let intermediate = if std_dev_x >= 0.5 {
        blur_pass_horizontal(input, std_dev_x)
    } else {
        input.clone()
    };

    // Vertical pass
    if std_dev_y >= 0.5 {
        blur_pass_vertical(&intermediate, std_dev_y)
    } else {
        intermediate
    }
}

/// Compute Gaussian kernel weights for the given standard deviation.
fn compute_kernel(std_dev: f64) -> Vec<f32> {
    let radius = (std_dev * 3.0).ceil() as i32;
    if radius == 0 {
        return vec![1.0];
    }
    let size = (radius * 2 + 1) as usize;
    let mut kernel = vec![0.0f32; size];
    let sigma2 = (std_dev * std_dev) as f32;
    let mut sum = 0.0f32;

    for i in 0..size {
        let x = (i as i32 - radius) as f32;
        let w = (-x * x / (2.0 * sigma2)).exp();
        kernel[i] = w;
        sum += w;
    }

    // Normalize
    for w in &mut kernel {
        *w /= sum;
    }

    kernel
}

fn blur_pass_horizontal(input: &FilterBuffer, std_dev: f64) -> FilterBuffer {
    let kernel = compute_kernel(std_dev);
    let radius = (kernel.len() / 2) as i32;
    let w = input.width as i32;
    let h = input.height as i32;
    let mut output = input.clone_empty();

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for k in 0..kernel.len() as i32 {
                let sx = (x + k - radius).clamp(0, w - 1);
                let px = input.get_pixel(sx as u32, y as u32);
                let weight = kernel[k as usize];
                r += px[0] * weight;
                g += px[1] * weight;
                b += px[2] * weight;
                a += px[3] * weight;
            }

            output.set_pixel(x as u32, y as u32, [r, g, b, a]);
        }
    }

    output
}

fn blur_pass_vertical(input: &FilterBuffer, std_dev: f64) -> FilterBuffer {
    let kernel = compute_kernel(std_dev);
    let radius = (kernel.len() / 2) as i32;
    let w = input.width as i32;
    let h = input.height as i32;
    let mut output = input.clone_empty();

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;

            for k in 0..kernel.len() as i32 {
                let sy = (y + k - radius).clamp(0, h - 1);
                let px = input.get_pixel(x as u32, sy as u32);
                let weight = kernel[k as usize];
                r += px[0] * weight;
                g += px[1] * weight;
                b += px[2] * weight;
                a += px[3] * weight;
            }

            output.set_pixel(x as u32, y as u32, [r, g, b, a]);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_std_dev_returns_copy() {
        let mut buf = FilterBuffer::new(4, 4);
        buf.set_pixel(2, 2, [1.0, 0.0, 0.0, 1.0]);
        let result = gaussian_blur(&buf, 0.0, 0.0);
        assert_eq!(result.get_pixel(2, 2), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_blur_spreads_color() {
        // Create a single bright pixel in the center of a 9x9 buffer
        let mut buf = FilterBuffer::new(9, 9);
        buf.set_pixel(4, 4, [1.0, 1.0, 1.0, 1.0]);

        let result = gaussian_blur(&buf, 2.0, 2.0);

        // The center pixel should have decreased in intensity
        let center = result.get_pixel(4, 4);
        assert!(center[0] < 1.0, "center should be dimmer after blur");
        assert!(center[0] > 0.0, "center should still have some value");

        // Neighboring pixels should have gained some intensity
        let neighbor = result.get_pixel(3, 4);
        assert!(
            neighbor[0] > 0.0,
            "neighboring pixel should pick up some color from blur"
        );

        // Far-away pixel should have very little
        let far = result.get_pixel(0, 0);
        assert!(far[0] < center[0], "far pixel should be dimmer than center");
    }

    #[test]
    fn test_blur_preserves_uniform_color() {
        // A uniformly colored buffer should remain largely unchanged after blur.
        let mut buf = FilterBuffer::new(10, 10);
        buf.fill(0.5, 0.5, 0.5, 1.0);

        let result = gaussian_blur(&buf, 3.0, 3.0);

        // Check a center pixel (edges may differ slightly due to clamping).
        let px = result.get_pixel(5, 5);
        assert!((px[0] - 0.5).abs() < 0.01);
        assert!((px[1] - 0.5).abs() < 0.01);
        assert!((px[2] - 0.5).abs() < 0.01);
        assert!((px[3] - 1.0).abs() < 0.01);
    }
}
