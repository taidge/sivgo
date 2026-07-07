//! Gears Live Path Effect.
//!
//! Generates an involute spur gear shape, replacing the input path with a
//! gear centred on the input's bounding-box centre. The gear geometry is
//! controlled by the number of teeth, the module (tooth size), and the
//! pressure angle.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Point};

/// Gears LPE.
///
/// Replaces the input path with an involute spur gear centred on the
/// input's bounding-box centre. The input path is only used to determine
/// the gear's position.
#[derive(Debug, Clone)]
pub struct GearsLpe {
    /// Number of teeth on the gear.
    teeth: usize,
    /// The module (metric gear sizing: pitch_diameter / teeth).
    module: LpeParam,
    /// Pressure angle in radians (typically ~20 deg = 0.349 rad).
    pressure_angle: LpeParam,
}

impl GearsLpe {
    /// Create a new Gears effect.
    ///
    /// * `teeth` - Number of teeth (minimum 3).
    /// * `module` - Gear module (controls overall size).
    /// * `pressure_angle` - Pressure angle in radians.
    pub fn new(teeth: usize, module: f64, pressure_angle: f64) -> Self {
        Self {
            teeth: teeth.max(3),
            module: LpeParam::new("module", module, 0.1, 10_000.0),
            pressure_angle: LpeParam::new(
                "pressure_angle",
                pressure_angle,
                0.0,
                std::f64::consts::FRAC_PI_2,
            ),
        }
    }

    /// Set the number of teeth.
    pub fn set_teeth(&mut self, teeth: usize) {
        self.teeth = teeth.max(3);
    }

    /// Get the number of teeth.
    pub fn teeth(&self) -> usize {
        self.teeth
    }

    /// Generate the gear profile as a polygon centred at `center`.
    fn generate_gear(&self, center: Point) -> BezPath {
        let m = self.module.value;
        let alpha = self.pressure_angle.value;
        let n = self.teeth;

        // Standard gear geometry.
        let pitch_radius = m * n as f64 / 2.0;
        let addendum = m; // tooth extends one module above pitch circle
        let dedendum = 1.25 * m; // root goes 1.25 modules below pitch circle

        let outer_radius = pitch_radius + addendum;
        let root_radius = (pitch_radius - dedendum).max(0.0);
        let _base_radius = pitch_radius * alpha.cos();

        // Angular half-width of a tooth at the pitch circle.
        let tooth_angle = std::f64::consts::TAU / n as f64;
        let half_tooth = tooth_angle / 4.0;

        let mut path = BezPath::new();
        let mut first = true;

        for i in 0..n {
            let mid_angle = i as f64 * tooth_angle;

            // Generate points for one tooth: root -> rising flank -> tip -> falling flank -> root.
            let angles = [
                mid_angle - half_tooth * 2.0, // root start
                mid_angle - half_tooth,       // start of tooth flank (rising)
                mid_angle - half_tooth * 0.5, // halfway up
                mid_angle,                    // tooth tip centre
                mid_angle + half_tooth * 0.5, // halfway down
                mid_angle + half_tooth,       // end of tooth flank (falling)
                mid_angle + half_tooth * 2.0, // root end
            ];

            let radii = [
                root_radius,
                root_radius,
                outer_radius,
                outer_radius,
                outer_radius,
                root_radius,
                root_radius,
            ];

            for (&angle, &radius) in angles.iter().zip(radii.iter()) {
                let px = center.x + radius * angle.cos();
                let py = center.y + radius * angle.sin();
                let pt = Point::new(px, py);

                if first {
                    path.move_to(pt);
                    first = false;
                } else {
                    path.line_to(pt);
                }
            }
        }

        path.close();
        path
    }
}

impl LivePathEffect for GearsLpe {
    fn name(&self) -> &str {
        "gears"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let bb = input.bounding_box();
        let center = bb.center();

        self.generate_gear(center)
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.module)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "module" => {
                self.module.value = value.clamp(self.module.min, self.module.max);
            }
            "pressure_angle" => {
                self.pressure_angle.value =
                    value.clamp(self.pressure_angle.min, self.pressure_angle.max);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    #[test]
    fn test_gears_basic() {
        let lpe = GearsLpe::new(12, 5.0, 20.0_f64.to_radians());
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // Should have many segments (7 per tooth * 12 teeth + close).
        assert!(
            output.element_count() > 20,
            "gear should have many segments: got {}",
            output.element_count()
        );
    }

    #[test]
    fn test_gears_centered() {
        let lpe = GearsLpe::new(8, 10.0, 20.0_f64.to_radians());
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        let center = bb.center();
        // Gear should be centred near (50, 50).
        assert!((center.x - 50.0).abs() < 1.0, "cx={}", center.x);
        assert!((center.y - 50.0).abs() < 1.0, "cy={}", center.y);
    }

    #[test]
    fn test_gears_min_teeth() {
        // 0 teeth should be clamped to 3.
        let lpe = GearsLpe::new(0, 5.0, 0.35);
        assert_eq!(lpe.teeth, 3);
    }

    #[test]
    fn test_gears_different_tooth_counts() {
        let lpe6 = GearsLpe::new(6, 5.0, 0.35);
        let lpe12 = GearsLpe::new(12, 5.0, 0.35);
        let input = square();
        let out6 = lpe6.apply(&input);
        let out12 = lpe12.apply(&input);
        // More teeth = more elements.
        assert!(
            out12.element_count() > out6.element_count(),
            "12-tooth ({}) should have more elements than 6-tooth ({})",
            out12.element_count(),
            out6.element_count()
        );
    }

    #[test]
    fn test_gears_empty_path() {
        let lpe = GearsLpe::new(8, 5.0, 0.35);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_gears_params() {
        let mut lpe = GearsLpe::new(8, 5.0, 0.35);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "module");
        assert!((params[0].value - 5.0).abs() < 1e-10);

        lpe.set_param("module", 10.0);
        assert!((lpe.module.value - 10.0).abs() < 1e-10);

        lpe.set_param("pressure_angle", 0.5);
        assert!((lpe.pressure_angle.value - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_gears_name() {
        let lpe = GearsLpe::new(8, 5.0, 0.35);
        assert_eq!(lpe.name(), "gears");
    }

    #[test]
    fn test_gears_module_affects_size() {
        let lpe_small = GearsLpe::new(8, 2.0, 0.35);
        let lpe_large = GearsLpe::new(8, 10.0, 0.35);
        let input = square();
        let out_s = lpe_small.apply(&input);
        let out_l = lpe_large.apply(&input);
        let bb_s = out_s.bounding_box();
        let bb_l = out_l.bounding_box();
        // Larger module should produce larger bounding box.
        assert!(
            bb_l.width() > bb_s.width(),
            "large module width {} should exceed small {}",
            bb_l.width(),
            bb_s.width()
        );
    }
}
