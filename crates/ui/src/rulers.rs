//! Ruler components (horizontal and vertical) that display tick marks and
//! the current cursor position indicator.

use dioxus::prelude::*;

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
#[component]
pub fn HorizontalRuler(
    /// Width of the ruler in pixels.
    width: f64,
    /// Current viewport scroll offset in the X direction (in document coords).
    scroll_x: f64,
    /// Current zoom factor.
    zoom: f64,
    /// Current cursor position in document coordinates.
    cursor_position: Signal<(f64, f64)>,
    /// Called when the user drags from the ruler to create a guide.
    /// The value is the Y position in document coordinates where the guide
    /// should be placed.
    on_guide_drag: EventHandler<f64>,
) -> Element {
    let w = width as i64;
    let ruler_h = RULER_SIZE;
    let (cursor_x, _) = *cursor_position.read();
    let cursor_screen_x = cursor_x * zoom + scroll_x;

    // Collect tick marks (screen_x, doc_value, is_major).
    let ticks = compute_ticks(width, scroll_x, zoom);

    rsx! {
        div {
            class: "ruler ruler-horizontal",
            style: "position: relative; width: {w}px; height: {ruler_h}px; background: #2a2a2a; border-bottom: 1px solid #444; overflow: hidden; user-select: none;",
            onmousedown: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let doc_y = coords.y;
                on_guide_drag.call(doc_y);
            },

            // Tick marks
            for (screen_x, doc_val, is_major) in ticks.iter() {
                {
                    let tick_height = if *is_major { ruler_h } else { ruler_h * 0.5 };
                    let top = ruler_h - tick_height;
                    let sx = *screen_x;
                    rsx! {
                        div {
                            key: "{doc_val}",
                            style: "position: absolute; left: {sx}px; top: {top}px; width: 1px; height: {tick_height}px; background: #666;",
                        }
                        if *is_major {
                            {
                                let label = format!("{}", *doc_val as i64);
                                rsx! {
                                    span {
                                        style: "position: absolute; left: {sx}px; top: 1px; font-size: 9px; color: #999; margin-left: 2px; white-space: nowrap;",
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Cursor position indicator
            div {
                style: "position: absolute; left: {cursor_screen_x}px; top: 0; width: 1px; height: {ruler_h}px; background: #ff4444;",
            }
        }
    }
}

/// A vertical ruler displayed to the left of the canvas.
///
/// Shows tick marks at regular intervals based on zoom level, with a
/// cursor position indicator line. Supports dragging from the ruler to
/// create vertical guides.
#[component]
pub fn VerticalRuler(
    /// Height of the ruler in pixels.
    height: f64,
    /// Current viewport scroll offset in the Y direction (in document coords).
    scroll_y: f64,
    /// Current zoom factor.
    zoom: f64,
    /// Current cursor position in document coordinates.
    cursor_position: Signal<(f64, f64)>,
    /// Called when the user drags from the ruler to create a guide.
    /// The value is the X position in document coordinates where the guide
    /// should be placed.
    on_guide_drag: EventHandler<f64>,
) -> Element {
    let h = height as i64;
    let ruler_w = RULER_SIZE;
    let (_, cursor_y) = *cursor_position.read();
    let cursor_screen_y = cursor_y * zoom + scroll_y;

    // Collect tick marks (screen_y, doc_value, is_major).
    let ticks = compute_ticks(height, scroll_y, zoom);

    rsx! {
        div {
            class: "ruler ruler-vertical",
            style: "position: relative; width: {ruler_w}px; height: {h}px; background: #2a2a2a; border-right: 1px solid #444; overflow: hidden; user-select: none;",
            onmousedown: move |evt: Event<dioxus::events::MouseData>| {
                let coords = evt.data().element_coordinates();
                let doc_x = coords.x;
                on_guide_drag.call(doc_x);
            },

            // Tick marks
            for (screen_y, doc_val, is_major) in ticks.iter() {
                {
                    let tick_width = if *is_major { ruler_w } else { ruler_w * 0.5 };
                    let left = ruler_w - tick_width;
                    let sy = *screen_y;
                    rsx! {
                        div {
                            key: "{doc_val}",
                            style: "position: absolute; left: {left}px; top: {sy}px; width: {tick_width}px; height: 1px; background: #666;",
                        }
                        if *is_major {
                            {
                                let label = format!("{}", *doc_val as i64);
                                rsx! {
                                    span {
                                        style: "position: absolute; left: 1px; top: {sy}px; font-size: 9px; color: #999; writing-mode: vertical-lr; white-space: nowrap; margin-top: 2px;",
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Cursor position indicator
            div {
                style: "position: absolute; left: 0; top: {cursor_screen_y}px; width: {ruler_w}px; height: 1px; background: #ff4444;",
            }
        }
    }
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
