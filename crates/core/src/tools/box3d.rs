//! 3D box tool: click-drag to create a perspective box.
//!
//! States:
//! - **Idle**: waiting for input.
//! - **Drawing**: user is dragging to define the front face rectangle.
//!
//! On mouse-down: start drawing (record the start point).
//! On mouse-drag: define the front face rectangle.
//! On mouse-up: create a `Box3d` element containing 6 `<path>` children
//! representing the box faces, with a stable cabinet-style depth projection.
//!
//! Parameters:
//! - `vanishing_point_x`, `vanishing_point_y`: serialized for SPBox3D compatibility.
//! - `z_depth`: how far back the box extends.

use sivgo_dom::types::{Color, LengthUnit, Paint, SvgLength};
use sivgo_dom::{Box3dAttrs, NodeId, PathAttrs, PresentationAttrs, SvgDocument, SvgElement};
use sivgo_math::{BezPath, Point, VanishingPoint};

use crate::command::Command;
use crate::input::{KeyEvent, MouseButton, MouseEvent};
use crate::selection::SelectionSet;
use crate::tool::{Tool, ToolResponse};
use crate::tools::drag::drag_rect;
use crate::viewport::Viewport;

// ---------------------------------------------------------------------------
// Box3dTool
// ---------------------------------------------------------------------------

/// Interactive tool for creating 3D perspective boxes.
///
/// Click-drag to define the front face rectangle, then the tool generates
/// a `Box3d` element with 6 path children (one per face).
#[derive(Debug)]
pub struct Box3dTool {
    /// Start point in document space.
    start: Option<Point>,
    /// Whether we are actively dragging.
    dragging: bool,
    /// X coordinate of the serialized vanishing point.
    pub vanishing_point_x: f64,
    /// Y coordinate of the serialized vanishing point.
    pub vanishing_point_y: f64,
    /// Depth of the box along the Z axis (in document units).
    pub z_depth: f64,
}

impl Box3dTool {
    pub fn new() -> Self {
        Self {
            start: None,
            dragging: false,
            vanishing_point_x: 400.0,
            vanishing_point_y: 300.0,
            z_depth: 60.0,
        }
    }

    /// Create with custom perspective parameters.
    pub fn with_params(vp_x: f64, vp_y: f64, z_depth: f64) -> Self {
        Self {
            start: None,
            dragging: false,
            vanishing_point_x: vp_x,
            vanishing_point_y: vp_y,
            z_depth: z_depth.max(1.0),
        }
    }
}

impl Default for Box3dTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a stable cabinet-style back-face offset.
///
/// Keeping all back-face corners translated by the same vector makes the
/// result read as a box instead of an arbitrary warped perspective quad.
fn compute_depth_offset(front: &[Point; 4], z_depth: f64) -> Point {
    let width = front[0].distance(front[1]);
    let height = front[1].distance(front[2]);
    let short_side = width.min(height).max(1.0);
    let depth = z_depth.min(short_side * 0.55).max(1.0);
    Point::new(depth * 0.55, -depth * 0.45)
}

fn compute_back_face(front: &[Point; 4], z_depth: f64) -> [Point; 4] {
    let offset = compute_depth_offset(front, z_depth);
    std::array::from_fn(|i| Point::new(front[i].x + offset.x, front[i].y + offset.y))
}

/// Build a `BezPath` for a quadrilateral from four corner points.
fn quad_path(corners: &[Point; 4]) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(corners[0]);
    path.line_to(corners[1]);
    path.line_to(corners[2]);
    path.line_to(corners[3]);
    path.close();
    path
}

/// Build the six face paths of the 3D box.
///
/// Front face corners are ordered: top-left, top-right, bottom-right, bottom-left.
/// Back face corners are in the same order, translated by the depth offset.
///
/// Returns paths for: front, back, top, bottom, left, right.
fn build_box_faces(front: &[Point; 4], back: &[Point; 4]) -> [BezPath; 6] {
    let face_front = quad_path(front);
    let face_back = quad_path(back);
    let face_top = quad_path(&[front[0], front[1], back[1], back[0]]);
    let face_bottom = quad_path(&[front[3], front[2], back[2], back[3]]);
    let face_left = quad_path(&[front[0], back[0], back[3], front[3]]);
    let face_right = quad_path(&[front[1], back[1], back[2], front[2]]);

    [
        face_front,
        face_back,
        face_top,
        face_bottom,
        face_left,
        face_right,
    ]
}

/// Face colors -- varying shades to give a 3D appearance.
const FACE_COLORS: [Color; 6] = [
    Color {
        r: 100,
        g: 149,
        b: 237,
        a: 255,
    }, // front  - cornflower blue
    Color {
        r: 60,
        g: 100,
        b: 180,
        a: 255,
    }, // back   - darker blue
    Color {
        r: 130,
        g: 170,
        b: 255,
        a: 255,
    }, // top    - light blue
    Color {
        r: 50,
        g: 80,
        b: 150,
        a: 255,
    }, // bottom - dark blue
    Color {
        r: 80,
        g: 120,
        b: 200,
        a: 255,
    }, // left   - medium blue
    Color {
        r: 110,
        g: 160,
        b: 240,
        a: 255,
    }, // right  - bright blue
];

/// Paint hidden/back faces first and the front face last so the front does not
/// get covered by the back face.
const FACE_PAINT_ORDER: [usize; 6] = [1, 2, 5, 3, 4, 0];

/// Format a corner point as a `"x,y"` string for `Box3dAttrs`.
fn corner_string(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

/// Parse a `"x,y"` corner string back into a `Point`.
///
/// Returns `None` if the string is not a valid coordinate pair.
pub fn parse_corner(s: &str) -> Option<Point> {
    let parts: Vec<&str> = s.trim().split(',').collect();
    if parts.len() == 2 {
        let x: f64 = parts[0].trim().parse().ok()?;
        let y: f64 = parts[1].trim().parse().ok()?;
        Some(Point::new(x, y))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// box3d_to_paths -- 17.1.4
// ---------------------------------------------------------------------------

/// Convert a `Box3d` element's corner attributes into 6 face `BezPath`s.
///
/// The returned paths represent the six faces of the box:
/// `[front, back, top, bottom, left, right]`.
///
/// Corner numbering:
/// - 0..3: front face (bottom-left, bottom-right, top-right, top-left)
/// - 4..7: back face  (bottom-left, bottom-right, top-right, top-left)
///
/// Returns `None` if any required corner attribute is missing or invalid.
pub fn box3d_to_paths(attrs: &Box3dAttrs) -> Option<[BezPath; 6]> {
    let c0 = parse_corner(attrs.corner0.as_deref()?)?;
    let c1 = parse_corner(attrs.corner1.as_deref()?)?;
    let c2 = parse_corner(attrs.corner2.as_deref()?)?;
    let c3 = parse_corner(attrs.corner3.as_deref()?)?;
    let c4 = parse_corner(attrs.corner4.as_deref()?)?;
    let c5 = parse_corner(attrs.corner5.as_deref()?)?;
    let c6 = parse_corner(attrs.corner6.as_deref()?)?;
    let c7 = parse_corner(attrs.corner7.as_deref()?)?;

    // Front face: corners 0,1,2,3 (BL, BR, TR, TL)
    let front = [c0, c1, c2, c3];
    // Back face:  corners 4,5,6,7 (BL, BR, TR, TL)
    let back = [c4, c5, c6, c7];

    Some(build_box_faces(&front, &back))
}

/// Convert a `Box3d` element into 6 `SvgElement::Path` elements with face
/// colors and thin stroke, suitable for rendering.
///
/// Returns `None` if the corners cannot be parsed.
pub fn box3d_to_path_elements(attrs: &Box3dAttrs) -> Option<[(SvgElement, PresentationAttrs); 6]> {
    let faces = box3d_to_paths(attrs)?;

    let result: [(SvgElement, PresentationAttrs); 6] = std::array::from_fn(|i| {
        let element = SvgElement::Path(PathAttrs {
            d: faces[i].clone(),
            ..Default::default()
        });
        let pres = PresentationAttrs {
            fill: Some(Paint::Color(FACE_COLORS[i])),
            stroke: Some(Paint::Color(Color::BLACK)),
            stroke_width: Some(SvgLength::new(0.5, LengthUnit::None)),
            ..Default::default()
        };
        (element, pres)
    });

    Some(result)
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

impl Tool for Box3dTool {
    fn name(&self) -> &str {
        "3D Box"
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
        ToolResponse::Handled
    }

    fn on_mouse_move(
        &mut self,
        _event: MouseEvent,
        _doc: &SvgDocument,
        _selection: &SelectionSet,
        _viewport: &Viewport,
    ) -> ToolResponse {
        if self.dragging {
            ToolResponse::Handled
        } else {
            ToolResponse::None
        }
    }

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
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

        let rect = drag_rect(start, end, event.modifiers.ctrl, event.modifiers.shift);

        // Don't create tiny boxes.
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return ToolResponse::None;
        }

        // Front face corners: TL, TR, BR, BL
        let front = [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
            Point::new(rect.x0, rect.y1),
        ];

        let back = compute_back_face(&front, self.z_depth);
        let faces = build_box_faces(&front, &back);

        let parent = match doc.root() {
            Some(r) => r,
            None => return ToolResponse::None,
        };

        // Build the Box3d element with vanishing point and corner attributes.
        let vp = VanishingPoint::finite(self.vanishing_point_x, self.vanishing_point_y);
        let box3d_attrs = Box3dAttrs {
            vp_x: Some(vp.to_svg_string()),
            vp_y: None,
            vp_z: None,
            corner0: Some(corner_string(front[0])),
            corner1: Some(corner_string(front[1])),
            corner2: Some(corner_string(front[2])),
            corner3: Some(corner_string(front[3])),
            corner4: Some(corner_string(back[0])),
            corner5: Some(corner_string(back[1])),
            corner6: Some(corner_string(back[2])),
            corner7: Some(corner_string(back[3])),
        };
        let box3d_element = SvgElement::Box3d(box3d_attrs);

        let cmd = CreateBox3dCommand::new(parent, box3d_element, faces);
        ToolResponse::ExecuteCommand(Box::new(cmd))
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
            self.start = None;
            self.dragging = false;
            return ToolResponse::Handled;
        }
        ToolResponse::None
    }

    fn on_activate(&mut self) {
        self.start = None;
        self.dragging = false;
    }

    fn on_deactivate(&mut self) {
        self.start = None;
        self.dragging = false;
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

// ---------------------------------------------------------------------------
// CreateBox3dCommand
// ---------------------------------------------------------------------------

/// Command that creates a `Box3d` element with six `<path>` children for a 3D box.
#[derive(Debug)]
struct CreateBox3dCommand {
    parent_id: NodeId,
    box3d_element: SvgElement,
    faces: [BezPath; 6],
    box3d_id: Option<NodeId>,
    face_ids: [Option<NodeId>; 6],
}

impl CreateBox3dCommand {
    fn new(parent_id: NodeId, box3d_element: SvgElement, faces: [BezPath; 6]) -> Self {
        Self {
            parent_id,
            box3d_element,
            faces,
            box3d_id: None,
            face_ids: [None; 6],
        }
    }
}

impl Command for CreateBox3dCommand {
    fn execute(&mut self, doc: &mut SvgDocument) {
        // Create the Box3d container node.
        let box3d_id = if let Some(prev_id) = self.box3d_id {
            let node = sivgo_dom::SvgNode::new(prev_id, self.box3d_element.clone());
            doc.insert_node(node);
            doc.append_child(self.parent_id, prev_id);
            prev_id
        } else {
            let id = doc.add_node(self.box3d_element.clone());
            doc.append_child(self.parent_id, id);
            id
        };
        self.box3d_id = Some(box3d_id);

        // Create each face path as a child of the Box3d element.
        for &i in &FACE_PAINT_ORDER {
            let face_path = &self.faces[i];
            let element = SvgElement::Path(PathAttrs {
                d: face_path.clone(),
                ..Default::default()
            });

            let pres = PresentationAttrs {
                fill: Some(Paint::Color(FACE_COLORS[i])),
                stroke: Some(Paint::Color(Color::BLACK)),
                stroke_width: Some(SvgLength::new(0.5, LengthUnit::None)),
                ..Default::default()
            };

            let face_id = if let Some(prev_id) = self.face_ids[i] {
                let mut node = sivgo_dom::SvgNode::new(prev_id, element);
                node.presentation = pres;
                doc.insert_node(node);
                doc.append_child(box3d_id, prev_id);
                prev_id
            } else {
                let id = doc.add_node(element);
                if let Some(node) = doc.get_node_mut(id) {
                    node.presentation = pres;
                }
                doc.append_child(box3d_id, id);
                id
            };
            self.face_ids[i] = Some(face_id);
        }
    }

    fn undo(&mut self, doc: &mut SvgDocument) {
        // Remove face nodes first, then the box3d container.
        for i in (0..6).rev() {
            if let Some(id) = self.face_ids[i] {
                doc.remove_node(id);
            }
        }
        if let Some(id) = self.box3d_id {
            doc.remove_node(id);
        }
    }

    fn description(&self) -> &str {
        "Create 3D box"
    }

    fn created_selection(&self) -> Vec<NodeId> {
        self.box3d_id.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Modifiers;
    use sivgo_dom::SvgSvgAttrs;

    fn make_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs::default()));
        doc.set_root(svg_id);
        doc
    }

    #[test]
    fn box3d_back_face_uses_one_shared_depth_offset() {
        let front = [
            Point::new(100.0, 100.0),
            Point::new(200.0, 100.0),
            Point::new(200.0, 200.0),
            Point::new(100.0, 200.0),
        ];
        let back = compute_back_face(&front, 50.0);
        let offset = Point::new(back[0].x - front[0].x, back[0].y - front[0].y);

        for (i, (&f, &b)) in front.iter().zip(back.iter()).enumerate() {
            assert!(
                (b.x - f.x - offset.x).abs() < 1e-10 && (b.y - f.y - offset.y).abs() < 1e-10,
                "back corner {} should use the shared depth offset",
                i
            );
        }
        assert!(offset.x > 0.0, "box depth should move right");
        assert!(offset.y < 0.0, "box depth should move upward");
    }

    #[test]
    fn box3d_back_face_keeps_parallel_front_edges() {
        let front = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let back = compute_back_face(&front, 60.0);

        assert!((front[0].distance(front[1]) - back[0].distance(back[1])).abs() < 1e-10);
        assert!((front[1].distance(front[2]) - back[1].distance(back[2])).abs() < 1e-10);
    }

    #[test]
    fn box3d_paints_front_face_last() {
        assert_eq!(FACE_PAINT_ORDER.last().copied(), Some(0));
        assert!(FACE_PAINT_ORDER[..5].contains(&1));
    }

    #[test]
    fn build_box_faces_produces_six_closed_paths() {
        let front = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let back = [
            Point::new(20.0, 20.0),
            Point::new(80.0, 20.0),
            Point::new(80.0, 80.0),
            Point::new(20.0, 80.0),
        ];

        let faces = build_box_faces(&front, &back);
        assert_eq!(faces.len(), 6);

        for (i, face) in faces.iter().enumerate() {
            let elems = face.elements();
            // Each face should have: MoveTo + 3 LineTo + ClosePath = 5 elements
            assert_eq!(
                elems.len(),
                5,
                "face {} should have 5 path elements, got {}",
                i,
                elems.len()
            );
        }
    }

    #[test]
    fn box3d_tool_creates_box3d_element_with_six_faces() {
        let doc = make_doc();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);
        let mut tool = Box3dTool::new();

        tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        let resp = tool.on_mouse_up(MouseEvent::left(200.0, 200.0), &doc, &sel, &vp);

        match resp {
            ToolResponse::ExecuteCommand(mut cmd) => {
                let mut doc = doc;
                cmd.execute(&mut doc);
                let root = doc.root().unwrap();
                let children = doc.children(root);
                assert_eq!(children.len(), 1);

                let box3d_node = doc.get_node(children[0]).unwrap();
                assert_eq!(box3d_node.element.tag_name(), "SPBox3D");

                // Verify the Box3d element has corner attributes set.
                match &box3d_node.element {
                    SvgElement::Box3d(attrs) => {
                        assert!(attrs.corner0.is_some(), "corner0 should be set");
                        assert!(attrs.corner7.is_some(), "corner7 should be set");
                        assert!(attrs.vp_x.is_some(), "vp_x should be set");
                    }
                    _ => panic!("Expected Box3d element"),
                }

                let face_children = doc.children(children[0]);
                assert_eq!(face_children.len(), 6, "box should have 6 face paths");

                for &face_id in face_children.iter() {
                    let face = doc.get_node(face_id).unwrap();
                    assert_eq!(face.element.tag_name(), "path");
                }
            }
            _ => panic!("Expected ExecuteCommand response from Box3dTool"),
        }
    }

    #[test]
    fn box3d_tool_ctrl_shift_uses_constrained_centered_front_face() {
        let doc = make_doc();
        let sel = SelectionSet::new();
        let vp = Viewport::new(800.0, 600.0);
        let mut tool = Box3dTool::new();

        tool.on_mouse_down(MouseEvent::left(100.0, 100.0), &doc, &sel, &vp);
        let resp = tool.on_mouse_up(
            MouseEvent {
                x: 120.0,
                y: 110.0,
                button: MouseButton::Left,
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
            },
            &doc,
            &sel,
            &vp,
        );

        match resp {
            ToolResponse::ExecuteCommand(mut cmd) => {
                let mut doc = doc;
                cmd.execute(&mut doc);
                let root = doc.root().unwrap();
                let box3d_node = doc.get_node(doc.children(root)[0]).unwrap();

                match &box3d_node.element {
                    SvgElement::Box3d(attrs) => {
                        assert_eq!(
                            parse_corner(attrs.corner0.as_deref().unwrap()).unwrap(),
                            Point::new(80.0, 80.0)
                        );
                        assert_eq!(
                            parse_corner(attrs.corner1.as_deref().unwrap()).unwrap(),
                            Point::new(120.0, 80.0)
                        );
                        assert_eq!(
                            parse_corner(attrs.corner2.as_deref().unwrap()).unwrap(),
                            Point::new(120.0, 120.0)
                        );
                        assert_eq!(
                            parse_corner(attrs.corner3.as_deref().unwrap()).unwrap(),
                            Point::new(80.0, 120.0)
                        );
                    }
                    _ => panic!("expected Box3d element"),
                }
            }
            _ => panic!("expected ExecuteCommand response from Box3dTool"),
        }
    }

    #[test]
    fn box3d_undo_removes_everything() {
        let mut doc = make_doc();
        let root = doc.root().unwrap();

        let front = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let back = compute_back_face(&front, 50.0);
        let faces = build_box_faces(&front, &back);
        let box3d_element = SvgElement::Box3d(Box3dAttrs {
            corner0: Some(corner_string(front[0])),
            corner1: Some(corner_string(front[1])),
            corner2: Some(corner_string(front[2])),
            corner3: Some(corner_string(front[3])),
            corner4: Some(corner_string(back[0])),
            corner5: Some(corner_string(back[1])),
            corner6: Some(corner_string(back[2])),
            corner7: Some(corner_string(back[3])),
            ..Default::default()
        });

        let mut cmd = CreateBox3dCommand::new(root, box3d_element, faces);
        cmd.execute(&mut doc);
        assert_eq!(doc.children(root).len(), 1);

        cmd.undo(&mut doc);
        assert_eq!(doc.children(root).len(), 0);
    }

    // -- 17.1.2: VanishingPoint tests --

    #[test]
    fn vanishing_point_finite_parse() {
        let vp = VanishingPoint::parse("400,300").unwrap();
        assert!(vp.is_finite);
        assert!((vp.x - 400.0).abs() < 1e-10);
        assert!((vp.y - 300.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_infinite_parse() {
        let vp = VanishingPoint::parse("inf").unwrap();
        assert!(!vp.is_finite);
    }

    #[test]
    fn vanishing_point_serialize_roundtrip() {
        let vp = VanishingPoint::finite(123.5, 456.0);
        let s = vp.to_svg_string();
        let parsed = VanishingPoint::parse(&s).unwrap();
        assert!(parsed.is_finite);
        assert!((parsed.x - 123.5).abs() < 1e-10);
        assert!((parsed.y - 456.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_project_finite() {
        let vp = VanishingPoint::finite(400.0, 300.0);
        let p = Point::new(100.0, 100.0);
        let projected = vp.project(p, 0.5);
        // Should be midpoint between p and vp
        assert!((projected.x - 250.0).abs() < 1e-10);
        assert!((projected.y - 200.0).abs() < 1e-10);
    }

    #[test]
    fn vanishing_point_project_infinite() {
        let vp = VanishingPoint::infinite(10.0, 5.0);
        let p = Point::new(100.0, 100.0);
        let projected = vp.project(p, 2.0);
        // Should translate by direction * t
        assert!((projected.x - 120.0).abs() < 1e-10);
        assert!((projected.y - 110.0).abs() < 1e-10);
    }

    // -- 17.1.4: box3d_to_paths tests --

    #[test]
    fn box3d_to_paths_produces_six_faces() {
        let attrs = Box3dAttrs {
            corner0: Some("0,100".to_string()),
            corner1: Some("100,100".to_string()),
            corner2: Some("100,0".to_string()),
            corner3: Some("0,0".to_string()),
            corner4: Some("20,80".to_string()),
            corner5: Some("80,80".to_string()),
            corner6: Some("80,20".to_string()),
            corner7: Some("20,20".to_string()),
            ..Default::default()
        };

        let paths = box3d_to_paths(&attrs).expect("should produce paths");
        assert_eq!(paths.len(), 6);

        for (i, path) in paths.iter().enumerate() {
            let elems = path.elements();
            assert_eq!(
                elems.len(),
                5,
                "face {} should have 5 path elements (M + 3L + Z), got {}",
                i,
                elems.len()
            );
        }
    }

    #[test]
    fn box3d_to_paths_returns_none_for_missing_corners() {
        let attrs = Box3dAttrs {
            corner0: Some("0,0".to_string()),
            // Missing remaining corners
            ..Default::default()
        };

        assert!(
            box3d_to_paths(&attrs).is_none(),
            "should return None when corners are missing"
        );
    }

    #[test]
    fn box3d_to_path_elements_produces_colored_faces() {
        let attrs = Box3dAttrs {
            corner0: Some("0,100".to_string()),
            corner1: Some("100,100".to_string()),
            corner2: Some("100,0".to_string()),
            corner3: Some("0,0".to_string()),
            corner4: Some("20,80".to_string()),
            corner5: Some("80,80".to_string()),
            corner6: Some("80,20".to_string()),
            corner7: Some("20,20".to_string()),
            ..Default::default()
        };

        let elements = box3d_to_path_elements(&attrs).expect("should produce path elements");
        assert_eq!(elements.len(), 6);

        for (i, (elem, pres)) in elements.iter().enumerate() {
            assert_eq!(elem.tag_name(), "path", "face {} should be a path", i);
            assert!(pres.fill.is_some(), "face {} should have fill", i);
            assert!(pres.stroke.is_some(), "face {} should have stroke", i);
        }
    }

    #[test]
    fn parse_corner_valid() {
        let p = parse_corner("100.5,200.3").unwrap();
        assert!((p.x - 100.5).abs() < 1e-10);
        assert!((p.y - 200.3).abs() < 1e-10);
    }

    #[test]
    fn parse_corner_invalid() {
        assert!(parse_corner("not_a_number").is_none());
        assert!(parse_corner("").is_none());
    }
}
