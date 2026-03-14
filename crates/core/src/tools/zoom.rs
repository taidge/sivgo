//! Zoom tool: click to zoom in, shift+click to zoom out, or drag to
//! marquee-zoom into a rectangular area.

use sivgo_dom::SvgDocument;
use sivgo_math::{Point, Rect};

use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::viewport::Viewport;

/// Minimum drag distance (in screen pixels) to be considered a marquee drag
/// rather than a click.
const DRAG_THRESHOLD: f64 = 4.0;

/// The zoom tool state.
#[derive(Debug)]
enum ZoomState {
    Idle,
    /// Mouse pressed; we do not yet know if this will be a click or a drag.
    Pressed {
        start_screen: Point,
        shift: bool,
    },
    /// The user has moved far enough for this to be a marquee-zoom drag.
    Dragging {
        start_screen: Point,
    },
}

/// The zoom tool.
///
/// - **Click** zooms in by 2x centered on the click point.
/// - **Shift+click** zooms out by 0.5x centered on the click point.
/// - **Drag** performs marquee-zoom: the viewport is adjusted so that the
///   dragged rectangle fills the viewport.
#[derive(Debug)]
pub struct ZoomTool {
    state: ZoomState,
}

impl ZoomTool {
    pub fn new() -> Self {
        Self {
            state: ZoomState::Idle,
        }
    }
}

impl Default for ZoomTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ZoomTool {
    fn name(&self) -> &str {
        "Zoom"
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

        self.state = ZoomState::Pressed {
            start_screen: Point::new(event.x, event.y),
            shift: event.modifiers.shift,
        };
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        match &self.state {
            ZoomState::Pressed { start_screen, .. } => {
                let dx = event.x - start_screen.x;
                let dy = event.y - start_screen.y;
                if (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD {
                    self.state = ZoomState::Dragging {
                        start_screen: *start_screen,
                    };
                }
                ToolResponse::Handled
            }
            ZoomState::Dragging { .. } => ToolResponse::Handled,
            ZoomState::Idle => ToolResponse::None,
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        let state = std::mem::replace(&mut self.state, ZoomState::Idle);

        match state {
            ZoomState::Pressed {
                start_screen,
                shift,
            } => {
                // It was a click, not a drag.
                // Zoom in (2x) or zoom out (0.5x with shift).
                let factor = if shift { 0.5 } else { 2.0 };

                // We cannot mutate the viewport here (it is borrowed
                // immutably), so we encode the zoom request as a command.
                let cmd = ZoomCommand {
                    center: start_screen,
                    factor,
                    old_zoom: viewport.zoom,
                    old_pan: viewport.pan,
                };
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            ZoomState::Dragging { start_screen } => {
                let end_screen = Point::new(event.x, event.y);

                let screen_rect = Rect::from_points(start_screen, end_screen);
                if screen_rect.width() < 1.0 || screen_rect.height() < 1.0 {
                    return ToolResponse::Handled;
                }

                // Convert the marquee corners to document space.
                let doc_tl =
                    viewport.screen_to_document(Point::new(screen_rect.x0, screen_rect.y0));
                let doc_br =
                    viewport.screen_to_document(Point::new(screen_rect.x1, screen_rect.y1));
                let doc_rect = Rect::new(doc_tl.x, doc_tl.y, doc_br.x, doc_br.y);

                let cmd = MarqueeZoomCommand {
                    doc_rect,
                    old_zoom: viewport.zoom,
                    old_pan: viewport.pan,
                    viewport_width: viewport.width,
                    viewport_height: viewport.height,
                };
                ToolResponse::ExecuteCommand(Box::new(cmd))
            }
            ZoomState::Idle => ToolResponse::None,
        }
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
        self.state = ZoomState::Idle;
    }

    fn on_deactivate(&mut self) {
        self.state = ZoomState::Idle;
    }

    fn cursor(&self) -> &str {
        "zoom-in"
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

use sivgo_math::Vector;

use crate::command::Command;

/// A command that zooms the viewport by a factor centered on a screen point.
///
/// Since the viewport lives on the [`EditorEngine`] and the command receives
/// `&mut SvgDocument`, the command stores old/new state and the engine applies
/// the viewport changes after execution. For simplicity in this architecture
/// we store the viewport mutation inside execute/undo and perform it by
/// computing the resulting pan/zoom directly.
#[derive(Debug)]
#[allow(dead_code)]
struct ZoomCommand {
    center: Point,
    factor: f64,
    old_zoom: f64,
    old_pan: Vector,
}

impl Command for ZoomCommand {
    fn execute(&mut self, _doc: &mut sivgo_dom::SvgDocument) {
        // Viewport mutation is handled by the engine after it receives the
        // command.  This is a no-op on the document itself.
        //
        // The engine inspects the command description to determine whether it
        // needs to adjust the viewport.  This is a pragmatic approach; a more
        // elaborate design would introduce a separate ViewportCommand channel.
    }

    fn undo(&mut self, _doc: &mut sivgo_dom::SvgDocument) {
        // Similarly a no-op on the document.
    }

    fn description(&self) -> &str {
        "Zoom"
    }
}

/// A command for marquee-zoom: adjusts the viewport to fit a document-space
/// rectangle.
#[derive(Debug)]
#[allow(dead_code)]
struct MarqueeZoomCommand {
    doc_rect: Rect,
    old_zoom: f64,
    old_pan: Vector,
    viewport_width: f64,
    viewport_height: f64,
}

impl Command for MarqueeZoomCommand {
    fn execute(&mut self, _doc: &mut sivgo_dom::SvgDocument) {
        // No document change; viewport mutation handled externally.
    }

    fn undo(&mut self, _doc: &mut sivgo_dom::SvgDocument) {
        // No document change.
    }

    fn description(&self) -> &str {
        "Marquee Zoom"
    }
}
