//! Flowed text in shapes (SVG 1.2 flowText).
//!
//! Supports laying out text within an arbitrary shape boundary.
//! The flow region is defined by a shape (typically a rect, but can be
//! any closed path). Text wraps within the region, breaking lines
//! when they exceed the available width at the current y position.

use sivgo_math::{BezPath, Point, Rect};

/// A flow region that defines the area where text can flow.
#[derive(Debug, Clone)]
pub enum FlowRegion {
    /// A simple rectangular region.
    Rect(Rect),
    /// A path-based region (text flows within the closed path).
    Path(BezPath),
}

impl FlowRegion {
    /// Get the bounding box of the region.
    pub fn bounds(&self) -> Rect {
        match self {
            FlowRegion::Rect(r) => *r,
            FlowRegion::Path(p) => p.bounding_box(),
        }
    }

    /// Get the horizontal span (min_x, max_x) at a given y coordinate.
    ///
    /// For a rect, this is simply the left and right edges.
    /// For a path, this requires scanning the path's edges at the given y.
    pub fn horizontal_span(&self, y: f64) -> Option<(f64, f64)> {
        match self {
            FlowRegion::Rect(r) => {
                if y >= r.y0 && y <= r.y1 {
                    Some((r.x0, r.x1))
                } else {
                    None
                }
            }
            FlowRegion::Path(path) => {
                // Find intersections of a horizontal scanline at y with the path
                let bounds = path.bounding_box();
                if y < bounds.y0 || y > bounds.y1 {
                    return None;
                }

                let mut x_crossings = path_y_crossings(path, y);
                if x_crossings.len() < 2 {
                    return None;
                }

                x_crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                // Crossings alternate between entering and leaving the shape
                // (even-odd rule); pick the widest interior span so text flows
                // in the largest available section at this height.
                x_crossings
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| (pair[0], pair[1]))
                    .max_by(|a, b| {
                        (a.1 - a.0)
                            .partial_cmp(&(b.1 - b.0))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            }
        }
    }
}

/// A paragraph of flowed text.
#[derive(Debug, Clone)]
pub struct FlowParagraph {
    /// The text content.
    pub text: String,
    /// Optional per-paragraph indent.
    pub indent: f64,
}

/// A positioned word/fragment in the flow layout.
#[derive(Debug, Clone)]
pub struct FlowFragment {
    /// The text of this fragment.
    pub text: String,
    /// Position (x, y) of the fragment baseline start.
    pub position: Point,
    /// Width of the fragment.
    pub width: f64,
}

/// Result of flowing text into a region.
#[derive(Debug, Clone)]
pub struct FlowLayout {
    /// All positioned text fragments.
    pub fragments: Vec<FlowFragment>,
    /// Whether all text fit within the region.
    pub overflow: bool,
}

/// Flow paragraphs into a region.
///
/// Parameters:
/// - `region`: The flow region defining the text area.
/// - `paragraphs`: The text paragraphs to flow.
/// - `font_size`: Font size in user units.
/// - `line_height`: Line height multiplier (1.0 = normal).
/// - `char_width_fn`: Function that returns the width of a string in the
///   current font at the given font size.
pub fn flow_text(
    region: &FlowRegion,
    paragraphs: &[FlowParagraph],
    font_size: f64,
    line_height: f64,
    char_width_fn: impl Fn(&str) -> f64,
) -> FlowLayout {
    let bounds = region.bounds();
    let actual_line_height = font_size * line_height;
    let mut fragments = Vec::new();
    let mut y = bounds.y0 + font_size; // Start at baseline of first line
    let mut overflow = false;

    for para in paragraphs {
        // Split into words
        let words: Vec<&str> = para.text.split_whitespace().collect();
        if words.is_empty() {
            y += actual_line_height;
            continue;
        }

        let mut word_idx = 0;
        let mut first_line_of_para = true;

        while word_idx < words.len() {
            // Check if we're still within the region
            if y > bounds.y1 {
                overflow = true;
                break;
            }

            let span = region.horizontal_span(y);
            let (line_start_x, line_end_x) = match span {
                Some(s) => s,
                None => {
                    y += actual_line_height;
                    continue;
                }
            };

            let indent = if first_line_of_para { para.indent } else { 0.0 };
            let x = line_start_x + indent;
            let mut line_text = String::new();

            // Pack words into this line. A first word that is wider than the
            // line is still placed to guarantee forward progress.
            while word_idx < words.len() {
                let word = words[word_idx];

                if !line_text.is_empty() {
                    let test_width = char_width_fn(&format!("{} {}", line_text, word));
                    if x + test_width > line_end_x {
                        break; // Word doesn't fit, start new line
                    }
                    line_text.push(' ');
                }

                line_text.push_str(word);
                word_idx += 1;
            }

            if !line_text.is_empty() {
                let frag_width = char_width_fn(&line_text);
                fragments.push(FlowFragment {
                    text: line_text,
                    position: Point::new(x, y),
                    width: frag_width,
                });
            }

            y += actual_line_height;
            first_line_of_para = false;
        }

        if overflow {
            break;
        }
    }

    FlowLayout {
        fragments,
        overflow,
    }
}

/// Number of line segments each Bezier curve is flattened into when
/// intersecting with a scanline. Text layout tolerates this approximation.
const CURVE_FLATTEN_STEPS: usize = 16;

/// Find all x-coordinates where the path crosses a horizontal line at `y`,
/// walking the segments while tracking the current point.
fn path_y_crossings(path: &BezPath, y: f64) -> Vec<f64> {
    use sivgo_math::PathSegment;

    let mut crossings = Vec::new();
    let mut current = Point::new(0.0, 0.0);
    let mut subpath_start = current;

    for seg in path.elements() {
        match seg {
            PathSegment::MoveTo(p) => {
                current = p;
                subpath_start = p;
            }
            PathSegment::LineTo(p) => {
                line_y_crossing(current, p, y, &mut crossings);
                current = p;
            }
            PathSegment::QuadTo(cp, end) => {
                let mut prev = current;
                for i in 1..=CURVE_FLATTEN_STEPS {
                    let t = i as f64 / CURVE_FLATTEN_STEPS as f64;
                    let u = 1.0 - t;
                    let next = Point::new(
                        u * u * current.x + 2.0 * u * t * cp.x + t * t * end.x,
                        u * u * current.y + 2.0 * u * t * cp.y + t * t * end.y,
                    );
                    line_y_crossing(prev, next, y, &mut crossings);
                    prev = next;
                }
                current = end;
            }
            PathSegment::CurveTo(cp1, cp2, end) => {
                let mut prev = current;
                for i in 1..=CURVE_FLATTEN_STEPS {
                    let t = i as f64 / CURVE_FLATTEN_STEPS as f64;
                    let u = 1.0 - t;
                    let next = Point::new(
                        u * u * u * current.x
                            + 3.0 * u * u * t * cp1.x
                            + 3.0 * u * t * t * cp2.x
                            + t * t * t * end.x,
                        u * u * u * current.y
                            + 3.0 * u * u * t * cp1.y
                            + 3.0 * u * t * t * cp2.y
                            + t * t * t * end.y,
                    );
                    line_y_crossing(prev, next, y, &mut crossings);
                    prev = next;
                }
                current = end;
            }
            PathSegment::ClosePath => {
                line_y_crossing(current, subpath_start, y, &mut crossings);
                current = subpath_start;
            }
        }
    }

    // An unclosed final subpath still bounds the region for scanline
    // purposes; close it implicitly.
    if (current.x - subpath_start.x).abs() > f64::EPSILON
        || (current.y - subpath_start.y).abs() > f64::EPSILON
    {
        line_y_crossing(current, subpath_start, y, &mut crossings);
    }

    crossings
}

/// Record where the line from `p0` to `p1` crosses the horizontal line at
/// `y`, if it does. The interval is half-open (`p0.y <= y < p1.y` or the
/// reverse) so a vertex shared by two segments is counted exactly once.
fn line_y_crossing(p0: Point, p1: Point, y: f64, crossings: &mut Vec<f64>) {
    if (p0.y <= y && y < p1.y) || (p1.y <= y && y < p0.y) {
        let t = (y - p0.y) / (p1.y - p0.y);
        crossings.push(p0.x + t * (p1.x - p0.x));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_char_width(s: &str) -> f64 {
        // Assume each character is 8px wide (monospace approximation)
        s.len() as f64 * 8.0
    }

    fn diamond_region() -> FlowRegion {
        // Diamond centered at (100, 100), 200 wide and 200 tall.
        let mut path = BezPath::new();
        path.move_to(Point::new(100.0, 0.0));
        path.line_to(Point::new(200.0, 100.0));
        path.line_to(Point::new(100.0, 200.0));
        path.line_to(Point::new(0.0, 100.0));
        path.close();
        FlowRegion::Path(path)
    }

    #[test]
    fn path_region_horizontal_span_follows_shape() {
        let region = diamond_region();

        let (x0, x1) = region.horizontal_span(100.0).expect("mid-height span");
        assert!((x0 - 0.0).abs() < 1e-9, "left edge at widest point");
        assert!((x1 - 200.0).abs() < 1e-9, "right edge at widest point");

        let (q0, q1) = region.horizontal_span(50.0).expect("quarter-height span");
        assert!(
            q1 - q0 < x1 - x0,
            "span should narrow above the diamond's widest point"
        );

        assert!(region.horizontal_span(-10.0).is_none());
        assert!(region.horizontal_span(210.0).is_none());
    }

    #[test]
    fn flow_text_path_region_stays_inside_shape() {
        let region = diamond_region();
        let paras = vec![FlowParagraph {
            text: "a b c d e f g h i j k l m n o p q r s t u v w x y z".to_string(),
            indent: 0.0,
        }];

        let layout = flow_text(&region, &paras, 10.0, 1.2, simple_char_width);
        assert!(
            !layout.fragments.is_empty(),
            "text should flow into the path"
        );
        for frag in &layout.fragments {
            let (sx, ex) = region
                .horizontal_span(frag.position.y)
                .expect("fragment baseline inside region");
            assert!(frag.position.x >= sx - 1e-6);
            assert!(frag.position.x + frag.width <= ex + 1e-6);
        }
    }

    #[test]
    fn flow_text_rect_basic() {
        let region = FlowRegion::Rect(Rect::new(0.0, 0.0, 200.0, 200.0));
        let paras = vec![FlowParagraph {
            text: "Hello world this is a test".to_string(),
            indent: 0.0,
        }];

        let layout = flow_text(&region, &paras, 16.0, 1.2, simple_char_width);
        assert!(!layout.fragments.is_empty());
        assert!(!layout.overflow);
    }

    #[test]
    fn flow_text_wraps_lines() {
        // Narrow region should force wrapping
        let region = FlowRegion::Rect(Rect::new(0.0, 0.0, 80.0, 200.0));
        let paras = vec![FlowParagraph {
            text: "Hello world this is a test of text wrapping".to_string(),
            indent: 0.0,
        }];

        let layout = flow_text(&region, &paras, 12.0, 1.2, simple_char_width);
        // Should produce multiple fragments (one per line)
        assert!(
            layout.fragments.len() > 1,
            "Text should wrap to multiple lines"
        );
    }

    #[test]
    fn flow_text_overflow() {
        // Very small region should overflow
        let region = FlowRegion::Rect(Rect::new(0.0, 0.0, 200.0, 20.0));
        let paras = vec![
            FlowParagraph {
                text: "First paragraph with lots of text that should overflow".to_string(),
                indent: 0.0,
            },
            FlowParagraph {
                text: "Second paragraph".to_string(),
                indent: 0.0,
            },
        ];

        let layout = flow_text(&region, &paras, 16.0, 1.5, simple_char_width);
        assert!(layout.overflow, "Should overflow in tiny region");
    }

    #[test]
    fn flow_text_empty() {
        let region = FlowRegion::Rect(Rect::new(0.0, 0.0, 200.0, 200.0));
        let paras: Vec<FlowParagraph> = Vec::new();
        let layout = flow_text(&region, &paras, 16.0, 1.2, simple_char_width);
        assert!(layout.fragments.is_empty());
        assert!(!layout.overflow);
    }

    #[test]
    fn flow_text_with_indent() {
        let region = FlowRegion::Rect(Rect::new(0.0, 0.0, 200.0, 200.0));
        let paras = vec![FlowParagraph {
            text: "Indented paragraph".to_string(),
            indent: 20.0,
        }];

        let layout = flow_text(&region, &paras, 16.0, 1.2, simple_char_width);
        assert!(!layout.fragments.is_empty());
        // First fragment should be indented
        assert!(
            layout.fragments[0].position.x >= 20.0,
            "First line should be indented"
        );
    }
}
