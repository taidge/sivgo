use std::fmt;

use crate::{Point, Size};

/// An axis-aligned rectangle defined by its minimum and maximum corners.
///
/// Wraps `kurbo::Rect`. Coordinates are (x0, y0) for the top-left
/// and (x1, y1) for the bottom-right in a y-down coordinate system.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    /// The zero rectangle at origin.
    pub const ZERO: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    };

    /// Create a rectangle from min/max coordinates.
    #[inline]
    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Rect { x0, y0, x1, y1 }
    }

    /// Create a rectangle from an origin point and a size.
    ///
    /// The result will have non-negative width and height.
    #[inline]
    pub fn from_origin_size(origin: impl Into<Point>, size: impl Into<Size>) -> Self {
        let origin = origin.into();
        let size = size.into();
        let kr = kurbo::Rect::from_origin_size(kurbo::Point::from(origin), kurbo::Size::from(size));
        Rect::from(kr)
    }

    /// Create a rectangle from two corner points.
    ///
    /// The result will have non-negative width and height.
    #[inline]
    pub fn from_points(p0: impl Into<Point>, p1: impl Into<Point>) -> Self {
        let p0 = p0.into();
        let p1 = p1.into();
        Rect::new(
            p0.x.min(p1.x),
            p0.y.min(p1.y),
            p0.x.max(p1.x),
            p0.y.max(p1.y),
        )
    }

    /// The width of the rectangle.
    #[inline]
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// The height of the rectangle.
    #[inline]
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    /// The size of the rectangle.
    #[inline]
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    /// The origin (top-left) point.
    #[inline]
    pub fn origin(&self) -> Point {
        Point::new(self.x0, self.y0)
    }

    /// The center point of the rectangle.
    #[inline]
    pub fn center(&self) -> Point {
        Point::new(0.5 * (self.x0 + self.x1), 0.5 * (self.y0 + self.y1))
    }

    /// Returns `true` if the point lies within the rectangle.
    ///
    /// The left and top edges are inclusive, the right and bottom edges are exclusive.
    #[inline]
    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.x0 && p.x < self.x1 && p.y >= self.y0 && p.y < self.y1
    }

    /// The smallest rectangle enclosing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: Rect) -> Rect {
        Rect::new(
            self.x0.min(other.x0),
            self.y0.min(other.y0),
            self.x1.max(other.x1),
            self.y1.max(other.y1),
        )
    }

    /// The intersection of two rectangles.
    ///
    /// Returns a zero-area rect if the rectangles do not overlap.
    #[inline]
    pub fn intersect(&self, other: Rect) -> Rect {
        let x0 = self.x0.max(other.x0);
        let y0 = self.y0.max(other.y0);
        let x1 = self.x1.min(other.x1);
        let y1 = self.y1.min(other.y1);
        Rect::new(x0, y0, x1.max(x0), y1.max(y0))
    }

    /// Expand (or shrink) the rectangle by the given amount on each side.
    #[inline]
    pub fn inflate(&self, width: f64, height: f64) -> Rect {
        Rect::new(
            self.x0 - width,
            self.y0 - height,
            self.x1 + width,
            self.y1 + height,
        )
    }

    /// Whether the rectangle has zero area.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width() <= 0.0 || self.height() <= 0.0
    }

    /// The area of the rectangle.
    #[inline]
    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect({}, {} {}x{})",
            self.x0,
            self.y0,
            self.width(),
            self.height()
        )
    }
}

impl From<kurbo::Rect> for Rect {
    #[inline]
    fn from(r: kurbo::Rect) -> Self {
        Rect::new(r.x0, r.y0, r.x1, r.y1)
    }
}

impl From<Rect> for kurbo::Rect {
    #[inline]
    fn from(r: Rect) -> Self {
        kurbo::Rect::new(r.x0, r.y0, r.x1, r.y1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_basic() {
        let r = Rect::new(10.0, 20.0, 110.0, 120.0);
        assert!((r.width() - 100.0).abs() < 1e-10);
        assert!((r.height() - 100.0).abs() < 1e-10);
        assert_eq!(r.center(), Point::new(60.0, 70.0));
    }

    #[test]
    fn test_rect_contains_point() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains_point(Point::new(5.0, 5.0)));
        assert!(r.contains_point(Point::new(0.0, 0.0)));
        assert!(!r.contains_point(Point::new(10.0, 10.0))); // exclusive
        assert!(!r.contains_point(Point::new(-1.0, 5.0)));
    }

    #[test]
    fn test_rect_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 20.0, 20.0);
        let u = a.union(b);
        assert_eq!(u, Rect::new(0.0, 0.0, 20.0, 20.0));
    }

    #[test]
    fn test_rect_intersect() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 20.0, 20.0);
        let i = a.intersect(b);
        assert_eq!(i, Rect::new(5.0, 5.0, 10.0, 10.0));
    }

    #[test]
    fn test_rect_intersect_no_overlap() {
        let a = Rect::new(0.0, 0.0, 5.0, 5.0);
        let b = Rect::new(10.0, 10.0, 20.0, 20.0);
        let i = a.intersect(b);
        assert!(i.is_empty());
    }

    #[test]
    fn test_rect_inflate() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        let inflated = r.inflate(5.0, 5.0);
        assert_eq!(inflated, Rect::new(5.0, 5.0, 25.0, 25.0));
    }

    #[test]
    fn test_rect_from_origin_size() {
        let r = Rect::from_origin_size(Point::new(10.0, 20.0), Size::new(30.0, 40.0));
        assert_eq!(r.x0, 10.0);
        assert_eq!(r.y0, 20.0);
        assert_eq!(r.x1, 40.0);
        assert_eq!(r.y1, 60.0);
    }

    #[test]
    fn test_rect_from_points() {
        // Should normalize regardless of point order
        let r = Rect::from_points(Point::new(20.0, 30.0), Point::new(10.0, 5.0));
        assert_eq!(r, Rect::new(10.0, 5.0, 20.0, 30.0));
    }

    #[test]
    fn test_kurbo_roundtrip() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        let kr: kurbo::Rect = r.into();
        let back: Rect = kr.into();
        assert_eq!(r, back);
    }
}
