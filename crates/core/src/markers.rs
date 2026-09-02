//! Built-in marker library providing common arrow and dot markers.
//!
//! These markers can be inserted into the document's `<defs>` and referenced
//! by `marker-start`, `marker-mid`, or `marker-end` attributes.

use sivgo_dom::{CircleAttrs, MarkerAttrs, MarkerUnits, Orient, PathAttrs, SvgElement, SvgLength};
use sivgo_math::{BezPath, Point};

/// A built-in marker definition.
#[derive(Debug, Clone)]
pub struct BuiltinMarker {
    /// The id to use in the `<defs>` section (e.g. "marker-arrow").
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// The marker attributes.
    pub attrs: MarkerAttrs,
    /// The child element(s) to place inside the marker.
    pub children: Vec<SvgElement>,
}

/// Returns a list of all built-in marker definitions.
pub fn builtin_markers() -> Vec<BuiltinMarker> {
    vec![
        arrow_marker(),
        arrow_open_marker(),
        arrow_wide_marker(),
        dot_marker(),
        dot_open_marker(),
        square_marker(),
        diamond_marker(),
        triangle_marker(),
        triangle_open_marker(),
    ]
}

/// Standard filled arrowhead.
fn arrow_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(0.0, 0.0));
    path.line_to(Point::new(10.0, 5.0));
    path.line_to(Point::new(0.0, 10.0));
    path.close();

    BuiltinMarker {
        id: "marker-arrow",
        name: "Arrow",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(10.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Open arrowhead (no fill, stroked).
fn arrow_open_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(0.0, 0.0));
    path.line_to(Point::new(10.0, 5.0));
    path.line_to(Point::new(0.0, 10.0));

    BuiltinMarker {
        id: "marker-arrow-open",
        name: "Arrow (open)",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(10.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Wide arrowhead.
fn arrow_wide_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(0.0, 0.0));
    path.line_to(Point::new(10.0, 5.0));
    path.line_to(Point::new(0.0, 10.0));
    path.line_to(Point::new(3.0, 5.0));
    path.close();

    BuiltinMarker {
        id: "marker-arrow-wide",
        name: "Arrow (wide)",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(10.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Filled dot marker.
fn dot_marker() -> BuiltinMarker {
    BuiltinMarker {
        id: "marker-dot",
        name: "Dot",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(5.0),
            cy: SvgLength::px(5.0),
            r: SvgLength::px(4.0),
        })],
    }
}

/// Open dot marker (stroked circle).
fn dot_open_marker() -> BuiltinMarker {
    BuiltinMarker {
        id: "marker-dot-open",
        name: "Dot (open)",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Circle(CircleAttrs {
            cx: SvgLength::px(5.0),
            cy: SvgLength::px(5.0),
            r: SvgLength::px(4.0),
        })],
    }
}

/// Square marker.
fn square_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(1.0, 1.0));
    path.line_to(Point::new(9.0, 1.0));
    path.line_to(Point::new(9.0, 9.0));
    path.line_to(Point::new(1.0, 9.0));
    path.close();

    BuiltinMarker {
        id: "marker-square",
        name: "Square",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Diamond marker.
fn diamond_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(5.0, 0.0));
    path.line_to(Point::new(10.0, 5.0));
    path.line_to(Point::new(5.0, 10.0));
    path.line_to(Point::new(0.0, 5.0));
    path.close();

    BuiltinMarker {
        id: "marker-diamond",
        name: "Diamond",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Filled triangle marker.
fn triangle_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(5.0, 0.0));
    path.line_to(Point::new(10.0, 10.0));
    path.line_to(Point::new(0.0, 10.0));
    path.close();

    BuiltinMarker {
        id: "marker-triangle",
        name: "Triangle",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

/// Open triangle marker.
fn triangle_open_marker() -> BuiltinMarker {
    let mut path = BezPath::new();
    path.move_to(Point::new(5.0, 0.0));
    path.line_to(Point::new(10.0, 10.0));
    path.line_to(Point::new(0.0, 10.0));
    path.close();

    BuiltinMarker {
        id: "marker-triangle-open",
        name: "Triangle (open)",
        attrs: MarkerAttrs {
            ref_x: Some(SvgLength::px(5.0)),
            ref_y: Some(SvgLength::px(5.0)),
            marker_width: Some(SvgLength::px(10.0)),
            marker_height: Some(SvgLength::px(10.0)),
            orient: Some(Orient::Auto),
            marker_units: Some(MarkerUnits::StrokeWidth),
            view_box: None,
            preserve_aspect_ratio: None,
        },
        children: vec![SvgElement::Path(PathAttrs {
            d: path,
            ..Default::default()
        })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_markers_have_unique_ids() {
        let markers = builtin_markers();
        let mut ids: Vec<&str> = markers.iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), markers.len(), "Duplicate marker IDs found");
    }

    #[test]
    fn all_markers_have_children() {
        for m in builtin_markers() {
            assert!(!m.children.is_empty(), "Marker {} has no children", m.id);
        }
    }
}
