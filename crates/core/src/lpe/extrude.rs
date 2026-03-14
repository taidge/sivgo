//! Extrude Live Path Effect.
//!
//! Adds visual depth to a path by connecting it to a translated copy,
//! creating an extruded (pseudo-3D) appearance.

use super::{LivePathEffect, LpeParam};
use sivgo_math::{BezPath, PathSegment, Point, Transform2D};

/// Extrude LPE.
///
/// Creates an extrusion effect by:
/// 1. Keeping the original (front face) path.
/// 2. Creating a translated copy (back face).
/// 3. Connecting corresponding vertices of the front and back faces with
///    quadrilateral "side" panels.
///
/// The result is a single combined path that gives the illusion of depth.
#[derive(Debug, Clone)]
pub struct ExtrudeLpe {
    /// Horizontal extrusion offset.
    offset_x: LpeParam,
    /// Vertical extrusion offset.
    offset_y: LpeParam,
}

impl ExtrudeLpe {
    /// Create a new Extrude effect.
    ///
    /// * `offset_x` - Horizontal offset for the back face.
    /// * `offset_y` - Vertical offset for the back face.
    pub fn new(offset_x: f64, offset_y: f64) -> Self {
        Self {
            offset_x: LpeParam::new("offset_x", offset_x, -100_000.0, 100_000.0),
            offset_y: LpeParam::new("offset_y", offset_y, -100_000.0, 100_000.0),
        }
    }
}

impl LivePathEffect for ExtrudeLpe {
    fn name(&self) -> &str {
        "extrude"
    }

    fn apply(&self, input: &BezPath) -> BezPath {
        let dx = self.offset_x.value;
        let dy = self.offset_y.value;

        if input.is_empty() || (dx.abs() < 1e-12 && dy.abs() < 1e-12) {
            return input.clone();
        }

        // Create the back face (translated copy).
        let transform = Transform2D::translate(dx, dy);
        let back = input.transformed(&transform);

        // Collect front-face vertices for generating side panels.
        let elements = input.elements();
        let vertices = collect_vertices(&elements);

        // Build the result: back face + side panels + front face (on top).
        let mut result = BezPath::new();

        // 1. Back face.
        result.extend(&back);

        // 2. Side panels: connect each consecutive pair of front vertices
        //    to their corresponding back vertices.
        for i in 0..vertices.len().saturating_sub(1) {
            let f0 = vertices[i];
            let f1 = vertices[i + 1];
            let b0 = Point::new(f0.x + dx, f0.y + dy);
            let b1 = Point::new(f1.x + dx, f1.y + dy);

            result.move_to(f0);
            result.line_to(f1);
            result.line_to(b1);
            result.line_to(b0);
            result.close();
        }

        // 3. Front face (on top).
        result.extend(input);

        result
    }

    fn params(&self) -> &[LpeParam] {
        std::slice::from_ref(&self.offset_x)
    }

    fn set_param(&mut self, name: &str, value: f64) {
        match name {
            "offset_x" => {
                self.offset_x.value = value.clamp(self.offset_x.min, self.offset_x.max);
            }
            "offset_y" => {
                self.offset_y.value = value.clamp(self.offset_y.min, self.offset_y.max);
            }
            _ => {}
        }
    }
}

/// Collect all on-curve vertices from path elements.
fn collect_vertices(elements: &[PathSegment]) -> Vec<Point> {
    let mut pts = Vec::new();
    for el in elements {
        match *el {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                pts.push(p);
            }
            PathSegment::QuadTo(_cp, end) => {
                pts.push(end);
            }
            PathSegment::CurveTo(_cp1, _cp2, end) => {
                pts.push(end);
            }
            PathSegment::ClosePath => {}
        }
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(100.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    fn triangle() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(50.0, 0.0));
        p.line_to(Point::new(100.0, 100.0));
        p.line_to(Point::new(0.0, 100.0));
        p.close();
        p
    }

    #[test]
    fn test_extrude_adds_depth() {
        let lpe = ExtrudeLpe::new(10.0, 10.0);
        let input = square();
        let output = lpe.apply(&input);
        // Should have back face + side panels + front face.
        assert!(
            output.element_count() > input.element_count() * 2,
            "extrude should add side panels: input={} output={}",
            input.element_count(),
            output.element_count()
        );
    }

    #[test]
    fn test_extrude_bbox_grows() {
        let lpe = ExtrudeLpe::new(20.0, 30.0);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        // Bounding box should grow by the offset.
        assert!((bb.x1 - 120.0).abs() < 1e-6, "x1={}", bb.x1);
        assert!((bb.y1 - 130.0).abs() < 1e-6, "y1={}", bb.y1);
    }

    #[test]
    fn test_extrude_negative_offset() {
        let lpe = ExtrudeLpe::new(-15.0, -15.0);
        let input = square();
        let output = lpe.apply(&input);
        let bb = output.bounding_box();
        assert!(bb.x0 < 0.0, "x0={}", bb.x0);
        assert!(bb.y0 < 0.0, "y0={}", bb.y0);
    }

    #[test]
    fn test_extrude_zero_offset_is_identity() {
        let lpe = ExtrudeLpe::new(0.0, 0.0);
        let input = square();
        let output = lpe.apply(&input);
        assert_eq!(input.to_svg_string(), output.to_svg_string());
    }

    #[test]
    fn test_extrude_on_triangle() {
        let lpe = ExtrudeLpe::new(10.0, 10.0);
        let input = triangle();
        let output = lpe.apply(&input);
        assert!(!output.is_empty());
        assert!(output.element_count() > input.element_count());
    }

    #[test]
    fn test_extrude_empty_path() {
        let lpe = ExtrudeLpe::new(10.0, 10.0);
        let input = BezPath::new();
        let output = lpe.apply(&input);
        assert!(output.is_empty());
    }

    #[test]
    fn test_extrude_params() {
        let mut lpe = ExtrudeLpe::new(10.0, 20.0);
        let params = lpe.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "offset_x");
        assert!((params[0].value - 10.0).abs() < 1e-10);

        lpe.set_param("offset_x", 30.0);
        assert!((lpe.offset_x.value - 30.0).abs() < 1e-10);

        lpe.set_param("offset_y", 40.0);
        assert!((lpe.offset_y.value - 40.0).abs() < 1e-10);
    }

    #[test]
    fn test_extrude_name() {
        let lpe = ExtrudeLpe::new(10.0, 10.0);
        assert_eq!(lpe.name(), "extrude");
    }
}
