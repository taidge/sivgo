//! feColorMatrix -- apply color transformation matrices.

use crate::buffer::FilterBuffer;

/// The type of color matrix operation.
#[derive(Debug, Clone)]
pub enum ColorMatrixType {
    /// A 4x5 matrix (20 values, row-major).
    Matrix([f32; 20]),
    /// Saturate: 0.0 = desaturated (grayscale), 1.0 = identity.
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Convert luminance to alpha (RGB become 0).
    LuminanceToAlpha,
}

/// Apply a color matrix operation to the input buffer.
pub fn color_matrix(input: &FilterBuffer, matrix_type: ColorMatrixType) -> FilterBuffer {
    let matrix = match matrix_type {
        ColorMatrixType::Matrix(m) => m,
        ColorMatrixType::Saturate(s) => saturate_matrix(s),
        ColorMatrixType::HueRotate(angle) => hue_rotate_matrix(angle),
        ColorMatrixType::LuminanceToAlpha => luminance_to_alpha_matrix(),
    };

    let mut output = input.clone_empty();
    let pixel_count = (input.width as usize) * (input.height as usize);

    for i in 0..pixel_count {
        let idx = i * 4;
        let r = input.data[idx];
        let g = input.data[idx + 1];
        let b = input.data[idx + 2];
        let a = input.data[idx + 3];

        // Apply 4x5 matrix: [R' G' B' A'] = M * [R G B A 1]
        let nr = (matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3] * a + matrix[4])
            .clamp(0.0, 1.0);
        let ng = (matrix[5] * r + matrix[6] * g + matrix[7] * b + matrix[8] * a + matrix[9])
            .clamp(0.0, 1.0);
        let nb = (matrix[10] * r + matrix[11] * g + matrix[12] * b + matrix[13] * a + matrix[14])
            .clamp(0.0, 1.0);
        let na = (matrix[15] * r + matrix[16] * g + matrix[17] * b + matrix[18] * a + matrix[19])
            .clamp(0.0, 1.0);

        output.data[idx] = nr;
        output.data[idx + 1] = ng;
        output.data[idx + 2] = nb;
        output.data[idx + 3] = na;
    }

    output
}

/// Build the 4x5 matrix for the saturate operation.
///
/// s=0 is fully desaturated (grayscale), s=1 is identity.
fn saturate_matrix(s: f32) -> [f32; 20] {
    // Per SVG spec:
    // | 0.213+0.787s  0.715-0.715s  0.072-0.072s  0  0 |
    // | 0.213-0.213s  0.715+0.285s  0.072-0.072s  0  0 |
    // | 0.213-0.213s  0.715-0.715s  0.072+0.928s  0  0 |
    // | 0             0             0              1  0 |
    [
        0.213 + 0.787 * s,
        0.715 - 0.715 * s,
        0.072 - 0.072 * s,
        0.0,
        0.0,
        0.213 - 0.213 * s,
        0.715 + 0.285 * s,
        0.072 - 0.072 * s,
        0.0,
        0.0,
        0.213 - 0.213 * s,
        0.715 - 0.715 * s,
        0.072 + 0.928 * s,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

/// Build the 4x5 matrix for hue rotation.
fn hue_rotate_matrix(angle_deg: f32) -> [f32; 20] {
    let angle = angle_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    // Per SVG spec, hueRotate is composed from:
    // luminance coefficients: 0.213, 0.715, 0.072
    [
        0.213 + cos_a * 0.787 - sin_a * 0.213,
        0.715 - cos_a * 0.715 - sin_a * 0.715,
        0.072 - cos_a * 0.072 + sin_a * 0.928,
        0.0,
        0.0,
        0.213 - cos_a * 0.213 + sin_a * 0.143,
        0.715 + cos_a * 0.285 + sin_a * 0.140,
        0.072 - cos_a * 0.072 - sin_a * 0.283,
        0.0,
        0.0,
        0.213 - cos_a * 0.213 - sin_a * 0.787,
        0.715 - cos_a * 0.715 + sin_a * 0.715,
        0.072 + cos_a * 0.928 + sin_a * 0.072,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

/// Build the 4x5 matrix for luminanceToAlpha.
fn luminance_to_alpha_matrix() -> [f32; 20] {
    // Per SVG spec:
    // | 0       0       0       0  0 |
    // | 0       0       0       0  0 |
    // | 0       0       0       0  0 |
    // | 0.2126  0.7152  0.0722  0  0 |
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.2126, 0.7152,
        0.0722, 0.0, 0.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saturate_identity() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.8, 0.3, 0.5, 1.0]);

        let result = color_matrix(&buf, ColorMatrixType::Saturate(1.0));
        let px = result.get_pixel(0, 0);

        assert!((px[0] - 0.8).abs() < 0.01);
        assert!((px[1] - 0.3).abs() < 0.01);
        assert!((px[2] - 0.5).abs() < 0.01);
        assert!((px[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_saturate_zero_produces_grayscale() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]); // pure red

        let result = color_matrix(&buf, ColorMatrixType::Saturate(0.0));
        let px = result.get_pixel(0, 0);

        // All channels should be the same (grayscale)
        assert!((px[0] - px[1]).abs() < 0.01);
        assert!((px[1] - px[2]).abs() < 0.01);
        // Luminance of pure red: ~0.213
        assert!((px[0] - 0.213).abs() < 0.01);
    }

    #[test]
    fn test_luminance_to_alpha() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [1.0, 1.0, 1.0, 1.0]); // white

        let result = color_matrix(&buf, ColorMatrixType::LuminanceToAlpha);
        let px = result.get_pixel(0, 0);

        // RGB should be 0
        assert!(px[0].abs() < 0.01);
        assert!(px[1].abs() < 0.01);
        assert!(px[2].abs() < 0.01);
        // Alpha should be ~1.0 (0.2126 + 0.7152 + 0.0722 = 1.0)
        assert!((px[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hue_rotate_360_is_identity() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.8, 0.3, 0.5, 1.0]);

        let result = color_matrix(&buf, ColorMatrixType::HueRotate(360.0));
        let px = result.get_pixel(0, 0);

        assert!((px[0] - 0.8).abs() < 0.02);
        assert!((px[1] - 0.3).abs() < 0.02);
        assert!((px[2] - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_custom_matrix() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        // Identity matrix
        let identity = [
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ];
        let result = color_matrix(&buf, ColorMatrixType::Matrix(identity));
        let px = result.get_pixel(0, 0);
        assert!((px[0] - 0.5).abs() < 0.01);
        assert!((px[1] - 0.5).abs() < 0.01);
        assert!((px[2] - 0.5).abs() < 0.01);
        assert!((px[3] - 1.0).abs() < 0.01);
    }
}
