//! SVG 2 mesh gradient rasterization.
//!
//! A mesh gradient is a grid of **Coons patches**. Each patch has four
//! boundary curves (cubic Beziers or lines) and four corner colors. The
//! interior is filled by bilinear interpolation of the corner colors across
//! a (u, v) parameter space, with the boundary curves defining the spatial
//! mapping.
//!
//! Because tiny-skia does not have a native mesh gradient shader, we
//! rasterize the mesh into a pixel buffer and wrap it as a non-repeating
//! pattern paint.
//!
//! # SVG 2 structure
//!
//! ```xml
//! <meshGradient x="50" y="50">
//!   <meshrow>
//!     <meshpatch>
//!       <stop path="C 100,50 150,50 200,50" stop-color="red"/>
//!       <stop path="C 200,100 200,150 200,200" stop-color="blue"/>
//!       <stop path="C 150,200 100,200 50,200" stop-color="green"/>
//!       <stop path="C 50,150 50,100 50,50" stop-color="yellow"/>
//!     </meshpatch>
//!   </meshrow>
//! </meshGradient>
//! ```
//!
//! The first stop starts from the meshGradient's (x, y). Each subsequent
//! stop's path starts implicitly from the previous stop's endpoint.
//! The fourth stop's path returns to the starting point.

use sivgo_dom::Color;
use sivgo_math::BezPath;

/// A single Coons patch with four boundary curves and four corner colors.
///
/// The boundary curves are stored as sequences of control points (cubic
/// Bezier) in the standard Coons patch order:
///
/// ```text
///   C0 ---top---> C1
///   |              |
///  left          right
///   |              |
///   C3 <--bottom-- C2
/// ```
///
/// Each edge is a cubic Bezier with 4 control points (start, cp1, cp2, end).
#[derive(Debug, Clone)]
pub struct CoonsPatch {
    /// Top edge: 4 control points from C0 to C1.
    pub top: [(f64, f64); 4],
    /// Right edge: 4 control points from C1 to C2.
    pub right: [(f64, f64); 4],
    /// Bottom edge: 4 control points from C2 to C3 (note: stored C2→C3).
    pub bottom: [(f64, f64); 4],
    /// Left edge: 4 control points from C3 to C0 (note: stored C3→C0).
    pub left: [(f64, f64); 4],
    /// Corner colors: [C0, C1, C2, C3] (top-left, top-right, bottom-right, bottom-left).
    pub colors: [Color; 4],
}

/// Parse the edge path data from a mesh gradient stop's `path` attribute.
///
/// The path attribute contains curve commands (C, L, or implicit L)
/// relative to a start point. Returns a list of control points forming
/// a cubic Bezier from `start` to the endpoint.
///
/// If the path has multiple segments, only the last cubic is used.
/// Lines are promoted to degenerate cubics.
fn parse_edge_path(start: (f64, f64), path_data: &str) -> Option<[(f64, f64); 4]> {
    // Try parsing as a full SVG path by prepending a MoveTo
    let full_path = format!("M {},{} {}", start.0, start.1, path_data);
    let bez = BezPath::from_svg_path(&full_path).ok()?;
    let segments = bez.elements();

    // Walk the segments to find the cubic (or line) after the initial MoveTo.
    let mut current = start;
    let mut result: Option<[(f64, f64); 4]> = None;

    for seg in &segments {
        match seg {
            sivgo_math::PathSegment::MoveTo(p) => {
                current = (p.x, p.y);
            }
            sivgo_math::PathSegment::LineTo(p) => {
                // Promote line to degenerate cubic.
                let end = (p.x, p.y);
                let cp1 = (
                    current.0 + (end.0 - current.0) / 3.0,
                    current.1 + (end.1 - current.1) / 3.0,
                );
                let cp2 = (
                    current.0 + 2.0 * (end.0 - current.0) / 3.0,
                    current.1 + 2.0 * (end.1 - current.1) / 3.0,
                );
                result = Some([current, cp1, cp2, end]);
                current = end;
            }
            sivgo_math::PathSegment::CurveTo(c1, c2, end) => {
                result = Some([current, (c1.x, c1.y), (c2.x, c2.y), (end.x, end.y)]);
                current = (end.x, end.y);
            }
            sivgo_math::PathSegment::QuadTo(ctrl, end) => {
                // Promote quadratic to cubic.
                let cp1 = (
                    current.0 + 2.0 / 3.0 * (ctrl.x - current.0),
                    current.1 + 2.0 / 3.0 * (ctrl.y - current.1),
                );
                let cp2 = (
                    end.x + 2.0 / 3.0 * (ctrl.x - end.x),
                    end.y + 2.0 / 3.0 * (ctrl.y - end.y),
                );
                result = Some([current, cp1, cp2, (end.x, end.y)]);
                current = (end.x, end.y);
            }
            sivgo_math::PathSegment::ClosePath => {}
        }
    }

    result
}

/// Create a straight-line edge (degenerate cubic) between two points.
fn line_edge(from: (f64, f64), to: (f64, f64)) -> [(f64, f64); 4] {
    let cp1 = (
        from.0 + (to.0 - from.0) / 3.0,
        from.1 + (to.1 - from.1) / 3.0,
    );
    let cp2 = (
        from.0 + 2.0 * (to.0 - from.0) / 3.0,
        from.1 + 2.0 * (to.1 - from.1) / 3.0,
    );
    [from, cp1, cp2, to]
}

/// Reverse the direction of a cubic Bezier edge.
fn reverse_edge(edge: &[(f64, f64); 4]) -> [(f64, f64); 4] {
    [edge[3], edge[2], edge[1], edge[0]]
}

/// Evaluate a cubic Bezier at parameter `t`.
fn cubic_eval(pts: &[(f64, f64); 4], t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let u2 = u * u;
    let u3 = u2 * u;
    let t2 = t * t;
    let t3 = t2 * t;
    (
        u3 * pts[0].0 + 3.0 * u2 * t * pts[1].0 + 3.0 * u * t2 * pts[2].0 + t3 * pts[3].0,
        u3 * pts[0].1 + 3.0 * u2 * t * pts[1].1 + 3.0 * u * t2 * pts[2].1 + t3 * pts[3].1,
    )
}

/// Evaluate the Coons patch surface at parameters (u, v) in [0, 1]^2.
///
/// Uses the standard Coons patch formula:
///   S(u,v) = L_c(u,v) + L_d(u,v) - B(u,v)
/// where:
///   L_c = ruled surface across top/bottom edges
///   L_d = ruled surface across left/right edges
///   B   = bilinear interpolation of the four corners
fn coons_eval(patch: &CoonsPatch, u: f64, v: f64) -> (f64, f64) {
    // L_c(u, v): interpolates between top(u) and bottom(u)
    let top_pt = cubic_eval(&patch.top, u);
    // bottom is stored C2→C3, but we need the curve going C3→C2 (i.e., in
    // the same u direction as top). So we reverse it.
    let bottom_rev = reverse_edge(&patch.bottom);
    let bottom_pt = cubic_eval(&bottom_rev, u);
    let lc = (
        (1.0 - v) * top_pt.0 + v * bottom_pt.0,
        (1.0 - v) * top_pt.1 + v * bottom_pt.1,
    );

    // L_d(u, v): interpolates between left(v) and right(v)
    // left is stored C3→C0, we need C0→C3 direction (v goes from top to bottom)
    let left_fwd = reverse_edge(&patch.left);
    let left_pt = cubic_eval(&left_fwd, v);
    let right_pt = cubic_eval(&patch.right, v);
    let ld = (
        (1.0 - u) * left_pt.0 + u * right_pt.0,
        (1.0 - u) * left_pt.1 + u * right_pt.1,
    );

    // B(u, v): bilinear interpolation of corners
    let c0 = patch.top[0];
    let c1 = patch.top[3];
    let c2 = patch.bottom[0]; // C2
    let c3 = patch.bottom[3]; // C3
    let b = (
        (1.0 - u) * (1.0 - v) * c0.0 + u * (1.0 - v) * c1.0 + u * v * c2.0 + (1.0 - u) * v * c3.0,
        (1.0 - u) * (1.0 - v) * c0.1 + u * (1.0 - v) * c1.1 + u * v * c2.1 + (1.0 - u) * v * c3.1,
    );

    // S(u,v) = L_c + L_d - B
    (lc.0 + ld.0 - b.0, lc.1 + ld.1 - b.1)
}

/// Bilinearly interpolate the corner colors at (u, v).
fn color_at(colors: &[Color; 4], u: f64, v: f64) -> Color {
    let r = (1.0 - u) * (1.0 - v) * colors[0].r as f64
        + u * (1.0 - v) * colors[1].r as f64
        + u * v * colors[2].r as f64
        + (1.0 - u) * v * colors[3].r as f64;
    let g = (1.0 - u) * (1.0 - v) * colors[0].g as f64
        + u * (1.0 - v) * colors[1].g as f64
        + u * v * colors[2].g as f64
        + (1.0 - u) * v * colors[3].g as f64;
    let b = (1.0 - u) * (1.0 - v) * colors[0].b as f64
        + u * (1.0 - v) * colors[1].b as f64
        + u * v * colors[2].b as f64
        + (1.0 - u) * v * colors[3].b as f64;
    let a = (1.0 - u) * (1.0 - v) * colors[0].a as f64
        + u * (1.0 - v) * colors[1].a as f64
        + u * v * colors[2].a as f64
        + (1.0 - u) * v * colors[3].a as f64;
    Color::new(
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        a.round().clamp(0.0, 255.0) as u8,
    )
}

/// Compute the bounding box of a Coons patch by sampling its boundary curves.
fn patch_bbox(patch: &CoonsPatch) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let edges = [&patch.top, &patch.right, &patch.bottom, &patch.left];
    for edge in &edges {
        for i in 0..=20 {
            let t = i as f64 / 20.0;
            let (x, y) = cubic_eval(edge, t);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    (min_x, min_y, max_x, max_y)
}

/// Rasterize a single Coons patch into a pixel buffer.
///
/// The patch is subdivided into `subdivisions x subdivisions` micro-quads.
/// Each micro-quad is filled with the interpolated color.
///
/// `pixels` is a mutable RGBA buffer of size `width * height * 4`.
/// `offset_x` and `offset_y` translate the patch coordinates to pixel coordinates.
fn rasterize_patch(
    patch: &CoonsPatch,
    pixels: &mut [u8],
    width: u32,
    height: u32,
    offset_x: f64,
    offset_y: f64,
    subdivisions: u32,
) {
    let sub = subdivisions as f64;

    for iv in 0..subdivisions {
        for iu in 0..subdivisions {
            let u_mid = (iu as f64 + 0.5) / sub;
            let v_mid = (iv as f64 + 0.5) / sub;

            // Evaluate position and color at the center of this micro-quad
            let (px, py) = coons_eval(patch, u_mid, v_mid);
            let color = color_at(&patch.colors, u_mid, v_mid);

            // Also compute the 4 corners of this micro-quad to determine
            // the pixel region to fill.
            let u0 = iu as f64 / sub;
            let u1 = (iu + 1) as f64 / sub;
            let v0 = iv as f64 / sub;
            let v1 = (iv + 1) as f64 / sub;

            let corners = [
                coons_eval(patch, u0, v0),
                coons_eval(patch, u1, v0),
                coons_eval(patch, u1, v1),
                coons_eval(patch, u0, v1),
            ];

            // Find the bounding box of the micro-quad in pixel space
            let mut qmin_x = f64::MAX;
            let mut qmin_y = f64::MAX;
            let mut qmax_x = f64::MIN;
            let mut qmax_y = f64::MIN;
            for &(cx, cy) in &corners {
                let sx = cx - offset_x;
                let sy = cy - offset_y;
                qmin_x = qmin_x.min(sx);
                qmin_y = qmin_y.min(sy);
                qmax_x = qmax_x.max(sx);
                qmax_y = qmax_y.max(sy);
            }

            // Also include the center point
            let sx_center = px - offset_x;
            let sy_center = py - offset_y;
            qmin_x = qmin_x.min(sx_center);
            qmin_y = qmin_y.min(sy_center);
            qmax_x = qmax_x.max(sx_center);
            qmax_y = qmax_y.max(sy_center);

            let pix_x0 = (qmin_x.floor() as i32).max(0) as u32;
            let pix_y0 = (qmin_y.floor() as i32).max(0) as u32;
            let pix_x1 = ((qmax_x.ceil() as i32) as u32).min(width);
            let pix_y1 = ((qmax_y.ceil() as i32) as u32).min(height);

            // Premultiply alpha for the color
            let a = color.a as f64 / 255.0;
            let pr = (color.r as f64 * a).round() as u8;
            let pg = (color.g as f64 * a).round() as u8;
            let pb = (color.b as f64 * a).round() as u8;
            for py in pix_y0..pix_y1 {
                for px in pix_x0..pix_x1 {
                    let idx = (py as usize * width as usize + px as usize) * 4;
                    if idx + 3 < pixels.len() {
                        // Source-over compositing (existing pixel may have been
                        // written by a neighboring micro-quad).
                        let dst_a = pixels[idx + 3] as f64 / 255.0;
                        let dst_r = pixels[idx] as f64;
                        let dst_g = pixels[idx + 1] as f64;
                        let dst_b = pixels[idx + 2] as f64;

                        let out_a = a + dst_a * (1.0 - a);
                        if out_a > 0.0 {
                            pixels[idx] = ((pr as f64 + dst_r * (1.0 - a)) / out_a * out_a)
                                .round()
                                .clamp(0.0, 255.0) as u8;
                            pixels[idx + 1] = ((pg as f64 + dst_g * (1.0 - a)) / out_a * out_a)
                                .round()
                                .clamp(0.0, 255.0)
                                as u8;
                            pixels[idx + 2] = ((pb as f64 + dst_b * (1.0 - a)) / out_a * out_a)
                                .round()
                                .clamp(0.0, 255.0)
                                as u8;
                            pixels[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }
    }
}

/// Collected data for one mesh patch: 4 edges and up to 4 stop colors.
#[derive(Debug)]
pub struct MeshPatchData {
    /// The stop colors for this patch (up to 4).
    pub colors: Vec<Color>,
    /// The edge path data strings for each stop.
    pub edge_paths: Vec<Option<String>>,
}

/// Build Coons patches from the parsed mesh gradient structure.
///
/// `origin` is the (x, y) from the meshGradient element.
/// `rows` contains the patch data for each row, each row containing patches.
pub fn build_patches(origin: (f64, f64), rows: &[Vec<MeshPatchData>]) -> Vec<CoonsPatch> {
    let mut patches = Vec::new();

    for (_row_idx, row) in rows.iter().enumerate() {
        for (_patch_idx, patch_data) in row.iter().enumerate() {
            let num_stops = patch_data.colors.len().min(4);
            if num_stops < 2 {
                continue;
            }

            // Build the 4 edges of this patch.
            // The first stop starts from the origin (for the first patch)
            // or from the previous patch's endpoint.
            let mut current_pos = origin;
            let mut edges: Vec<[(f64, f64); 4]> = Vec::new();

            for i in 0..num_stops {
                let edge = if let Some(Some(ref path_str)) = patch_data.edge_paths.get(i) {
                    if let Some(e) = parse_edge_path(current_pos, path_str) {
                        e
                    } else {
                        // If path parsing fails, try a straight line to origin
                        // for the closing edge.
                        if i == num_stops - 1 {
                            line_edge(current_pos, origin)
                        } else {
                            continue;
                        }
                    }
                } else {
                    // No path data: straight line back to origin for closing edge
                    if i == num_stops - 1 {
                        line_edge(current_pos, origin)
                    } else if i == 0 {
                        // First stop with no path: degenerate, skip
                        continue;
                    } else {
                        continue;
                    }
                };

                current_pos = edge[3];
                edges.push(edge);
            }

            // We need exactly 4 edges for a Coons patch. If we have fewer,
            // fill in with straight lines.
            while edges.len() < 4 {
                let last_end = edges.last().map(|e| e[3]).unwrap_or(origin);
                let target = if edges.len() == 3 {
                    origin // Close back to origin
                } else {
                    last_end // Degenerate
                };
                edges.push(line_edge(last_end, target));
            }

            // Colors: pad with last color if fewer than 4
            let mut colors = [Color::BLACK; 4];
            for i in 0..4 {
                colors[i] = if i < patch_data.colors.len() {
                    patch_data.colors[i]
                } else if !patch_data.colors.is_empty() {
                    patch_data.colors[patch_data.colors.len() - 1]
                } else {
                    Color::BLACK
                };
            }

            // Map edges to Coons patch sides:
            // Stop 0: top edge (C0 → C1)
            // Stop 1: right edge (C1 → C2)
            // Stop 2: bottom edge (C2 → C3)
            // Stop 3: left edge (C3 → C0)
            patches.push(CoonsPatch {
                top: edges[0],
                right: edges[1],
                bottom: edges[2],
                left: edges[3],
                colors,
            });
        }
    }

    patches
}

/// Rasterize a complete mesh gradient into a pixel buffer.
///
/// Returns `(width, height, pixels)` where `pixels` is premultiplied RGBA.
/// Returns `None` if no patches could be built.
///
/// The `offset_x` and `offset_y` in the returned result indicate the
/// top-left corner of the rasterized region in user-space coordinates.
pub fn rasterize_mesh(
    patches: &[CoonsPatch],
    padding: f64,
) -> Option<(u32, u32, f64, f64, Vec<u8>)> {
    if patches.is_empty() {
        return None;
    }

    // Compute overall bounding box of all patches
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for patch in patches {
        let (pmin_x, pmin_y, pmax_x, pmax_y) = patch_bbox(patch);
        min_x = min_x.min(pmin_x);
        min_y = min_y.min(pmin_y);
        max_x = max_x.max(pmax_x);
        max_y = max_y.max(pmax_y);
    }

    min_x -= padding;
    min_y -= padding;
    max_x += padding;
    max_y += padding;

    let w = (max_x - min_x).ceil().max(1.0).min(4096.0) as u32;
    let h = (max_y - min_y).ceil().max(1.0).min(4096.0) as u32;

    if w == 0 || h == 0 {
        return None;
    }

    let mut pixels = vec![0u8; w as usize * h as usize * 4];

    // Determine subdivision level based on patch size
    for patch in patches {
        let (pmin_x, pmin_y, pmax_x, pmax_y) = patch_bbox(patch);
        let patch_w = pmax_x - pmin_x;
        let patch_h = pmax_y - pmin_y;
        let max_dim = patch_w.max(patch_h);
        // Use ~1 subdivision per 2 pixels for reasonable quality
        let subdivisions = (max_dim / 2.0).ceil().clamp(8.0, 256.0) as u32;

        rasterize_patch(patch, &mut pixels, w, h, min_x, min_y, subdivisions);
    }

    Some((w, h, min_x, min_y, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_eval_endpoints() {
        let pts = [(0.0, 0.0), (1.0, 2.0), (3.0, 4.0), (5.0, 6.0)];
        let start = cubic_eval(&pts, 0.0);
        assert!((start.0 - 0.0).abs() < 1e-10);
        assert!((start.1 - 0.0).abs() < 1e-10);
        let end = cubic_eval(&pts, 1.0);
        assert!((end.0 - 5.0).abs() < 1e-10);
        assert!((end.1 - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_line_edge() {
        let edge = line_edge((0.0, 0.0), (9.0, 9.0));
        assert_eq!(edge[0], (0.0, 0.0));
        assert_eq!(edge[3], (9.0, 9.0));
        // Midpoint should be at (4.5, 4.5)
        let mid = cubic_eval(&edge, 0.5);
        assert!((mid.0 - 4.5).abs() < 1e-10);
        assert!((mid.1 - 4.5).abs() < 1e-10);
    }

    #[test]
    fn test_reverse_edge() {
        let edge = [(1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0)];
        let rev = reverse_edge(&edge);
        assert_eq!(rev[0], (7.0, 8.0));
        assert_eq!(rev[3], (1.0, 2.0));
    }

    #[test]
    fn test_color_at_corners() {
        let colors = [
            Color::new(255, 0, 0, 255),
            Color::new(0, 255, 0, 255),
            Color::new(0, 0, 255, 255),
            Color::new(255, 255, 0, 255),
        ];
        let c00 = color_at(&colors, 0.0, 0.0);
        assert_eq!(c00.r, 255);
        assert_eq!(c00.g, 0);
        assert_eq!(c00.b, 0);

        let c10 = color_at(&colors, 1.0, 0.0);
        assert_eq!(c10.r, 0);
        assert_eq!(c10.g, 255);
        assert_eq!(c10.b, 0);
    }

    #[test]
    fn test_parse_edge_path_cubic() {
        let edge = parse_edge_path((0.0, 0.0), "C 50,0 100,0 100,50");
        assert!(edge.is_some());
        let e = edge.unwrap();
        assert_eq!(e[0], (0.0, 0.0));
        assert!((e[3].0 - 100.0).abs() < 1e-10);
        assert!((e[3].1 - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_edge_path_line() {
        let edge = parse_edge_path((0.0, 0.0), "L 100,100");
        assert!(edge.is_some());
        let e = edge.unwrap();
        assert_eq!(e[0], (0.0, 0.0));
        assert!((e[3].0 - 100.0).abs() < 1e-10);
        assert!((e[3].1 - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_coons_eval_square_patch() {
        // A simple square patch with straight-line edges
        let patch = CoonsPatch {
            top: line_edge((0.0, 0.0), (100.0, 0.0)),
            right: line_edge((100.0, 0.0), (100.0, 100.0)),
            bottom: line_edge((100.0, 100.0), (0.0, 100.0)),
            left: line_edge((0.0, 100.0), (0.0, 0.0)),
            colors: [
                Color::new(255, 0, 0, 255),
                Color::new(0, 255, 0, 255),
                Color::new(0, 0, 255, 255),
                Color::new(255, 255, 0, 255),
            ],
        };

        // Center should be at (50, 50)
        let center = coons_eval(&patch, 0.5, 0.5);
        assert!((center.0 - 50.0).abs() < 1e-10, "center x: {}", center.0);
        assert!((center.1 - 50.0).abs() < 1e-10, "center y: {}", center.1);

        // Corner C0 (u=0, v=0) should be at (0, 0)
        let c0 = coons_eval(&patch, 0.0, 0.0);
        assert!((c0.0 - 0.0).abs() < 1e-10);
        assert!((c0.1 - 0.0).abs() < 1e-10);

        // Corner C1 (u=1, v=0) should be at (100, 0)
        let c1 = coons_eval(&patch, 1.0, 0.0);
        assert!((c1.0 - 100.0).abs() < 1e-10);
        assert!((c1.1 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_build_patches_simple() {
        let patch_data = MeshPatchData {
            colors: vec![
                Color::new(255, 0, 0, 255),
                Color::new(0, 255, 0, 255),
                Color::new(0, 0, 255, 255),
                Color::new(255, 255, 0, 255),
            ],
            edge_paths: vec![
                Some("L 100,0".to_string()),
                Some("L 100,100".to_string()),
                Some("L 0,100".to_string()),
                Some("L 0,0".to_string()),
            ],
        };
        let rows = vec![vec![patch_data]];
        let patches = build_patches((0.0, 0.0), &rows);
        assert_eq!(patches.len(), 1);
    }

    #[test]
    fn test_rasterize_mesh_produces_pixels() {
        let patch = CoonsPatch {
            top: line_edge((0.0, 0.0), (50.0, 0.0)),
            right: line_edge((50.0, 0.0), (50.0, 50.0)),
            bottom: line_edge((50.0, 50.0), (0.0, 50.0)),
            left: line_edge((0.0, 50.0), (0.0, 0.0)),
            colors: [
                Color::new(255, 0, 0, 255),
                Color::new(0, 255, 0, 255),
                Color::new(0, 0, 255, 255),
                Color::new(255, 255, 0, 255),
            ],
        };

        let result = rasterize_mesh(&[patch], 1.0);
        assert!(result.is_some());
        let (w, h, _ox, _oy, pixels) = result.unwrap();
        assert!(w > 0);
        assert!(h > 0);
        assert_eq!(pixels.len(), w as usize * h as usize * 4);

        // Check that some pixels are non-zero (the patch should have
        // produced visible content)
        let has_content = pixels.iter().any(|&b| b > 0);
        assert!(has_content, "rasterized mesh should have non-zero pixels");
    }
}
