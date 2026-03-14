use crate::threshold::BinaryBitmap;

/// A contour is a closed loop of integer pixel coordinates representing a boundary.
#[derive(Debug, Clone)]
pub struct Contour {
    /// The ordered boundary points of the contour.
    pub points: Vec<(i32, i32)>,
    /// Whether this is an outer contour (true) or a hole (false).
    pub is_outer: bool,
}

/// The 8 Moore-neighborhood directions: E, NE, N, NW, W, SW, S, SE.
/// Stored as (dx, dy) offsets ordered clockwise starting from East.
const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),   // 0: East
    (1, -1),  // 1: NE
    (0, -1),  // 2: North
    (-1, -1), // 3: NW
    (-1, 0),  // 4: West
    (-1, 1),  // 5: SW
    (0, 1),   // 6: South
    (1, 1),   // 7: SE
];

/// Find all contours in a binary bitmap using Moore-Neighbor boundary tracing.
///
/// Returns a list of contours. Each contour is a closed polygon of boundary pixel
/// coordinates. Outer contours go clockwise; holes go counter-clockwise.
pub fn find_contours(bitmap: &BinaryBitmap) -> Vec<Contour> {
    let w = bitmap.width as i32;
    let h = bitmap.height as i32;

    // Track which pixels have been assigned to a contour boundary
    let mut visited = BinaryBitmap::new(bitmap.width, bitmap.height);
    let mut contours = Vec::new();

    // Scan top-to-bottom, left-to-right
    for y in 0..h {
        let mut was_foreground = false;
        for x in 0..w {
            let is_fg = bitmap.get(x, y);

            if is_fg && !was_foreground && !visited.get(x, y) {
                // Found an outer boundary: transition from background to foreground
                if let Some(points) = trace_boundary(bitmap, x, y, &mut visited) {
                    if points.len() >= 3 {
                        let is_outer = polygon_signed_area(&points) > 0.0;
                        contours.push(Contour { points, is_outer });
                    }
                }
            } else if !is_fg && was_foreground {
                // Transition from foreground to background — potential inner boundary (hole)
                let bx = x - 1; // the last foreground pixel
                if bx >= 0 && !visited.get(bx, y) {
                    if let Some(points) = trace_boundary(bitmap, bx, y, &mut visited) {
                        if points.len() >= 3 {
                            let is_outer = polygon_signed_area(&points) > 0.0;
                            contours.push(Contour { points, is_outer });
                        }
                    }
                }
            }

            was_foreground = is_fg;
        }
    }

    contours
}

/// Trace the boundary of a single connected foreground region starting at (start_x, start_y).
///
/// Uses the Moore-Neighbor tracing algorithm. Returns the ordered boundary points.
fn trace_boundary(
    bitmap: &BinaryBitmap,
    start_x: i32,
    start_y: i32,
    visited: &mut BinaryBitmap,
) -> Option<Vec<(i32, i32)>> {
    if !bitmap.get(start_x, start_y) {
        return None;
    }

    let mut points = Vec::new();
    points.push((start_x, start_y));
    if start_x >= 0 && start_y >= 0 {
        visited.set(start_x as u32, start_y as u32, true);
    }

    // Determine the initial backtrack direction.
    // We entered from the left (scanning left-to-right), so backtrack direction is West (index 4).
    let initial_dir = 4; // West

    let mut cx = start_x;
    let mut cy = start_y;
    let mut dir = initial_dir;

    // Safety limit to prevent infinite loops on malformed bitmaps
    let max_steps = (bitmap.width as usize) * (bitmap.height as usize) * 4;

    for _ in 0..max_steps {
        // Search clockwise from the direction opposite to where we came from
        let search_start = (dir + 5) % 8; // start searching clockwise from (dir+1) offset from backtrack
        let mut found = false;

        for i in 0..8 {
            let check_dir = (search_start + i) % 8;
            let (dx, dy) = DIRECTIONS[check_dir];
            let nx = cx + dx;
            let ny = cy + dy;

            if bitmap.get(nx, ny) {
                // Found the next boundary pixel
                cx = nx;
                cy = ny;
                // The backtrack direction is opposite of the direction we moved
                dir = (check_dir + 4) % 8;

                if cx == start_x && cy == start_y {
                    // Returned to start — contour is complete
                    return Some(points);
                }

                points.push((cx, cy));
                if cx >= 0 && cy >= 0 && (cx as u32) < bitmap.width && (cy as u32) < bitmap.height {
                    visited.set(cx as u32, cy as u32, true);
                }

                found = true;
                break;
            }
        }

        if !found {
            // Isolated single pixel
            return Some(points);
        }
    }

    // If we exceeded the step limit, return what we have
    log::warn!(
        "Contour tracing exceeded max steps at ({}, {})",
        start_x,
        start_y
    );
    Some(points)
}

/// Compute the signed area of a polygon using the shoelace formula.
///
/// Positive = clockwise in screen coordinates (y-down), negative = counter-clockwise.
pub fn polygon_signed_area(points: &[(i32, i32)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0_f64;
    let n = points.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (x0, y0) = points[i];
        let (x1, y1) = points[j];
        area += (x0 as f64) * (y1 as f64);
        area -= (x1 as f64) * (y0 as f64);
    }
    area / 2.0
}

/// Compute the absolute area enclosed by a contour.
pub fn contour_area(contour: &Contour) -> f64 {
    polygon_signed_area(&contour.points).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threshold::BinaryBitmap;

    fn make_rect_bitmap(
        width: u32,
        height: u32,
        rx: u32,
        ry: u32,
        rw: u32,
        rh: u32,
    ) -> BinaryBitmap {
        let mut bmp = BinaryBitmap::new(width, height);
        for y in ry..(ry + rh).min(height) {
            for x in rx..(rx + rw).min(width) {
                bmp.set(x, y, true);
            }
        }
        bmp
    }

    #[test]
    fn test_find_contours_rectangle() {
        // A 5x5 filled rectangle within a 10x10 bitmap
        let bmp = make_rect_bitmap(10, 10, 2, 2, 5, 5);
        let contours = find_contours(&bmp);
        assert!(
            !contours.is_empty(),
            "Should find at least one contour for a rectangle"
        );
        // The contour should have at least 4 points (corners)
        let c = &contours[0];
        assert!(
            c.points.len() >= 4,
            "Rectangle contour should have at least 4 points, got {}",
            c.points.len()
        );
    }

    #[test]
    fn test_find_contours_single_pixel() {
        let mut bmp = BinaryBitmap::new(5, 5);
        bmp.set(2, 2, true);
        let contours = find_contours(&bmp);
        // A single pixel has an area less than 3 points — it may or may not appear
        // depending on the algorithm. Either 0 or 1 contours is acceptable.
        assert!(
            contours.len() <= 1,
            "Single pixel should produce at most 1 contour"
        );
    }

    #[test]
    fn test_find_contours_circle_approx() {
        // Create a rough filled circle of radius 10 centered at (15, 15) in a 30x30 bitmap
        let mut bmp = BinaryBitmap::new(30, 30);
        let cx = 15.0_f64;
        let cy = 15.0_f64;
        let r = 10.0_f64;
        for y in 0..30 {
            for x in 0..30 {
                let dx = (x as f64) - cx;
                let dy = (y as f64) - cy;
                if dx * dx + dy * dy <= r * r {
                    bmp.set(x, y, true);
                }
            }
        }

        let contours = find_contours(&bmp);
        assert!(
            !contours.is_empty(),
            "Should find at least one contour for a circle"
        );
        // The contour should have many points (circle has ~2πr boundary pixels)
        let c = &contours[0];
        assert!(
            c.points.len() >= 20,
            "Circle contour should have many boundary points, got {}",
            c.points.len()
        );
    }

    #[test]
    fn test_contour_area_triangle() {
        // A triangle with vertices (0,0), (4,0), (0,3) → area = 6.0
        let contour = Contour {
            points: vec![(0, 0), (4, 0), (0, 3)],
            is_outer: true,
        };
        let area = contour_area(&contour);
        assert!(
            (area - 6.0).abs() < 0.001,
            "Triangle area should be 6.0, got {}",
            area
        );
    }

    #[test]
    fn test_contour_area_square() {
        // A 10x10 square
        let contour = Contour {
            points: vec![(0, 0), (10, 0), (10, 10), (0, 10)],
            is_outer: true,
        };
        let area = contour_area(&contour);
        assert!(
            (area - 100.0).abs() < 0.001,
            "Square area should be 100.0, got {}",
            area
        );
    }

    #[test]
    fn test_polygon_signed_area_direction() {
        // Clockwise in y-down coordinates → positive
        let cw = vec![(0, 0), (10, 0), (10, 10), (0, 10)];
        assert!(
            polygon_signed_area(&cw) > 0.0,
            "CW winding should have positive signed area"
        );

        // Counter-clockwise → negative
        let ccw = vec![(0, 0), (0, 10), (10, 10), (10, 0)];
        assert!(
            polygon_signed_area(&ccw) < 0.0,
            "CCW winding should have negative signed area"
        );
    }
}
