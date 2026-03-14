//! Copy Rotate Live Path Effect.
//!
//! Creates rotational copies of the input path around a specified center
//! point. Each copy is rotated by `360 / copies` degrees from the previous.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Point, Transform2D};

/// Copy Rotate LPE.
///
/// Produces `copies` rotational duplicates of the input path, evenly
/// distributed around 360 degrees about a center point. Optionally includes
/// the original (un-rotated) path.
#[derive(Debug, Clone)]
pub struct CopyRotateLpe {
    /// Center of rotation.
    center: Point,
    /// Number of rotational copies (including or excluding the original,
    /// depending on `include_original`).
    copies: usize,
    /// Whether the original path is included in the output.
    include_original: bool,
    /// Exposed as a parameter: the starting angle offset in radians.
    start_angle: LpeParam,
}

impl CopyRotateLpe {
    /// Create a new Copy Rotate effect.
    ///
    /// * `center` - The rotation center point.
    /// * `copies` - Number of copies to generate (the total number of
    ///   instances, including the original if `include_original` is true).
    /// * `include_original` - Whether to include the un-rotated input.
    pub fn new(center: Point, copies: usize, include_original: bool) -> Self {
        Self {
            center,
            copies: copies.max(1),
            include_original,
            start_angle: LpeParam::new(
                "start_angle",
                0.0,
                -std::f64::consts::TAU,
                std::f64::consts::TAU,
            ),
        }
    }

    /// Set the rotation center.
    pub fn set_center(&mut self, center: Point) {
        self.center = center;
    }

    /// Get the rotation center.
    pub fn center(&self) -> Point {
        self.center
    }

    /// Set the number of copies.
    pub fn set_copies(&mut self, copies: usize) {
        self.copies = copies.max(1);
    }

    /// Set whether to include the original.
    pub fn set_include_original(&mut self, include: bool) {
        self.include_original = include;
    }

    /// Rotate an entire path about the center by the given angle.
    fn rotate_path(&self, path: &BezPath, angle: f64) -> BezPath {
        let transform = Transform2D::rotate_about(angle, self.center);
        path.transformed(&transform)
    }
}

impl LivePathEffect for CopyRotateLpe {
    fn name(&self) -> &str {
        "copy_rotate"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if input.is_empty() || self.copies == 0 {
            return input.clone();
        }

        let mut result = BezPath::new();
        let angle_step = std::f64::consts::TAU / self.copies as f64;
        let start = self.start_angle.value;

        for i in 0..self.copies {
            if i == 0 && !self.include_original && start.abs() < 1e-12 {
                // Skip the original if not included and no start angle offset.
                continue;
            }

            let angle = start + i as f64 * angle_step;

            if angle.abs() < 1e-12 {
                // No rotation needed -- append original.
                result.extend(input);
            } else {
                let rotated = self.rotate_path(input, angle);
                result.extend(&rotated);
            }
        }

        // If include_original is true and we haven't already added the
        // un-rotated copy (which happens when start_angle == 0 and i == 0),
        // make sure it's there.
        if self.include_original && start.abs() >= 1e-12 {
            // The original at angle 0 wasn't added because all copies are
            // offset. Prepend it.
            let mut with_original = input.clone();
            with_original.extend(&result);
            return with_original;
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.start_angle)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        if name == "start_angle" {
            self.start_angle.value = value.clamp(self.start_angle.min, self.start_angle.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn triangle() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(110.0, 0.0));
        p.line_to(Point::new(105.0, 10.0));
        p.close();
        p
    }

    #[test]
    fn test_copy_rotate_four_copies() {
        let lpe = CopyRotateLpe::new(Point::new(50.0, 50.0), 4, true);
        let input = triangle();
        let output = lpe.apply(&input);

        // 4 copies of a 4-element triangle = 16 elements.
        assert_eq!(
            output.element_count(),
            input.element_count() * 4,
            "4 copies should give 4x elements: got {}",
            output.element_count()
        );
    }

    #[test]
    fn test_copy_rotate_exclude_original() {
        let lpe = CopyRotateLpe::new(Point::new(50.0, 50.0), 4, false);
        let input = triangle();
        let output = lpe.apply(&input);

        // Excludes the i=0 (original), so 3 copies.
        assert_eq!(
            output.element_count(),
            input.element_count() * 3,
            "excluding original: got {}",
            output.element_count()
        );
    }

    #[test]
    fn test_copy_rotate_single_copy_include() {
        let lpe = CopyRotateLpe::new(Point::new(0.0, 0.0), 1, true);
        let input = triangle();
        let output = lpe.apply(&input);

        // 1 copy with include_original = just the original.
        assert_eq!(output.element_count(), input.element_count());
    }

    #[test]
    fn test_copy_rotate_symmetry() {
        // 2 copies at center (50, 0), triangle at x=100.
        // Second copy should be at x=0 (180 degrees).
        let lpe = CopyRotateLpe::new(Point::new(50.0, 0.0), 2, true);
        let mut input = BezPath::new();
        input.move_to(Point::new(100.0, 0.0));
        input.line_to(Point::new(110.0, 0.0));

        let output = lpe.apply(&input);
        let bb = output.bounding_box();

        // The rotated copy at 180 deg about (50, 0) maps (100, 0) -> (0, 0)
        // and (110, 0) -> (-10, 0).
        assert!(bb.x0 < 1.0, "x0={} should be near -10 or 0", bb.x0);
    }

    #[test]
    fn test_copy_rotate_empty_path() {
        let lpe = CopyRotateLpe::new(Point::new(0.0, 0.0), 4, true);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_copy_rotate_params() {
        let mut lpe = CopyRotateLpe::new(Point::new(0.0, 0.0), 4, true);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "start_angle");

        lpe.set_param("start_angle", FRAC_PI_2);
        assert!((lpe.start_angle.value - FRAC_PI_2).abs() < 1e-10);
    }

    #[test]
    fn test_copy_rotate_name() {
        let lpe = CopyRotateLpe::new(Point::new(0.0, 0.0), 3, true);
        assert_eq!(lpe.name(), "copy_rotate");
    }

    #[test]
    fn test_copy_rotate_six_fold() {
        let lpe = CopyRotateLpe::new(Point::new(0.0, 0.0), 6, true);
        let input = triangle();
        let output = lpe.apply(&input);
        assert_eq!(output.element_count(), input.element_count() * 6);
    }
}
