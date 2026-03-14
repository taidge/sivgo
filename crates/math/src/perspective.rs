//! Perspective (projective) transform for the 3D box tool.
//!
//! A perspective transform is a full 3×3 homogeneous matrix, unlike
//! an affine transform which constrains the bottom row to `[0, 0, 1]`.
//!
//! ```text
//! | a  b  c |   | x |   | x' |
//! | d  e  f | × | y | = | y' |
//! | g  h  1 |   | 1 |   | w' |
//! ```
//!
//! The result in 2D is `(x'/w', y'/w')`.

use crate::Point;

// ---------------------------------------------------------------------------
// VanishingPoint
// ---------------------------------------------------------------------------

/// A vanishing point for perspective projection.
///
/// In a one-, two-, or three-point perspective system each axis of the 3D box
/// converges towards a vanishing point.  When `is_finite` is `false` the axis
/// uses parallel (axonometric) projection -- the vanishing point is at
/// infinity and the `x`/`y` fields store the direction rather than a position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VanishingPoint {
    /// X coordinate (or X direction component when `is_finite` is `false`).
    pub x: f64,
    /// Y coordinate (or Y direction component when `is_finite` is `false`).
    pub y: f64,
    /// `true` when the vanishing point is at a finite location; `false` for
    /// parallel (infinite) projection along this axis.
    pub is_finite: bool,
}

impl VanishingPoint {
    /// Create a finite vanishing point at the given coordinates.
    pub fn finite(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            is_finite: true,
        }
    }

    /// Create an infinite (parallel) vanishing point with the given direction.
    pub fn infinite(dx: f64, dy: f64) -> Self {
        Self {
            x: dx,
            y: dy,
            is_finite: false,
        }
    }

    /// Parse a vanishing-point coordinate string.
    ///
    /// The string is expected to be either `"inf"` (infinite / parallel
    /// projection) or a coordinate pair `"x,y"`.  Returns `None` on parse
    /// failure.
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("inf") {
            return Some(Self::infinite(0.0, 0.0));
        }
        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() == 2 {
            let x: f64 = parts[0].trim().parse().ok()?;
            let y: f64 = parts[1].trim().parse().ok()?;
            Some(Self::finite(x, y))
        } else {
            // Try as a single number (used as x, y=0)
            let x: f64 = trimmed.parse().ok()?;
            Some(Self::finite(x, 0.0))
        }
    }

    /// Serialize to a string suitable for an SVG attribute.
    pub fn to_svg_string(&self) -> String {
        if self.is_finite {
            format!("{},{}", self.x, self.y)
        } else {
            "inf".to_string()
        }
    }

    /// Project a point towards this vanishing point by the given `t` factor.
    ///
    /// For a finite VP the result moves from `point` towards the VP by `t`
    /// (0 = at point, 1 = at VP).
    ///
    /// For an infinite VP the point is translated by `t * direction`.
    pub fn project(&self, point: Point, t: f64) -> Point {
        if self.is_finite {
            point.lerp(Point::new(self.x, self.y), t)
        } else {
            Point::new(point.x + self.x * t, point.y + self.y * t)
        }
    }
}

impl Default for VanishingPoint {
    fn default() -> Self {
        Self::finite(0.0, 0.0)
    }
}

// ---------------------------------------------------------------------------
// PerspectiveTransform
// ---------------------------------------------------------------------------

/// A 3×3 perspective transform matrix.
///
/// Stored as 8 coefficients `[a, b, c, d, e, f, g, h]`, with the bottom-right
/// entry implicitly 1. This is sufficient for any projective mapping of
/// the 2D plane (including affine transforms as a special case where g=h=0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerspectiveTransform {
    /// Coefficients: `[a, b, c, d, e, f, g, h]`.
    m: [f64; 8],
}

impl PerspectiveTransform {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    };

    /// Create from explicit coefficients `[a, b, c, d, e, f, g, h]`.
    pub fn new(m: [f64; 8]) -> Self {
        Self { m }
    }

    /// The raw coefficients.
    pub fn as_coeffs(&self) -> &[f64; 8] {
        &self.m
    }

    /// Apply the perspective transform to a point.
    ///
    /// Returns `None` if the projected `w` is zero or very close to zero.
    pub fn transform_point(&self, p: Point) -> Option<Point> {
        let [a, b, c, d, e, f, g, h] = self.m;
        let x = a * p.x + b * p.y + c;
        let y = d * p.x + e * p.y + f;
        let w = g * p.x + h * p.y + 1.0;

        if w.abs() < 1e-12 {
            return None;
        }

        Some(Point::new(x / w, y / w))
    }

    /// Compose two perspective transforms: `self` then `other`.
    ///
    /// The resulting transform first applies `self`, then `other`.
    /// In matrix terms this is `other * self`.
    pub fn then(&self, other: &PerspectiveTransform) -> Self {
        // other * self  (so that `self` is applied first).
        let [a1, b1, c1, d1, e1, f1, g1, h1] = other.m;
        let [a2, b2, c2, d2, e2, f2, g2, h2] = self.m;

        // self matrix:
        // | a1 b1 c1 |
        // | d1 e1 f1 |
        // | g1 h1  1 |
        //
        // other matrix:
        // | a2 b2 c2 |
        // | d2 e2 f2 |
        // | g2 h2  1 |

        let ra = a1 * a2 + b1 * d2 + c1 * g2;
        let rb = a1 * b2 + b1 * e2 + c1 * h2;
        let rc = a1 * c2 + b1 * f2 + c1;

        let rd = d1 * a2 + e1 * d2 + f1 * g2;
        let re = d1 * b2 + e1 * e2 + f1 * h2;
        let rf = d1 * c2 + e1 * f2 + f1;

        let rg = g1 * a2 + h1 * d2 + g2;
        let rh = g1 * b2 + h1 * e2 + h2;
        let rw = g1 * c2 + h1 * f2 + 1.0;

        // Normalize so bottom-right = 1.
        if rw.abs() < 1e-12 {
            return Self::new([ra, rb, rc, rd, re, rf, rg, rh]);
        }

        Self::new([
            ra / rw,
            rb / rw,
            rc / rw,
            rd / rw,
            re / rw,
            rf / rw,
            rg / rw,
            rh / rw,
        ])
    }

    /// Compute the inverse perspective transform.
    ///
    /// Returns `None` if the matrix is singular.
    pub fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f, g, h] = self.m;
        // Full 3×3 inverse using cofactors.
        // Matrix:
        // | a  b  c |
        // | d  e  f |
        // | g  h  1 |

        let det = a * (e - f * h) - b * (d - f * g) + c * (d * h - e * g);

        if det.abs() < 1e-12 {
            return None;
        }

        let inv_det = 1.0 / det;

        // Cofactor matrix transposed / det.
        let ia = (e - f * h) * inv_det;
        let ib = -(b - c * h) * inv_det;
        let ic = (b * f - c * e) * inv_det;

        let id = -(d - f * g) * inv_det;
        let ie = (a - c * g) * inv_det;
        let i_f = -(a * f - c * d) * inv_det;

        let ig = (d * h - e * g) * inv_det;
        let ih = -(a * h - b * g) * inv_det;
        let iw = (a * e - b * d) * inv_det;

        // Normalize so bottom-right = 1.
        if iw.abs() < 1e-12 {
            return None;
        }

        Some(Self::new([
            ia / iw,
            ib / iw,
            ic / iw,
            id / iw,
            ie / iw,
            i_f / iw,
            ig / iw,
            ih / iw,
        ]))
    }

    /// Compute a perspective transform that maps 4 source points to 4 destination points.
    ///
    /// This is the classic "quad-to-quad" mapping used for 3D box tools.
    /// Both `src` and `dst` must have exactly 4 points (corners of a quadrilateral).
    ///
    /// Returns `None` if the mapping is degenerate.
    pub fn from_quad_to_quad(src: &[Point; 4], dst: &[Point; 4]) -> Option<Self> {
        // First compute unit-square-to-src, then unit-square-to-dst,
        // then compose: src⁻¹ * dst.
        let to_src = Self::from_unit_square_to_quad(src)?;
        let to_dst = Self::from_unit_square_to_quad(dst)?;
        let from_src = to_src.inverse()?;
        Some(from_src.then(&to_dst))
    }

    /// Compute a perspective transform mapping the unit square
    /// `[(0,0), (1,0), (1,1), (0,1)]` to the given quad.
    fn from_unit_square_to_quad(q: &[Point; 4]) -> Option<Self> {
        let (x0, y0) = (q[0].x, q[0].y);
        let (x1, y1) = (q[1].x, q[1].y);
        let (x2, y2) = (q[2].x, q[2].y);
        let (x3, y3) = (q[3].x, q[3].y);

        let dx1 = x1 - x2;
        let dx2 = x3 - x2;
        let dx3 = x0 - x1 + x2 - x3;

        let dy1 = y1 - y2;
        let dy2 = y3 - y2;
        let dy3 = y0 - y1 + y2 - y3;

        let denom = dx1 * dy2 - dx2 * dy1;
        if denom.abs() < 1e-12 {
            return None;
        }

        let g = (dx3 * dy2 - dx2 * dy3) / denom;
        let h = (dx1 * dy3 - dx3 * dy1) / denom;

        let a = x1 - x0 + g * x1;
        let b = x3 - x0 + h * x3;
        let c = x0;

        let d = y1 - y0 + g * y1;
        let e = y3 - y0 + h * y3;
        let f = y0;

        Some(Self::new([a, b, c, d, e, f, g, h]))
    }
}

impl Default for PerspectiveTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_preserves_point() {
        let p = Point::new(3.0, 4.0);
        let q = PerspectiveTransform::IDENTITY.transform_point(p).unwrap();
        assert!((q.x - 3.0).abs() < 1e-10);
        assert!((q.y - 4.0).abs() < 1e-10);
    }

    #[test]
    fn translation_via_coefficients() {
        // Translate by (10, 20):
        // | 1 0 10 |
        // | 0 1 20 |
        // | 0 0  1 |
        let t = PerspectiveTransform::new([1.0, 0.0, 10.0, 0.0, 1.0, 20.0, 0.0, 0.0]);
        let p = t.transform_point(Point::new(5.0, 5.0)).unwrap();
        assert!((p.x - 15.0).abs() < 1e-10);
        assert!((p.y - 25.0).abs() < 1e-10);
    }

    #[test]
    fn inverse_roundtrip() {
        let t = PerspectiveTransform::new([2.0, 0.5, 10.0, 0.3, 1.5, 20.0, 0.001, 0.002]);
        let inv = t.inverse().unwrap();
        let p = Point::new(7.0, 13.0);
        let q = t.transform_point(p).unwrap();
        let r = inv.transform_point(q).unwrap();
        assert!((r.x - p.x).abs() < 1e-6);
        assert!((r.y - p.y).abs() < 1e-6);
    }

    #[test]
    fn compose_then() {
        let t1 = PerspectiveTransform::new([1.0, 0.0, 10.0, 0.0, 1.0, 20.0, 0.0, 0.0]);
        let t2 = PerspectiveTransform::new([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0]);
        let combined = t1.then(&t2);
        // First translate (10,20), then scale by 2: (5,5) -> (15,25) -> (30,50)
        let p = combined.transform_point(Point::new(5.0, 5.0)).unwrap();
        assert!((p.x - 30.0).abs() < 1e-10);
        assert!((p.y - 50.0).abs() < 1e-10);
    }

    #[test]
    fn quad_to_quad_identity() {
        let unit = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let t = PerspectiveTransform::from_quad_to_quad(&unit, &unit).unwrap();
        let p = t.transform_point(Point::new(0.5, 0.5)).unwrap();
        assert!((p.x - 0.5).abs() < 1e-8);
        assert!((p.y - 0.5).abs() < 1e-8);
    }

    #[test]
    fn quad_to_quad_simple_scale() {
        let src = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let dst = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let t = PerspectiveTransform::from_quad_to_quad(&src, &dst).unwrap();
        let p = t.transform_point(Point::new(0.5, 0.5)).unwrap();
        assert!((p.x - 1.0).abs() < 1e-8);
        assert!((p.y - 1.0).abs() < 1e-8);
    }

    // -- VanishingPoint tests --

    #[test]
    fn vanishing_point_finite() {
        let vp = VanishingPoint::finite(100.0, 200.0);
        assert!(vp.is_finite);
        assert!((vp.x - 100.0).abs() < 1e-10);
        assert!((vp.y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_infinite() {
        let vp = VanishingPoint::infinite(1.0, 0.0);
        assert!(!vp.is_finite);
    }

    #[test]
    fn vanishing_point_parse_coordinate_pair() {
        let vp = VanishingPoint::parse("300.5,400.5").unwrap();
        assert!(vp.is_finite);
        assert!((vp.x - 300.5).abs() < 1e-10);
        assert!((vp.y - 400.5).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_parse_inf() {
        let vp = VanishingPoint::parse("inf").unwrap();
        assert!(!vp.is_finite);
    }

    #[test]
    fn vanishing_point_parse_inf_case_insensitive() {
        let vp = VanishingPoint::parse("INF").unwrap();
        assert!(!vp.is_finite);
    }

    #[test]
    fn vanishing_point_serialize_finite() {
        let vp = VanishingPoint::finite(42.0, 99.0);
        assert_eq!(vp.to_svg_string(), "42,99");
    }

    #[test]
    fn vanishing_point_serialize_infinite() {
        let vp = VanishingPoint::infinite(0.0, 0.0);
        assert_eq!(vp.to_svg_string(), "inf");
    }

    #[test]
    fn vanishing_point_project_finite_midpoint() {
        let vp = VanishingPoint::finite(200.0, 200.0);
        let p = Point::new(0.0, 0.0);
        let mid = vp.project(p, 0.5);
        assert!((mid.x - 100.0).abs() < 1e-10);
        assert!((mid.y - 100.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_project_finite_at_t0() {
        let vp = VanishingPoint::finite(200.0, 200.0);
        let p = Point::new(50.0, 50.0);
        let result = vp.project(p, 0.0);
        assert!((result.x - 50.0).abs() < 1e-10);
        assert!((result.y - 50.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_project_finite_at_t1() {
        let vp = VanishingPoint::finite(200.0, 200.0);
        let p = Point::new(50.0, 50.0);
        let result = vp.project(p, 1.0);
        assert!((result.x - 200.0).abs() < 1e-10);
        assert!((result.y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_project_infinite_translates() {
        let vp = VanishingPoint::infinite(10.0, 5.0);
        let p = Point::new(100.0, 100.0);
        let result = vp.project(p, 3.0);
        assert!((result.x - 130.0).abs() < 1e-10);
        assert!((result.y - 115.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_default_is_finite_origin() {
        let vp = VanishingPoint::default();
        assert!(vp.is_finite);
        assert!((vp.x).abs() < 1e-10);
        assert!((vp.y).abs() < 1e-10);
    }
}
