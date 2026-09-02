use std::fmt;
use std::ops::Mul;

use crate::error::MathError;
use crate::{Point, Rect};

/// A 2D affine transformation matrix.
///
/// Wraps `kurbo::Affine`. The matrix is stored as 6 coefficients `[a, b, c, d, e, f]`
/// representing the augmented matrix:
///
/// ```text
/// | a c e |
/// | b d f |
/// | 0 0 1 |
/// ```
///
/// Transforming a point (x, y) gives:
///   x' = a*x + c*y + e
///   y' = b*x + d*y + f
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    inner: kurbo::Affine,
}

impl Transform2D {
    /// The identity transform.
    pub const IDENTITY: Transform2D = Transform2D {
        inner: kurbo::Affine::IDENTITY,
    };

    /// Create a transform from raw coefficients [a, b, c, d, e, f].
    #[inline]
    pub fn new(coeffs: [f64; 6]) -> Self {
        Transform2D {
            inner: kurbo::Affine::new(coeffs),
        }
    }

    /// The identity transform (no-op).
    #[inline]
    pub fn identity() -> Self {
        Self::IDENTITY
    }

    /// A translation transform.
    #[inline]
    pub fn translate(dx: f64, dy: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::translate(kurbo::Vec2::new(dx, dy)),
        }
    }

    /// A uniform scale transform.
    #[inline]
    pub fn scale(s: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::scale(s),
        }
    }

    /// A non-uniform scale transform.
    #[inline]
    pub fn scale_xy(sx: f64, sy: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::scale_non_uniform(sx, sy),
        }
    }

    /// A rotation transform (angle in radians).
    ///
    /// Positive angle rotates counter-clockwise in standard math coords (clockwise in y-down).
    #[inline]
    pub fn rotate(angle: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::rotate(angle),
        }
    }

    /// A rotation about a specific point.
    #[inline]
    pub fn rotate_about(angle: f64, center: Point) -> Self {
        Transform2D {
            inner: kurbo::Affine::rotate_about(angle, kurbo::Point::from(center)),
        }
    }

    /// A horizontal skew transform (angle in radians).
    #[inline]
    pub fn skew_x(angle: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::skew(angle.tan(), 0.0),
        }
    }

    /// A vertical skew transform (angle in radians).
    #[inline]
    pub fn skew_y(angle: f64) -> Self {
        Transform2D {
            inner: kurbo::Affine::skew(0.0, angle.tan()),
        }
    }

    /// Transform a point.
    #[inline]
    pub fn transform_point(&self, p: Point) -> Point {
        let kp = self.inner * kurbo::Point::from(p);
        Point::from(kp)
    }

    /// Transform a rectangle, returning the bounding box of the transformed rectangle.
    #[inline]
    pub fn transform_rect(&self, r: Rect) -> Rect {
        let kr = self.inner.transform_rect_bbox(kurbo::Rect::from(r));
        Rect::from(kr)
    }

    /// Compute the inverse transform.
    ///
    /// Returns a transform with NaN values if the matrix is singular (determinant is zero).
    #[inline]
    pub fn inverse(&self) -> Self {
        Transform2D {
            inner: self.inner.inverse(),
        }
    }

    /// Compose two transforms: apply `self` first, then `other`.
    ///
    /// This is equivalent to `other * self` in matrix notation.
    #[inline]
    pub fn then(&self, other: &Transform2D) -> Self {
        Transform2D {
            inner: other.inner * self.inner,
        }
    }

    /// Compute the determinant of the transform.
    #[inline]
    pub fn determinant(&self) -> f64 {
        self.inner.determinant()
    }

    /// Whether this is the identity transform.
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.inner == kurbo::Affine::IDENTITY
    }

    /// Get the raw coefficients [a, b, c, d, e, f].
    #[inline]
    pub fn as_coeffs(&self) -> [f64; 6] {
        self.inner.as_coeffs()
    }

    /// Access the inner kurbo Affine.
    #[inline]
    pub fn to_kurbo(&self) -> kurbo::Affine {
        self.inner
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl fmt::Display for Transform2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.as_coeffs();
        write!(
            f,
            "matrix({}, {}, {}, {}, {}, {})",
            c[0], c[1], c[2], c[3], c[4], c[5]
        )
    }
}

// Transform2D * Transform2D = composition (apply left first, then right, matching matrix multiply order)
impl Mul for Transform2D {
    type Output = Transform2D;

    #[inline]
    fn mul(self, rhs: Transform2D) -> Transform2D {
        Transform2D {
            inner: self.inner * rhs.inner,
        }
    }
}

// Transform2D * Point = transformed point
impl Mul<Point> for Transform2D {
    type Output = Point;

    #[inline]
    fn mul(self, p: Point) -> Point {
        self.transform_point(p)
    }
}

impl From<kurbo::Affine> for Transform2D {
    #[inline]
    fn from(a: kurbo::Affine) -> Self {
        Transform2D { inner: a }
    }
}

impl From<Transform2D> for kurbo::Affine {
    #[inline]
    fn from(t: Transform2D) -> Self {
        t.inner
    }
}

// --- SVG transform attribute parsing ---

/// Parse an SVG `transform` attribute string into a `Transform2D`.
///
/// Supports the following SVG transform functions:
/// - `matrix(a, b, c, d, e, f)`
/// - `translate(tx [, ty])`
/// - `scale(sx [, sy])`
/// - `rotate(angle [, cx, cy])` (angle in degrees)
/// - `skewX(angle)` (angle in degrees)
/// - `skewY(angle)` (angle in degrees)
///
/// Multiple transforms are composed left-to-right (first listed is applied first).
pub fn parse_transform(s: &str) -> Result<Transform2D, MathError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Transform2D::identity());
    }

    let mut result = Transform2D::identity();
    let mut remaining = s;

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // Skip optional commas between transform functions
        if remaining.starts_with(',') {
            remaining = remaining[1..].trim_start();
            continue;
        }

        // Find the function name
        let paren_pos = remaining.find('(').ok_or_else(|| {
            MathError::ParseError(format!("expected '(' in transform: {}", remaining))
        })?;

        let func_name = remaining[..paren_pos].trim();
        let after_paren = &remaining[paren_pos + 1..];

        let close_paren = after_paren.find(')').ok_or_else(|| {
            MathError::ParseError(format!("expected ')' in transform: {}", remaining))
        })?;

        let args_str = &after_paren[..close_paren];
        remaining = after_paren[close_paren + 1..].trim_start();

        let args = parse_number_list(args_str)?;

        let t = match func_name {
            "matrix" => {
                if args.len() != 6 {
                    return Err(MathError::ParseError(format!(
                        "matrix() requires 6 arguments, got {}",
                        args.len()
                    )));
                }
                Transform2D::new([args[0], args[1], args[2], args[3], args[4], args[5]])
            }
            "translate" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(MathError::ParseError(format!(
                        "translate() requires 1 or 2 arguments, got {}",
                        args.len()
                    )));
                }
                let tx = args[0];
                let ty = if args.len() > 1 { args[1] } else { 0.0 };
                Transform2D::translate(tx, ty)
            }
            "scale" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(MathError::ParseError(format!(
                        "scale() requires 1 or 2 arguments, got {}",
                        args.len()
                    )));
                }
                let sx = args[0];
                let sy = if args.len() > 1 { args[1] } else { sx };
                Transform2D::scale_xy(sx, sy)
            }
            "rotate" => {
                if args.is_empty() || args.len() == 2 || args.len() > 3 {
                    return Err(MathError::ParseError(format!(
                        "rotate() requires 1 or 3 arguments, got {}",
                        args.len()
                    )));
                }
                let angle_deg = args[0];
                let angle_rad = angle_deg.to_radians();
                if args.len() == 3 {
                    let cx = args[1];
                    let cy = args[2];
                    Transform2D::rotate_about(angle_rad, Point::new(cx, cy))
                } else {
                    Transform2D::rotate(angle_rad)
                }
            }
            "skewX" => {
                if args.len() != 1 {
                    return Err(MathError::ParseError(format!(
                        "skewX() requires 1 argument, got {}",
                        args.len()
                    )));
                }
                let angle_rad = args[0].to_radians();
                // skewX shears x by tan(angle)
                Transform2D::new([1.0, 0.0, angle_rad.tan(), 1.0, 0.0, 0.0])
            }
            "skewY" => {
                if args.len() != 1 {
                    return Err(MathError::ParseError(format!(
                        "skewY() requires 1 argument, got {}",
                        args.len()
                    )));
                }
                let angle_rad = args[0].to_radians();
                // skewY shears y by tan(angle)
                Transform2D::new([1.0, angle_rad.tan(), 0.0, 1.0, 0.0, 0.0])
            }
            _ => {
                return Err(MathError::ParseError(format!(
                    "unknown transform function: {}",
                    func_name
                )));
            }
        };

        // `transform="f1 f2"` is the matrix product `f1 * f2`: coordinates go
        // through the rightmost function first. Building left to right, each
        // new function is applied *before* everything accumulated so far.
        result = t.then(&result);
    }

    Ok(result)
}

/// Parse a comma/whitespace separated list of numbers.
fn parse_number_list(s: &str) -> Result<Vec<f64>, MathError> {
    let mut result = Vec::new();
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(result);
    }

    // Split on commas and whitespace
    for token in trimmed.split(|c: char| c == ',' || c.is_whitespace()) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let val: f64 = token
            .parse()
            .map_err(|_| MathError::ParseError(format!("invalid number: {}", token)))?;
        result.push(val);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-9;

    fn assert_point_near(a: Point, b: Point) {
        assert!(
            (a.x - b.x).abs() < EPSILON && (a.y - b.y).abs() < EPSILON,
            "Point {:?} != {:?}",
            a,
            b
        );
    }

    fn assert_coeffs_near(t: &Transform2D, expected: [f64; 6]) {
        let c = t.as_coeffs();
        for i in 0..6 {
            assert!(
                (c[i] - expected[i]).abs() < EPSILON,
                "coeff[{}]: {} != {}",
                i,
                c[i],
                expected[i]
            );
        }
    }

    #[test]
    fn test_identity() {
        let t = Transform2D::identity();
        assert!(t.is_identity());
        let p = Point::new(5.0, 7.0);
        assert_eq!(t.transform_point(p), p);
    }

    #[test]
    fn test_translate() {
        let t = Transform2D::translate(10.0, 20.0);
        let p = Point::new(5.0, 5.0);
        assert_point_near(t.transform_point(p), Point::new(15.0, 25.0));
    }

    #[test]
    fn test_scale() {
        let t = Transform2D::scale(2.0);
        let p = Point::new(3.0, 4.0);
        assert_point_near(t.transform_point(p), Point::new(6.0, 8.0));
    }

    #[test]
    fn test_scale_xy() {
        let t = Transform2D::scale_xy(2.0, 3.0);
        let p = Point::new(4.0, 5.0);
        assert_point_near(t.transform_point(p), Point::new(8.0, 15.0));
    }

    #[test]
    fn test_rotate() {
        let t = Transform2D::rotate(PI / 2.0);
        let p = Point::new(1.0, 0.0);
        assert_point_near(t.transform_point(p), Point::new(0.0, 1.0));
    }

    #[test]
    fn test_rotate_about() {
        let center = Point::new(5.0, 5.0);
        let t = Transform2D::rotate_about(PI / 2.0, center);
        let p = Point::new(10.0, 5.0);
        assert_point_near(t.transform_point(p), Point::new(5.0, 10.0));
    }

    #[test]
    fn test_compose_then() {
        // Translate then scale
        let t1 = Transform2D::translate(10.0, 0.0);
        let t2 = Transform2D::scale(2.0);
        let composed = t1.then(&t2);
        let p = Point::new(5.0, 0.0);
        // First translate: (15, 0), then scale: (30, 0)
        assert_point_near(composed.transform_point(p), Point::new(30.0, 0.0));
    }

    #[test]
    fn test_inverse() {
        let t = Transform2D::new([0.1, 1.2, 2.3, 3.4, 4.5, 5.6]);
        let inv = t.inverse();
        let p = Point::new(7.0, 11.0);
        let transformed = t.transform_point(p);
        let back = inv.transform_point(transformed);
        assert_point_near(back, p);
    }

    #[test]
    fn test_determinant() {
        let t = Transform2D::scale_xy(2.0, 3.0);
        assert!((t.determinant() - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_mul_operator() {
        let t = Transform2D::translate(5.0, 10.0);
        let p = Point::new(1.0, 2.0);
        assert_point_near(t * p, Point::new(6.0, 12.0));
    }

    #[test]
    fn test_transform_rect() {
        let t = Transform2D::scale(2.0);
        let r = Rect::new(1.0, 1.0, 3.0, 4.0);
        let tr = t.transform_rect(r);
        assert_eq!(tr, Rect::new(2.0, 2.0, 6.0, 8.0));
    }

    // --- SVG parsing tests ---

    #[test]
    fn test_parse_matrix() {
        let t = parse_transform("matrix(1, 0, 0, 1, 10, 20)").unwrap();
        assert_coeffs_near(&t, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    }

    #[test]
    fn test_parse_translate() {
        let t = parse_transform("translate(10, 20)").unwrap();
        let p = Point::new(0.0, 0.0);
        assert_point_near(t.transform_point(p), Point::new(10.0, 20.0));
    }

    #[test]
    fn test_parse_translate_single() {
        let t = parse_transform("translate(10)").unwrap();
        let p = Point::new(5.0, 5.0);
        assert_point_near(t.transform_point(p), Point::new(15.0, 5.0));
    }

    #[test]
    fn test_parse_scale() {
        let t = parse_transform("scale(2)").unwrap();
        let p = Point::new(5.0, 3.0);
        assert_point_near(t.transform_point(p), Point::new(10.0, 6.0));
    }

    #[test]
    fn test_parse_scale_xy() {
        let t = parse_transform("scale(2, 3)").unwrap();
        let p = Point::new(5.0, 4.0);
        assert_point_near(t.transform_point(p), Point::new(10.0, 12.0));
    }

    #[test]
    fn test_parse_rotate() {
        let t = parse_transform("rotate(90)").unwrap();
        let p = Point::new(1.0, 0.0);
        assert_point_near(t.transform_point(p), Point::new(0.0, 1.0));
    }

    #[test]
    fn test_parse_rotate_about() {
        let t = parse_transform("rotate(90, 5, 5)").unwrap();
        let p = Point::new(10.0, 5.0);
        assert_point_near(t.transform_point(p), Point::new(5.0, 10.0));
    }

    #[test]
    fn test_parse_skewx() {
        let t = parse_transform("skewX(45)").unwrap();
        let p = Point::new(0.0, 10.0);
        // skewX(45) means x' = x + tan(45)*y = 0 + 1*10 = 10, y' = 10
        assert_point_near(t.transform_point(p), Point::new(10.0, 10.0));
    }

    #[test]
    fn test_parse_skewy() {
        let t = parse_transform("skewY(45)").unwrap();
        let p = Point::new(10.0, 0.0);
        // skewY(45) means x' = 10, y' = tan(45)*10 + 0 = 10
        assert_point_near(t.transform_point(p), Point::new(10.0, 10.0));
    }

    #[test]
    fn test_parse_multiple_transforms() {
        // Per SVG, `transform="translate(10,20) scale(2)"` is the matrix
        // product T * S, so a point is scaled first and translated second.
        let t = parse_transform("translate(10, 20) scale(2)").unwrap();
        let p = Point::new(5.0, 5.0);
        // scale first: (10, 10), then translate: (20, 30)
        assert_point_near(t.transform_point(p), Point::new(20.0, 30.0));
    }

    #[test]
    fn test_parse_transform_list_order_matters() {
        // The same two functions in the other order place the point elsewhere.
        let p = Point::new(5.0, 5.0);
        let a = parse_transform("translate(10, 20) scale(2)").unwrap();
        let b = parse_transform("scale(2) translate(10, 20)").unwrap();
        assert_point_near(a.transform_point(p), Point::new(20.0, 30.0));
        assert_point_near(b.transform_point(p), Point::new(30.0, 50.0));
    }

    #[test]
    fn test_parse_transform_matches_manual_composition() {
        let parsed = parse_transform("translate(4, 6) rotate(30) scale(2, 3)").unwrap();
        let manual = Transform2D::scale_xy(2.0, 3.0)
            .then(&Transform2D::rotate(30.0_f64.to_radians()))
            .then(&Transform2D::translate(4.0, 6.0));
        for p in [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(-3.5, 7.25),
        ] {
            assert_point_near(parsed.transform_point(p), manual.transform_point(p));
        }
    }

    #[test]
    fn test_parse_empty() {
        let t = parse_transform("").unwrap();
        assert!(t.is_identity());
    }

    #[test]
    fn test_parse_error_unknown() {
        let result = parse_transform("flip(1)");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_bad_args() {
        let result = parse_transform("matrix(1, 2, 3)");
        assert!(result.is_err());
    }
}
