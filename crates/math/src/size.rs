use std::fmt;
use std::ops::{Div, Mul};

/// A 2D size with width and height.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    /// A zero size.
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// Create a new size.
    #[inline]
    pub const fn new(width: f64, height: f64) -> Self {
        Size { width, height }
    }

    /// The area of this size (width * height).
    #[inline]
    pub fn area(self) -> f64 {
        self.width * self.height
    }

    /// Whether the area is zero.
    #[inline]
    pub fn is_zero_area(self) -> bool {
        self.area() == 0.0
    }

    /// Convert to a vector.
    #[inline]
    pub fn to_vector(self) -> crate::Vector {
        crate::Vector::new(self.width, self.height)
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Mul<f64> for Size {
    type Output = Size;

    #[inline]
    fn mul(self, s: f64) -> Size {
        Size::new(self.width * s, self.height * s)
    }
}

impl Div<f64> for Size {
    type Output = Size;

    #[inline]
    fn div(self, s: f64) -> Size {
        Size::new(self.width / s, self.height / s)
    }
}

impl From<kurbo::Size> for Size {
    #[inline]
    fn from(s: kurbo::Size) -> Self {
        Size::new(s.width, s.height)
    }
}

impl From<Size> for kurbo::Size {
    #[inline]
    fn from(s: Size) -> Self {
        kurbo::Size::new(s.width, s.height)
    }
}

impl From<(f64, f64)> for Size {
    #[inline]
    fn from((w, h): (f64, f64)) -> Self {
        Size::new(w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_basic() {
        let s = Size::new(10.0, 20.0);
        assert_eq!(s.width, 10.0);
        assert_eq!(s.height, 20.0);
        assert!((s.area() - 200.0).abs() < 1e-10);
    }

    #[test]
    fn test_size_zero() {
        assert!(Size::ZERO.is_zero_area());
        assert!(!Size::new(1.0, 1.0).is_zero_area());
    }
}
