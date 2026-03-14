//! Centerline tracing for line art.
//!
//! Unlike boundary tracing which outlines regions, centerline tracing finds
//! the medial axis (skeleton) of shapes, producing single-stroke paths.
//! This is useful for tracing hand-drawn line art, text, or technical drawings.
//!
//! Algorithm:
//! 1. Threshold the image to binary.
//! 2. Apply Zhang-Suen thinning to reduce shapes to 1-pixel-wide skeletons.
//! 3. Walk the skeleton pixels to produce connected polylines.
//! 4. Smooth the polylines into Bezier curves.

use sivgo_math::{BezPath, Point};

use crate::bitmap::GrayBitmap;
use crate::threshold::{threshold, BinaryBitmap};

/// Options for centerline tracing.
#[derive(Debug, Clone)]
pub struct CenterlineOptions {
    /// Brightness threshold for binarization.
    /// `None` = automatic (Otsu's method).
    pub threshold: Option<u8>,

    /// Smoothing factor for output curves (0.0 = polyline, 1.0 = full smoothing).
    /// Default: 1.0.
    pub smoothing: f64,

    /// Minimum chain length in pixels. Chains shorter than this are discarded.
    /// Default: 5.
    pub min_length: usize,
}

impl Default for CenterlineOptions {
    fn default() -> Self {
        Self {
            threshold: None,
            smoothing: 1.0,
            min_length: 5,
        }
    }
}

/// Apply Zhang-Suen thinning to a binary bitmap, producing a skeleton.
///
/// Returns a new `BinaryBitmap` where foreground pixels form 1-pixel-wide lines.
pub fn thin(binary: &BinaryBitmap) -> BinaryBitmap {
    let w = binary.width;
    let h = binary.height;
    let mut data: Vec<bool> = (0..((w as usize) * (h as usize)))
        .map(|i| {
            let x = (i % w as usize) as i32;
            let y = (i / w as usize) as i32;
            binary.get(x, y)
        })
        .collect();

    let get = |d: &[bool], x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            false
        } else {
            d[(y as usize) * (w as usize) + (x as usize)]
        }
    };

    loop {
        let mut changed = false;

        // Sub-iteration 1
        let mut to_remove = Vec::new();
        for y in 1..(h as i32 - 1) {
            for x in 1..(w as i32 - 1) {
                if !get(&data, x, y) {
                    continue;
                }
                // 8-neighbors: P2=N, P3=NE, P4=E, P5=SE, P6=S, P7=SW, P8=W, P9=NW
                let p2 = get(&data, x, y - 1);
                let p3 = get(&data, x + 1, y - 1);
                let p4 = get(&data, x + 1, y);
                let p5 = get(&data, x + 1, y + 1);
                let p6 = get(&data, x, y + 1);
                let p7 = get(&data, x - 1, y + 1);
                let p8 = get(&data, x - 1, y);
                let p9 = get(&data, x - 1, y - 1);

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];
                let b = neighbors.iter().filter(|&&v| v).count();
                if !(2..=6).contains(&b) {
                    continue;
                }

                // Count 0->1 transitions in the ordered sequence P2..P9..P2
                let a = transitions(&neighbors);
                if a != 1 {
                    continue;
                }

                // Conditions for sub-iteration 1
                if p2 && p4 && p6 {
                    continue;
                }
                if p4 && p6 && p8 {
                    continue;
                }

                to_remove.push((x as usize, y as usize));
            }
        }
        for (x, y) in &to_remove {
            data[y * w as usize + x] = false;
            changed = true;
        }

        // Sub-iteration 2
        let mut to_remove = Vec::new();
        for y in 1..(h as i32 - 1) {
            for x in 1..(w as i32 - 1) {
                if !get(&data, x, y) {
                    continue;
                }
                let p2 = get(&data, x, y - 1);
                let p3 = get(&data, x + 1, y - 1);
                let p4 = get(&data, x + 1, y);
                let p5 = get(&data, x + 1, y + 1);
                let p6 = get(&data, x, y + 1);
                let p7 = get(&data, x - 1, y + 1);
                let p8 = get(&data, x - 1, y);
                let p9 = get(&data, x - 1, y - 1);

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];
                let b = neighbors.iter().filter(|&&v| v).count();
                if !(2..=6).contains(&b) {
                    continue;
                }

                let a = transitions(&neighbors);
                if a != 1 {
                    continue;
                }

                // Conditions for sub-iteration 2
                if p2 && p4 && p8 {
                    continue;
                }
                if p2 && p6 && p8 {
                    continue;
                }

                to_remove.push((x as usize, y as usize));
            }
        }
        for (x, y) in &to_remove {
            data[y * w as usize + x] = false;
            changed = true;
        }

        if !changed {
            break;
        }
    }

    let mut result = BinaryBitmap::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if data[(y as usize) * (w as usize) + (x as usize)] {
                result.set(x, y, true);
            }
        }
    }
    result
}

/// Count 0->1 transitions in the circular neighbor sequence.
fn transitions(neighbors: &[bool; 8]) -> usize {
    let mut count = 0;
    for i in 0..8 {
        if !neighbors[i] && neighbors[(i + 1) % 8] {
            count += 1;
        }
    }
    count
}

/// Walk skeleton pixels to extract connected chains of points.
fn extract_chains(skeleton: &BinaryBitmap) -> Vec<Vec<(u32, u32)>> {
    let w = skeleton.width;
    let h = skeleton.height;
    let mut visited = vec![false; (w as usize) * (h as usize)];

    let mut chains = Vec::new();

    // Find endpoints and junction points to start walks
    for y in 0..h {
        for x in 0..w {
            let idx = (y as usize) * (w as usize) + (x as usize);
            if !skeleton.get(x as i32, y as i32) || visited[idx] {
                continue;
            }

            // Walk from this pixel
            let mut chain = Vec::new();
            let mut cx = x;
            let mut cy = y;
            loop {
                let cidx = (cy as usize) * (w as usize) + (cx as usize);
                if visited[cidx] {
                    break;
                }
                visited[cidx] = true;
                chain.push((cx, cy));

                // Find unvisited neighbor
                let mut found = false;
                for &(dx, dy) in &[
                    (-1i32, -1i32),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let nidx = (ny as usize) * (w as usize) + (nx as usize);
                    if skeleton.get(nx, ny) && !visited[nidx] {
                        cx = nx as u32;
                        cy = ny as u32;
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }

            if chain.len() >= 2 {
                chains.push(chain);
            }
        }
    }

    chains
}

/// Smooth a chain of pixel coordinates into a BezPath.
fn smooth_chain(chain: &[(u32, u32)], smoothing: f64) -> BezPath {
    let mut path = BezPath::new();
    if chain.is_empty() {
        return path;
    }

    let points: Vec<Point> = chain
        .iter()
        .map(|&(x, y)| Point::new(x as f64, y as f64))
        .collect();

    path.move_to(points[0]);

    if points.len() < 3 || smoothing < 1e-6 {
        // Just line segments
        for &p in &points[1..] {
            path.line_to(p);
        }
        return path;
    }

    // Catmull-Rom to cubic Bezier conversion
    let alpha = smoothing.clamp(0.0, 1.0);

    for i in 0..points.len() - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < points.len() {
            points[i + 2]
        } else {
            points[i + 1]
        };

        let cp1 = Point::new(
            p1.x + alpha * (p2.x - p0.x) / 6.0,
            p1.y + alpha * (p2.y - p0.y) / 6.0,
        );
        let cp2 = Point::new(
            p2.x - alpha * (p3.x - p1.x) / 6.0,
            p2.y - alpha * (p3.y - p1.y) / 6.0,
        );
        path.curve_to(cp1, cp2, p2);
    }

    path
}

/// Trace the centerlines (skeleton) of shapes in a grayscale bitmap.
///
/// Returns one `BezPath` per connected skeleton segment.
pub fn trace_centerline(bitmap: &GrayBitmap, options: &CenterlineOptions) -> Vec<BezPath> {
    let thresh = options
        .threshold
        .unwrap_or_else(|| crate::threshold::otsu_threshold(bitmap));

    let binary = threshold(bitmap, thresh);
    let skeleton = thin(&binary);
    let chains = extract_chains(&skeleton);

    chains
        .iter()
        .filter(|c| c.len() >= options.min_length)
        .map(|c| smooth_chain(c, options.smoothing))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thin_horizontal_bar() {
        // A thick horizontal bar should thin to roughly a single line
        let mut bmp = GrayBitmap::new(30, 10);
        for y in 2..8 {
            for x in 2..28 {
                bmp.set(x, y, 0); // black
            }
        }
        let binary = threshold(&bmp, 128);
        let skeleton = thin(&binary);

        // Count skeleton pixels - should be much fewer than the original
        let mut skel_count = 0;
        let mut orig_count = 0;
        for y in 0..10 {
            for x in 0..30 {
                if skeleton.get(x as i32, y as i32) {
                    skel_count += 1;
                }
                if binary.get(x as i32, y as i32) {
                    orig_count += 1;
                }
            }
        }
        assert!(
            skel_count < orig_count,
            "Skeleton ({skel_count}) should have fewer pixels than original ({orig_count})"
        );
        assert!(skel_count > 0, "Skeleton should not be empty");
    }

    #[test]
    fn trace_centerline_horizontal_line() {
        // A horizontal bar
        let mut bmp = GrayBitmap::new(40, 10);
        for y in 3..7 {
            for x in 5..35 {
                bmp.set(x, y, 0);
            }
        }
        let opts = CenterlineOptions {
            threshold: Some(128),
            smoothing: 0.0,
            min_length: 3,
        };
        let paths = trace_centerline(&bmp, &opts);
        assert!(
            !paths.is_empty(),
            "Should produce at least one centerline path"
        );
    }

    #[test]
    fn trace_centerline_empty() {
        let bmp = GrayBitmap::new(20, 20); // all white
        let opts = CenterlineOptions::default();
        let paths = trace_centerline(&bmp, &opts);
        assert!(
            paths.is_empty(),
            "White image should produce no centerlines"
        );
    }

    #[test]
    fn transitions_count() {
        // 0,1,0,0,0,0,0,0 -> one transition (at index 0->1)
        let n = [false, true, false, false, false, false, false, false];
        assert_eq!(transitions(&n), 1);
    }
}
