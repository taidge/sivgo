//! feGaussianBlur -- separable Gaussian blur filter.
//!
//! Uses the SVG-spec three-box-blur approximation of a true Gaussian. Each box
//! pass runs in O(w·h) via a sliding-window running sum, independent of the
//! blur radius, so total cost is O(w·h) instead of the old O(w·h·σ) true-kernel
//! convolution.

use crate::buffer::FilterBuffer;

/// Apply Gaussian blur with the given standard deviations.
///
/// Edge pixels are handled by shrinking the averaging window at the borders.
pub fn gaussian_blur(input: &FilterBuffer, std_dev_x: f64, std_dev_y: f64) -> FilterBuffer {
    if input.width == 0 || input.height == 0 {
        return input.clone();
    }

    // If both deviations are effectively zero, return a copy.
    if std_dev_x < 0.5 && std_dev_y < 0.5 {
        return input.clone();
    }

    let radii_x = box_radii_for_gauss(std_dev_x);
    let radii_y = box_radii_for_gauss(std_dev_y);

    let mut current = input.clone();
    // Three successive box blurs approximate a Gaussian (central-limit theorem).
    for pass in 0..3 {
        if std_dev_x >= 0.5 && radii_x[pass] > 0 {
            current = box_blur_horizontal(&current, radii_x[pass]);
        }
        if std_dev_y >= 0.5 && radii_y[pass] > 0 {
            current = box_blur_vertical(&current, radii_y[pass]);
        }
    }
    current
}

/// Compute the three box-blur radii whose successive application best
/// approximates a Gaussian of the given standard deviation (per Kovesi/the SVG
/// filter spec's three-box recommendation).
fn box_radii_for_gauss(std_dev: f64) -> [i32; 3] {
    if std_dev < 0.5 {
        return [0, 0, 0];
    }
    let n = 3.0;
    let w_ideal = (12.0 * std_dev * std_dev / n + 1.0).sqrt();
    let mut wl = w_ideal.floor();
    if (wl as i64) % 2 == 0 {
        wl -= 1.0;
    }
    let wu = wl + 2.0;
    let m_ideal =
        (12.0 * std_dev * std_dev - n * wl * wl - 4.0 * n * wl - 3.0 * n) / (-4.0 * wl - 4.0);
    let m = m_ideal.round() as i32;

    let mut radii = [0i32; 3];
    for (i, radius) in radii.iter_mut().enumerate() {
        let size = if (i as i32) < m { wl } else { wu };
        *radius = ((size as i32) - 1) / 2;
    }
    radii
}

/// One horizontal box-blur pass with a `2*radius+1` window, using a running sum
/// so each row is processed in O(width). Window shrinks at the borders.
fn box_blur_horizontal(input: &FilterBuffer, radius: i32) -> FilterBuffer {
    let w = input.width as i32;
    let h = input.height as i32;
    let mut output = input.clone_empty();
    if radius <= 0 {
        return input.clone();
    }

    for y in 0..h {
        // Prefix sums per channel across the row (length w+1).
        let mut prefix = [
            vec![0.0f32; (w + 1) as usize],
            vec![0.0f32; (w + 1) as usize],
            vec![0.0f32; (w + 1) as usize],
            vec![0.0f32; (w + 1) as usize],
        ];
        for x in 0..w {
            let px = input.get_pixel(x as u32, y as u32);
            for c in 0..4 {
                prefix[c][(x + 1) as usize] = prefix[c][x as usize] + px[c];
            }
        }
        for x in 0..w {
            let left = (x - radius).max(0);
            let right = (x + radius).min(w - 1);
            let count = (right - left + 1) as f32;
            let mut out = [0.0f32; 4];
            for c in 0..4 {
                let sum = prefix[c][(right + 1) as usize] - prefix[c][left as usize];
                out[c] = sum / count;
            }
            output.set_pixel(x as u32, y as u32, out);
        }
    }

    output
}

/// One vertical box-blur pass; analogous to [`box_blur_horizontal`].
fn box_blur_vertical(input: &FilterBuffer, radius: i32) -> FilterBuffer {
    let w = input.width as i32;
    let h = input.height as i32;
    let mut output = input.clone_empty();
    if radius <= 0 {
        return input.clone();
    }

    for x in 0..w {
        let mut prefix = [
            vec![0.0f32; (h + 1) as usize],
            vec![0.0f32; (h + 1) as usize],
            vec![0.0f32; (h + 1) as usize],
            vec![0.0f32; (h + 1) as usize],
        ];
        for y in 0..h {
            let px = input.get_pixel(x as u32, y as u32);
            for c in 0..4 {
                prefix[c][(y + 1) as usize] = prefix[c][y as usize] + px[c];
            }
        }
        for y in 0..h {
            let top = (y - radius).max(0);
            let bottom = (y + radius).min(h - 1);
            let count = (bottom - top + 1) as f32;
            let mut out = [0.0f32; 4];
            for c in 0..4 {
                let sum = prefix[c][(bottom + 1) as usize] - prefix[c][top as usize];
                out[c] = sum / count;
            }
            output.set_pixel(x as u32, y as u32, out);
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
