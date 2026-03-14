//! feDiffuseLighting -- compute diffuse lighting from a light source.
//!
//! This implements the SVG feDiffuseLighting filter primitive, which uses the alpha
//! channel of the input as a bump map (height field) and computes Lambertian diffuse
//! reflection for a given light source and surface color.

use crate::buffer::FilterBuffer;

/// A light source for lighting filter primitives.
#[derive(Debug, Clone)]
pub enum LightSource {
    /// Distant light: parallel rays from a given direction.
    /// `azimuth` and `elevation` are in degrees.
    Distant { azimuth: f64, elevation: f64 },
    /// Point light at a fixed 3D position.
    Point { x: f64, y: f64, z: f64 },
    /// Spot light at a position pointing at a target, with cone angle and exponent.
    Spot {
        x: f64,
        y: f64,
        z: f64,
        points_at_x: f64,
        points_at_y: f64,
        points_at_z: f64,
        specular_exponent: f64,
        limiting_cone_angle: Option<f64>,
    },
}

/// Compute the light direction vector for a given surface point.
/// Returns a normalised (Lx, Ly, Lz) pointing *toward* the light.
fn light_vector(light: &LightSource, px: f64, py: f64, pz: f64) -> [f64; 3] {
    match light {
        LightSource::Distant { azimuth, elevation } => {
            let az = azimuth.to_radians();
            let el = elevation.to_radians();
            let lx = az.cos() * el.cos();
            let ly = az.sin() * el.cos();
            let lz = el.sin();
            [lx, ly, lz]
        }
        LightSource::Point { x, y, z } => {
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
        LightSource::Spot {
            x,
            y,
            z,
            points_at_x: _,
            points_at_y: _,
            points_at_z: _,
            specular_exponent: _,
            limiting_cone_angle: _,
        } => {
            // Light vector from surface toward spot position.
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

/// Compute the spot-light attenuation factor for the given light direction.
/// Returns a multiplier in [0, 1].
fn spot_attenuation(light: &LightSource, light_dir: [f64; 3]) -> f64 {
    match light {
        LightSource::Spot {
            x,
            y,
            z,
            points_at_x,
            points_at_y,
            points_at_z,
            specular_exponent,
            limiting_cone_angle,
        } => {
            // Direction from light position toward the target.
            let sx = points_at_x - x;
            let sy = points_at_y - y;
            let sz = points_at_z - z;
            let len = (sx * sx + sy * sy + sz * sz).sqrt();
            if len < 1e-12 {
                return 1.0;
            }
            let s = [sx / len, sy / len, sz / len];

            // `light_dir` points from surface toward the light.
            // The spot direction points from the light toward the target.
            // cos(angle) = dot(-light_dir, s) = -dot(light_dir, s)
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
                cos_angle.powf(*specular_exponent)
            }
        }
        _ => 1.0,
    }
}

/// Compute the surface normal from the alpha channel used as a bump/height map.
///
/// Uses the Sobel-like central-difference approach recommended by SVG spec:
///   nx = -surface_scale * (dα/dx)
///   ny = -surface_scale * (dα/dy)
///   nz = 1.0
/// then normalise.
fn surface_normal(input: &FilterBuffer, x: u32, y: u32, surface_scale: f64) -> [f64; 3] {
    let w = input.width as i32;
    let h = input.height as i32;
    let ix = x as i32;
    let iy = y as i32;

    // Fetch alpha at a coordinate, clamping to edges.
    let alpha_at = |cx: i32, cy: i32| -> f64 {
        let cx = cx.clamp(0, w - 1) as u32;
        let cy = cy.clamp(0, h - 1) as u32;
        input.get_pixel(cx, cy)[3] as f64
    };

    // Sobel-like factors depending on position (edge vs interior).
    // For interior pixels, use central differences scaled by 1/4 per the SVG spec factor tables.
    // Simplified: use central differences everywhere, clamping at edges.
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

/// Apply feDiffuseLighting.
///
/// The input's alpha channel is used as a height map. The output is an RGBA image
/// whose RGB channels contain the lighting color scaled by the diffuse factor
/// `kd * max(0, N . L)`, and whose alpha channel is 1.0.
///
/// # Parameters
/// - `input`: the source buffer (alpha channel = height map).
/// - `surface_scale`: scales the bump map heights.
/// - `diffuse_constant`: kd, the diffuse reflection constant.
/// - `light`: the light source.
/// - `light_color`: [r, g, b] of the light, each in [0, 1].
pub fn diffuse_lighting(
    input: &FilterBuffer,
    surface_scale: f64,
    diffuse_constant: f64,
    light: &LightSource,
    light_color: [f32; 3],
) -> FilterBuffer {
    let w = input.width;
    let h = input.height;
    let mut output = FilterBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let n = surface_normal(input, x, y, surface_scale);

            let pz = surface_scale * (input.get_pixel(x, y)[3] as f64);
            let l = light_vector(light, x as f64, y as f64, pz);
            let attenuation = spot_attenuation(light, l);

            // N . L
            let n_dot_l = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
            let factor = (diffuse_constant * n_dot_l.max(0.0) * attenuation) as f32;

            let out = [
                (light_color[0] * factor).clamp(0.0, 1.0),
                (light_color[1] * factor).clamp(0.0, 1.0),
                (light_color[2] * factor).clamp(0.0, 1.0),
                1.0,
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
    fn test_flat_surface_distant_light() {
        // Flat surface (uniform alpha) should get uniform lighting.
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 90.0,
        };
        let result = diffuse_lighting(&buf, 1.0, 1.0, &light, [1.0, 1.0, 1.0]);

        // With elevation=90, light is straight down (Lz=1).
        // Flat surface: normal is (0,0,1). N.L = 1.0.
        let px = result.get_pixel(2, 2);
        assert!((px[0] - 1.0).abs() < 0.05, "expected ~1.0, got {}", px[0]);
        assert_eq!(px[3], 1.0);
    }

    #[test]
    fn test_point_light_falloff() {
        // Even flat surface gets different lighting from a point light at different positions.
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Point {
            x: 2.0,
            y: 2.0,
            z: 2.0,
        };
        let result = diffuse_lighting(&buf, 1.0, 1.0, &light, [1.0, 1.0, 1.0]);

        // Center pixel should be brightest (directly below the point light).
        let center = result.get_pixel(2, 2);
        let corner = result.get_pixel(0, 0);
        assert!(
            center[0] >= corner[0],
            "center ({}) should be >= corner ({})",
            center[0],
            corner[0]
        );
    }

    #[test]
    fn test_zero_diffuse_constant() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.0, 0.0, 0.0, 1.0);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 45.0,
        };
        let result = diffuse_lighting(&buf, 1.0, 0.0, &light, [1.0, 1.0, 1.0]);

        // kd=0 means no diffuse reflection, output should be black.
        let px = result.get_pixel(1, 1);
        assert!((px[0]).abs() < 1e-5);
        assert!((px[1]).abs() < 1e-5);
        assert!((px[2]).abs() < 1e-5);
        assert_eq!(px[3], 1.0);
    }

    #[test]
    fn test_spot_light_outside_cone() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        // Spot light pointing straight down at (2,2) with a very narrow cone.
        let light = LightSource::Spot {
            x: 2.0,
            y: 2.0,
            z: 10.0,
            points_at_x: 2.0,
            points_at_y: 2.0,
            points_at_z: 0.0,
            specular_exponent: 1.0,
            limiting_cone_angle: Some(1.0), // very narrow: ~1 degree
        };
        let result = diffuse_lighting(&buf, 1.0, 1.0, &light, [1.0, 1.0, 1.0]);

        // Corner pixel should be outside the cone and thus dark (or very dim).
        let corner = result.get_pixel(0, 0);
        assert!(
            corner[0] < 0.1,
            "corner outside narrow cone should be dim, got {}",
            corner[0]
        );
    }

    #[test]
    fn test_light_color_applied() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.0, 0.0, 0.0, 0.5);

        let light = LightSource::Distant {
            azimuth: 0.0,
            elevation: 90.0,
        };
        // Red light
        let result = diffuse_lighting(&buf, 1.0, 1.0, &light, [1.0, 0.0, 0.0]);

        let px = result.get_pixel(1, 1);
        assert!(px[0] > 0.5, "red channel should be lit");
        assert!((px[1]).abs() < 1e-5, "green should be zero");
        assert!((px[2]).abs() < 1e-5, "blue should be zero");
    }
}
