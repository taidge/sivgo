//! Snapping engine for the editor.
//!
//! Implements grid, guide, bounding-box, node, path, midpoint, intersection,
//! and smart distribution snapping. Each snap type can be individually
//! enabled/disabled through [`SnapPreferences`].

use sivgo_math::{BezPath, Point, Rect, Vector};

use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// SnapTarget -- what was snapped to
// ---------------------------------------------------------------------------

/// Describes the kind of geometry that a point snapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTarget {
    /// Snapped to the document grid.
    Grid,
    /// Snapped to a guide line.
    Guide,
    /// Snapped to a bounding-box edge (top, right, bottom, or left).
    BBoxEdge,
    /// Snapped to a bounding-box center.
    BBoxCenter,
    /// Snapped to a bounding-box corner.
    BBoxCorner,
    /// Snapped to a path node (endpoint / on-curve point).
    PathNode,
    /// Snapped to the closest point on a path.
    PathPoint,
    /// Snapped to an intersection of edges / paths.
    Intersection,
    /// Snapped to the midpoint of a bounding-box edge.
    Midpoint,
    /// Snapped so that equal spacing is maintained among neighbours.
    EqualSpacing,
}

/// Describes which source point initiated a snap pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSourceKind {
    /// Raw cursor / tool point.
    Cursor,
    /// Explicit object/path node.
    ObjectNode,
    /// Object bounding-box corner.
    BBoxCorner,
    /// Object bounding-box center.
    BBoxCenter,
    /// Object bounding-box edge midpoint.
    BBoxMidpoint,
}

/// A candidate source point that can be snapped to enabled targets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapSource {
    pub point: Point,
    pub kind: SnapSourceKind,
}

impl SnapSource {
    pub fn new(point: Point, kind: SnapSourceKind) -> Self {
        Self { point, kind }
    }
}

// ---------------------------------------------------------------------------
// SnapResult
// ---------------------------------------------------------------------------

/// Result of a snap operation.
#[derive(Debug, Clone, Copy)]
pub struct SnapResult {
    /// The snapped point.
    pub point: Point,
    /// Whether the x coordinate was snapped.
    pub snapped_x: bool,
    /// Whether the y coordinate was snapped.
    pub snapped_y: bool,
    /// The distance between the original and snapped point.
    pub distance: f64,
    /// What kind of geometry the point snapped to (if any).
    pub target: Option<SnapTarget>,
    /// An optional indicator line to draw (from, to).
    pub indicator_line: Option<(Point, Point)>,
}

/// Result of snapping a specific source point.
#[derive(Debug, Clone, Copy)]
pub struct SourcedSnapResult {
    pub source: SnapSource,
    pub snap: SnapResult,
}

impl SnapResult {
    /// Create a no-snap result that simply returns the original point.
    fn no_snap(point: Point) -> Self {
        Self {
            point,
            snapped_x: false,
            snapped_y: false,
            distance: 0.0,
            target: None,
            indicator_line: None,
        }
    }
}

/// A guide line represented by one point on the line and a unit normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapGuide {
    /// Any point on the guide line, in document coordinates.
    pub origin: Point,
    /// Unit normal to the guide line, in document coordinates.
    pub normal: Vector,
}

impl SnapGuide {
    /// Build a guide from an origin and normal. Returns `None` for invalid
    /// zero-length or non-finite normals.
    pub fn new(origin: Point, normal: Vector) -> Option<Self> {
        let length = normal.length();
        if !origin.x.is_finite() || !origin.y.is_finite() || !length.is_finite() || length <= 1e-12
        {
            return None;
        }
        Some(Self {
            origin,
            normal: normal / length,
        })
    }

    /// Horizontal visual guide that constrains the y coordinate.
    pub fn horizontal(y: f64) -> Self {
        Self {
            origin: Point::new(0.0, y),
            normal: Vector::new(0.0, 1.0),
        }
    }

    /// Vertical visual guide that constrains the x coordinate.
    pub fn vertical(x: f64) -> Self {
        Self {
            origin: Point::new(x, 0.0),
            normal: Vector::new(1.0, 0.0),
        }
    }

    /// Return the closest point on this guide to `point` plus the absolute
    /// perpendicular distance.
    pub fn project(self, point: Point) -> (Point, f64) {
        let signed = (point - self.origin).dot(self.normal);
        (point - self.normal * signed, signed.abs())
    }

    /// Direction vector along the guide line.
    pub fn direction(self) -> Vector {
        self.normal.perp()
    }
}

// ---------------------------------------------------------------------------
// SnapPreferences
// ---------------------------------------------------------------------------

/// Per-type enable/disable flags and tolerance for snapping.
#[derive(Debug, Clone)]
pub struct SnapPreferences {
    /// Snap to the document grid.
    pub snap_to_grid: bool,
    /// Snap to guide lines.
    pub snap_to_guides: bool,
    /// Snap to bounding-box edges.
    pub snap_to_bbox_edges: bool,
    /// Snap to bounding-box centers.
    pub snap_to_bbox_centers: bool,
    /// Snap to bounding-box corners.
    pub snap_to_bbox_corners: bool,
    /// Snap to path nodes (endpoints, control points).
    pub snap_to_path_nodes: bool,
    /// Snap to the closest point on a path.
    pub snap_to_path: bool,
    /// Snap to intersections where edges / paths cross.
    pub snap_to_intersections: bool,
    /// Snap to midpoints of bounding-box edges.
    pub snap_to_midpoints: bool,
    /// Snap to positions that create equal spacing between neighbours.
    pub snap_to_equal_spacing: bool,
    /// Maximum snap distance in screen pixels (default 10.0).
    pub tolerance: f64,
    /// Grid spacing in document units.
    pub grid_spacing: f64,
    /// Which object/cursor source points can initiate snapping.
    pub sources: SnapSourcePreferences,
}

/// Per-source enable/disable flags for source-aware snapping.
#[derive(Debug, Clone)]
pub struct SnapSourcePreferences {
    pub snap_from_cursor: bool,
    pub snap_from_object_nodes: bool,
    pub snap_from_bbox_corners: bool,
    pub snap_from_bbox_centers: bool,
    pub snap_from_bbox_midpoints: bool,
}

impl Default for SnapPreferences {
    fn default() -> Self {
        Self {
            snap_to_grid: true,
            snap_to_guides: true,
            snap_to_bbox_edges: true,
            snap_to_bbox_centers: true,
            snap_to_bbox_corners: true,
            snap_to_path_nodes: true,
            snap_to_path: true,
            snap_to_intersections: true,
            snap_to_midpoints: true,
            snap_to_equal_spacing: true,
            tolerance: 10.0,
            grid_spacing: 10.0,
            sources: SnapSourcePreferences::default(),
        }
    }
}

impl Default for SnapSourcePreferences {
    fn default() -> Self {
        Self {
            snap_from_cursor: true,
            snap_from_object_nodes: true,
            snap_from_bbox_corners: true,
            snap_from_bbox_centers: true,
            snap_from_bbox_midpoints: true,
        }
    }
}

impl SnapPreferences {
    /// Returns `true` if *any* snap type is enabled.
    pub fn any_enabled(&self) -> bool {
        self.snap_to_grid
            || self.snap_to_guides
            || self.snap_to_bbox_edges
            || self.snap_to_bbox_centers
            || self.snap_to_bbox_corners
            || self.snap_to_path_nodes
            || self.snap_to_path
            || self.snap_to_intersections
            || self.snap_to_midpoints
            || self.snap_to_equal_spacing
    }

    /// Returns `true` if this source kind is currently enabled.
    pub fn source_enabled(&self, kind: SnapSourceKind) -> bool {
        match kind {
            SnapSourceKind::Cursor => self.sources.snap_from_cursor,
            SnapSourceKind::ObjectNode => self.sources.snap_from_object_nodes,
            SnapSourceKind::BBoxCorner => self.sources.snap_from_bbox_corners,
            SnapSourceKind::BBoxCenter => self.sources.snap_from_bbox_centers,
            SnapSourceKind::BBoxMidpoint => self.sources.snap_from_bbox_midpoints,
        }
    }
}

// ---------------------------------------------------------------------------
// SnapIndicator  --  visual feedback description
// ---------------------------------------------------------------------------

/// Describes what to draw on the canvas to indicate a snap.
#[derive(Debug, Clone)]
pub struct SnapIndicator {
    /// Crosshair lines to draw (pairs of endpoints).
    pub crosshair_lines: Vec<(Point, Point)>,
    /// Optional distance label text and where to place it.
    pub distance_label: Option<(String, Point)>,
    /// The snap target type (for colour coding in the UI).
    pub target: SnapTarget,
}

// ---------------------------------------------------------------------------
// SnapContext  --  all data needed for a snap pass
// ---------------------------------------------------------------------------

/// Collects all the scene data that the snapping engine needs.
///
/// Callers fill this in before calling [`SnappingEngine::snap_all`].
#[derive(Debug, Clone, Default)]
pub struct SnapContext {
    /// Guide lines.
    pub guides: Vec<SnapGuide>,
    /// Bounding boxes of other objects on the canvas.
    pub bboxes: Vec<Rect>,
    /// On-curve / endpoint nodes from paths.
    pub nodes: Vec<Point>,
    /// Bezier paths to snap to.
    pub paths: Vec<BezPath>,
    /// Intersection points where edges or paths cross.
    pub intersections: Vec<Point>,
    /// Sorted X-coordinates of neighbour centres (for equal-spacing).
    pub neighbor_centers_x: Vec<f64>,
    /// Sorted Y-coordinates of neighbour centres (for equal-spacing).
    pub neighbor_centers_y: Vec<f64>,
}

// ---------------------------------------------------------------------------
// SnappingEngine
// ---------------------------------------------------------------------------

/// Configuration and logic for snapping points to grids, guides, and objects.
#[derive(Debug, Clone)]
pub struct SnappingEngine {
    /// Per-type snap preferences.
    pub prefs: SnapPreferences,
}

impl SnappingEngine {
    /// Create a new snapping engine with reasonable defaults.
    pub fn new() -> Self {
        Self {
            prefs: SnapPreferences::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Individual snap methods
    // -----------------------------------------------------------------------

    /// 9.4.3  Snap to guide lines.
    pub fn snap_to_guides(
        &self,
        point: Point,
        guides: &[SnapGuide],
        doc_tol: f64,
    ) -> Option<SnapResult> {
        if !self.prefs.snap_to_guides || guides.is_empty() {
            return None;
        }
        let mut best: Option<SnapResult> = None;
        for &guide in guides {
            let (snapped, dist) = guide.project(point);
            if dist > doc_tol {
                continue;
            }
            let full_dist = snapped.distance(point);
            if best.as_ref().is_none_or(|b| full_dist < b.distance) {
                let direction = guide.direction();
                let indicator = Some((snapped - direction * 20.0, snapped + direction * 20.0));
                best = Some(SnapResult {
                    point: snapped,
                    snapped_x: (snapped.x - point.x).abs() > 1e-12,
                    snapped_y: (snapped.y - point.y).abs() > 1e-12,
                    distance: full_dist,
                    target: Some(SnapTarget::Guide),
                    indicator_line: indicator,
                });
            }
        }
        best
    }

    /// 9.4.4 / 9.4.7  Snap to bounding-box edges, centers, corners, and
    /// midpoints.
    pub fn snap_to_bboxes(
        &self,
        point: Point,
        bboxes: &[Rect],
        doc_tol: f64,
    ) -> Option<SnapResult> {
        if bboxes.is_empty() {
            return None;
        }

        let mut best: Option<SnapResult> = None;

        let mut consider = |candidate: Point, target: SnapTarget| {
            let d = candidate.distance(point);
            if d <= doc_tol && best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(SnapResult {
                    point: candidate,
                    snapped_x: (candidate.x - point.x).abs() > 1e-12,
                    snapped_y: (candidate.y - point.y).abs() > 1e-12,
                    distance: d,
                    target: Some(target),
                    indicator_line: Some((point, candidate)),
                });
            }
        };

        for bbox in bboxes {
            // -- corners (9.4.4) --
            if self.prefs.snap_to_bbox_corners {
                consider(Point::new(bbox.x0, bbox.y0), SnapTarget::BBoxCorner);
                consider(Point::new(bbox.x1, bbox.y0), SnapTarget::BBoxCorner);
                consider(Point::new(bbox.x0, bbox.y1), SnapTarget::BBoxCorner);
                consider(Point::new(bbox.x1, bbox.y1), SnapTarget::BBoxCorner);
            }

            // -- center (9.4.4) --
            if self.prefs.snap_to_bbox_centers {
                consider(bbox.center(), SnapTarget::BBoxCenter);
            }

            // -- edges: nearest point on each edge (9.4.4) --
            if self.prefs.snap_to_bbox_edges {
                let cx = point.x.clamp(bbox.x0, bbox.x1);
                let cy = point.y.clamp(bbox.y0, bbox.y1);
                // top edge
                consider(Point::new(cx, bbox.y0), SnapTarget::BBoxEdge);
                // bottom edge
                consider(Point::new(cx, bbox.y1), SnapTarget::BBoxEdge);
                // left edge
                consider(Point::new(bbox.x0, cy), SnapTarget::BBoxEdge);
                // right edge
                consider(Point::new(bbox.x1, cy), SnapTarget::BBoxEdge);
            }

            // -- midpoints of edges (9.4.7) --
            if self.prefs.snap_to_midpoints {
                let mx = 0.5 * (bbox.x0 + bbox.x1);
                let my = 0.5 * (bbox.y0 + bbox.y1);
                // top-edge midpoint
                consider(Point::new(mx, bbox.y0), SnapTarget::Midpoint);
                // bottom-edge midpoint
                consider(Point::new(mx, bbox.y1), SnapTarget::Midpoint);
                // left-edge midpoint
                consider(Point::new(bbox.x0, my), SnapTarget::Midpoint);
                // right-edge midpoint
                consider(Point::new(bbox.x1, my), SnapTarget::Midpoint);
            }
        }

        best
    }

    /// 9.4.5  Snap to explicit nodes (path endpoints, control points).
    pub fn snap_to_nodes(&self, point: Point, nodes: &[Point], doc_tol: f64) -> Option<SnapResult> {
        if !self.prefs.snap_to_path_nodes || nodes.is_empty() {
            return None;
        }
        let mut best: Option<SnapResult> = None;
        for &node in nodes {
            let d = node.distance(point);
            if d <= doc_tol && best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(SnapResult {
                    point: node,
                    snapped_x: (node.x - point.x).abs() > 1e-12,
                    snapped_y: (node.y - point.y).abs() > 1e-12,
                    distance: d,
                    target: Some(SnapTarget::PathNode),
                    indicator_line: Some((point, node)),
                });
            }
        }
        best
    }

    /// 9.4.8  Snap to the closest point on a Bezier path.
    pub fn snap_to_path(&self, point: Point, path: &BezPath, doc_tol: f64) -> Option<SnapResult> {
        if !self.prefs.snap_to_path {
            return None;
        }
        let accuracy = 0.5;
        let (_, dist_sq) = path.nearest(point, accuracy)?;
        let dist = dist_sq.sqrt();
        if dist > doc_tol {
            return None;
        }
        // Re-derive the actual nearest point by evaluating at the parameter.
        let (t, _) = path.nearest(point, accuracy)?;
        let snapped = path.eval(t)?;
        Some(SnapResult {
            point: snapped,
            snapped_x: (snapped.x - point.x).abs() > 1e-12,
            snapped_y: (snapped.y - point.y).abs() > 1e-12,
            distance: dist,
            target: Some(SnapTarget::PathPoint),
            indicator_line: Some((point, snapped)),
        })
    }

    /// 9.4.6  Snap to pre-computed intersection points.
    pub fn snap_to_intersections(
        &self,
        point: Point,
        intersections: &[Point],
        doc_tol: f64,
    ) -> Option<SnapResult> {
        if !self.prefs.snap_to_intersections || intersections.is_empty() {
            return None;
        }
        let mut best: Option<SnapResult> = None;
        for &ix in intersections {
            let d = ix.distance(point);
            if d <= doc_tol && best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(SnapResult {
                    point: ix,
                    snapped_x: (ix.x - point.x).abs() > 1e-12,
                    snapped_y: (ix.y - point.y).abs() > 1e-12,
                    distance: d,
                    target: Some(SnapTarget::Intersection),
                    indicator_line: Some((point, ix)),
                });
            }
        }
        best
    }

    /// 9.4.9  Smart distribution snapping (equal spacing).
    ///
    /// Given sorted centres of neighbouring objects along each axis, this
    /// checks whether the cursor position would create an equal gap between
    /// consecutive neighbours.  If so, the snapped position is returned.
    pub fn snap_to_equal_spacing(
        &self,
        point: Point,
        centers_x: &[f64],
        centers_y: &[f64],
        doc_tol: f64,
    ) -> Option<SnapResult> {
        if !self.prefs.snap_to_equal_spacing {
            return None;
        }

        let mut best: Option<SnapResult> = None;

        // Check one axis at a time.
        let check_axis = |sorted: &[f64], val: f64| -> Option<(f64, f64)> {
            if sorted.len() < 2 {
                return None;
            }
            // For each consecutive pair, compute the gap and see if placing
            // `val` at a position that extends the pattern would work.
            // Strategy: find the gap between the first two sorted values,
            // then test val == sorted.last() + gap and val == sorted.first() - gap.
            let gap = sorted[1] - sorted[0];
            if gap.abs() < 1e-12 {
                return None;
            }
            // Check if all existing gaps are equal.
            let uniform = sorted
                .windows(2)
                .all(|w| ((w[1] - w[0]) - gap).abs() < 1e-6);
            if !uniform {
                return None;
            }
            // Candidate: after the last element.
            let after = sorted[sorted.len() - 1] + gap;
            if (val - after).abs() <= doc_tol {
                return Some((after, (val - after).abs()));
            }
            // Candidate: before the first element.
            let before = sorted[0] - gap;
            if (val - before).abs() <= doc_tol {
                return Some((before, (val - before).abs()));
            }
            None
        };

        if let Some((snapped_x, dx)) = check_axis(centers_x, point.x) {
            let candidate = Point::new(snapped_x, point.y);
            let d = candidate.distance(point);
            if d <= doc_tol && best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(SnapResult {
                    point: candidate,
                    snapped_x: true,
                    snapped_y: false,
                    distance: dx,
                    target: Some(SnapTarget::EqualSpacing),
                    indicator_line: None,
                });
            }
        }

        if let Some((snapped_y, dy)) = check_axis(centers_y, point.y) {
            let candidate = Point::new(best.as_ref().map_or(point.x, |b| b.point.x), snapped_y);
            let d = candidate.distance(point);
            if d <= doc_tol && best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(SnapResult {
                    point: candidate,
                    snapped_x: best.as_ref().is_some_and(|b| b.snapped_x),
                    snapped_y: true,
                    distance: dy,
                    target: Some(SnapTarget::EqualSpacing),
                    indicator_line: None,
                });
            }
        }

        best
    }

    // -----------------------------------------------------------------------
    // Grid snap (internal helper matching the new signature)
    // -----------------------------------------------------------------------

    fn snap_grid(&self, point: Point, doc_tol: f64) -> Option<SnapResult> {
        if !self.prefs.snap_to_grid || self.prefs.grid_spacing <= 0.0 {
            return None;
        }
        let gs = self.prefs.grid_spacing;
        let snapped_x = (point.x / gs).round() * gs;
        let snapped_y = (point.y / gs).round() * gs;
        let dx = (snapped_x - point.x).abs();
        let dy = (snapped_y - point.y).abs();

        let sx = dx <= doc_tol;
        let sy = dy <= doc_tol;

        if !sx && !sy {
            return None;
        }

        let p = Point::new(
            if sx { snapped_x } else { point.x },
            if sy { snapped_y } else { point.y },
        );
        Some(SnapResult {
            point: p,
            snapped_x: sx,
            snapped_y: sy,
            distance: p.distance(point),
            target: Some(SnapTarget::Grid),
            indicator_line: None,
        })
    }

    // -----------------------------------------------------------------------
    // Combined snap: tries all enabled types and returns the closest
    // -----------------------------------------------------------------------

    /// Try all enabled snap types against the provided [`SnapContext`] and
    /// return the closest match within tolerance.
    pub fn snap_all(&self, point: Point, context: &SnapContext, viewport: &Viewport) -> SnapResult {
        if !self.prefs.any_enabled() {
            return SnapResult::no_snap(point);
        }

        let doc_tol = self.prefs.tolerance / viewport.zoom;
        let mut candidates: Vec<SnapResult> = Vec::new();

        // Grid
        if let Some(r) = self.snap_grid(point, doc_tol) {
            candidates.push(r);
        }

        // Guides
        if let Some(r) = self.snap_to_guides(point, &context.guides, doc_tol) {
            candidates.push(r);
        }

        // Bounding boxes (edges, corners, centers, midpoints)
        if let Some(r) = self.snap_to_bboxes(point, &context.bboxes, doc_tol) {
            candidates.push(r);
        }

        // Path nodes
        if let Some(r) = self.snap_to_nodes(point, &context.nodes, doc_tol) {
            candidates.push(r);
        }

        // Closest point on paths
        for path in &context.paths {
            if let Some(r) = self.snap_to_path(point, path, doc_tol) {
                candidates.push(r);
            }
        }

        // Intersections
        if let Some(r) = self.snap_to_intersections(point, &context.intersections, doc_tol) {
            candidates.push(r);
        }

        // Equal spacing
        if let Some(r) = self.snap_to_equal_spacing(
            point,
            &context.neighbor_centers_x,
            &context.neighbor_centers_y,
            doc_tol,
        ) {
            candidates.push(r);
        }

        // Pick closest, with deterministic target priority for numerical ties.
        candidates
            .into_iter()
            .min_by(|a, b| {
                let distance_order = a
                    .distance
                    .partial_cmp(&b.distance)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if distance_order != std::cmp::Ordering::Equal
                    && (a.distance - b.distance).abs() > 1e-9
                {
                    distance_order
                } else {
                    Self::target_priority(a.target).cmp(&Self::target_priority(b.target))
                }
            })
            .unwrap_or_else(|| SnapResult::no_snap(point))
    }

    /// Try all enabled snap targets for each enabled source point and return
    /// the best source/target pair.
    pub fn snap_all_sources(
        &self,
        sources: &[SnapSource],
        context: &SnapContext,
        viewport: &Viewport,
    ) -> Option<SourcedSnapResult> {
        sources
            .iter()
            .copied()
            .filter(|source| self.prefs.source_enabled(source.kind))
            .filter_map(|source| {
                let snap = self.snap_all(source.point, context, viewport);
                snap.target
                    .is_some()
                    .then_some(SourcedSnapResult { source, snap })
            })
            .min_by(|a, b| {
                let distance_order = a
                    .snap
                    .distance
                    .partial_cmp(&b.snap.distance)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if distance_order != std::cmp::Ordering::Equal
                    && (a.snap.distance - b.snap.distance).abs() > 1e-9
                {
                    distance_order
                } else {
                    Self::source_priority(a.source.kind)
                        .cmp(&Self::source_priority(b.source.kind))
                        .then_with(|| {
                            Self::target_priority(a.snap.target)
                                .cmp(&Self::target_priority(b.snap.target))
                        })
                }
            })
    }

    fn source_priority(source: SnapSourceKind) -> u8 {
        match source {
            SnapSourceKind::ObjectNode => 0,
            SnapSourceKind::BBoxCorner => 1,
            SnapSourceKind::BBoxCenter => 2,
            SnapSourceKind::BBoxMidpoint => 3,
            SnapSourceKind::Cursor => 4,
        }
    }

    fn target_priority(target: Option<SnapTarget>) -> u8 {
        match target {
            Some(SnapTarget::PathNode) => 0,
            Some(SnapTarget::Intersection) => 1,
            Some(SnapTarget::BBoxCorner) => 2,
            Some(SnapTarget::BBoxCenter) => 3,
            Some(SnapTarget::Guide) => 4,
            Some(SnapTarget::PathPoint) => 5,
            Some(SnapTarget::Midpoint) => 6,
            Some(SnapTarget::BBoxEdge) => 7,
            Some(SnapTarget::EqualSpacing) => 8,
            Some(SnapTarget::Grid) => 9,
            None => u8::MAX,
        }
    }

    /// Build a [`SnapIndicator`] for UI rendering from a [`SnapResult`].
    pub fn build_indicator(&self, result: &SnapResult) -> Option<SnapIndicator> {
        let target = result.target?;
        let mut crosshair_lines = Vec::new();

        // Always draw a small crosshair at the snapped point.
        let arm = 8.0; // pixels; the UI layer can scale this
        let p = result.point;
        crosshair_lines.push((Point::new(p.x - arm, p.y), Point::new(p.x + arm, p.y)));
        crosshair_lines.push((Point::new(p.x, p.y - arm), Point::new(p.x, p.y + arm)));

        if let Some(line) = result.indicator_line {
            crosshair_lines.push(line);
        }

        let distance_label = if result.distance > 1e-6 {
            Some((format!("{:.1}", result.distance), p))
        } else {
            None
        };

        Some(SnapIndicator {
            crosshair_lines,
            distance_label,
            target,
        })
    }
}

impl Default for SnappingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_math::{BezPath, Point, Rect};

    fn make_viewport() -> Viewport {
        Viewport::new(800.0, 600.0)
    }

    // -- SnapPreferences -------------------------------------------------

    #[test]
    fn test_default_prefs_all_enabled() {
        let prefs = SnapPreferences::default();
        assert!(prefs.any_enabled());
        assert_eq!(prefs.tolerance, 10.0);
        assert_eq!(prefs.grid_spacing, 10.0);
    }

    #[test]
    fn test_prefs_none_enabled() {
        let prefs = SnapPreferences {
            snap_to_grid: false,
            snap_to_guides: false,
            snap_to_bbox_edges: false,
            snap_to_bbox_centers: false,
            snap_to_bbox_corners: false,
            snap_to_path_nodes: false,
            snap_to_path: false,
            snap_to_intersections: false,
            snap_to_midpoints: false,
            snap_to_equal_spacing: false,
            tolerance: 10.0,
            grid_spacing: 10.0,
            sources: SnapSourcePreferences::default(),
        };
        assert!(!prefs.any_enabled());
    }

    // -- Grid snapping ---------------------------------------------------

    #[test]
    fn test_grid_snap_basic() {
        let engine = SnappingEngine::new();
        let vp = make_viewport();
        let context = SnapContext::default();
        let result = engine.snap_all(Point::new(12.0, 18.0), &context, &vp);
        assert_eq!(result.point, Point::new(10.0, 20.0));
        assert!(result.snapped_x);
        assert!(result.snapped_y);
        assert_eq!(result.target, Some(SnapTarget::Grid));
    }

    #[test]
    fn test_grid_snap_outside_tolerance() {
        let mut engine = SnappingEngine::new();
        engine.prefs.tolerance = 1.0;
        let vp = make_viewport();
        let context = SnapContext::default();
        // Point is 2 units from nearest grid line, tolerance is 1 px (= 1 doc unit at zoom 1).
        let result = engine.snap_all(Point::new(12.0, 18.0), &context, &vp);
        assert_eq!(result.point, Point::new(12.0, 18.0));
        assert!(!result.snapped_x);
        assert!(!result.snapped_y);
    }

    // -- Guide snapping --------------------------------------------------

    #[test]
    fn test_guide_snap_horizontal() {
        let engine = SnappingEngine::new();
        let guides = vec![SnapGuide::horizontal(100.0)];
        let result = engine.snap_to_guides(Point::new(50.0, 103.0), &guides, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(50.0, 100.0));
        assert!(!r.snapped_x);
        assert!(r.snapped_y);
        assert_eq!(r.target, Some(SnapTarget::Guide));
    }

    #[test]
    fn test_guide_snap_vertical() {
        let engine = SnappingEngine::new();
        let guides = vec![SnapGuide::vertical(200.0)];
        let result = engine.snap_to_guides(Point::new(203.0, 50.0), &guides, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(200.0, 50.0));
        assert!(r.snapped_x);
        assert!(!r.snapped_y);
        assert_eq!(r.target, Some(SnapTarget::Guide));
    }

    #[test]
    fn test_guide_snap_out_of_tolerance() {
        let engine = SnappingEngine::new();
        let guides = vec![SnapGuide::horizontal(100.0)];
        let result = engine.snap_to_guides(Point::new(50.0, 120.0), &guides, 10.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_guide_snap_closest_wins() {
        let engine = SnappingEngine::new();
        let guides = vec![SnapGuide::horizontal(100.0), SnapGuide::horizontal(105.0)];
        // Point y=103 is distance 3 from guide y=100, distance 2 from guide y=105.
        // Guide y=105 is closer, so it should win.
        let result = engine.snap_to_guides(Point::new(50.0, 103.0), &guides, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.point.y - 105.0).abs() < 1e-12);
    }

    #[test]
    fn test_guide_snap_disabled() {
        let mut engine = SnappingEngine::new();
        engine.prefs.snap_to_guides = false;
        let guides = vec![SnapGuide::horizontal(100.0)];
        let result = engine.snap_to_guides(Point::new(50.0, 103.0), &guides, 10.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_guide_snap_angled() {
        let engine = SnappingEngine::new();
        let guide = SnapGuide::new(Point::new(0.0, 0.0), Vector::new(1.0, -1.0)).unwrap();
        let result = engine.snap_to_guides(Point::new(11.0, 9.0), &[guide], 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.point.x - 10.0).abs() < 1e-9);
        assert!((r.point.y - 10.0).abs() < 1e-9);
        assert_eq!(r.target, Some(SnapTarget::Guide));
    }

    // -- BBox snapping ---------------------------------------------------

    #[test]
    fn test_bbox_snap_corner() {
        let mut engine = SnappingEngine::new();
        // Disable edges, centers, midpoints so only corners remain.
        engine.prefs.snap_to_bbox_edges = false;
        engine.prefs.snap_to_bbox_centers = false;
        engine.prefs.snap_to_midpoints = false;
        let bboxes = vec![Rect::new(100.0, 100.0, 200.0, 200.0)];
        let result = engine.snap_to_bboxes(Point::new(102.0, 101.0), &bboxes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(100.0, 100.0));
        assert_eq!(r.target, Some(SnapTarget::BBoxCorner));
    }

    #[test]
    fn test_bbox_snap_center() {
        let engine = SnappingEngine::new();
        let bboxes = vec![Rect::new(100.0, 100.0, 200.0, 200.0)];
        let result = engine.snap_to_bboxes(Point::new(151.0, 149.0), &bboxes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(150.0, 150.0));
        assert_eq!(r.target, Some(SnapTarget::BBoxCenter));
    }

    #[test]
    fn test_bbox_snap_edge() {
        let mut engine = SnappingEngine::new();
        // Disable corners, center, midpoints so only edges remain.
        engine.prefs.snap_to_bbox_corners = false;
        engine.prefs.snap_to_bbox_centers = false;
        engine.prefs.snap_to_midpoints = false;
        let bboxes = vec![Rect::new(100.0, 100.0, 200.0, 200.0)];
        // Point near the top edge, x is inside the bbox range.
        let result = engine.snap_to_bboxes(Point::new(130.0, 102.0), &bboxes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(130.0, 100.0));
        assert_eq!(r.target, Some(SnapTarget::BBoxEdge));
    }

    #[test]
    fn test_bbox_snap_midpoint() {
        let mut engine = SnappingEngine::new();
        // Disable corners, center, edges so only midpoints remain.
        engine.prefs.snap_to_bbox_corners = false;
        engine.prefs.snap_to_bbox_centers = false;
        engine.prefs.snap_to_bbox_edges = false;
        let bboxes = vec![Rect::new(100.0, 100.0, 200.0, 200.0)];
        // Midpoint of top edge = (150, 100). Test a point near there.
        let result = engine.snap_to_bboxes(Point::new(152.0, 101.0), &bboxes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(150.0, 100.0));
        assert_eq!(r.target, Some(SnapTarget::Midpoint));
    }

    #[test]
    fn test_bbox_snap_out_of_tolerance() {
        let engine = SnappingEngine::new();
        let bboxes = vec![Rect::new(100.0, 100.0, 200.0, 200.0)];
        let result = engine.snap_to_bboxes(Point::new(50.0, 50.0), &bboxes, 10.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_bbox_snap_multiple_boxes() {
        let mut engine = SnappingEngine::new();
        // Disable edges, centers, midpoints so only corners remain.
        engine.prefs.snap_to_bbox_edges = false;
        engine.prefs.snap_to_bbox_centers = false;
        engine.prefs.snap_to_midpoints = false;
        let bboxes = vec![
            Rect::new(100.0, 100.0, 200.0, 200.0),
            Rect::new(300.0, 300.0, 400.0, 400.0),
        ];
        // Closer to second box corner.
        let result = engine.snap_to_bboxes(Point::new(301.0, 301.0), &bboxes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(300.0, 300.0));
    }

    // -- Node snapping ---------------------------------------------------

    #[test]
    fn test_node_snap() {
        let engine = SnappingEngine::new();
        let nodes = vec![Point::new(50.0, 50.0), Point::new(150.0, 150.0)];
        let result = engine.snap_to_nodes(Point::new(52.0, 51.0), &nodes, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(50.0, 50.0));
        assert_eq!(r.target, Some(SnapTarget::PathNode));
    }

    #[test]
    fn test_node_snap_disabled() {
        let mut engine = SnappingEngine::new();
        engine.prefs.snap_to_path_nodes = false;
        let nodes = vec![Point::new(50.0, 50.0)];
        let result = engine.snap_to_nodes(Point::new(52.0, 51.0), &nodes, 10.0);
        assert!(result.is_none());
    }

    // -- Path snapping ---------------------------------------------------

    #[test]
    fn test_path_snap() {
        let engine = SnappingEngine::new();
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        // Point is 5 units above the line at x=50.
        let result = engine.snap_to_path(Point::new(50.0, 5.0), &path, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.point.x - 50.0).abs() < 1.0);
        assert!((r.point.y).abs() < 1.0);
        assert_eq!(r.target, Some(SnapTarget::PathPoint));
    }

    #[test]
    fn test_path_snap_out_of_tolerance() {
        let engine = SnappingEngine::new();
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(100.0, 0.0));
        let result = engine.snap_to_path(Point::new(50.0, 50.0), &path, 10.0);
        assert!(result.is_none());
    }

    // -- Intersection snapping -------------------------------------------

    #[test]
    fn test_intersection_snap() {
        let engine = SnappingEngine::new();
        let intersections = vec![Point::new(75.0, 75.0)];
        let result = engine.snap_to_intersections(Point::new(77.0, 76.0), &intersections, 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.point, Point::new(75.0, 75.0));
        assert_eq!(r.target, Some(SnapTarget::Intersection));
    }

    // -- Equal spacing snapping ------------------------------------------

    #[test]
    fn test_equal_spacing_snap() {
        let engine = SnappingEngine::new();
        // Three centres at x=100, 200, 300 with gap=100.
        // If the cursor is near x=400, it should snap to 400.
        let centers_x = vec![100.0, 200.0, 300.0];
        let result = engine.snap_to_equal_spacing(Point::new(403.0, 50.0), &centers_x, &[], 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.point.x - 400.0).abs() < 1e-12);
        assert_eq!(r.target, Some(SnapTarget::EqualSpacing));
    }

    #[test]
    fn test_equal_spacing_snap_before() {
        let engine = SnappingEngine::new();
        let centers_x = vec![100.0, 200.0, 300.0];
        let result = engine.snap_to_equal_spacing(Point::new(2.0, 50.0), &centers_x, &[], 10.0);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!((r.point.x - 0.0).abs() < 1e-12);
    }

    // -- snap_all combined -----------------------------------------------

    #[test]
    fn test_snap_all_picks_closest() {
        let mut engine = SnappingEngine::new();
        engine.prefs.snap_to_grid = false; // disable grid for this test
        let vp = make_viewport();

        let context = SnapContext {
            guides: vec![SnapGuide::horizontal(100.0)],
            bboxes: vec![Rect::new(200.0, 200.0, 300.0, 300.0)],
            nodes: vec![Point::new(50.0, 98.0)], // very close node
            ..Default::default()
        };

        let result = engine.snap_all(Point::new(50.0, 99.0), &context, &vp);
        // The node at (50, 98) is distance 1 from (50, 99).
        // The guide at y=100 is distance 1 from (50, 99).
        // Both are within tolerance; node (distance 1) equals guide (distance 1).
        // Either is acceptable; just verify we *did* snap.
        assert!(result.target.is_some());
        assert!(result.distance <= 1.0 + 1e-6);
    }

    #[test]
    fn test_snap_all_uses_target_priority_for_ties() {
        let engine = SnappingEngine::new();
        let vp = make_viewport();
        let context = SnapContext {
            nodes: vec![Point::new(12.0, 8.0)],
            ..Default::default()
        };

        let result = engine.snap_all(Point::new(12.0, 10.0), &context, &vp);

        assert_eq!(result.target, Some(SnapTarget::PathNode));
        assert_eq!(result.point, Point::new(12.0, 8.0));
    }

    #[test]
    fn test_snap_all_sources_prefers_object_node_over_cursor_on_tie() {
        let engine = SnappingEngine::new();
        let vp = make_viewport();
        let context = SnapContext {
            nodes: vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0)],
            ..Default::default()
        };
        let sources = vec![
            SnapSource::new(Point::new(12.0, 10.0), SnapSourceKind::Cursor),
            SnapSource::new(Point::new(22.0, 20.0), SnapSourceKind::ObjectNode),
        ];

        let result = engine.snap_all_sources(&sources, &context, &vp).unwrap();

        assert_eq!(result.source.kind, SnapSourceKind::ObjectNode);
        assert_eq!(result.snap.target, Some(SnapTarget::PathNode));
        assert_eq!(result.snap.distance, 2.0);
    }

    #[test]
    fn test_snap_all_sources_respects_source_preferences() {
        let mut engine = SnappingEngine::new();
        engine.prefs.sources.snap_from_object_nodes = false;
        let vp = make_viewport();
        let context = SnapContext {
            nodes: vec![Point::new(10.0, 10.0), Point::new(20.0, 20.0)],
            ..Default::default()
        };
        let sources = vec![
            SnapSource::new(Point::new(12.0, 10.0), SnapSourceKind::Cursor),
            SnapSource::new(Point::new(20.5, 20.0), SnapSourceKind::ObjectNode),
        ];

        let result = engine.snap_all_sources(&sources, &context, &vp).unwrap();

        assert_eq!(result.source.kind, SnapSourceKind::Cursor);
        assert_eq!(result.snap.point, Point::new(10.0, 10.0));
    }

    #[test]
    fn test_snap_all_no_snap_when_disabled() {
        let mut engine = SnappingEngine::new();
        engine.prefs = SnapPreferences {
            snap_to_grid: false,
            snap_to_guides: false,
            snap_to_bbox_edges: false,
            snap_to_bbox_centers: false,
            snap_to_bbox_corners: false,
            snap_to_path_nodes: false,
            snap_to_path: false,
            snap_to_intersections: false,
            snap_to_midpoints: false,
            snap_to_equal_spacing: false,
            tolerance: 10.0,
            grid_spacing: 10.0,
            sources: SnapSourcePreferences::default(),
        };
        let vp = make_viewport();
        let context = SnapContext {
            guides: vec![SnapGuide::horizontal(100.0)],
            bboxes: vec![Rect::new(0.0, 0.0, 100.0, 100.0)],
            ..Default::default()
        };
        let result = engine.snap_all(Point::new(50.0, 50.0), &context, &vp);
        assert!(result.target.is_none());
    }

    // -- SnapIndicator ---------------------------------------------------

    #[test]
    fn test_build_indicator() {
        let engine = SnappingEngine::new();
        let result = SnapResult {
            point: Point::new(100.0, 100.0),
            snapped_x: true,
            snapped_y: true,
            distance: 2.0,
            target: Some(SnapTarget::BBoxCorner),
            indicator_line: Some((Point::new(102.0, 101.0), Point::new(100.0, 100.0))),
        };
        let indicator = engine.build_indicator(&result);
        assert!(indicator.is_some());
        let ind = indicator.unwrap();
        assert_eq!(ind.target, SnapTarget::BBoxCorner);
        // crosshair (2 lines) + indicator_line (1 line) = 3
        assert_eq!(ind.crosshair_lines.len(), 3);
        assert!(ind.distance_label.is_some());
    }

    #[test]
    fn test_build_indicator_no_snap() {
        let engine = SnappingEngine::new();
        let result = SnapResult::no_snap(Point::new(50.0, 50.0));
        assert!(engine.build_indicator(&result).is_none());
    }
}
