use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A 2D vector with f64 components.
///
/// Wraps `kurbo::Vec2` and represents a displacement or direction, not a position.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector {
    pub x: f64,
    pub y: f64,
}

impl Vector {
    /// The zero vector.
    pub const ZERO: Vector = Vector { x: 0.0, y: 0.0 };

    /// Create a new vector.
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Vector { x, y }
    }

    /// Length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Squared length (avoids sqrt).
    #[inline]
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Return a unit vector in the same direction.
    ///
    /// Returns a vector of NaN components if the length is zero.
    #[inline]
    pub fn normalize(self) -> Vector {
        let len = self.length();
        Vector::new(self.x / len, self.y / len)
    }

    /// Dot product.
    #[inline]
    pub fn dot(self, other: Vector) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// 2D cross product (the z-component of the 3D cross product).
    ///
    /// Positive when `other` is counter-clockwise from `self` in standard math coords.
    #[inline]
    pub fn cross(self, other: Vector) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// Angle in radians from the positive x-axis.
    #[inline]
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Create a unit vector from an angle (in radians).
    #[inline]
    pub fn from_angle(angle: f64) -> Vector {
        let (sin, cos) = angle.sin_cos();
        Vector::new(cos, sin)
    }

    /// Linearly interpolate between two vectors.
    #[inline]
    pub fn lerp(self, other: Vector, t: f64) -> Vector {
        self + (other - self) * t
    }

    /// Convert to a Point (treating the vector as position from origin).
    #[inline]
    pub const fn to_point(self) -> crate::Point {
        crate::Point::new(self.x, self.y)
    }

    /// Rotate 90 degrees counter-clockwise.
    #[inline]
    pub fn perp(self) -> Vector {
        Vector::new(-self.y, self.x)
    }
}

impl fmt::Display for Vector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}, {}>", self.x, self.y)
    }
}

// Vector + Vector = Vector
impl Add for Vector {
    type Output = Vector;

    #[inline]
    fn add(self, other: Vector) -> Vector {
        Vector::new(self.x + other.x, self.y + other.y)
    }
}

// Vector - Vector = Vector
impl Sub for Vector {
    type Output = Vector;

    #[inline]
    fn sub(self, other: Vector) -> Vector {
        Vector::new(self.x - other.x, self.y - other.y)
    }
}

// -Vector
impl Neg for Vector {
    type Output = Vector;

    #[inline]
    fn neg(self) -> Vector {
        Vector::new(-self.x, -self.y)
    }
}

// Vector * f64
impl Mul<f64> for Vector {
    type Output = Vector;

    #[inline]
    fn mul(self, s: f64) -> Vector {
        Vector::new(self.x * s, self.y * s)
    }
}

// f64 * Vector
impl Mul<Vector> for f64 {
    type Output = Vector;

    #[inline]
    fn mul(self, v: Vector) -> Vector {
        Vector::new(self * v.x, self * v.y)
    }
}

// Vector / f64
impl Div<f64> for Vector {
    type Output = Vector;

    #[inline]
    fn div(self, s: f64) -> Vector {
        Vector::new(self.x / s, self.y / s)
    }
}

// Conversions with kurbo types
impl From<kurbo::Vec2> for Vector {
    #[inline]
    fn from(v: kurbo::Vec2) -> Self {
        Vector::new(v.x, v.y)
    }
}

impl From<Vector> for kurbo::Vec2 {
    #[inline]
    fn from(v: Vector) -> Self {
        kurbo::Vec2::new(v.x, v.y)
    }
}

impl From<(f64, f64)> for Vector {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Vector::new(x, y)
    }
}

impl From<Vector> for (f64, f64) {
    #[inline]
    fn from(v: Vector) -> Self {
        (v.x, v.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_length() {
        let v = Vector::new(3.0, 4.0);
        assert!((v.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_vector_normalize() {
        let v = Vector::new(3.0, 4.0);
        let n = v.normalize();
        assert!((n.length() - 1.0).abs() < 1e-10);
        assert!((n.x - 0.6).abs() < 1e-10);
        assert!((n.y - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_vector_dot() {
        let a = Vector::new(1.0, 0.0);
        let b = Vector::new(0.0, 1.0);
        assert!((a.dot(b)).abs() < 1e-10);

        let c = Vector::new(2.0, 3.0);
        let d = Vector::new(4.0, 5.0);
        assert!((c.dot(d) - 23.0).abs() < 1e-10);
    }

    #[test]
    fn test_vector_cross() {
        let a = Vector::new(1.0, 0.0);
        let b = Vector::new(0.0, 1.0);
        assert!((a.cross(b) - 1.0).abs() < 1e-10);
        assert!((b.cross(a) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vector_neg() {
        let v = Vector::new(1.0, -2.0);
        let neg = -v;
        assert_eq!(neg.x, -1.0);
        assert_eq!(neg.y, 2.0);
    }

    #[test]
    fn test_vector_scale() {
        let v = Vector::new(1.0, 2.0);
        let scaled = v * 3.0;
        assert_eq!(scaled, Vector::new(3.0, 6.0));

        let scaled2 = 3.0 * v;
        assert_eq!(scaled2, Vector::new(3.0, 6.0));
    }

    #[test]
    fn test_kurbo_roundtrip() {
        let v = Vector::new(1.5, 2.5);
        let kv: kurbo::Vec2 = v.into();
        let back: Vector = kv.into();
        assert_eq!(v, back);
    }
}
