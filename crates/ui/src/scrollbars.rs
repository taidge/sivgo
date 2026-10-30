//! Scrollbar components (horizontal and vertical) that reflect the viewport
//! position relative to the document extents.

use crate::Callback;
use glory::prelude::*;

/// Minimum thumb size in pixels to keep it visible and clickable.
const MIN_THUMB_SIZE: f64 = 20.0;

/// Scrollbar thickness in pixels.
const SCROLLBAR_SIZE: f64 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScrollbarMetrics {
    thumb_size: f64,
    thumb_pos: f64,
    thumb_size_pct: f64,
    thumb_pos_pct: f64,
}

fn scrollbar_metrics(
    track_length: f64,
    document_length: f64,
    pan: f64,
    zoom: f64,
) -> ScrollbarMetrics {
    let track_length = track_length.max(1.0);
    let zoom = zoom.max(0.000_001);
    let visible_length = track_length / zoom;
    let total = document_length.max(visible_length);
    let thumb_ratio = (visible_length / total).min(1.0);
    let thumb_size = (thumb_ratio * track_length)
        .max(MIN_THUMB_SIZE)
        .min(track_length);
    let available_track = (track_length - thumb_size).max(0.0);
    let doc_offset = -pan / zoom;
    let scroll_ratio = if total > visible_length {
        (doc_offset / (total - visible_length)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_pos = scroll_ratio * available_track;
    ScrollbarMetrics {
        thumb_size,
        thumb_pos,
        thumb_size_pct: thumb_size / track_length * 100.0,
        thumb_pos_pct: thumb_pos / track_length * 100.0,
    }
}

/// Compute the visible length (in document coords) and the maximum document
/// offset for a scrollbar axis. Shared by both scrollbar components to derive
/// the click-to-scroll target.
fn scroll_extents(track_length: f64, document_length: f64, zoom: f64) -> (f64, f64, f64) {
    let visible_length = track_length.max(1.0) / zoom.max(0.000_001);
    let total = document_length.max(visible_length);
    let max_doc_offset = (total - visible_length).max(0.0);
    (visible_length, total, max_doc_offset)
}

#[cfg(test)]
fn horizontal_track_style(bar_h: f64) -> String {
    format!(
        "position: relative; width: 100%; height: {bar_h}px; background: #1e1e1e; border-top: 1px solid #444;"
    )
}

#[cfg(test)]
fn vertical_track_style(bar_w: f64) -> String {
    format!(
        "position: relative; width: {bar_w}px; height: 100%; background: #1e1e1e; border-left: 1px solid #444;"
    )
}

/// A horizontal scrollbar linked to the viewport pan.
///
/// Shows a thumb whose position and size are proportional to the viewport
/// width relative to the full document width.
pub struct HorizontalScrollbar {
    pub track_length: f64,
    pub document_width: f64,
    pub pan_x: f64,
    pub zoom: f64,
    pub on_scroll: Callback<f64>,
}
pub struct VerticalScrollbar {
    pub track_length: f64,
    pub document_height: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub on_scroll: Callback<f64>,
}
impl Widget for HorizontalScrollbar {
    fn build(&mut self, scope: &Scope) -> Result {
        scrollbar(
            scope,
            true,
            self.track_length,
            self.document_width,
            self.pan_x,
            self.zoom,
            self.on_scroll.clone(),
        )
    }
}
impl Widget for VerticalScrollbar {
    fn build(&mut self, scope: &Scope) -> Result {
        scrollbar(
            scope,
            false,
            self.track_length,
            self.document_height,
            self.pan_y,
            self.zoom,
            self.on_scroll.clone(),
        )
    }
}
fn scrollbar(
    scope: &Scope,
    horizontal: bool,
    length: f64,
    document: f64,
    pan: f64,
    zoom: f64,
    callback: Callback<f64>,
) -> Result {
    use glory::render::{DragCapture, DragPhase, EventKind, Query, QueryValue, Semantics};
    use glory::style::{Color, Position};
    let length = length.max(1.0);
    let zoom = zoom.max(0.000001);
    let metrics = scrollbar_metrics(length, document, pan, zoom);
    let preview = scope.cage(metrics);
    let request_serial = std::rc::Rc::new(std::cell::Cell::new(0_u64));
    let owner = scope.downgrade();
    let node = scope.cage(None);
    view()
        .class(if horizontal {
            "scrollbar scrollbar-horizontal"
        } else {
            "scrollbar scrollbar-vertical"
        })
        .name(if horizontal {
            "Horizontal canvas scrollbar"
        } else {
            "Vertical canvas scrollbar"
        })
        .node_ref(node)
        .style(Style {
            background: Some(Color(30, 30, 30, 255)),
            ..Style::new()
                .position(Position::Relative)
                .width(if horizontal {
                    Length::Percent(100.0)
                } else {
                    Length::Px(SCROLLBAR_SIZE)
                })
                .height(if horizontal {
                    Length::Px(SCROLLBAR_SIZE)
                } else {
                    Length::Percent(100.0)
                })
        })
        .semantics(Some(Box::new(Semantics {
            drag_capture: Some(DragCapture::Press),
            ..Default::default()
        })))
        .on(EventKind::Drag, move |event| {
            if let Event::Drag { phase, x, y, .. } = event {
                let serial = request_serial.get().wrapping_add(1);
                request_serial.set(serial);
                if phase == DragPhase::Cancel {
                    preview.revise(|value| *value = metrics);
                    return;
                }
                let (Some(scope), Some(node)) = (owner.upgrade(), node.get()) else {
                    return;
                };
                let callback = callback.clone();
                let request_serial = request_serial.clone();
                // Pointer coordinates are viewport-relative. Query the actual
                // track box instead of assuming that the track starts at 0.
                let result = scope.request(Query::Bounds(node), move |reply| {
                    if request_serial.get() != serial {
                        return;
                    }
                    let Ok(QueryValue::Bounds(bounds)) = reply else {
                        return;
                    };
                    let (position, actual_length) = if horizontal {
                        (x - bounds.x, bounds.width)
                    } else {
                        (y - bounds.y, bounds.height)
                    };
                    if actual_length <= 0.0 {
                        return;
                    }
                    let (visible, total, max_offset) =
                        scroll_extents(actual_length, document, zoom);
                    let offset =
                        (position / actual_length * total - visible / 2.0).clamp(0.0, max_offset);
                    let next_pan = -offset * zoom;
                    preview.revise(|value| {
                        *value = scrollbar_metrics(actual_length, document, next_pan, zoom)
                    });
                    // Keep the pointer's owner mounted during the gesture; the
                    // editor can rebuild its snapshot after the release.
                    if phase == DragPhase::Drop {
                        callback.call(next_pan);
                    }
                });
                if let Err(error) = result {
                    scope.report_error(error);
                }
            }
        })
        .show_in(scope)?;
    let child = scope.child_in(node.get().ok_or(glory::ui::Error::Gone)?)?;
    view()
        .style(preview.map(move |metrics| {
            Style {
                background: Some(Color(100, 100, 100, 255)),
                ..Style::new()
                    .position(Position::Absolute)
                    .radius(3.0)
                    .left(if horizontal {
                        Length::Percent(metrics.thumb_pos_pct)
                    } else {
                        Length::Px(2.0)
                    })
                    .top(if horizontal {
                        Length::Px(2.0)
                    } else {
                        Length::Percent(metrics.thumb_pos_pct)
                    })
                    .width(if horizontal {
                        Length::Percent(metrics.thumb_size_pct)
                    } else {
                        Length::Px(SCROLLBAR_SIZE - 4.0)
                    })
                    .height(if horizontal {
                        Length::Px(SCROLLBAR_SIZE - 4.0)
                    } else {
                        Length::Percent(metrics.thumb_size_pct)
                    })
            }
        }))
        .show_in(&child)?;
    scope.retain(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_styles_stretch_to_grid_slot() {
        assert!(horizontal_track_style(SCROLLBAR_SIZE).contains("width: 100%"));
        assert!(vertical_track_style(SCROLLBAR_SIZE).contains("height: 100%"));
    }

    #[test]
    fn scrollbar_metrics_clamp_thumb_to_track() {
        let metrics = scrollbar_metrics(12.0, 500.0, 0.0, 1.0);
        assert_eq!(metrics.thumb_size, 12.0);
        assert_eq!(metrics.thumb_pos, 0.0);
        assert_eq!(metrics.thumb_size_pct, 100.0);
        assert_eq!(metrics.thumb_pos_pct, 0.0);
    }

    #[test]
    fn scrollbar_metrics_track_pan_position() {
        let metrics = scrollbar_metrics(100.0, 200.0, -50.0, 1.0);
        assert!((metrics.thumb_size - 50.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_pos - 25.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_size_pct - 50.0).abs() < f64::EPSILON);
        assert!((metrics.thumb_pos_pct - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_metrics_activate_when_viewport_is_narrower_than_document() {
        let metrics = scrollbar_metrics(600.0, 800.0, 0.0, 1.0);
        assert!((metrics.thumb_size - 450.0).abs() < f64::EPSILON);
        assert_eq!(metrics.thumb_pos, 0.0);
        assert!((metrics.thumb_size_pct - 75.0).abs() < f64::EPSILON);

        let scrolled = scrollbar_metrics(600.0, 800.0, -200.0, 1.0);
        assert!((scrolled.thumb_pos - 150.0).abs() < f64::EPSILON);
        assert!((scrolled.thumb_pos_pct - 25.0).abs() < f64::EPSILON);
    }
}
