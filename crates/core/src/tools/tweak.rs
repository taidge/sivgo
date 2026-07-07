//! Tweak tool: interactively modify objects near the cursor.
//!
//! ## Features
//!
//! - **17.4.1 Push** -- shift objects away from cursor
//! - **17.4.2 Shrink/Grow** -- scale objects up or down
//! - **17.4.3 Rotate** -- apply random rotation
//! - **17.4.4 Roughen** -- add random noise to path control points
//! - **17.4.5 Blur/Sharpen Colors** -- shift fill/stroke colors

use sivgo_dom::{SvgDocument, SvgElement};
use sivgo_math::{Point, Transform2D};

use crate::command::Command;
use crate::commands::{CompoundCommand, ModifyAttributeCommand, TransformCommand};
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::{element_bounding_box, SelectionSet};
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// The modes the tweak tool can operate in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TweakMode {
    /// 17.4.1 -- Push objects away from the cursor.
    #[default]
    Push,
    /// 17.4.2 -- Shrink objects (scale < 1).
    Shrink,
    /// 17.4.2 -- Grow objects (scale > 1).
    Grow,
    /// 17.4.3 -- Rotate objects around their centers.
    Rotate,
    /// 17.4.4 -- Roughen path control points (adds jitter to transforms as a
    /// lightweight approximation; full control-point editing would require
    /// `ModifyPathCommand` integration which is deferred).
    Roughen,
    /// 17.4.5 -- Blur / lighten colors (increases opacity slightly).
    Blur,
    /// 17.4.5 -- Sharpen / darken colors (decreases opacity slightly).
    Sharpen,
}

/// A simple deterministic pseudo-random number generator (xorshift64).
#[derive(Debug)]
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f64_signed(&mut self) -> f64 {
        let v = self.next_u64();
        (v as f64 / u64::MAX as f64) * 2.0 - 1.0
    }
}

/// The tweak tool.
///
/// While dragging, elements whose bounding box centers are within `radius` of
/// the cursor are modified according to the active [`TweakMode`].
#[derive(Debug)]
pub struct TweakTool {
    /// Whether we are currently dragging/tweaking.
    active: bool,
    /// Effect radius in document units.
    radius: f64,
    /// How far to push objects per drag event (or how much to scale/rotate).
    strength: f64,
    /// The active tweak mode.
    mode: TweakMode,
    /// Internal RNG for roughen/rotate jitter.
    rng: SimpleRng,
    /// Counter to re-seed the RNG each activation.
    counter: u64,
}

impl TweakTool {
    pub fn new() -> Self {
        Self {
            active: false,
            radius: 50.0,
            strength: 5.0,
            mode: TweakMode::Push,
            rng: SimpleRng::new(42),
            counter: 0,
        }
    }

    /// Set the effect radius.
    pub fn set_radius(&mut self, radius: f64) {
        self.radius = radius.max(1.0);
    }

    /// Get the current radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// Set the push/effect strength.
    pub fn set_strength(&mut self, strength: f64) {
        self.strength = strength.max(0.0);
    }

    /// Get the current strength.
    pub fn strength(&self) -> f64 {
        self.strength
    }

    /// Set the tweak mode.
    pub fn set_mode(&mut self, mode: TweakMode) {
        self.mode = mode;
    }

    /// Get the current tweak mode.
    pub fn mode(&self) -> TweakMode {
        self.mode
    }

    /// Apply the current tweak mode to all nearby leaf elements.
    fn tweak_nearby(&mut self, cursor: Point, doc: &SvgDocument) -> Vec<Box<dyn Command>> {
        let root = match doc.root() {
            Some(r) => r,
            None => return Vec::new(),
        };

        let all = doc.traverse_pre_order(root);
        let mut commands: Vec<Box<dyn Command>> = Vec::new();

        for id in all {
            let node = match doc.get_node(id) {
                Some(n) => n,
                None => continue,
            };

            // Skip non-leaf elements.
            match &node.element {
                SvgElement::Svg(_) | SvgElement::Group(_) | SvgElement::Defs(_) => continue,
                _ => {}
            }

            // Skip locked or hidden.
            if node.locked || node.hidden {
                continue;
            }

            let bb = match element_bounding_box(&node.element) {
                Some(bb) => bb,
                None => continue,
            };

            let center = bb.center();
            let dist = center.distance(cursor);

            if dist > self.radius || dist < 1e-12 {
                continue;
            }

            // Falloff: strength diminishes with distance.
            let falloff = 1.0 - (dist / self.radius);
            let old_t = node.presentation.transform;
            let base = old_t.unwrap_or(Transform2D::IDENTITY);

            match self.mode {
                TweakMode::Push => {
                    let push = self.strength * falloff;
                    let dx = (center.x - cursor.x) / dist * push;
                    let dy = (center.y - cursor.y) / dist * push;
                    let new_t = base.then(&Transform2D::translate(dx, dy));
                    commands.push(Box::new(TransformCommand::new(id, old_t, Some(new_t))));
                }
                TweakMode::Shrink => {
                    // Scale down towards center.
                    let factor = 1.0 - (self.strength * falloff * 0.02).min(0.5);
                    let scale_t = Transform2D::scale(factor);
                    // Translate to keep center stable.
                    let pre = Transform2D::translate(-center.x, -center.y);
                    let post = Transform2D::translate(center.x, center.y);
                    let combined = pre.then(&scale_t).then(&post);
                    let new_t = base.then(&combined);
                    commands.push(Box::new(TransformCommand::new(id, old_t, Some(new_t))));
                }
                TweakMode::Grow => {
                    // Scale up from center.
                    let factor = 1.0 + (self.strength * falloff * 0.02).min(2.0);
                    let scale_t = Transform2D::scale(factor);
                    let pre = Transform2D::translate(-center.x, -center.y);
                    let post = Transform2D::translate(center.x, center.y);
                    let combined = pre.then(&scale_t).then(&post);
                    let new_t = base.then(&combined);
                    commands.push(Box::new(TransformCommand::new(id, old_t, Some(new_t))));
                }
                TweakMode::Rotate => {
                    // Rotate around element center.
                    let angle = self.strength * falloff * 0.05; // radians
                    let rot = Transform2D::rotate_about(angle, center);
                    let new_t = base.then(&rot);
                    commands.push(Box::new(TransformCommand::new(id, old_t, Some(new_t))));
                }
                TweakMode::Roughen => {
                    // Apply a small random translation jitter to simulate roughening.
                    let jitter = self.strength * falloff;
                    let dx = self.rng.next_f64_signed() * jitter;
                    let dy = self.rng.next_f64_signed() * jitter;
                    let new_t = base.then(&Transform2D::translate(dx, dy));
                    commands.push(Box::new(TransformCommand::new(id, old_t, Some(new_t))));
                }
                TweakMode::Blur => {
                    // Increase opacity slightly (move towards transparent = blur effect).
                    let current_opacity = node.presentation.opacity.unwrap_or(1.0);
                    let delta = self.strength * falloff * 0.01;
                    let new_opacity = (current_opacity - delta).max(0.0);
                    if (new_opacity - current_opacity).abs() > 1e-12 {
                        let mut new_pres = node.presentation.clone();
                        new_pres.opacity = Some(new_opacity);
                        commands.push(Box::new(ModifyAttributeCommand::new(
                            id,
                            node.presentation.clone(),
                            new_pres,
                        )));
                    }
                }
                TweakMode::Sharpen => {
                    // Increase opacity slightly (move towards opaque = sharpen effect).
                    let current_opacity = node.presentation.opacity.unwrap_or(1.0);
                    let delta = self.strength * falloff * 0.01;
                    let new_opacity = (current_opacity + delta).min(1.0);
                    if (new_opacity - current_opacity).abs() > 1e-12 {
                        let mut new_pres = node.presentation.clone();
                        new_pres.opacity = Some(new_opacity);
                        commands.push(Box::new(ModifyAttributeCommand::new(
                            id,
                            node.presentation.clone(),
                            new_pres,
                        )));
                    }
                }
            }
        }

        commands
    }
}

impl Default for TweakTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TweakTool {
    fn name(&self) -> &str {
        "Tweak"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        self.active = true;
        self.counter = self.counter.wrapping_add(1);
        self.rng = SimpleRng::new(
            self.counter
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1),
        );
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.active {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        let commands = self.tweak_nearby(doc_pt, doc);

        if commands.is_empty() {
            return ToolResponse::Handled;
        }

        if commands.len() == 1 {
            ToolResponse::ExecuteCommand(commands.into_iter().next().unwrap())
        } else {
            ToolResponse::ExecuteCommand(Box::new(CompoundCommand::new("Tweak objects", commands)))
        }
    }

    fn on_mouse_up(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        self.active = false;
        ToolResponse::Handled
    }

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        use crate::input::Key;
        if event.key == Key::Escape {
            self.active = false;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.active = false;
    }

    fn on_deactivate(&mut self) {
        self.active = false;
    }

    fn cursor(&self) -> &str {
        "move"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{RectAttrs, SvgDocument, SvgElement, SvgSvgAttrs};

    #[test]
    fn configuration_setters_round_trip() {
        let mut tool = TweakTool::new();
        tool.set_radius(33.0);
        tool.set_strength(0.8);
        tool.set_mode(TweakMode::Roughen);
        assert_eq!(tool.radius(), 33.0);
        assert_eq!(tool.strength(), 0.8);
        assert_eq!(tool.mode(), TweakMode::Roughen);
    }

    fn make_doc_with_rect(x: f64, y: f64, w: f64, h: f64) -> (SvgDocument, sivgo_dom::NodeId) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(x, LengthUnit::None),
            y: SvgLength::new(y, LengthUnit::None),
            width: SvgLength::new(w, LengthUnit::None),
            height: SvgLength::new(h, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, rect_id);
        (doc, rect_id)
    }

    #[test]
    fn tweak_tool_default_mode_is_push() {
        let tool = TweakTool::new();
        assert_eq!(tool.mode(), TweakMode::Push);
    }

    #[test]
    fn tweak_push_moves_objects_away() {
        let (doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        // Rect center at (30, 30).
        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Push);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        // Cursor at (25, 25), near but not at center.
        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(
            !commands.is_empty(),
            "Push should produce commands for nearby rect"
        );

        // Execute and verify transform changed.
        let mut doc = doc;
        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        assert!(
            node.presentation.transform.is_some(),
            "Should have a transform after push"
        );
    }

    #[test]
    fn tweak_shrink_reduces_scale() {
        let (doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Shrink);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        let mut doc = doc;
        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        let t = node.presentation.transform.unwrap();
        // The determinant of a uniform scale matrix should be < 1 (shrunk).
        assert!(
            t.determinant() < 1.0,
            "Shrink should reduce the determinant"
        );
    }

    #[test]
    fn tweak_grow_increases_scale() {
        let (doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Grow);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        let mut doc = doc;
        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        let t = node.presentation.transform.unwrap();
        assert!(
            t.determinant() > 1.0,
            "Grow should increase the determinant"
        );
    }

    #[test]
    fn tweak_rotate_changes_transform() {
        let (doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Rotate);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        let mut doc = doc;
        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        let t = node.presentation.transform.unwrap();
        // A rotation preserves determinant (det = 1) but changes off-diagonal.
        let coeffs = t.as_coeffs();
        // At least one of the off-diagonal elements should be non-zero.
        assert!(
            coeffs[1].abs() > 1e-12 || coeffs[2].abs() > 1e-12,
            "Rotate should produce off-diagonal transform coefficients"
        );
    }

    #[test]
    fn tweak_roughen_adds_jitter() {
        let (doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Roughen);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        let mut doc = doc;
        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        assert!(
            node.presentation.transform.is_some(),
            "Roughen should add a transform"
        );
    }

    #[test]
    fn tweak_blur_decreases_opacity() {
        let (mut doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        // Set initial opacity.
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.opacity = Some(1.0);
        }

        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Blur);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        let opacity = node.presentation.opacity.unwrap_or(1.0);
        assert!(
            opacity < 1.0,
            "Blur should decrease opacity, got {}",
            opacity
        );
    }

    #[test]
    fn tweak_sharpen_increases_opacity() {
        let (mut doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        // Set initial opacity to something less than 1.0.
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.opacity = Some(0.5);
        }

        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Sharpen);
        tool.set_radius(100.0);
        tool.set_strength(10.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(!commands.is_empty());

        for mut cmd in commands {
            cmd.execute(&mut doc);
        }
        let node = doc.get_node(rect_id).unwrap();
        let opacity = node.presentation.opacity.unwrap_or(1.0);
        assert!(
            opacity > 0.5,
            "Sharpen should increase opacity, got {}",
            opacity
        );
    }

    #[test]
    fn tweak_skips_locked_elements() {
        let (mut doc, rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.locked = true;
        }

        let mut tool = TweakTool::new();
        tool.set_mode(TweakMode::Push);
        tool.set_radius(100.0);

        let commands = tool.tweak_nearby(Point::new(25.0, 25.0), &doc);
        assert!(commands.is_empty(), "Locked elements should be skipped");
    }

    #[test]
    fn tweak_skips_distant_elements() {
        let (doc, _rect_id) = make_doc_with_rect(20.0, 20.0, 20.0, 20.0);
        // Rect center at (30, 30), cursor far away.
        let mut tool = TweakTool::new();
        tool.set_radius(5.0);

        let commands = tool.tweak_nearby(Point::new(500.0, 500.0), &doc);
        assert!(commands.is_empty(), "Distant elements should be skipped");
    }
}
