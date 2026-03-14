//! Bend Path Live Path Effect.
//!
//! Deforms the input path along a user-supplied *bend path*. X-coordinates of
//! the input are mapped to positions along the bend path, and Y-coordinates
//! become perpendicular offsets from the bend path.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Vector};

/// Bend Path LPE.
///
/// The effect flattens the input path to line segments, then maps each point
/// `(x, y)` as follows:
///
/// 1. Normalise `x` to `[0, 1]` relative to the input bounding box width.
/// 2. Find the corresponding position on the bend path at that fraction of
///    its arc length.
/// 3. Compute the tangent and normal at that position.
/// 4. Offset perpendicular to the bend path by `y` (relative to the input's
///    vertical centre).
///
/// This produces a deformed version of the input path that follows the bend.
#[derive(Debug, Clone)]
pub struct BendPathLpe {
    bend_path: BezPath,
    samples_param: LpeParam,
}

impl BendPathLpe {
    /// Create a new Bend Path effect with the given guide path.
    pub fn new(bend_path: BezPath) -> Self {
        Self {
            bend_path,
            samples_param: LpeParam::new("samples", 64.0, 4.0, 1024.0),
        }
    }

    /// Replace the bend path.
    pub fn set_bend_path(&mut self, path: BezPath) {
        self.bend_path = path;
    }

    /// Get a reference to the current bend path.
    pub fn bend_path(&self) -> &BezPath {
        &self.bend_path
    }
}

impl LivePathEffect for BendPathLpe {
    fn name(&self) -> &str {
        "bend_path"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if self.bend_path.is_empty() || input.is_empty() {
            return input.clone();
        }

        let bend_len = self.bend_path.arc_length(0.5);
        if bend_len < 1e-10 {
            return input.clone();
        }

        // Compute input bounding box for normalisation.
        let bb = input.bounding_box();
        let in_width = bb.x1 - bb.x0;
        let in_center_y = (bb.y0 + bb.y1) * 0.5;

        if in_width < 1e-10 {
            return input.clone();
        }

        // Process each element of the input path, mapping points.
        let elements = input.elements();
        let mut result = BezPath::new();

        for el in &elements {
            match *el {
                PathSegment::MoveTo(p) => {
                    result.move_to(self.map_point(p, bb.x0, in_width, in_center_y, bend_len));
                }
                PathSegment::LineTo(p) => {
                    result.line_to(self.map_point(p, bb.x0, in_width, in_center_y, bend_len));
                }
                PathSegment::QuadTo(cp, end) => {
                    result.quad_to(
                        self.map_point(cp, bb.x0, in_width, in_center_y, bend_len),
                        self.map_point(end, bb.x0, in_width, in_center_y, bend_len),
                    );
                }
                PathSegment::CurveTo(cp1, cp2, end) => {
                    result.curve_to(
                        self.map_point(cp1, bb.x0, in_width, in_center_y, bend_len),
                        self.map_point(cp2, bb.x0, in_width, in_center_y, bend_len),
                        self.map_point(end, bb.x0, in_width, in_center_y, bend_len),
                    );
                }
                PathSegment::ClosePath => {
                    result.close();
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.samples_param)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "samples" {
            self.samples_param.value = value.clamp(self.samples_param.min, self.samples_param.max);
        }
    }
}

impl BendPathLpe {
    /// Map a single input point onto the bend path.
    fn map_point(&self, p: Point, x_min: f64, x_range: f64, y_center: f64, bend_len: f64) -> Point {
        // Normalise x to [0, 1].
        let t_norm = ((p.x - x_min) / x_range).clamp(0.0, 1.0);
        let dist = t_norm * bend_len;

        // Perpendicular offset.
        let y_offset = p.y - y_center;

        // Find position on bend path.
        let param = self.bend_path.param_at_arclen(dist, 0.5).unwrap_or(0.0);
        let center = self.bend_path.eval(param).unwrap_or(Point::ZERO);

        // Tangent via finite difference.
        let eps = 0.01;
        let p_fwd = self.bend_path.eval(param + eps).unwrap_or(center);
        let p_bck = self.bend_path.eval(param - eps).unwrap_or(center);

        let tangent = Vector::new(p_fwd.x - p_bck.x, p_fwd.y - p_bck.y);
        let len = tangent.length();
        let normal = if len > 1e-12 {
            Vector::new(-tangent.y / len, tangent.x / len)
        } else {
            Vector::new(0.0, 1.0)
        };

        Point::new(
            center.x + normal.x * y_offset,
            center.y + normal.y * y_offset,
        )
    }
}
