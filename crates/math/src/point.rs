use std::fmt;
use std::ops::{Add, Sub};

use crate::Vector;

/// A 2D point with f64 coordinates.
///
/// Wraps `kurbo::Point` and provides ergonomic conversions and operations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// The origin point (0, 0).
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    /// Create a new point.
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Euclidean distance between two points.
    #[inline]
    pub fn distance(self, other: Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Squared Euclidean distance between two points (avoids sqrt).
    #[inline]
    pub fn distance_squared(self, other: Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    /// Convert this point to a vector (from origin to this point).
    #[inline]
    pub const fn to_vector(self) -> Vector {
        Vector::new(self.x, self.y)
    }

    /// Linearly interpolate between two points.
    #[inline]
    pub fn lerp(self, other: Point, t: f64) -> Point {
        Point::new(
            self.x + t * (other.x - self.x),
            self.y + t * (other.y - self.y),
        )
    }

    /// Midpoint between two points.
    #[inline]
    pub fn midpoint(self, other: Point) -> Point {
        Point::new(0.5 * (self.x + other.x), 0.5 * (self.y + other.y))
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// Point + Vector = Point
impl Add<Vector> for Point {
    type Output = Point;

    #[inline]
    fn add(self, v: Vector) -> Point {
        Point::new(self.x + v.x, self.y + v.y)
    }
}

// Point - Vector = Point
impl Sub<Vector> for Point {
    type Output = Point;

    #[inline]
    fn sub(self, v: Vector) -> Point {
        Point::new(self.x - v.x, self.y - v.y)
    }
}

// Point - Point = Vector
impl Sub<Point> for Point {
    type Output = Vector;

    #[inline]
    fn sub(self, other: Point) -> Vector {
        Vector::new(self.x - other.x, self.y - other.y)
    }
}

// Conversions with kurbo types
impl From<kurbo::Point> for Point {
    #[inline]
    fn from(p: kurbo::Point) -> Self {
        Point::new(p.x, p.y)
    }
}

impl From<Point> for kurbo::Point {
    #[inline]
    fn from(p: Point) -> Self {
        kurbo::Point::new(p.x, p.y)
    }
}

impl From<(f64, f64)> for Point {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Point::new(x, y)
    }
}

impl From<Point> for (f64, f64) {
    #[inline]
    fn from(p: Point) -> Self {
        (p.x, p.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 4.0);
    }

    #[test]
    fn test_point_distance() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert!((a.distance(b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_sub_gives_vector() {
        let a = Point::new(5.0, 7.0);
        let b = Point::new(2.0, 3.0);
        let v = a - b;
        assert_eq!(v.x, 3.0);
        assert_eq!(v.y, 4.0);
    }

    #[test]
    fn test_point_add_vector() {
        let p = Point::new(1.0, 2.0);
        let v = Vector::new(3.0, 4.0);
        let result = p + v;
        assert_eq!(result, Point::new(4.0, 6.0));
    }

    #[test]
    fn test_point_lerp() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 20.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-10);
        assert!((mid.y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_kurbo_roundtrip() {
        let p = Point::new(1.5, 2.5);
        let kp: kurbo::Point = p.into();
        let back: Point = kp.into();
        assert_eq!(p, back);
    }
}
