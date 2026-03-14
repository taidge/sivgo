//! feComponentTransfer -- per-channel transfer functions.

use crate::buffer::FilterBuffer;

/// A transfer function applied to a single channel.
#[derive(Debug, Clone)]
pub enum TransferFunction {
    /// Output = Input (no change).
    Identity,
    /// Linear interpolation between table values.
    /// The input range [0,1] is divided into `n` equal segments where `n = table.len() - 1`.
    /// For an input value `v`, find the segment it falls in and linearly interpolate.
    Table { table: Vec<f32> },
    /// Step function from table values.
    /// The input range [0,1] is divided into `n` equal segments where `n = table.len()`.
    /// For an input value `v`, select the table entry for the segment it falls in.
    Discrete { table: Vec<f32> },
    /// Output = slope * input + intercept, clamped to [0,1].
    Linear { slope: f32, intercept: f32 },
    /// Output = amplitude * input^exponent + offset, clamped to [0,1].
    Gamma {
        amplitude: f32,
        exponent: f32,
        offset: f32,
    },
}

impl TransferFunction {
    /// Apply the transfer function to a single channel value in [0,1].
    pub fn apply(&self, v: f32) -> f32 {
        let v = v.clamp(0.0, 1.0);
        let result = match self {
            TransferFunction::Identity => v,
            TransferFunction::Table { table } => {
                if table.len() < 2 {
                    // Degenerate: with 0 entries return 0, with 1 entry return that value.
                    return table.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
                }
                let n = table.len() - 1;
                let scaled = v * n as f32;
                let i = (scaled.floor() as usize).min(n - 1);
                let frac = scaled - i as f32;
                table[i] * (1.0 - frac) + table[i + 1] * frac
            }
            TransferFunction::Discrete { table } => {
                if table.is_empty() {
                    return 0.0;
                }
                let n = table.len();
                let i = (v * n as f32).floor() as usize;
                let i = i.min(n - 1);
                table[i]
            }
            TransferFunction::Linear { slope, intercept } => slope * v + intercept,
            TransferFunction::Gamma {
                amplitude,
                exponent,
                offset,
            } => amplitude * v.powf(*exponent) + offset,
        };
        result.clamp(0.0, 1.0)
    }
}

/// Per-channel transfer function configuration.
#[derive(Debug, Clone)]
pub struct ComponentTransferFunctions {
    pub func_r: TransferFunction,
    pub func_g: TransferFunction,
    pub func_b: TransferFunction,
    pub func_a: TransferFunction,
}

impl Default for ComponentTransferFunctions {
    fn default() -> Self {
        Self {
            func_r: TransferFunction::Identity,
            func_g: TransferFunction::Identity,
            func_b: TransferFunction::Identity,
            func_a: TransferFunction::Identity,
        }
    }
}

/// Apply per-channel transfer functions to every pixel.
pub fn component_transfer(
    input: &FilterBuffer,
    functions: &ComponentTransferFunctions,
) -> FilterBuffer {
    let mut output = input.clone_empty();
    let w = input.width;
    let h = input.height;

    for y in 0..h {
        for x in 0..w {
            let px = input.get_pixel(x, y);
            let out = [
                functions.func_r.apply(px[0]),
                functions.func_g.apply(px[1]),
                functions.func_b.apply(px[2]),
                functions.func_a.apply(px[3]),
            ];
            output.set_pixel(x, y, out);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_is_noop() {
        let mut buf = FilterBuffer::new(2, 2);
        buf.set_pixel(0, 0, [0.2, 0.4, 0.6, 0.8]);
        buf.set_pixel(1, 0, [1.0, 0.0, 0.5, 1.0]);

        let funcs = ComponentTransferFunctions::default();
        let result = component_transfer(&buf, &funcs);

        assert_eq!(result.get_pixel(0, 0), [0.2, 0.4, 0.6, 0.8]);
        assert_eq!(result.get_pixel(1, 0), [1.0, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn test_linear_transfer() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.5, 0.5, 0.5, 1.0]);

        let funcs = ComponentTransferFunctions {
            func_r: TransferFunction::Linear {
                slope: 2.0,
                intercept: -0.5,
            },
            func_g: TransferFunction::Linear {
                slope: 0.0,
                intercept: 0.25,
            },
            func_b: TransferFunction::Identity,
            func_a: TransferFunction::Identity,
        };
        let result = component_transfer(&buf, &funcs);
        let px = result.get_pixel(0, 0);

        // R: 2.0 * 0.5 - 0.5 = 0.5
        assert!((px[0] - 0.5).abs() < 1e-6);
        // G: 0.0 * 0.5 + 0.25 = 0.25
        assert!((px[1] - 0.25).abs() < 1e-6);
        // B: identity = 0.5
        assert!((px[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_gamma_transfer() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.25, 0.5, 1.0, 1.0]);

        let funcs = ComponentTransferFunctions {
            func_r: TransferFunction::Gamma {
                amplitude: 1.0,
                exponent: 2.0,
                offset: 0.0,
            },
            func_g: TransferFunction::Identity,
            func_b: TransferFunction::Identity,
            func_a: TransferFunction::Identity,
        };
        let result = component_transfer(&buf, &funcs);
        let px = result.get_pixel(0, 0);

        // R: 1.0 * 0.25^2 + 0.0 = 0.0625
        assert!((px[0] - 0.0625).abs() < 1e-5);
    }

    #[test]
    fn test_table_transfer() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.5, 0.0, 1.0, 0.25]);

        let funcs = ComponentTransferFunctions {
            func_r: TransferFunction::Table {
                table: vec![0.0, 0.5, 1.0],
            },
            func_g: TransferFunction::Table {
                table: vec![0.0, 0.5, 1.0],
            },
            func_b: TransferFunction::Table {
                table: vec![0.0, 0.5, 1.0],
            },
            func_a: TransferFunction::Identity,
        };
        let result = component_transfer(&buf, &funcs);
        let px = result.get_pixel(0, 0);

        // With table [0, 0.5, 1.0], input 0.5 falls at boundary of first and second segment:
        // segment 0: [0.0, 0.5] -> [0.0, 0.5], segment 1: [0.5, 1.0] -> [0.5, 1.0]
        // input 0.5 * 2 = 1.0, i=0, frac=1.0 -> 0.0*(0.0) + 0.5*(1.0) = 0.5
        assert!((px[0] - 0.5).abs() < 1e-5);
        // input 0.0 -> 0.0
        assert!((px[1] - 0.0).abs() < 1e-5);
        // input 1.0 -> 1.0
        assert!((px[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_discrete_transfer() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [0.1, 0.5, 0.9, 1.0]);

        let funcs = ComponentTransferFunctions {
            func_r: TransferFunction::Discrete {
                table: vec![0.0, 0.5, 1.0],
            },
            func_g: TransferFunction::Discrete {
                table: vec![0.0, 0.5, 1.0],
            },
            func_b: TransferFunction::Discrete {
                table: vec![0.0, 0.5, 1.0],
            },
            func_a: TransferFunction::Identity,
        };
        let result = component_transfer(&buf, &funcs);
        let px = result.get_pixel(0, 0);

        // 3 table entries -> 3 segments: [0, 1/3), [1/3, 2/3), [2/3, 1]
        // R: 0.1 falls in first segment -> 0.0
        assert!((px[0] - 0.0).abs() < 1e-5);
        // G: 0.5 falls in second segment -> 0.5
        assert!((px[1] - 0.5).abs() < 1e-5);
        // B: 0.9 falls in third segment -> 1.0
        assert!((px[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_linear_clamps_output() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [1.0, 0.0, 0.5, 1.0]);

        let funcs = ComponentTransferFunctions {
            func_r: TransferFunction::Linear {
                slope: 2.0,
                intercept: 0.5,
            }, // 2.0*1.0+0.5 = 2.5 -> clamped to 1.0
            func_g: TransferFunction::Linear {
                slope: 1.0,
                intercept: -0.5,
            }, // 1.0*0.0-0.5 = -0.5 -> clamped to 0.0
            func_b: TransferFunction::Identity,
            func_a: TransferFunction::Identity,
        };
        let result = component_transfer(&buf, &funcs);
        let px = result.get_pixel(0, 0);

        assert!((px[0] - 1.0).abs() < 1e-6);
        assert!((px[1] - 0.0).abs() < 1e-6);
    }
}
