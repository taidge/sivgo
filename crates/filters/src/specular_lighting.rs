//! feSpecularLighting -- compute specular lighting from a light source.
//!
//! This implements the SVG feSpecularLighting filter primitive, which uses the alpha
//! channel of the input as a bump map (height field) and computes Phong-style specular
//! reflection for a given light source.

use crate::buffer::FilterBuffer;
use crate::diffuse_lighting::LightSource;

/// Compute the surface normal from the alpha channel used as a bump/height map.
///
/// Reuses the same Sobel-like central-difference approach as feDiffuseLighting.
fn surface_normal(input: &FilterBuffer, x: u32, y: u32, surface_scale: f64) -> [f64; 3] {
    let w = input.width as i32;
    let h = input.height as i32;
    let ix = x as i32;
    let iy = y as i32;

    let alpha_at = |cx: i32, cy: i32| -> f64 {
        let cx = cx.clamp(0, w - 1) as u32;
        let cy = cy.clamp(0, h - 1) as u32;
        input.get_pixel(cx, cy)[3] as f64
    };

    let dx = if ix == 0 {
        alpha_at(1, iy) - alpha_at(0, iy)
    } else if ix == w - 1 {
        alpha_at(w - 1, iy) - alpha_at(w - 2, iy)
    } else {
        (alpha_at(ix + 1, iy) - alpha_at(ix - 1, iy)) * 0.5
    };

    let dy = if iy == 0 {
        alpha_at(ix, 1) - alpha_at(ix, 0)
    } else if iy == h - 1 {
        alpha_at(ix, h - 1) - alpha_at(ix, h - 2)
    } else {
        (alpha_at(ix, iy + 1) - alpha_at(ix, iy - 1)) * 0.5
    };

    let nx = -surface_scale * dx;
    let ny = -surface_scale * dy;
    let nz: f64 = 1.0;

    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [nx / len, ny / len, nz / len]
    }
}

/// Compute the light direction vector from the surface point toward the light.
fn light_vector(light: &LightSource, px: f64, py: f64, pz: f64) -> [f64; 3] {
    match light {
        LightSource::Distant { azimuth, elevation } => {
            let az = azimuth.to_radians();
            let el = elevation.to_radians();
            [az.cos() * el.cos(), az.sin() * el.cos(), el.sin()]
        }
        LightSource::Point { x, y, z } | LightSource::Spot { x, y, z, .. } => {
            let dx = x - px;
            let dy = y - py;
            let dz = z - pz;
            let len = (dx * dx + dy * dy + dz * dz).sqrt();
            if len < 1e-12 {
                [0.0, 0.0, 1.0]
            } else {
                [dx / len, dy / len, dz / len]
            }
        }
    }
}

/// Compute spot-light attenuation (replicating the logic from diffuse_lighting).
fn spot_attenuation(light: &LightSource, light_dir: [f64; 3]) -> f64 {
    match light {
        LightSource::Spot {
            x,
            y,
            z,
            points_at_x,
            points_at_y,
            points_at_z,
            specular_exponent: spot_exp,
            limiting_cone_angle,
        } => {
            let sx = points_at_x - x;
            let sy = points_at_y - y;
            let sz = points_at_z - z;
            let len = (sx * sx + sy * sy + sz * sz).sqrt();
            if len < 1e-12 {
                return 1.0;
            }
            let s = [sx / len, sy / len, sz / len];
            let minus_l = [-light_dir[0], -light_dir[1], -light_dir[2]];
            let cos_angle = minus_l[0] * s[0] + minus_l[1] * s[1] + minus_l[2] * s[2];

            if let Some(cone) = limiting_cone_angle {
                let cos_limit = cone.to_radians().cos();
                if cos_angle < cos_limit {
                    return 0.0;
                }
            }

            if cos_angle <= 0.0 {
                0.0
            } else {
                cos_angle.powf(*spot_exp)
            }
        }
        _ => 1.0,
    }
}

/// Apply feSpecularLighting.
///
/// The input's alpha channel is used as a height map. The output is an RGBA image
/// whose RGB channels contain the specular highlight color and whose alpha channel
/// is `max(R, G, B)` (as required by the SVG spec so the highlight can be composited).
///
/// The specular factor per pixel is:
///   ks * pow(max(0, N . H), specular_exponent)
/// where H is the half-vector between the light direction L and the eye direction
/// (the SVG spec assumes the eye is at (0, 0, infinity), so the eye vector E = (0, 0, 1)).
///
/// # Parameters
/// - `input`: the source buffer (alpha channel = height map).
/// - `surface_scale`: scales the bump map heights.
/// - `specular_constant`: ks.
/// - `specular_exponent`: the Phong specular exponent.
/// - `light`: the light source.
/// - `light_color`: [r, g, b] of the light, each in [0, 1].
pub fn specular_lighting(
    input: &FilterBuffer,
    surface_scale: f64,
    specular_constant: f64,
    specular_exponent: f64,
    light: &LightSource,
    light_color: [f32; 3],
) -> FilterBuffer {
    let w = input.width;
    let h = input.height;
    let mut output = FilterBuffer::new(w, h);

    // Eye direction in SVG is always (0, 0, 1).
    let eye = [0.0f64, 0.0, 1.0];

    for y in 0..h {
        for x in 0..w {
            let n = surface_normal(input, x, y, surface_scale);

            let pz = surface_scale * (input.get_pixel(x, y)[3] as f64);
            let l = light_vector(light, x as f64, y as f64, pz);
            let attenuation = spot_attenuation(light, l);

            // Half-vector H = normalise(L + E)
            let hx = l[0] + eye[0];
            let hy = l[1] + eye[1];
            let hz = l[2] + eye[2];
            let h_len = (hx * hx + hy * hy + hz * hz).sqrt();
            let (hx, hy, hz) = if h_len < 1e-12 {
                (0.0, 0.0, 1.0)
            } else {
                (hx / h_len, hy / h_len, hz / h_len)
            };

            let n_dot_h = (n[0] * hx + n[1] * hy + n[2] * hz).max(0.0);
            let factor = (specular_constant * n_dot_h.powf(specular_exponent) * attenuation) as f32;

            let r = (light_color[0] * factor).clamp(0.0, 1.0);
            let g = (light_color[1] * factor).clamp(0.0, 1.0);
            let b = (light_color[2] * factor).clamp(0.0, 1.0);
            let a = r.max(g).max(b);

            output.set_pixel(x, y, [r, g, b, a]);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_surface_specular_distant() {
        // Flat surface with light straight above => strong specular highlight.
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 90.0,
        };
        let result = specular_lighting(&buf, 1.0, 1.0, 20.0, &light, [1.0, 1.0, 1.0]);

        // Light straight down, eye straight up, flat surface: H = (0,0,1), N = (0,0,1)
        // N.H = 1.0, factor = 1.0^20 = 1.0
        let px = result.get_pixel(2, 2);
        assert!((px[0] - 1.0).abs() < 0.05, "expected ~1.0, got {}", px[0]);
        // Alpha should equal max(R,G,B)
        assert!((px[3] - px[0].max(px[1]).max(px[2])).abs() < 1e-5);
    }

    #[test]
    fn test_zero_specular_constant() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.0, 0.0, 0.0, 1.0);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 45.0,
        };
        let result = specular_lighting(&buf, 1.0, 0.0, 10.0, &light, [1.0, 1.0, 1.0]);

        let px = result.get_pixel(1, 1);
        assert!((px[0]).abs() < 1e-5);
        assert!((px[1]).abs() < 1e-5);
        assert!((px[2]).abs() < 1e-5);
        assert!((px[3]).abs() < 1e-5);
    }

    #[test]
    fn test_high_exponent_narrows_highlight() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Distant {
            azimuth: 45.0,
            elevation: 45.0,
        };

        let low_exp = specular_lighting(&buf, 1.0, 1.0, 1.0, &light, [1.0, 1.0, 1.0]);
        let high_exp = specular_lighting(&buf, 1.0, 1.0, 50.0, &light, [1.0, 1.0, 1.0]);

        // With a higher exponent, the highlight should be narrower (less bright at off-peak).
        let low_val = low_exp.get_pixel(2, 2)[0];
        let high_val = high_exp.get_pixel(2, 2)[0];

        // For off-axis light, higher exponent gives smaller value.
        assert!(
            high_val <= low_val + 0.01,
            "high exp ({}) should be <= low exp ({})",
            high_val,
            low_val
        );
    }

    #[test]
    fn test_specular_alpha_is_max_rgb() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 60.0,
        };
        // Use a colored light so R != G != B potentially
        let result = specular_lighting(&buf, 1.0, 1.0, 5.0, &light, [1.0, 0.5, 0.2]);

        let px = result.get_pixel(1, 1);
        let expected_alpha = px[0].max(px[1]).max(px[2]);
        assert!(
            (px[3] - expected_alpha).abs() < 1e-5,
            "alpha ({}) should equal max(R,G,B) ({})",
            px[3],
            expected_alpha
        );
    }

    #[test]
    fn test_point_light_specular() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Point {
            x: 2.0,
            y: 2.0,
            z: 5.0,
        };
        let result = specular_lighting(&buf, 1.0, 1.0, 10.0, &light, [1.0, 1.0, 1.0]);

        // Should produce some highlight, especially at center.
        let center = result.get_pixel(2, 2);
        assert!(center[0] > 0.0, "center should have some specular light");
    }
}
