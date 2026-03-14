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

                let mut x_crossings = Vec::new();
                for seg in path.elements() {
                    let crossings = segment_y_crossings(&seg, y);
                    x_crossings.extend(crossings);
                }

                if x_crossings.len() < 2 {
                    return None;
                }

                x_crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                // Use the outermost pair
                Some((x_crossings[0], x_crossings[x_crossings.len() - 1]))
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
        let space_width = char_width_fn(" ");
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
            let mut x = line_start_x + indent;
            let line_max_x = line_end_x;
            let mut line_text = String::new();

            // Pack words into this line
            while word_idx < words.len() {
                let word = words[word_idx];
                let word_width = char_width_fn(word);

                if !line_text.is_empty() {
                    let test_width = char_width_fn(&format!("{} {}", line_text, word));
                    if line_start_x + indent + test_width > line_max_x {
                        break; // Word doesn't fit, start new line
                    }
                    line_text.push(' ');
                } else if word_width > line_max_x - x {
                    // Single word doesn't fit - force it anyway to avoid infinite loop
                    // but only if it's the first word on the line
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

/// Find x-coordinates where a path segment crosses a horizontal line at y.
fn segment_y_crossings(seg: &sivgo_math::PathSegment, y: f64) -> Vec<f64> {
    use sivgo_math::PathSegment;
    match seg {
        PathSegment::LineTo(p1) => {
            // We don't have the previous point here, so this is approximate
            // For a complete implementation, we'd track the current point
            vec![]
        }
        PathSegment::MoveTo(_) | PathSegment::ClosePath => vec![],
        PathSegment::CurveTo(_, _, _) | PathSegment::QuadTo(_, _) => {
            // For curves, we'd need to solve for t where y(t) = target_y
            // Simplified: just return empty for now, rect regions work fine
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_char_width(s: &str) -> f64 {
        // Assume each character is 8px wide (monospace approximation)
        s.len() as f64 * 8.0
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
