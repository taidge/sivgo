//! feComposite -- composite two buffers using Porter-Duff operators or arithmetic.

use crate::buffer::FilterBuffer;

/// Compositing operators for feComposite.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CompositeOp {
    #[default]
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic {
        k1: f32,
        k2: f32,
        k3: f32,
        k4: f32,
    },
}

/// Composite two buffers.
///
/// `input1` is the primary input ("in"), `input2` is the secondary input ("in2").
/// Both buffers must have the same dimensions.
pub fn composite(input1: &FilterBuffer, input2: &FilterBuffer, op: CompositeOp) -> FilterBuffer {
    assert_eq!(input1.width, input2.width);
    assert_eq!(input1.height, input2.height);

    let mut output = input1.clone_empty();
    let pixel_count = (input1.width as usize) * (input1.height as usize);

    for i in 0..pixel_count {
        let idx = i * 4;
        let a = &input1.data[idx..idx + 4];
        let b = &input2.data[idx..idx + 4];
        let result = composite_pixel(a, b, &op);
        output.data[idx..idx + 4].copy_from_slice(&result);
    }

    output
}

fn composite_pixel(i1: &[f32], i2: &[f32], op: &CompositeOp) -> [f32; 4] {
    let a1 = i1[3];
    let a2 = i2[3];

    match op {
        CompositeOp::Over => {
            // result = i1 + i2 * (1 - i1.a)
            let ra = a1 + a2 * (1.0 - a1);
            if ra == 0.0 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            [
                (i1[0] * a1 + i2[0] * a2 * (1.0 - a1)) / ra,
                (i1[1] * a1 + i2[1] * a2 * (1.0 - a1)) / ra,
                (i1[2] * a1 + i2[2] * a2 * (1.0 - a1)) / ra,
                ra,
            ]
        }
        CompositeOp::In => {
            // result = i1 * i2.a
            let ra = a1 * a2;
            [i1[0], i1[1], i1[2], ra]
        }
        CompositeOp::Out => {
            // result = i1 * (1 - i2.a)
            let ra = a1 * (1.0 - a2);
            [i1[0], i1[1], i1[2], ra]
        }
        CompositeOp::Atop => {
            // Premultiplied: Cp1*a1*a2 + Cp2*a2*(1-a1), with result alpha a2.
            // Dividing through by a2 leaves the straight-alpha colour below.
            // The first term scales by the *source* alpha, a1; using a2 there
            // makes the result depend on the wrong layer's transparency.
            let ra = a2;
            if ra == 0.0 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            [
                i1[0] * a1 + i2[0] * (1.0 - a1),
                i1[1] * a1 + i2[1] * (1.0 - a1),
                i1[2] * a1 + i2[2] * (1.0 - a1),
                ra,
            ]
        }
        CompositeOp::Xor => {
            // result = i1 * (1 - i2.a) + i2 * (1 - i1.a)
            let ra = a1 * (1.0 - a2) + a2 * (1.0 - a1);
            if ra == 0.0 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            [
                (i1[0] * a1 * (1.0 - a2) + i2[0] * a2 * (1.0 - a1)) / ra,
                (i1[1] * a1 * (1.0 - a2) + i2[1] * a2 * (1.0 - a1)) / ra,
                (i1[2] * a1 * (1.0 - a2) + i2[2] * a2 * (1.0 - a1)) / ra,
                ra,
            ]
        }
        CompositeOp::Arithmetic { k1, k2, k3, k4 } => {
            // result = k1 * i1 * i2 + k2 * i1 + k3 * i2 + k4
            // Applied per-channel including alpha.
            let mut result = [0.0f32; 4];
            for c in 0..4 {
                result[c] = (k1 * i1[c] * i2[c] + k2 * i1[c] + k3 * i2[c] + k4).clamp(0.0, 1.0);
            }
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `atop` keeps the backdrop's alpha and blends by the source's alpha.
    #[test]
    fn atop_blends_by_the_source_alpha() {
        // Source: white at 50% over an opaque black backdrop.
        let mut src = FilterBuffer::new(1, 1);
        src.set_pixel(0, 0, [1.0, 1.0, 1.0, 0.5]);
        let mut dst = FilterBuffer::new(1, 1);
        dst.set_pixel(0, 0, [0.0, 0.0, 0.0, 1.0]);

        let out = composite(&src, &dst, CompositeOp::Atop).get_pixel(0, 0);

        // C = C1*a1 + C2*(1-a1) = 1*0.5 + 0*0.5 = 0.5, alpha = backdrop's 1.0.
        assert!((out[0] - 0.5).abs() < 1e-5, "expected 0.5, got {}", out[0]);
        assert!((out[3] - 1.0).abs() < 1e-5, "alpha comes from the backdrop");
    }

    #[test]
    fn atop_with_an_opaque_source_replaces_the_backdrop() {
        let mut src = FilterBuffer::new(1, 1);
        src.set_pixel(0, 0, [0.25, 0.5, 0.75, 1.0]);
        let mut dst = FilterBuffer::new(1, 1);
        dst.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);

        let out = composite(&src, &dst, CompositeOp::Atop).get_pixel(0, 0);
        for (got, want) in out.iter().zip([0.25, 0.5, 0.75, 1.0].iter()) {
            assert!((got - want).abs() < 1e-5, "got {out:?}");
        }
    }

    #[test]
    fn atop_with_a_transparent_source_leaves_the_backdrop() {
        let mut src = FilterBuffer::new(1, 1);
        src.set_pixel(0, 0, [1.0, 1.0, 1.0, 0.0]);
        let mut dst = FilterBuffer::new(1, 1);
        dst.set_pixel(0, 0, [0.25, 0.5, 0.75, 1.0]);

        let out = composite(&src, &dst, CompositeOp::Atop).get_pixel(0, 0);
        for (got, want) in out.iter().zip([0.25, 0.5, 0.75, 1.0].iter()) {
            assert!((got - want).abs() < 1e-5, "got {out:?}");
        }
    }

    #[test]
    fn test_composite_over() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 0.5]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]);

        let result = composite(&fg, &bg, CompositeOp::Over);
        let px = result.get_pixel(0, 0);

        // alpha = 0.5 + 1.0 * 0.5 = 1.0
        assert!((px[3] - 1.0).abs() < 0.01);
        // R = (1.0 * 0.5 + 0.0 * 1.0 * 0.5) / 1.0 = 0.5
        assert!((px[0] - 0.5).abs() < 0.01);
        // B = (0.0 * 0.5 + 1.0 * 1.0 * 0.5) / 1.0 = 0.5
        assert!((px[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_composite_in() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 0.5]);

        let result = composite(&fg, &bg, CompositeOp::In);
        let px = result.get_pixel(0, 0);

        // In: result = i1 * i2.a => alpha = 1.0 * 0.5 = 0.5
        assert!((px[3] - 0.5).abs() < 0.01);
        assert!((px[0] - 1.0).abs() < 0.01); // color stays red
    }

    #[test]
    fn test_composite_out() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 0.3]);

        let result = composite(&fg, &bg, CompositeOp::Out);
        let px = result.get_pixel(0, 0);

        // Out: alpha = 1.0 * (1 - 0.3) = 0.7
        assert!((px[3] - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_composite_xor() {
        let mut fg = FilterBuffer::new(1, 1);
        fg.set_pixel(0, 0, [1.0, 0.0, 0.0, 1.0]);

        let mut bg = FilterBuffer::new(1, 1);
        bg.set_pixel(0, 0, [0.0, 0.0, 1.0, 1.0]);

        let result = composite(&fg, &bg, CompositeOp::Xor);
        let px = result.get_pixel(0, 0);

        // Xor: both fully opaque => alpha = 1*(1-1) + 1*(1-1) = 0
        assert!(px[3].abs() < 0.01);
    }

    #[test]
    fn test_composite_arithmetic() {
        let mut a = FilterBuffer::new(1, 1);
        a.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        let mut b = FilterBuffer::new(1, 1);
        b.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        let result = composite(
            &a,
            &b,
            CompositeOp::Arithmetic {
                k1: 0.0,
                k2: 0.5,
                k3: 0.5,
                k4: 0.0,
            },
        );
        let px = result.get_pixel(0, 0);

        // result = 0*0.5*0.5 + 0.5*0.5 + 0.5*0.5 + 0 = 0.5
        assert!((px[0] - 0.5).abs() < 0.01);
    }
}
