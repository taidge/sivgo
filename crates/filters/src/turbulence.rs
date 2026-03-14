//! feTurbulence -- Perlin noise generation per the SVG specification.

use crate::buffer::FilterBuffer;

/// Turbulence type: classic turbulence or fractal noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurbulenceType {
    FractalNoise,
    Turbulence,
}

impl Default for TurbulenceType {
    fn default() -> Self {
        TurbulenceType::Turbulence
    }
}

// ---------------------------------------------------------------------------
// Perlin noise implementation based on the SVG spec reference
// ---------------------------------------------------------------------------

const B_SIZE: usize = 0x100;
const BM: usize = 0xff;
const PERLIN_N: usize = 0x1000;

/// Internal state for the Perlin noise generator.
struct PerlinNoise {
    lattice_selector: [usize; B_SIZE + B_SIZE + 2],
    gradient: [[[f64; 2]; B_SIZE + B_SIZE + 2]; 4],
}

impl PerlinNoise {
    fn new(seed: i32) -> Self {
        let mut pn = PerlinNoise {
            lattice_selector: [0; B_SIZE + B_SIZE + 2],
            gradient: [[[0.0; 2]; B_SIZE + B_SIZE + 2]; 4],
        };
        pn.init(seed);
        pn
    }

    fn init(&mut self, seed: i32) {
        // Use a simple LCG seeded by the input seed
        let mut s = seed as u32;
        let next = |s: &mut u32| -> u32 {
            // Simple LCG
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            (*s >> 16) & 0x7fff
        };

        for k in 0..4 {
            for i in 0..B_SIZE {
                self.lattice_selector[i] = i;
                for j in 0..2 {
                    let v = next(&mut s);
                    self.gradient[k][i][j] =
                        ((v as f64 % (2 * B_SIZE) as f64) - B_SIZE as f64) / B_SIZE as f64;
                }
                // Normalize gradient
                let d = (self.gradient[k][i][0] * self.gradient[k][i][0]
                    + self.gradient[k][i][1] * self.gradient[k][i][1])
                    .sqrt();
                if d > 0.0 {
                    self.gradient[k][i][0] /= d;
                    self.gradient[k][i][1] /= d;
                }
            }
        }

        // Shuffle using Fisher-Yates
        for i in (1..B_SIZE).rev() {
            let j = next(&mut s) as usize % (i + 1);
            self.lattice_selector.swap(i, j);
        }

        // Duplicate the table
        for i in 0..B_SIZE + 2 {
            self.lattice_selector[B_SIZE + i] = self.lattice_selector[i];
            for k in 0..4 {
                self.gradient[k][B_SIZE + i] = self.gradient[k][i];
            }
        }
    }

    fn noise2(&self, channel: usize, vec: [f64; 2]) -> f64 {
        let t = vec[0] + PERLIN_N as f64;
        let bx0 = t as usize & BM;
        let bx1 = (bx0 + 1) & BM;
        let rx0 = t - t.floor();
        let rx1 = rx0 - 1.0;

        let t = vec[1] + PERLIN_N as f64;
        let by0 = t as usize & BM;
        let by1 = (by0 + 1) & BM;
        let ry0 = t - t.floor();
        let ry1 = ry0 - 1.0;

        let i = self.lattice_selector[bx0];
        let j = self.lattice_selector[bx1];

        let b00 = self.lattice_selector[i + by0];
        let b10 = self.lattice_selector[j + by0];
        let b01 = self.lattice_selector[i + by1];
        let b11 = self.lattice_selector[j + by1];

        let sx = s_curve(rx0);
        let sy = s_curve(ry0);

        let u = rx0 * self.gradient[channel][b00][0] + ry0 * self.gradient[channel][b00][1];
        let v = rx1 * self.gradient[channel][b10][0] + ry0 * self.gradient[channel][b10][1];
        let a = lerp(sx, u, v);

        let u = rx0 * self.gradient[channel][b01][0] + ry1 * self.gradient[channel][b01][1];
        let v = rx1 * self.gradient[channel][b11][0] + ry1 * self.gradient[channel][b11][1];
        let b = lerp(sx, u, v);

        lerp(sy, a, b)
    }
}

fn s_curve(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

/// Generate a turbulence or fractal noise pattern.
///
/// Parameters mirror the SVG `<feTurbulence>` element attributes.
pub fn turbulence(
    width: u32,
    height: u32,
    base_freq_x: f64,
    base_freq_y: f64,
    num_octaves: u32,
    seed: i32,
    turb_type: TurbulenceType,
    _stitch: bool,
) -> FilterBuffer {
    let pn = PerlinNoise::new(seed);
    let mut output = FilterBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let mut pixel = [0.0f32; 4];

            for channel in 0..4 {
                let mut freq_x = base_freq_x;
                let mut freq_y = base_freq_y;
                let mut amplitude = 1.0;
                let mut value = 0.0;

                for _ in 0..num_octaves {
                    let noise = pn.noise2(channel % 4, [x as f64 * freq_x, y as f64 * freq_y]);

                    match turb_type {
                        TurbulenceType::FractalNoise => {
                            value += noise * amplitude;
                        }
                        TurbulenceType::Turbulence => {
                            value += noise.abs() * amplitude;
                        }
                    }

                    freq_x *= 2.0;
                    freq_y *= 2.0;
                    amplitude *= 0.5;
                }

                match turb_type {
                    TurbulenceType::FractalNoise => {
                        // Map from [-1, 1] to [0, 1]
                        pixel[channel] = ((value + 1.0) / 2.0).clamp(0.0, 1.0) as f32;
                    }
                    TurbulenceType::Turbulence => {
                        pixel[channel] = value.clamp(0.0, 1.0) as f32;
                    }
                }
            }

            output.set_pixel(x, y, pixel);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turbulence_produces_nonzero() {
        let buf = turbulence(10, 10, 0.05, 0.05, 2, 42, TurbulenceType::Turbulence, false);

        // At least some pixels should be non-zero
        let mut has_nonzero = false;
        for y in 0..10 {
            for x in 0..10 {
                let px = buf.get_pixel(x, y);
                if px[0] > 0.0 || px[1] > 0.0 || px[2] > 0.0 {
                    has_nonzero = true;
                    break;
                }
            }
        }
        assert!(
            has_nonzero,
            "turbulence should produce some non-zero pixels"
        );
    }

    #[test]
    fn test_fractal_noise_produces_values() {
        let buf = turbulence(
            10,
            10,
            0.1,
            0.1,
            3,
            123,
            TurbulenceType::FractalNoise,
            false,
        );

        let px = buf.get_pixel(5, 5);
        // FractalNoise maps to [0, 1], should have some value
        assert!(px[0] >= 0.0 && px[0] <= 1.0);
        assert!(px[3] >= 0.0 && px[3] <= 1.0);
    }

    #[test]
    fn test_different_seeds_produce_different_results() {
        let buf1 = turbulence(5, 5, 0.1, 0.1, 1, 1, TurbulenceType::Turbulence, false);
        let buf2 = turbulence(5, 5, 0.1, 0.1, 1, 999, TurbulenceType::Turbulence, false);

        // Very unlikely that all pixels are identical with different seeds.
        let mut all_same = true;
        for i in 0..buf1.data.len() {
            if (buf1.data[i] - buf2.data[i]).abs() > 0.001 {
                all_same = false;
                break;
            }
        }
        assert!(!all_same, "different seeds should produce different noise");
    }
}
