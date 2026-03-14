//! Live Path Effects (LPE) framework for non-destructive path manipulation.
//!
//! Provides:
//! - [`LivePathEffect`] trait defining the interface for all effects.
//! - [`LpeParam`] describing a named, bounded numeric parameter.
//! - [`LpeStack`] for chaining multiple effects on a single object.
//! - Built-in effects: [`FilletChamferLpe`], [`PowerStrokeLpe`], [`BendPathLpe`],
//!   [`PatternAlongPathLpe`], [`SpiroLpe`], [`SimplifyLpe`], [`RoughenLpe`],
//!   [`MirrorSymmetryLpe`], [`CopyRotateLpe`], [`BSplineLpe`], [`SketchLpe`],
//!   [`TilingLpe`], [`OffsetLpe`], [`EnvelopeLpe`], [`PerspectiveEnvelopeLpe`],
//!   [`LatticeLpe`], [`MeasureSegmentsLpe`], [`GearsLpe`], [`DashedStrokeLpe`],
//!   [`ExtrudeLpe`], [`KnotLpe`], [`HatchingLpe`], [`TaperStrokeLpe`],
//!   [`VonKochLpe`], [`InterpolateLpe`], [`ConstructGridLpe`], [`SliceLpe`].

mod bend;
mod bspline;
mod construct_grid;
mod copy_rotate;
mod dashed_stroke;
mod envelope;
mod extrude;
mod fillet;
mod gears;
mod hatching;
mod interpolate;
mod knot;
mod lattice;
mod measure_segments;
mod mirror;
mod offset;
mod pattern_along_path;
mod perspective_envelope;
mod power_stroke;
mod roughen;
mod simplify;
mod sketch;
mod slice_lpe;
mod spiro;
mod taper_stroke;
mod tiling;
mod von_koch;

pub use bend::BendPathLpe;
pub use bspline::BSplineLpe;
pub use construct_grid::ConstructGridLpe;
pub use copy_rotate::CopyRotateLpe;
pub use dashed_stroke::DashedStrokeLpe;
pub use envelope::EnvelopeLpe;
pub use extrude::ExtrudeLpe;
pub use fillet::{FilletChamferLpe, FilletMode};
pub use gears::GearsLpe;
pub use hatching::HatchingLpe;
pub use interpolate::InterpolateLpe;
pub use knot::KnotLpe;
pub use lattice::LatticeLpe;
pub use measure_segments::MeasureSegmentsLpe;
pub use mirror::MirrorSymmetryLpe;
pub use offset::{OffsetJoin, OffsetLpe};
pub use pattern_along_path::PatternAlongPathLpe;
pub use perspective_envelope::PerspectiveEnvelopeLpe;
pub use power_stroke::PowerStrokeLpe;
pub use roughen::RoughenLpe;
pub use simplify::SimplifyLpe;
pub use sketch::SketchLpe;
pub use slice_lpe::SliceLpe;
pub use spiro::SpiroLpe;
pub use taper_stroke::TaperStrokeLpe;
pub use tiling::TilingLpe;
pub use von_koch::VonKochLpe;

use sivgo_math::BezPath;

// ---------------------------------------------------------------------------
// Parameter descriptor
// ---------------------------------------------------------------------------

/// A named, bounded numeric parameter exposed by an LPE.
#[derive(Debug, Clone)]
pub struct LpeParam {
    /// Human-readable parameter name (e.g. `"radius"`).
    pub name: String,
    /// Current value.
    pub value: f64,
    /// Minimum allowed value.
    pub min: f64,
    /// Maximum allowed value.
    pub max: f64,
}

impl LpeParam {
    /// Create a new parameter with bounds.
    pub fn new(name: impl Into<String>, value: f64, min: f64, max: f64) -> Self {
        Self {
            name: name.into(),
            value: value.clamp(min, max),
            min,
            max,
        }
    }
}

// ---------------------------------------------------------------------------
// LivePathEffect trait
// ---------------------------------------------------------------------------

/// Trait implemented by every Live Path Effect.
///
/// Effects are applied non-destructively: the original path is kept intact and
/// the effect produces a new path each time [`apply`](LivePathEffect::apply) is
/// called.
pub trait LivePathEffect: std::fmt::Debug + Send + Sync {
    /// A short, unique name identifying the effect (e.g. `"fillet_chamfer"`).
    fn name(&self) -> &str;

    /// Apply this effect to `input` and return the resulting path.
    fn apply(&self, input: &BezPath) -> BezPath;

    /// Return the list of adjustable numeric parameters.
    fn params(&self) -> &[LpeParam];

    /// Set a parameter by name. Values are clamped to the parameter's
    /// `[min, max]` range. Does nothing if the name is not recognised.
    fn set_param(&mut self, name: &str, value: f64);
}

// ---------------------------------------------------------------------------
// LpeStack -- ordered collection of effects
// ---------------------------------------------------------------------------

/// An ordered stack of [`LivePathEffect`]s applied to a single object.
///
/// Effects are applied in order (index 0 first). The output of each effect
/// becomes the input of the next.
#[derive(Debug, Default)]
pub struct LpeStack {
    effects: Vec<Box<dyn LivePathEffect>>,
}

impl LpeStack {
    /// Create an empty stack.
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Apply all effects in order. If the stack is empty the input is returned
    /// unchanged.
    pub fn apply(&self, input: &BezPath) -> BezPath {
        let mut current = input.clone();
        for effect in &self.effects {
            current = effect.apply(&current);
        }
        current
    }

    /// Push an effect onto the end of the stack.
    pub fn push(&mut self, effect: Box<dyn LivePathEffect>) {
        self.effects.push(effect);
    }

    /// Remove the effect at `index`. Panics if out of bounds.
    pub fn remove(&mut self, index: usize) -> Box<dyn LivePathEffect> {
        self.effects.remove(index)
    }

    /// Move the effect at position `from` to position `to`.
    ///
    /// Both indices must be valid (< `len()`), otherwise this panics.
    pub fn reorder(&mut self, from: usize, to: usize) {
        let effect = self.effects.remove(from);
        self.effects.insert(to, effect);
    }

    /// Number of effects in the stack.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Whether the stack has no effects.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get a reference to the effect at `index`.
    pub fn get(&self, index: usize) -> Option<&dyn LivePathEffect> {
        self.effects.get(index).map(|e| e.as_ref())
    }

    /// Get a mutable reference to the effect at `index`.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Box<dyn LivePathEffect>> {
        self.effects.get_mut(index)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::Point;

    /// Helper: build a simple triangle path.
    fn triangle() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(50.0, 100.0));
        p.close();
        p
    }

    /// Helper: build a simple square path.
    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    // -----------------------------------------------------------------------
    // LpeStack basics
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_stack_is_identity() {
        let stack = LpeStack::new();
        let input = triangle();
        let output = stack.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_stack_push_and_len() {
        let mut stack = LpeStack::new();
        assert!(stack.is_empty());
        stack.push(Box::new(FilletChamferLpe::new(5.0, FilletMode::Fillet)));
        assert_eq!(stack.len(), 1);
        stack.push(Box::new(FilletChamferLpe::new(10.0, FilletMode::Chamfer)));
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_stack_remove() {
        let mut stack = LpeStack::new();
        stack.push(Box::new(FilletChamferLpe::new(5.0, FilletMode::Fillet)));
        stack.push(Box::new(FilletChamferLpe::new(10.0, FilletMode::Chamfer)));
        let removed = stack.remove(0);
        assert_eq!(removed.name(), "fillet_chamfer");
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_stack_reorder() {
        let mut stack = LpeStack::new();
        stack.push(Box::new(FilletChamferLpe::new(5.0, FilletMode::Fillet)));
        stack.push(Box::new(FilletChamferLpe::new(10.0, FilletMode::Chamfer)));

        // Before: [Fillet(5), Chamfer(10)]
        let p0 = stack.get(0).unwrap().params();
        assert_eq!(p0[0].value, 5.0);

        // Reorder: move index 1 to index 0
        stack.reorder(1, 0);

        // After: [Chamfer(10), Fillet(5)]
        let p0 = stack.get(0).unwrap().params();
        assert_eq!(p0[0].value, 10.0);
    }

    #[test]
    fn test_stack_apply_chains_effects() {
        let mut stack = LpeStack::new();
        // Two successive fillets on a square
        stack.push(Box::new(FilletChamferLpe::new(5.0, FilletMode::Fillet)));
        stack.push(Box::new(FilletChamferLpe::new(3.0, FilletMode::Fillet)));

        let input = square();
        let output = stack.apply(&input);
        // The output should differ from the input (corners are rounded).
        assert_ne!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_stack_get_mut_set_param() {
        let mut stack = LpeStack::new();
        stack.push(Box::new(FilletChamferLpe::new(5.0, FilletMode::Fillet)));

        // Mutate the effect via the stack.
        let effect = stack.get_mut(0).unwrap();
        effect.set_param("radius", 20.0);
        let params = stack.get(0).unwrap().params();
        assert!(
            (params[0].value - 20.0).abs() < 1e-10,
            "value={}",
            params[0].value
        );
    }

    // -----------------------------------------------------------------------
    // Fillet / Chamfer
    // -----------------------------------------------------------------------

    #[test]
    fn test_fillet_on_square() {
        let lpe = FilletChamferLpe::new(10.0, FilletMode::Fillet);
        let input = square();
        let output = lpe.apply(&input);

        // Output should have more segments than the input (arcs inserted).
        assert!(
            output.element_count() > input.element_count(),
            "fillet should add segments: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_chamfer_on_square() {
        let lpe = FilletChamferLpe::new(10.0, FilletMode::Chamfer);
        let input = square();
        let output = lpe.apply(&input);

        // Chamfer replaces each corner with a straight cut so there should be
        // more segments.
        assert!(
            output.element_count() > input.element_count(),
            "chamfer should add segments: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_fillet_zero_radius_is_identity() {
        let lpe = FilletChamferLpe::new(0.0, FilletMode::Fillet);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_fillet_large_radius_clamped() {
        // Radius larger than the available half-edge length -- should still
        // produce a valid path without panicking.
        let lpe = FilletChamferLpe::new(200.0, FilletMode::Fillet);
        let input = square(); // edges are 100px
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_fillet_params_and_set() {
        let mut lpe = FilletChamferLpe::new(10.0, FilletMode::Fillet);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "radius");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("radius", 25.0);
        assert!((lpe.params()[0].value - 25.0).abs() < 1e-10);

        // Clamped to max
        lpe.set_param("radius", 99999.0);
        assert!((lpe.params()[0].value - lpe.params()[0].max).abs() < 1e-10);
    }

    #[test]
    fn test_fillet_on_triangle() {
        let lpe = FilletChamferLpe::new(8.0, FilletMode::Fillet);
        let input = triangle();
        let output = lpe.apply(&input);
        assert!(output.element_count() > input.element_count());
    }

    #[test]
    fn test_chamfer_on_triangle() {
        let lpe = FilletChamferLpe::new(8.0, FilletMode::Chamfer);
        let input = triangle();
        let output = lpe.apply(&input);
        assert!(output.element_count() > input.element_count());
    }

    // -----------------------------------------------------------------------
    // Power Stroke
    // -----------------------------------------------------------------------

    #[test]
    fn test_power_stroke_basic() {
        let lpe = PowerStrokeLpe::new(vec![(0.0, 5.0), (1.0, 5.0)]);
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(100.0, 0.0));

        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        // The output should form a closed outline around the stroke.
        let elems = output.elements();
        assert!(elems.len() > 2);
    }

    #[test]
    fn test_power_stroke_variable_width() {
        // Narrow at start, wide at end
        let lpe = PowerStrokeLpe::new(vec![(0.0, 2.0), (1.0, 20.0)]);
        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, 0.0));
        input.line_to(Point::new(200.0, 0.0));

        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    // -----------------------------------------------------------------------
    // Bend Path
    // -----------------------------------------------------------------------

    #[test]
    fn test_bend_path_identity_straight() {
        // Bending along a straight horizontal path should roughly preserve the
        // input shape's width.
        let mut bend = BezPath::new();
        bend.move_to(Point::new(0.0, 0.0));
        bend.line_to(Point::new(100.0, 0.0));

        let lpe = BendPathLpe::new(bend);

        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, -10.0));
        input.line_to(Point::new(100.0, -10.0));
        input.line_to(Point::new(100.0, 10.0));
        input.line_to(Point::new(0.0, 10.0));
        input.close();

        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }

    #[test]
    fn test_bend_path_curved() {
        // Bend along a quarter-circle
        let mut bend = BezPath::new();
        bend.move_to(Point::new(0.0, 0.0));
        bend.curve_to(
            Point::new(0.0, 55.0),
            Point::new(45.0, 100.0),
            Point::new(100.0, 100.0),
        );

        let lpe = BendPathLpe::new(bend);

        let mut input = BezPath::new();
        input.move_to(Point::new(0.0, -5.0));
        input.line_to(Point::new(100.0, -5.0));
        input.line_to(Point::new(100.0, 5.0));
        input.line_to(Point::new(0.0, 5.0));
        input.close();

        let output = lpe.apply(&input);
        assert!(!output.is_empty());
    }
}
