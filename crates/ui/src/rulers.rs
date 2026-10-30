//! Ruler components (horizontal and vertical) that display tick marks and
//! the current cursor position indicator.

use crate::Callback;
use glory::prelude::{text, view, Cage, Event, Length, Result, Scope, Style, Widget};

/// Ruler size in pixels (the thickness of the ruler strip).
const RULER_SIZE: f64 = 20.0;

/// Compute the visible tick marks for a ruler along a single axis.
///
/// Returns a list of `(screen_pos, doc_value, is_major)` tuples. The logic is
/// identical for the horizontal and vertical rulers (only the axis differs),
/// so both components share this pure computation.
fn compute_ticks(length: f64, scroll: f64, zoom: f64) -> Vec<(f64, f64, bool)> {
    let tick_interval = compute_tick_interval(zoom);
    let tick_interval_screen = tick_interval * zoom;

    // Compute the first tick position in document coords aligned to tick_interval.
    let doc_start = -scroll / zoom;
    let first_tick_doc = (doc_start / tick_interval).floor() * tick_interval;

    let mut ticks: Vec<(f64, f64, bool)> = Vec::new();
    let mut doc_val = first_tick_doc;
    loop {
        let screen_pos = doc_val * zoom + scroll;
        if screen_pos > length + tick_interval_screen {
            break;
        }
        if screen_pos >= -tick_interval_screen {
            let is_major = (doc_val / (tick_interval * 5.0)).fract().abs() < 0.01;
            ticks.push((screen_pos, doc_val, is_major));
        }
        doc_val += tick_interval;
    }
    ticks
}

/// A horizontal ruler displayed above the canvas.
///
/// Shows tick marks at regular intervals based on zoom level, with a
/// cursor position indicator line. Supports dragging from the ruler to
/// create horizontal guides.
pub struct HorizontalRuler {
    pub width: f64,
    pub scroll_x: f64,
    pub zoom: f64,
    pub cursor_position: Cage<(f64, f64)>,
    /// Dropped Y position in viewport CSS pixels; the host applies the canvas transform.
    pub on_guide_drag: Callback<f64>,
}
pub struct VerticalRuler {
    pub height: f64,
    pub scroll_y: f64,
    pub zoom: f64,
    pub cursor_position: Cage<(f64, f64)>,
    /// Dropped X position in viewport CSS pixels; the host applies the canvas transform.
    pub on_guide_drag: Callback<f64>,
}
impl Widget for HorizontalRuler {
    fn build(&mut self, scope: &Scope) -> Result {
        ruler(
            scope,
            true,
            self.width,
            self.scroll_x,
            self.zoom,
            self.cursor_position,
            self.on_guide_drag.clone(),
        )
    }
}
impl Widget for VerticalRuler {
    fn build(&mut self, scope: &Scope) -> Result {
        ruler(
            scope,
            false,
            self.height,
            self.scroll_y,
            self.zoom,
            self.cursor_position,
            self.on_guide_drag.clone(),
        )
    }
}
fn ruler(
    scope: &Scope,
    horizontal: bool,
    length: f64,
    scroll: f64,
    zoom: f64,
    cursor: Cage<(f64, f64)>,
    callback: Callback<f64>,
) -> Result {
    use glory::render::{DragCapture, DragPhase, EventKind, Semantics};
    use glory::style::{Color, Overflow, Position};
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let length = if length.is_finite() {
        length.clamp(1.0, 32768.0)
    } else {
        1.0
    };
    let scroll = if scroll.is_finite() { scroll } else { 0.0 };
    let node = scope.cage(None);
    view()
        .class(if horizontal {
            "ruler ruler-horizontal"
        } else {
            "ruler ruler-vertical"
        })
        .name(if horizontal {
            "Horizontal ruler; drag to add guide"
        } else {
            "Vertical ruler; drag to add guide"
        })
        .node_ref(node)
        .style(Style {
            background: Some(Color(42, 42, 42, 255)),
            ..Style::new()
                .position(Position::Relative)
                .width(Length::Px(if horizontal { length } else { RULER_SIZE }))
                .height(Length::Px(if horizontal { RULER_SIZE } else { length }))
                .overflow_x(Overflow::Hidden)
                .overflow_y(Overflow::Hidden)
        })
        .semantics(Some(Box::new(Semantics {
            drag_capture: Some(DragCapture::Press),
            ..Default::default()
        })))
        .on(EventKind::Drag, move |event| {
            if let Event::Drag {
                phase: DragPhase::Drop,
                x,
                y,
                ..
            } = event
            {
                callback.call(if horizontal { y } else { x });
            }
        })
        .show_in(scope)?;
    let child = scope.child_in(node.get().ok_or(glory::ui::Error::Gone)?)?;
    for (screen, value, major) in compute_ticks(length, scroll, zoom) {
        let mark = if major { RULER_SIZE } else { RULER_SIZE * 0.5 };
        view()
            .style(Style {
                background: Some(Color(102, 102, 102, 255)),
                ..Style::new()
                    .position(Position::Absolute)
                    .left(Length::Px(if horizontal {
                        screen
                    } else {
                        RULER_SIZE - mark
                    }))
                    .top(Length::Px(if horizontal {
                        RULER_SIZE - mark
                    } else {
                        screen
                    }))
                    .width(Length::Px(if horizontal { 1.0 } else { mark }))
                    .height(Length::Px(if horizontal { mark } else { 1.0 }))
            })
            .show_in(&child)?;
        if major {
            text()
                .content(format!("{}", value as i64))
                .style(Style {
                    font_size: Some(9.0),
                    color: Some(Color(170, 170, 170, 255)),
                    ..Style::new()
                        .position(Position::Absolute)
                        .left(Length::Px(if horizontal { screen + 2.0 } else { 1.0 }))
                        .top(Length::Px(if horizontal { 1.0 } else { screen + 2.0 }))
                })
                .show_in(&child)?;
        }
    }
    view()
        .style(cursor.map(move |(x, y)| {
            Style {
                background: Some(Color(255, 68, 68, 255)),
                ..Style::new()
                    .position(Position::Absolute)
                    .left(Length::Px(if horizontal { x * zoom + scroll } else { 0.0 }))
                    .top(Length::Px(if horizontal { 0.0 } else { y * zoom + scroll }))
                    .width(Length::Px(if horizontal { 1.0 } else { RULER_SIZE }))
                    .height(Length::Px(if horizontal { RULER_SIZE } else { 1.0 }))
            }
        }))
        .show_in(&child)?;
    scope.retain(child)
}

/// Compute a nice tick interval (in document units) based on the current
/// zoom level. Aims for roughly 50-100 screen pixels between major ticks.
fn compute_tick_interval(zoom: f64) -> f64 {
    let target_screen_pixels = 10.0;
    let raw = target_screen_pixels / zoom;

    // Snap to a "nice" value: 1, 2, 5, 10, 20, 50, 100, ...
    let magnitude = 10.0_f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    nice * magnitude
}
