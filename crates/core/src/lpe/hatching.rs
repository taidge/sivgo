//! Hatching Live Path Effect.
//!
//! Fills a shape with parallel hatch lines at a given angle and spacing.
//! The lines are clipped to the interior of the input path's bounding box,
//! producing a cross-hatch or line-fill pattern.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Point};

/// Hatching LPE.
///
/// Generates parallel hatch lines that fill the input path's bounding box.
/// The angle, spacing, and line width of the hatching are adjustable.
#[derive(Debug, Clone)]
pub struct HatchingLpe {
    /// Angle of hatch lines in degrees (0 = horizontal, 90 = vertical).
    angle: LpeParam,
    /// Distance between adjacent hatch lines (in user units).
    spacing: LpeParam,
    /// Width of each hatch line (in user units). Rendered as a thin
    /// rectangular strip rather than a zero-width stroke.
    line_width: LpeParam,
}

impl HatchingLpe {
    /// Create a new Hatching effect.
    ///
    /// * `angle` - Angle of hatch lines in degrees.
    /// * `spacing` - Distance between adjacent hatch lines.
    /// * `line_width` - Width of each hatch line.
    pub fn new(angle: f64, spacing: f64, line_width: f64) -> Self {
        Self {
            angle: LpeParam::new("angle", angle, 0.0, 360.0),
            spacing: LpeParam::new("spacing", spacing, 0.1, 100_000.0),
            line_width: LpeParam::new("line_width", line_width, 0.01, 10_000.0),
        }
    }
}

impl LivePathEffect for HatchingLpe {
    fn name(&self) -> &str {
        "hatching"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() {
            return input.clone();
        }

        let bb = input.bounding_box();
        let cx = 0.5 * (bb.x0 + bb.x1);
        let cy = 0.5 * (bb.y0 + bb.y1);
        let w = bb.width();
        let h = bb.height();

        if w < 1e-10 || h < 1e-10 {
            return input.clone();
        }

        let angle_rad = self.angle.value.to_radians();
        let spacing = self.spacing.value;
        let half_lw = self.line_width.value * 0.5;

        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // The diagonal of the bounding box gives us the maximum extent we
        // need to cover when drawing rotated lines through the centre.
        let diag = (w * w + h * h).sqrt();
        let half_diag = diag * 0.5;

        // Number of lines on each side of the centre.
        let n = (half_diag / spacing).ceil() as i32;

        let mut result = BezPath::new();

        for i in -n..=n {
            let offset = i as f64 * spacing;

            // The line runs perpendicular to the offset direction. The offset
            // direction is (cos_a, sin_a), so the line direction is
            // (-sin_a, cos_a).

            // Centre of this particular hatch line, shifted along the angle
            // normal.
            let lx = cx + offset * cos_a;
            let ly = cy + offset * sin_a;

            // Endpoints of the hatch line (long enough to span the bbox).
            let p1 = Point::new(lx - half_diag * (-sin_a), ly - half_diag * cos_a);
            let p2 = Point::new(lx + half_diag * (-sin_a), ly + half_diag * cos_a);

            // Clip endpoints to the bounding box.
            if let Some((c1, c2)) = clip_line_to_rect(p1, p2, bb.x0, bb.y0, bb.x1, bb.y1) {
                if half_lw > 1e-6 {
                    // Draw as a thin rectangle.
                    let nx = cos_a * half_lw;
                    let ny = sin_a * half_lw;

                    result.move_to(Point::new(c1.x + nx, c1.y + ny));
                    result.line_to(Point::new(c2.x + nx, c2.y + ny));
                    result.line_to(Point::new(c2.x - nx, c2.y - ny));
                    result.line_to(Point::new(c1.x - nx, c1.y - ny));
                    result.close();
                } else {
                    result.move_to(c1);
                    result.line_to(c2);
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.angle)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "angle" => {
                self.angle.value = value.clamp(self.angle.min, self.angle.max);
            }
            "spacing" => {
                self.spacing.value = value.clamp(self.spacing.min, self.spacing.max);
            }
            "line_width" => {
                self.line_width.value = value.clamp(self.line_width.min, self.line_width.max);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Cohen-Sutherland line clipping against an axis-aligned rectangle.
/// Returns the clipped segment endpoints, or `None` if the line is
/// entirely outside.
fn clip_line_to_rect(
    mut p1: Point,
    mut p2: Point,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
) -> Option<(Point, Point)> {
    const INSIDE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;

    let outcode = |p: Point| -> u8 {
        let mut code = INSIDE;
        if p.x < x_min {
            code |= LEFT;
        } else if p.x > x_max {
            code |= RIGHT;
        }
        if p.y < y_min {
            code |= BOTTOM;
        } else if p.y > y_max {
            code |= TOP;
        }
        code
    };

    let mut code1 = outcode(p1);
    let mut code2 = outcode(p2);

    loop {
        if (code1 | code2) == 0 {
            return Some((p1, p2));
        }
        if (code1 & code2) != 0 {
            return None;
        }

        let code_out = if code1 != 0 { code1 } else { code2 };
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;

        let (nx, ny);
        if code_out & TOP != 0 {
            nx = p1.x + dx * (y_max - p1.y) / dy;
            ny = y_max;
        } else if code_out & BOTTOM != 0 {
            nx = p1.x + dx * (y_min - p1.y) / dy;
            ny = y_min;
        } else if code_out & RIGHT != 0 {
            ny = p1.y + dy * (x_max - p1.x) / dx;
            nx = x_max;
        } else {
            // LEFT
            ny = p1.y + dy * (x_min - p1.x) / dx;
            nx = x_min;
        }

        if code_out == code1 {
            p1 = Point::new(nx, ny);
            code1 = outcode(p1);
        } else {
            p2 = Point::new(nx, ny);
            code2 = outcode(p2);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
    fn test_hatching_produces_lines() {
        let lpe = HatchingLpe::new(0.0, 10.0, 0.5);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // Should produce many hatch line segments.
        assert!(
            output.element_count() > 4,
            "hatching should produce segments: got {}",
            output.element_count()
        );
    }

    #[test]
    fn test_hatching_more_lines_with_smaller_spacing() {
        let lpe_wide = HatchingLpe::new(0.0, 20.0, 0.5);
        let lpe_narrow = HatchingLpe::new(0.0, 5.0, 0.5);
        let input = square();
        let out_wide = lpe_wide.apply(&input);
        let out_narrow = lpe_narrow.apply(&input);

        assert!(
            out_narrow.element_count() > out_wide.element_count(),
            "narrow spacing ({}) should produce more elements than wide ({})",
            out_narrow.element_count(),
            out_wide.element_count()
        );
    }

    #[test]
    fn test_hatching_empty_input() {
        let lpe = HatchingLpe::new(45.0, 10.0, 1.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_hatching_angled() {
        let lpe = HatchingLpe::new(45.0, 10.0, 0.5);
        let input = square();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // At 45 degrees, should still produce lines.
        assert!(output.element_count() > 4);
    }

    #[test]
    fn test_hatching_params() {
        let mut lpe = HatchingLpe::new(30.0, 10.0, 1.0);
        let params = lpe.params();
        assert_eq!(params[0].name, "angle");
        assert!((params[0].value - 30.0).abs() < 1e-10);

        lpe.set_param("angle", 45.0);
        assert!((lpe.angle.value - 45.0).abs() < 1e-10);

        lpe.set_param("spacing", 5.0);
        assert!((lpe.spacing.value - 5.0).abs() < 1e-10);

        lpe.set_param("line_width", 2.0);
        assert!((lpe.line_width.value - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_hatching_name() {
        let lpe = HatchingLpe::new(0.0, 10.0, 1.0);
        assert_eq!(lpe.name(), "hatching");
    }

    #[test]
    fn test_clip_line_inside() {
        let p1 = Point::new(10.0, 10.0);
        let p2 = Point::new(90.0, 90.0);
        let result = clip_line_to_rect(p1, p2, 0.0, 0.0, 100.0, 100.0);
        assert!(result.is_some());
        let (c1, c2) = result.unwrap();
        assert!((c1.x - 10.0).abs() < 1e-10);
        assert!((c2.x - 90.0).abs() < 1e-10);
    }

    #[test]
    fn test_clip_line_outside() {
        let p1 = Point::new(-50.0, -50.0);
        let p2 = Point::new(-10.0, -10.0);
        let result = clip_line_to_rect(p1, p2, 0.0, 0.0, 100.0, 100.0);
        assert!(result.is_none());
    }
}
