//! Measure tool: click and drag to measure distance and angle between two
//! points. Does not create any document elements.

use sivgo_dom::SvgDocument;
use sivgo_math::Point;

use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// The measure tool.
///
/// Click and drag to measure the distance and angle between the start and
/// end points in document space. The measurement state is stored in the
/// `measurement` field as `(start, end, distance, angle_degrees)`.
///
/// This tool does not create any SVG elements or commands.
#[derive(Debug)]
pub struct MeasureTool {
    /// Whether the user is currently dragging.
    dragging: bool,
    /// Start point in document space (recorded on mouse down).
    start: Option<Point>,
    /// Current measurement result: (start, end, distance, angle_degrees).
    pub measurement: Option<(Point, Point, f64, f64)>,
}

impl MeasureTool {
    pub fn new() -> Self {
        Self {
            dragging: false,
            start: None,
            measurement: None,
        }
    }
}

impl Default for MeasureTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the Euclidean distance between two points.
fn distance(a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

/// Compute the angle in degrees from point `a` to point `b`, measured from
/// the positive X axis, counter-clockwise.
fn angle_degrees(a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dy.atan2(dx).to_degrees()
}

impl Tool for MeasureTool {
    fn name(&self) -> &str {
        "Measure"
    }

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if event.button != MouseButton::Left {
            return ToolResponse::None;
        }

        let doc_pt = viewport.screen_to_document(Point::new(event.x, event.y));
        self.start = Some(doc_pt);
        self.dragging = true;
        self.measurement = None;
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.dragging {
            return ToolResponse::None;
        }

        let start = match self.start {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let dist = distance(start, end);
        let angle = angle_degrees(start, end);
        self.measurement = Some((start, end, dist, angle));

        ToolResponse::Handled
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        if !self.dragging {
            return ToolResponse::None;
        }
        self.dragging = false;

        let start = match self.start.take() {
            Some(p) => p,
            None => return ToolResponse::None,
        };

        let end = viewport.screen_to_document(Point::new(event.x, event.y));
        let dist = distance(start, end);
        let angle = angle_degrees(start, end);
        self.measurement = Some((start, end, dist, angle));

        // No command is produced; we just record the measurement.
        ToolResponse::Handled
    }

    fn on_key_down(
        &mut self,
        _event: KeyEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.dragging = false;
        self.start = None;
        self.measurement = None;
    }

    fn on_deactivate(&mut self) {
        self.dragging = false;
        self.start = None;
        self.measurement = None;
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
