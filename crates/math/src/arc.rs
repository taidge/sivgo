//! SVG arc parameterization: endpoint ↔ center-point conversion
//! and arc-to-cubic-Bezier approximation.

use crate::Point;

/// An SVG arc in endpoint parameterization (as it appears in an SVG path `d`
/// attribute `A` command).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgArc {
    /// Start point of the arc.
    pub from: Point,
    /// End point of the arc.
    pub to: Point,
    /// X and Y radii.
    pub rx: f64,
    pub ry: f64,
    /// X-axis rotation in degrees.
    pub x_rotation: f64,
    /// Large arc flag.
    pub large_arc: bool,
    /// Sweep flag.
    pub sweep: bool,
}

/// An arc in center-point parameterization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CenterArc {
    /// Center of the ellipse.
    pub center: Point,
    /// X and Y radii (after correction).
    pub rx: f64,
    pub ry: f64,
    /// X-axis rotation in radians.
    pub x_rotation: f64,
    /// Start angle in radians.
    pub start_angle: f64,
    /// Sweep angle in radians (positive = counter-clockwise).
    pub sweep_angle: f64,
}

impl SvgArc {
    /// Returns `true` if the arc degenerates to a straight line.
    pub fn is_straight_line(&self) -> bool {
        self.rx.abs() <= 1e-5 || self.ry.abs() <= 1e-5 || (self.from == self.to)
    }

    /// Convert endpoint parameterization to center-point parameterization.
    ///
    /// Returns `None` if the arc degenerates to a line (zero radii or
    /// coincident endpoints).
    pub fn to_center_arc(&self) -> Option<CenterArc> {
        let k_arc = kurbo::SvgArc {
            from: kurbo::Point::new(self.from.x, self.from.y),
            to: kurbo::Point::new(self.to.x, self.to.y),
            radii: kurbo::Vec2::new(self.rx, self.ry),
            x_rotation: self.x_rotation.to_radians(),
            large_arc: self.large_arc,
            sweep: self.sweep,
        };
        kurbo::Arc::from_svg_arc(&k_arc).map(|arc| CenterArc {
            center: Point::new(arc.center.x, arc.center.y),
            rx: arc.radii.x,
            ry: arc.radii.y,
            x_rotation: arc.x_rotation,
            start_angle: arc.start_angle,
            sweep_angle: arc.sweep_angle,
        })
    }

    /// Approximate this arc as a sequence of cubic Bezier curves.
    ///
    /// Returns a `Vec` of `(cp1, cp2, end)` tuples, each describing one cubic
    /// Bezier segment. If the arc degenerates to a line, the result is empty.
    pub fn to_cubics(&self, tolerance: f64) -> Vec<(Point, Point, Point)> {
        let k_arc = kurbo::SvgArc {
            from: kurbo::Point::new(self.from.x, self.from.y),
            to: kurbo::Point::new(self.to.x, self.to.y),
            radii: kurbo::Vec2::new(self.rx, self.ry),
            x_rotation: self.x_rotation.to_radians(),
            large_arc: self.large_arc,
            sweep: self.sweep,
        };
        let Some(arc) = kurbo::Arc::from_svg_arc(&k_arc) else {
            return Vec::new();
        };
        let mut cubics = Vec::new();
        arc.to_cubic_beziers(tolerance, |p1, p2, p3| {
            cubics.push((
                Point::new(p1.x, p1.y),
                Point::new(p2.x, p2.y),
                Point::new(p3.x, p3.y),
            ));
        });
        cubics
    }
}

impl CenterArc {
    /// Convert center-point parameterization back to endpoint parameterization.
    pub fn to_svg_arc(&self, from: Point) -> SvgArc {
        use std::f64::consts::PI;

        let end_angle = self.start_angle + self.sweep_angle;
        let (sin_rot, cos_rot) = self.x_rotation.sin_cos();
        let (sin_end, cos_end) = end_angle.sin_cos();

        let to_x = self.center.x + self.rx * cos_rot * cos_end - self.ry * sin_rot * sin_end;
        let to_y = self.center.y + self.rx * sin_rot * cos_end + self.ry * cos_rot * sin_end;

        SvgArc {
            from,
            to: Point::new(to_x, to_y),
            rx: self.rx,
            ry: self.ry,
            x_rotation: self.x_rotation.to_degrees(),
            large_arc: self.sweep_angle.abs() > PI,
            sweep: self.sweep_angle > 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_line_arc() {
        let arc = SvgArc {
            from: Point::new(0.0, 0.0),
            to: Point::new(10.0, 0.0),
            rx: 0.0,
            ry: 5.0,
            x_rotation: 0.0,
            large_arc: false,
            sweep: true,
        };
        assert!(arc.is_straight_line());
        assert!(arc.to_center_arc().is_none());
        assert!(arc.to_cubics(0.25).is_empty());
    }

    #[test]
    fn semicircle_center_conversion() {
        // A semicircular arc from (0,0) to (100,0) with r=50
        let arc = SvgArc {
            from: Point::new(0.0, 0.0),
            to: Point::new(100.0, 0.0),
            rx: 50.0,
            ry: 50.0,
            x_rotation: 0.0,
            large_arc: false,
            sweep: true,
        };
        let center = arc.to_center_arc().expect("should convert");
        assert!(
            (center.center.x - 50.0).abs() < 1e-6,
            "cx={}",
            center.center.x
        );
        assert!((center.rx - 50.0).abs() < 1e-6);
        assert!((center.ry - 50.0).abs() < 1e-6);
    }

    #[test]
    fn arc_to_cubics_nonempty() {
        let arc = SvgArc {
            from: Point::new(0.0, 0.0),
            to: Point::new(100.0, 0.0),
            rx: 50.0,
            ry: 50.0,
            x_rotation: 0.0,
            large_arc: false,
            sweep: true,
        };
        let cubics = arc.to_cubics(0.25);
        assert!(!cubics.is_empty(), "should produce cubic segments");
    }

    #[test]
    fn center_to_svg_roundtrip() {
        let original = SvgArc {
            from: Point::new(10.0, 20.0),
            to: Point::new(80.0, 20.0),
            rx: 40.0,
            ry: 40.0,
            x_rotation: 0.0,
            large_arc: false,
            sweep: true,
        };
        let center = original.to_center_arc().unwrap();
        let back = center.to_svg_arc(original.from);
        assert!((back.to.x - original.to.x).abs() < 1e-4, "to.x mismatch");
        assert!((back.to.y - original.to.y).abs() < 1e-4, "to.y mismatch");
        assert!((back.rx - original.rx).abs() < 1e-6);
        assert_eq!(back.large_arc, original.large_arc);
        assert_eq!(back.sweep, original.sweep);
    }
}
