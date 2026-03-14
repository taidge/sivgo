//! feBlend -- blend two buffers using various blend modes.

use crate::buffer::FilterBuffer;

/// SVG/CSS blend modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::Normal
    }
}

/// Blend two buffers using the specified mode.
///
/// `input1` is the foreground ("in"), `input2` is the background ("in2").
/// Both buffers must have the same dimensions.
pub fn blend(input1: &FilterBuffer, input2: &FilterBuffer, mode: BlendMode) -> FilterBuffer {
    assert_eq!(input1.width, input2.width);
    assert_eq!(input1.height, input2.height);

    let mut output = input1.clone_empty();
    let pixel_count = (input1.width as usize) * (input1.height as usize);

    for i in 0..pixel_count {
        let idx = i * 4;
        let fg = &input1.data[idx..idx + 4];
        let bg = &input2.data[idx..idx + 4];
        let blended = blend_pixel(fg, bg, mode);
        output.data[idx..idx + 4].copy_from_slice(&blended);
    }

    output
}

/// Blend a single pixel.
///
/// Uses the SVG compositing model: apply the blend formula to the RGB channels,
/// then composite with Porter-Duff source-over for alpha.
fn blend_pixel(fg: &[f32], bg: &[f32], mode: BlendMode) -> [f32; 4] {
    let fa = fg[3];
    let ba = bg[3];

    // If both are fully transparent, result is transparent.
    if fa == 0.0 && ba == 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    // Result alpha (Porter-Duff source-over)
    let ra = fa + ba * (1.0 - fa);

    if ra == 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    let mut result = [0.0f32; 4];
    result[3] = ra;

    for c in 0..3 {
        let fc = fg[c];
        let bc = bg[c];

        // Apply blend mode to get the blended color
        let blended = match mode {
            BlendMode::Normal => fc,
            BlendMode::Multiply => fc * bc,
            BlendMode::Screen => fc + bc - fc * bc,
            BlendMode::Overlay => {
                if bc <= 0.5 {
                    2.0 * fc * bc
                } else {
                    1.0 - 2.0 * (1.0 - fc) * (1.0 - bc)
                }
            }
            BlendMode::Darken => fc.min(bc),
            BlendMode::Lighten => fc.max(bc),
            BlendMode::ColorDodge => {
                if fc >= 1.0 {
                    1.0
                } else {
                    (bc / (1.0 - fc)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if fc <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - bc) / fc).min(1.0)
                }
            }
            BlendMode::HardLight => {
                if fc <= 0.5 {
                    2.0 * fc * bc
                } else {
                    1.0 - 2.0 * (1.0 - fc) * (1.0 - bc)
                }
            }
            BlendMode::SoftLight => {
                if fc <= 0.5 {
                    bc - (1.0 - 2.0 * fc) * bc * (1.0 - bc)
                } else {
                    let d = if bc <= 0.25 {
                        ((16.0 * bc - 12.0) * bc + 4.0) * bc
                    } else {
                        bc.sqrt()
                    };
                    bc + (2.0 * fc - 1.0) * (d - bc)
                }
            }
            BlendMode::Difference => (fc - bc).abs(),
            BlendMode::Exclusion => fc + bc - 2.0 * fc * bc,
        };

        // Composite: Cs = (fa * blended + ba * bc * (1 - fa)) / ra
        // But per SVG spec, the blended value replaces the fg contribution in the
        // overlapping region.
        result[c] = (fa * ba * blended + fa * fc * (1.0 - ba) + ba * bc * (1.0 - fa)) / ra;

        // For Normal mode, simplify to standard source-over:
        if mode == BlendMode::Normal {
            result[c] = (fa * fc + ba * bc * (1.0 - fa)) / ra;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_normal_opaque_fg_over_bg() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]); // red, fully opaque

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]); // blue, fully opaque

        let result = blend(&fg, &bg, BlendMode::Normal);
        let px = result.get_pixel(0, 0);
        // Opaque red over opaque blue = red
        assert!((px[0] - 1.0).abs() < 0.01);
        assert!(px[1].abs() < 0.01);
        assert!(px[2].abs() < 0.01);
        assert!((px[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_blend_normal_semi_transparent() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 0.5]); // semi-transparent red

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]); // opaque blue

        let result = blend(&fg, &bg, BlendMode::Normal);
        let px = result.get_pixel(0, 0);
        // alpha = 0.5 + 1.0 * 0.5 = 1.0
        // R = (0.5 * 1.0 + 1.0 * 0.0 * 0.5) / 1.0 = 0.5
        // B = (0.5 * 0.0 + 1.0 * 1.0 * 0.5) / 1.0 = 0.5
        assert!((px[0] - 0.5).abs() < 0.01);
        assert!((px[2] - 0.5).abs() < 0.01);
        assert!((px[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_blend_multiply() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.8, 0.4, 0.2, 1.0]);

        let result = blend(&fg, &bg, BlendMode::Multiply);
        let px = result.get_pixel(0, 0);
        // Both opaque => blended = fg * bg, then composite = blended
        assert!((px[0] - 0.4).abs() < 0.01); // 0.5 * 0.8
        assert!((px[1] - 0.2).abs() < 0.01); // 0.5 * 0.4
        assert!((px[2] - 0.1).abs() < 0.01); // 0.5 * 0.2
    }

    #[test]
    fn test_blend_screen() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [0.5, 0.0, 0.0, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.5, 0.0, 0.0, 1.0]);

        let result = blend(&fg, &bg, BlendMode::Screen);
        let px = result.get_pixel(0, 0);
        // Screen: 0.5 + 0.5 - 0.5*0.5 = 0.75
        assert!((px[0] - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_blend_difference() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [0.8, 0.2, 0.5, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.3, 0.7, 0.5, 1.0]);

        let result = blend(&fg, &bg, BlendMode::Difference);
        let px = result.get_pixel(0, 0);
        assert!((px[0] - 0.5).abs() < 0.01); // |0.8 - 0.3|
        assert!((px[1] - 0.5).abs() < 0.01); // |0.2 - 0.7|
        assert!(px[2].abs() < 0.01); // |0.5 - 0.5|
    }
}
