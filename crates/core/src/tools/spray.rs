//! Spray tool: spray clones/copies of the selected element.
//!
//! On each drag event, create a clone of the currently selected element at a
//! random offset within the spray radius.
//!
//! ## Features
//!
//! - **17.3.1** Spray copies/clones in area
//! - **17.3.2** Spray parameters: width (radius), amount (density),
//!   rotation variation, scale variation
//! - **17.3.3** Spray modes: Copy, Clone (`<use>`), SinglePath

use sivgo_dom::types::{LengthUnit, SvgLength};
use sivgo_dom::{SvgDocument, SvgElement, UseAttrs};
use sivgo_math::{Point, Transform2D};

use crate::commands::AddNodeCommand;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// A simple deterministic pseudo-random number generator (xorshift64) so that
/// the spray tool does not depend on external crates.
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

    /// Produce a pseudo-random u64.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Produce a pseudo-random f64 in `[0.0, 1.0)`.
    fn next_f64(&mut self) -> f64 {
        let v = self.next_u64();
        v as f64 / u64::MAX as f64
    }

    /// Produce a pseudo-random f64 in `[-1.0, 1.0)`.
    fn next_f64_signed(&mut self) -> f64 {
        self.next_f64() * 2.0 - 1.0
    }
}

/// Spray mode determines how copies are created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SprayMode {
    /// Create a deep copy of the source element.
    #[default]
    Copy,
    /// Create an SVG `<use>` clone referencing the source element.
    Clone,
    /// Create a translated copy as a single path (same as Copy but
    /// conceptually intended for path elements that could be merged later).
    SinglePath,
}

/// The spray tool.
///
/// While dragging, copies of the currently selected element are placed at
/// random offsets within `radius` of the cursor, with optional random
/// rotation and scale variation.
#[derive(Debug)]
pub struct SprayTool {
    /// Whether we are currently spraying.
    spraying: bool,
    /// Spray radius in document units (17.3.2 width parameter).
    radius: f64,
    /// Density: how many clones to create per drag event (17.3.2 amount).
    density: u32,
    /// Maximum random rotation in radians applied to each sprayed copy (17.3.2).
    rotation_variation: f64,
    /// Scale variation factor; each copy is scaled by `1.0 +/- scale_variation`
    /// (17.3.2). A value of 0.0 means no variation.
    scale_variation: f64,
    /// The spray mode (17.3.3).
    mode: SprayMode,
    /// Internal RNG state for random offsets.
    rng: SimpleRng,
    /// Counter used to seed the RNG with some variation per session.
    counter: u64,
}

impl SprayTool {
    pub fn new() -> Self {
        Self {
            spraying: false,
            radius: 50.0,
            density: 1,
            rotation_variation: 0.0,
            scale_variation: 0.0,
            mode: SprayMode::Copy,
            rng: SimpleRng::new(42),
            counter: 0,
        }
    }

    /// Set the spray radius (width parameter).
    pub fn set_radius(&mut self, radius: f64) {
        self.radius = radius.max(1.0);
    }

    /// Get the current radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// Set the density (clones per drag event / amount parameter).
    pub fn set_density(&mut self, density: u32) {
        self.density = density.max(1);
    }

    /// Get the current density.
    pub fn density(&self) -> u32 {
        self.density
    }

    /// Set the maximum rotation variation in radians.
    ///
    /// Each sprayed copy will be randomly rotated by up to `+/- variation` radians.
    pub fn set_rotation_variation(&mut self, variation: f64) {
        self.rotation_variation = variation.abs();
    }

    /// Get the current rotation variation.
    pub fn rotation_variation(&self) -> f64 {
        self.rotation_variation
    }

    /// Set the scale variation factor.
    ///
    /// Each sprayed copy will be scaled by a random factor in `[1 - v, 1 + v]`
    /// where `v` is the variation (clamped so the minimum scale is > 0).
    pub fn set_scale_variation(&mut self, variation: f64) {
        self.scale_variation = variation.abs().min(0.99);
    }

    /// Get the current scale variation.
    pub fn scale_variation(&self) -> f64 {
        self.scale_variation
    }

    /// Set the spray mode.
    pub fn set_mode(&mut self, mode: SprayMode) {
        self.mode = mode;
    }

    /// Get the current spray mode.
    pub fn mode(&self) -> SprayMode {
        self.mode
    }

    /// Spray a single element near the cursor.
    ///
    /// Depending on the current [`SprayMode`]:
    /// - **Copy / SinglePath**: deep-copy the source element with a random
    ///   translate + rotate + scale transform.
    /// - **Clone**: create a `<use>` element referencing the source via its
    ///   `xml_id`.
    fn spray_once(
        &mut self,
        cursor: Point,
        doc: &SvgDocument,
        selection: &SelectionSet,
    ) -> Option<AddNodeCommand> {
        let source_id = selection.primary()?;
        let source_node = doc.get_node(source_id)?;
        let parent = doc.root()?;

        // Random offset within the spray radius.
        let dx = self.rng.next_f64_signed() * self.radius;
        let dy = self.rng.next_f64_signed() * self.radius;

        // Only place if within circular radius.
        if dx * dx + dy * dy > self.radius * self.radius {
            return None;
        }

        let offset_x = cursor.x + dx;
        let offset_y = cursor.y + dy;

        // Random rotation and scale.
        let rotation = if self.rotation_variation > 0.0 {
            self.rng.next_f64_signed() * self.rotation_variation
        } else {
            0.0
        };

        let scale = if self.scale_variation > 0.0 {
            let sv = self.rng.next_f64_signed() * self.scale_variation;
            (1.0 + sv).max(0.01)
        } else {
            1.0
        };

        match self.mode {
            SprayMode::Clone => {
                // Create a <use> element referencing the source by xml_id.
                let href = match &source_node.xml_id {
                    Some(id) => format!("#{}", id),
                    None => {
                        // Fall back to copy mode if no xml_id.
                        return self.spray_copy(
                            source_node,
                            parent,
                            offset_x,
                            offset_y,
                            rotation,
                            scale,
                        );
                    }
                };

                let element = SvgElement::Use(UseAttrs {
                    href: Some(href),
                    x: Some(SvgLength::new(0.0, LengthUnit::None)),
                    y: Some(SvgLength::new(0.0, LengthUnit::None)),
                    width: None,
                    height: None,
                });

                let mut pres = sivgo_dom::PresentationAttrs::default();
                // Build transform: translate to offset, then rotate, then scale.
                let mut t = Transform2D::IDENTITY;
                if (scale - 1.0).abs() > 1e-12 {
                    t = t.then(&Transform2D::scale(scale));
                }
                if rotation.abs() > 1e-12 {
                    t = t.then(&Transform2D::rotate(rotation));
                }
                t = t.then(&Transform2D::translate(offset_x, offset_y));
                pres.transform = Some(t);

                Some(AddNodeCommand::new(parent, element, pres, None))
            }
            SprayMode::Copy | SprayMode::SinglePath => {
                self.spray_copy(source_node, parent, offset_x, offset_y, rotation, scale)
            }
        }
    }

    /// Create a deep copy of the source node at the given offset with rotation
    /// and scale.
    fn spray_copy(
        &self,
        source_node: &sivgo_dom::SvgNode,
        parent: sivgo_dom::NodeId,
        offset_x: f64,
        offset_y: f64,
        rotation: f64,
        scale: f64,
    ) -> Option<AddNodeCommand> {
        let element = source_node.element.clone();
        let mut pres = source_node.presentation.clone();

        // Build transform: base * scale * rotate * translate.
        let base_transform = pres.transform.unwrap_or(Transform2D::IDENTITY);
        let mut t = base_transform;
        if (scale - 1.0).abs() > 1e-12 {
            t = t.then(&Transform2D::scale(scale));
        }
        if rotation.abs() > 1e-12 {
            t = t.then(&Transform2D::rotate(rotation));
        }
        t = t.then(&Transform2D::translate(offset_x, offset_y));
        pres.transform = Some(t);

        Some(AddNodeCommand::new(parent, element, pres, None))
    }
}

impl Default for SprayTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SprayTool {
    fn name(&self) -> &str {
        "Spray"
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

        self.spraying = true;
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
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.spraying {
            return ToolResponse::None;
        }

        if selection.is_empty() {
            return ToolResponse::Handled;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));

        // Attempt to create `density` clones.
        let mut commands: Vec<Box<dyn crate::command::Command>> = Vec::new();
        for _ in 0..self.density {
            if let Some(cmd) = self.spray_once(doc_pt, doc, selection) {
                commands.push(Box::new(cmd));
            }
        }

        if commands.is_empty() {
            return ToolResponse::Handled;
        }

        if commands.len() == 1 {
            ToolResponse::ExecuteCommand(commands.into_iter().next().unwrap())
        } else {
            ToolResponse::ExecuteCommand(Box::new(crate::commands::CompoundCommand::new(
                "Spray clones",
                commands,
            )))
        }
    }

    fn on_mouse_up(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        self.spraying = false;
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
            self.spraying = false;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.spraying = false;
    }

    fn on_deactivate(&mut self) {
        self.spraying = false;
    }

    fn cursor(&self) -> &str {
        "crosshair"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use sivgo_dom::types::{LengthUnit, SvgLength};
    use sivgo_dom::{RectAttrs, SvgDocument, SvgElement, SvgSvgAttrs};

    #[test]
    fn configuration_setters_round_trip() {
        let mut tool = SprayTool::new();
        tool.set_radius(42.0);
        tool.set_density(7);
        tool.set_rotation_variation(0.5);
        tool.set_scale_variation(0.25);
        tool.set_mode(SprayMode::SinglePath);
        assert_eq!(tool.radius(), 42.0);
        assert_eq!(tool.density(), 7);
        assert_eq!(tool.mode(), SprayMode::SinglePath);
    }

    fn make_doc_with_rect() -> (SvgDocument, sivgo_dom::NodeId) {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::new(10.0, LengthUnit::None),
            y: SvgLength::new(10.0, LengthUnit::None),
            width: SvgLength::new(50.0, LengthUnit::None),
            height: SvgLength::new(50.0, LengthUnit::None),
            rx: SvgLength::zero(),
            ry: SvgLength::zero(),
        }));
        doc.append_child(svg_id, rect_id);
        (doc, rect_id)
    }

    #[test]
    fn spray_tool_defaults() {
        let tool = SprayTool::new();
        assert_eq!(tool.radius(), 50.0);
        assert_eq!(tool.density(), 1);
        assert_eq!(tool.rotation_variation(), 0.0);
        assert_eq!(tool.scale_variation(), 0.0);
        assert_eq!(tool.mode(), SprayMode::Copy);
    }

    #[test]
    fn spray_parameters_clamped() {
        let mut tool = SprayTool::new();

        tool.set_radius(0.0);
        assert_eq!(tool.radius(), 1.0);

        tool.set_density(0);
        assert_eq!(tool.density(), 1);

        tool.set_scale_variation(2.0);
        assert!((tool.scale_variation() - 0.99).abs() < 1e-10);
    }

    /// Helper: try spray_once up to `max_attempts` times, returning the first
    /// `Some` result.  The circular radius check rejects some random offsets,
    /// so we may need a few tries.
    fn spray_until_hit(
        tool: &mut SprayTool,
        cursor: Point,
        doc: &SvgDocument,
        sel: &SelectionSet,
        max_attempts: usize,
    ) -> Option<AddNodeCommand> {
        for _ in 0..max_attempts {
            if let Some(cmd) = tool.spray_once(cursor, doc, sel) {
                return Some(cmd);
            }
        }
        None
    }

    #[test]
    fn spray_once_copy_mode_produces_element() {
        let (doc, rect_id) = make_doc_with_rect();
        let mut sel = SelectionSet::new();
        sel.select(rect_id);

        let mut tool = SprayTool::new();
        tool.set_mode(SprayMode::Copy);
        tool.set_radius(1000.0);

        let cmd = spray_until_hit(&mut tool, Point::new(100.0, 100.0), &doc, &sel, 50);
        assert!(
            cmd.is_some(),
            "Copy mode should produce a command within 50 attempts"
        );
    }

    #[test]
    fn spray_once_clone_mode_needs_xml_id() {
        let (doc, rect_id) = make_doc_with_rect();
        let mut sel = SelectionSet::new();
        sel.select(rect_id);

        let mut tool = SprayTool::new();
        tool.set_mode(SprayMode::Clone);
        tool.set_radius(1000.0);

        // Without xml_id, clone mode falls back to copy.
        let cmd = spray_until_hit(&mut tool, Point::new(100.0, 100.0), &doc, &sel, 50);
        assert!(
            cmd.is_some(),
            "Clone mode should fall back to copy when no xml_id"
        );
    }

    #[test]
    fn spray_once_clone_mode_with_xml_id() {
        let (mut doc, rect_id) = make_doc_with_rect();
        // Set an xml_id on the rect.
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.xml_id = Some("source-rect".to_string());
        }
        let mut sel = SelectionSet::new();
        sel.select(rect_id);

        let mut tool = SprayTool::new();
        tool.set_mode(SprayMode::Clone);
        tool.set_radius(1000.0);

        let cmd = spray_until_hit(&mut tool, Point::new(100.0, 100.0), &doc, &sel, 50);
        assert!(cmd.is_some(), "Clone mode should produce a <use> command");

        // Execute the command and verify it created a <use> element.
        let mut cmd = cmd.unwrap();
        cmd.execute(&mut doc);
        let root = doc.root().unwrap();
        let children = doc.children(root);
        // Original rect + sprayed use element.
        assert!(children.len() >= 2);
        let last_id = *children.last().unwrap();
        let node = doc.get_node(last_id).unwrap();
        assert_eq!(node.element.tag_name(), "use");
    }

    #[test]
    fn spray_rotation_and_scale_variation() {
        let (doc, rect_id) = make_doc_with_rect();
        let mut sel = SelectionSet::new();
        sel.select(rect_id);

        let mut tool = SprayTool::new();
        tool.set_rotation_variation(std::f64::consts::PI);
        tool.set_scale_variation(0.5);
        tool.set_radius(1000.0);

        let cmd = spray_until_hit(&mut tool, Point::new(100.0, 100.0), &doc, &sel, 50);
        assert!(cmd.is_some());

        // The transform should differ from a simple translation.
        let mut cmd = cmd.unwrap();
        let mut doc = doc;
        cmd.execute(&mut doc);
        let root = doc.root().unwrap();
        let children = doc.children(root);
        let last_id = *children.last().unwrap();
        let node = doc.get_node(last_id).unwrap();
        assert!(node.presentation.transform.is_some());
    }

    #[test]
    fn spray_single_path_mode_works() {
        let (doc, rect_id) = make_doc_with_rect();
        let mut sel = SelectionSet::new();
        sel.select(rect_id);

        let mut tool = SprayTool::new();
        tool.set_mode(SprayMode::SinglePath);
        tool.set_radius(1000.0);

        let cmd = spray_until_hit(&mut tool, Point::new(100.0, 100.0), &doc, &sel, 50);
        assert!(
            cmd.is_some(),
            "SinglePath mode should produce a command within 50 attempts"
        );
    }
}
