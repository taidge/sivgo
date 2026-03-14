//! Pattern Along Path Live Path Effect.
//!
//! Repeats a pattern path along a skeleton path. The pattern is mapped so that
//! its X-axis follows the skeleton and its Y-axis becomes the normal offset.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Vector};

/// Pattern Along Path LPE.
///
/// Takes a user-supplied *pattern* path and repeats it along the input
/// (skeleton) path. Each copy of the pattern is deformed so that its
/// X-coordinates map to arc-length along the skeleton, and Y-coordinates
/// become perpendicular offsets.
#[derive(Debug, Clone)]
pub struct PatternAlongPathLpe {
    /// The pattern path to repeat along the skeleton.
    pattern: BezPath,
    /// Number of copies (0 = auto-fill based on pattern width and skeleton length).
    copies: usize,
    /// Spacing between consecutive copies (in skeleton path units).
    spacing: LpeParam,
    /// Offset perpendicular to the skeleton path.
    normal_offset: LpeParam,
    /// Offset along the tangent direction.
    tangent_offset: LpeParam,
    /// Whether to stretch the pattern to exactly fit the skeleton length.
    stretch_to_fit: bool,
}

impl PatternAlongPathLpe {
    /// Create a new Pattern Along Path effect.
    ///
    /// * `pattern` - The shape to repeat along the skeleton.
    /// * `copies` - Number of copies; 0 means auto (fill the skeleton).
    /// * `spacing` - Gap between successive copies.
    /// * `normal_offset` - Perpendicular offset from the skeleton.
    /// * `tangent_offset` - Tangential offset (shifts the start along the skeleton).
    /// * `stretch_to_fit` - If true, scale the pattern so copies tile exactly.
    pub fn new(
        pattern: BezPath,
        copies: usize,
        spacing: f64,
        normal_offset: f64,
        tangent_offset: f64,
        stretch_to_fit: bool,
    ) -> Self {
        Self {
            pattern,
            copies,
            spacing: LpeParam::new("spacing", spacing, 0.0, 100_000.0),
            normal_offset: LpeParam::new("normal_offset", normal_offset, -100_000.0, 100_000.0),
            tangent_offset: LpeParam::new("tangent_offset", tangent_offset, -100_000.0, 100_000.0),
            stretch_to_fit,
        }
    }

    /// Set the pattern path.
    pub fn set_pattern(&mut self, pattern: BezPath) {
        self.pattern = pattern;
    }

    /// Get a reference to the pattern path.
    pub fn pattern(&self) -> &BezPath {
        &self.pattern
    }

    /// Set the number of copies (0 = auto).
    pub fn set_copies(&mut self, copies: usize) {
        self.copies = copies;
    }

    /// Set whether to stretch to fit.
    pub fn set_stretch_to_fit(&mut self, v: bool) {
        self.stretch_to_fit = v;
    }

    /// Map a single pattern point onto the skeleton at a given offset.
    fn map_point(
        &self,
        skeleton: &BezPath,
        p: Point,
        pat_x_min: f64,
        pat_x_range: f64,
        pat_y_center: f64,
        skeleton_len: f64,
        copy_offset: f64,
        x_scale: f64,
    ) -> Point {
        // X in pattern space -> distance along skeleton.
        let x_norm = if pat_x_range.abs() > 1e-10 {
            (p.x - pat_x_min) / pat_x_range
        } else {
            0.0
        };
        let dist = (copy_offset + x_norm * pat_x_range * x_scale + self.tangent_offset.value)
            .clamp(0.0, skeleton_len);

        // Y in pattern space -> perpendicular offset.
        let y_offset = p.y - pat_y_center + self.normal_offset.value;

        // Find position on skeleton.
        let param = skeleton.param_at_arclen(dist, 0.5).unwrap_or(0.0);
        let center = skeleton.eval(param).unwrap_or(Point::ZERO);

        // Tangent via finite difference.
        let eps = 0.01;
        let p_fwd = skeleton.eval(param + eps).unwrap_or(center);
        let p_bck = skeleton.eval(param - eps).unwrap_or(center);

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

impl LivePathEffect for PatternAlongPathLpe {
    fn name(&self) -> &str {
        "pattern_along_path"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        if self.pattern.is_empty() || input.is_empty() {
            return input.clone();
        }

        let skeleton_len = input.arc_length(0.5);
        if skeleton_len < 1e-10 {
            return input.clone();
        }

        // Pattern bounding box.
        let pat_bb = self.pattern.bounding_box();
        let pat_x_min = pat_bb.x0;
        let pat_x_range = pat_bb.x1 - pat_bb.x0;
        let pat_y_center = (pat_bb.y0 + pat_bb.y1) * 0.5;

        if pat_x_range < 1e-10 {
            return input.clone();
        }

        // Determine number of copies and X-scale.
        let spacing = self.spacing.value;
        let (num_copies, x_scale) = if self.stretch_to_fit {
            let n = if self.copies > 0 {
                self.copies
            } else {
                ((skeleton_len / (pat_x_range + spacing)).floor() as usize).max(1)
            };
            let scale = skeleton_len / (n as f64 * pat_x_range);
            (n, scale)
        } else if self.copies > 0 {
            (self.copies, 1.0)
        } else {
            let n = ((skeleton_len / (pat_x_range + spacing)).floor() as usize).max(1);
            (n, 1.0)
        };

        let copy_span = pat_x_range * x_scale + spacing;

        let mut result = BezPath::new();
        let pattern_elements = self.pattern.elements();

        for ci in 0..num_copies {
            let copy_offset = ci as f64 * copy_span;

            for el in &pattern_elements {
                match *el {
                    PathSegment::MoveTo(p) => {
                        result.move_to(self.map_point(
                            input,
                            p,
                            pat_x_min,
                            pat_x_range,
                            pat_y_center,
                            skeleton_len,
                            copy_offset,
                            x_scale,
                        ));
                    }
                    PathSegment::LineTo(p) => {
                        result.line_to(self.map_point(
                            input,
                            p,
                            pat_x_min,
                            pat_x_range,
                            pat_y_center,
                            skeleton_len,
                            copy_offset,
                            x_scale,
                        ));
                    }
                    PathSegment::QuadTo(cp, end) => {
                        result.quad_to(
                            self.map_point(
                                input,
                                cp,
                                pat_x_min,
                                pat_x_range,
                                pat_y_center,
                                skeleton_len,
                                copy_offset,
                                x_scale,
                            ),
                            self.map_point(
                                input,
                                end,
                                pat_x_min,
                                pat_x_range,
                                pat_y_center,
                                skeleton_len,
                                copy_offset,
                                x_scale,
                            ),
                        );
                    }
                    PathSegment::CurveTo(cp1, cp2, end) => {
                        result.curve_to(
                            self.map_point(
                                input,
                                cp1,
                                pat_x_min,
                                pat_x_range,
                                pat_y_center,
                                skeleton_len,
                                copy_offset,
                                x_scale,
                            ),
                            self.map_point(
                                input,
                                cp2,
                                pat_x_min,
                                pat_x_range,
                                pat_y_center,
                                skeleton_len,
                                copy_offset,
                                x_scale,
                            ),
                            self.map_point(
                                input,
                                end,
                                pat_x_min,
                                pat_x_range,
                                pat_y_center,
                                skeleton_len,
                                copy_offset,
                                x_scale,
                            ),
                        );
                    }
                    PathSegment::ClosePath => {
                        result.close();
                    }
                }
            }
        }

        result
    }

    fn params(&self) -> &[LpeParam] {
        // Return all three numeric params. We store them contiguously so we
        // can return a slice if we pack them into an array. For simplicity we
        // allocate on the stack via a leak-free approach.
        // Because the trait returns &[LpeParam], we return only spacing as the
        // "primary" parameter and expose the rest via set_param.
        std::slice::from_ref(&self.spacing)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "spacing" => self.spacing.value = value.clamp(self.spacing.min, self.spacing.max),
            "normal_offset" => {
                self.normal_offset.value =
                    value.clamp(self.normal_offset.min, self.normal_offset.max)
            }
            "tangent_offset" => {
                self.tangent_offset.value =
                    value.clamp(self.tangent_offset.min, self.tangent_offset.max)
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small triangle pattern.
    fn triangle_pattern() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, -5.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(0.0, 5.0));
        p.close();
        p
    }

    /// Straight horizontal skeleton.
    fn straight_skeleton() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 50.0));
        p.line_to(Point::new(200.0, 50.0));
        p
    }

    #[test]
    fn test_pattern_along_straight_path() {
        let lpe = PatternAlongPathLpe::new(triangle_pattern(), 3, 0.0, 0.0, 0.0, false);
        let skeleton = straight_skeleton();
        let output = lpe.apply(&skeleton);
        assert!(!output.is_empty());
        // Should have more elements than a single copy of the pattern.
        assert!(
            output.element_count() > triangle_pattern().element_count(),
            "output should contain multiple copies"
        );
    }

    #[test]
    fn test_pattern_auto_copies() {
        let lpe = PatternAlongPathLpe::new(triangle_pattern(), 0, 0.0, 0.0, 0.0, false);
        let skeleton = straight_skeleton();
        let output = lpe.apply(&skeleton);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_pattern_stretch_to_fit() {
        let lpe = PatternAlongPathLpe::new(triangle_pattern(), 2, 0.0, 0.0, 0.0, true);
        let skeleton = straight_skeleton();
        let output = lpe.apply(&skeleton);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_pattern_with_normal_offset() {
        let lpe = PatternAlongPathLpe::new(triangle_pattern(), 2, 0.0, 10.0, 0.0, false);
        let skeleton = straight_skeleton();
        let output = lpe.apply(&skeleton);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_pattern_empty_pattern() {
        let lpe = PatternAlongPathLpe::new(BezPath::new(), 2, 0.0, 0.0, 0.0, false);
        let skeleton = straight_skeleton();
        let output = lpe.apply(&skeleton);
        // Should return the input unchanged.
        assert_eq!(skeleton.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_pattern_empty_skeleton() {
        let lpe = PatternAlongPathLpe::new(triangle_pattern(), 2, 0.0, 0.0, 0.0, false);
        let skeleton = BezPath::new();
        let output = lpe.apply(&skeleton);
        assert!(output.is_empty());
    }

    #[test]
    fn test_pattern_set_param() {
        let mut lpe = PatternAlongPathLpe::new(triangle_pattern(), 2, 5.0, 0.0, 0.0, false);
        lpe.set_param("spacing", 10.0);
        assert!((lpe.spacing.value - 10.0).abs() < 1e-10);
        lpe.set_param("normal_offset", 3.0);
        assert!((lpe.normal_offset.value - 3.0).abs() < 1e-10);
    }
}
