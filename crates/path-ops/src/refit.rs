//! Polygon-to-BezPath conversion -- convert polygon contours back to BezPath.

use sivgo_math::{BezPath, Point};

/// Convert polygon contours (as returned by i_overlay) back into a `BezPath`.
///
/// Each contour becomes a closed subpath consisting of line segments.
/// The contours from i_overlay are already auto-closed, so we close them explicitly.
pub fn polygon_to_path(contours: &[Vec<[f64; 2]>]) -> BezPath {
    let mut path = BezPath::new();
    for contour in contours {
        if contour.is_empty() {
            continue;
        }
        path.move_to(Point::new(contour[0][0], contour[0][1]));
        for pt in &contour[1..] {
            path.line_to(Point::new(pt[0], pt[1]));
        }
        path.close();
    }
    path
}

/// Convert i_overlay `Shapes` result (Vec<Vec<Vec<[f64; 2]>>>) into a `BezPath`.
///
/// Each shape consists of one or more contours (first is outer boundary, rest are holes).
/// All contours are combined into a single BezPath with multiple subpaths.
pub fn shapes_to_path(shapes: &[Vec<Vec<[f64; 2]>>]) -> BezPath {
    let contours: Vec<Vec<[f64; 2]>> = shapes
        .iter()
        .flat_map(|shape| shape.iter().cloned())
        .collect();
    polygon_to_path(&contours)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_to_path_simple_triangle() {
        let contours = vec![vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]]];
        let path = polygon_to_path(&contours);

        assert_eq!(path.element_count(), 4); // MoveTo, LineTo, LineTo, Close
        let svg = path.to_svg_string();
        assert!(!svg.is_empty());
    }

    #[test]
    fn polygon_to_path_empty() {
        let contours: Vec<Vec<[f64; 2]>> = Vec::new();
        let path = polygon_to_path(&contours);
        assert!(path.is_empty());
    }

    #[test]
    fn polygon_to_path_multiple_contours() {
        let contours = vec![
            vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]],
            vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0]],
        ];
        let path = polygon_to_path(&contours);
        assert_eq!(path.element_count(), 8); // 2 * (MoveTo + LineTo + LineTo + Close)
    }

    #[test]
    fn shapes_to_path_works() {
        let shapes = vec![vec![
            vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
            vec![[20.0, 20.0], [80.0, 20.0], [80.0, 80.0], [20.0, 80.0]],
        ]];
        let path = shapes_to_path(&shapes);
        // Outer + hole = 2 subpaths, each with MoveTo + 3 LineTo + Close = 5
        assert_eq!(path.element_count(), 10);
    }
}
