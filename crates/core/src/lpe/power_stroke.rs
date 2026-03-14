//! Power Stroke Live Path Effect.
//!
//! Generates a closed outline around the input path using a variable-width
//! profile. The width at each point along the path is interpolated from a
//! set of user-defined control points.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, Point, Vector};

/// A single width-profile control point.
///
/// `position` is in the range `[0, 1]` (fraction of total arc length).
/// `width` is the full stroke width at that position (half-width is applied
/// on each side).
#[derive(Debug, Clone, Copy)]
pub struct WidthPoint {
    /// Normalised position along the path (0 = start, 1 = end).
    pub position: f64,
    /// Full width at this position.
    pub width: f64,
}

/// Power Stroke LPE.
///
/// Produces a closed outline path by offsetting the input path at a variable
/// width defined by a series of [`WidthPoint`]s. Internally the path is
/// sampled at regular intervals and the width at each sample is linearly
/// interpolated from the profile.
#[derive(Debug, Clone)]
pub struct PowerStrokeLpe {
    /// Sorted width profile points.
    profile: Vec<WidthPoint>,
    /// Exposed as a single "samples" parameter to the generic param API.
    samples_param: LpeParam,
}

impl PowerStrokeLpe {
    /// Create a new Power Stroke effect.
    ///
    /// `profile` is a list of `(position, width)` pairs where position is in
    /// `[0, 1]` and width is the full stroke width.
    pub fn new(profile: Vec<(f64, f64)>) -> Self {
        let mut pts: Vec<WidthPoint> = profile
            .into_iter()
            .map(|(pos, w)| WidthPoint {
                position: pos,
                width: w,
            })
            .collect();
        pts.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());

        Self {
            profile: pts,
            samples_param: LpeParam::new("samples", 64.0, 4.0, 1024.0),
        }
    }

    /// Current width profile.
    pub fn profile(&self) -> &[WidthPoint] {
        &self.profile
    }

    /// Set or replace the width profile.
    pub fn set_profile(&mut self, profile: Vec<(f64, f64)>) {
        let mut pts: Vec<WidthPoint> = profile
            .into_iter()
            .map(|(pos, w)| WidthPoint {
                position: pos,
                width: w,
            })
            .collect();
        pts.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
        self.profile = pts;
    }

    /// Interpolate the width at normalised position `t` (0..1).
    fn width_at(&self, t: f64) -> f64 {
        if self.profile.is_empty() {
            return 0.0;
        }
        if self.profile.len() == 1 {
            return self.profile[0].width;
        }

        // Clamp
        let t = t.clamp(0.0, 1.0);

        // Find the two surrounding profile points.
        if t <= self.profile[0].position {
            return self.profile[0].width;
        }
        if t >= self.profile.last().unwrap().position {
            return self.profile.last().unwrap().width;
        }

        for i in 0..self.profile.len() - 1 {
            let a = &self.profile[i];
            let b = &self.profile[i + 1];
            if t >= a.position && t <= b.position {
                let span = b.position - a.position;
                if span.abs() < 1e-12 {
                    return a.width;
                }
                let local = (t - a.position) / span;
                return a.width + local * (b.width - a.width);
            }
        }

        self.profile.last().unwrap().width
    }
}

impl LivePathEffect for PowerStrokeLpe {
    fn name(&self) -> &str {
        "power_stroke"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let total_len = input.arc_length(0.5);
        if total_len < 1e-10 || self.profile.is_empty() {
            return input.clone();
        }

        let num_samples = self.samples_param.value as usize;

        // Sample positions along the path.
        let mut left_points: Vec<Point> = Vec::with_capacity(num_samples + 1);
        let mut right_points: Vec<Point> = Vec::with_capacity(num_samples + 1);

        for i in 0..=num_samples {
            let frac = i as f64 / num_samples as f64;
            let dist = frac * total_len;

            // Evaluate point on path.
            let param = match input.param_at_arclen(dist, 0.5) {
                Some(t) => t,
                None => continue,
            };
            let center = match input.eval(param) {
                Some(p) => p,
                None => continue,
            };

            // Compute tangent via finite difference.
            let eps = 0.01;
            let p_fwd = input
                .eval(param + eps)
                .or_else(|| input.eval(param))
                .unwrap_or(center);
            let p_bck = input
                .eval(param - eps)
                .or_else(|| input.eval(param))
                .unwrap_or(center);

            let tangent = Vector::new(p_fwd.x - p_bck.x, p_fwd.y - p_bck.y);
            let len = tangent.length();
            let normal = if len > 1e-12 {
                Vector::new(-tangent.y / len, tangent.x / len)
            } else {
                Vector::new(0.0, 1.0)
            };

            let half_w = self.width_at(frac) * 0.5;
            left_points.push(Point::new(
                center.x + normal.x * half_w,
                center.y + normal.y * half_w,
            ));
            right_points.push(Point::new(
                center.x - normal.x * half_w,
                center.y - normal.y * half_w,
            ));
        }

        // Build closed outline: left side forward, right side backward.
        let mut result = BezPath::new();
        if left_points.is_empty() {
            return result;
        }

        result.move_to(left_points[0]);
        for p in left_points.iter().skip(1) {
            result.line_to(*p);
        }
        for p in right_points.iter().rev() {
            result.line_to(*p);
        }
        result.close();

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
