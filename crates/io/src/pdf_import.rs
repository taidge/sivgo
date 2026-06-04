//! Bounded PDF importer for simple vector/text pages.
//!
//! This is not a full PDF interpreter.  It parses indirect objects, page
//! dictionaries, MediaBox values, plain/Flate content streams, basic text
//! operators, rectangle/path painting operators, and image XObject metadata.
//! Unsupported PDF features are preserved as explicit image placeholders
//! instead of pretending the page was rasterized.

use std::collections::HashMap;
use std::io::Read;

use flate2::read::ZlibDecoder;
use sivgo_dom::{
    Color, ImageAttrs, LengthUnit, Paint, PathAttrs, PresentationAttrs, RectAttrs, SvgDocument,
    SvgElement, SvgLength, SvgSvgAttrs, TextAttrs, ViewBox,
};
use sivgo_math::{BezPath, Point};

/// Options for PDF import.
#[derive(Debug, Clone)]
pub struct PdfImportOptions {
    /// Zero-based page index to import.
    pub page: usize,
    /// Resolution (dots per inch) used for sizing PDF points into SVG pixels.
    pub dpi: f64,
}

impl Default for PdfImportOptions {
    fn default() -> Self {
        Self { page: 0, dpi: 96.0 }
    }
}

#[derive(Debug, Clone)]
struct PdfObject {
    id: u32,
    body: String,
    stream: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct PdfPage {
    media_box: [f64; 4],
    content_refs: Vec<u32>,
}

#[derive(Debug, Clone)]
struct PdfTextRun {
    text: String,
    x: f64,
    y: f64,
    font_size: f64,
}

#[derive(Debug, Clone)]
struct PdfPathRun {
    path: BezPath,
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f64,
}

#[derive(Debug, Clone)]
struct PdfImageRef {
    object_id: u32,
    width: f64,
    height: f64,
}

/// Import a PDF from raw bytes and return an `SvgDocument`.
///
/// Supported in this bounded importer:
///
/// * page selection and page sizing from `/MediaBox`;
/// * plain and `/FlateDecode` content streams;
/// * text in `BT`/`ET` blocks using `Tj` and `TJ`;
/// * simple vector rectangles and path paint operators;
/// * embedded image XObject metadata as SVG `<image>` placeholders.
pub fn import_pdf(data: &[u8], options: &PdfImportOptions) -> Result<SvgDocument, String> {
    if data.is_empty() {
        return Err("PDF data is empty".into());
    }
    if data.len() < 4 || &data[0..4] != b"%PDF" {
        return Err("Not a valid PDF (missing %PDF header)".into());
    }

    let objects = parse_pdf_objects(data);
    let object_map: HashMap<u32, PdfObject> = objects
        .iter()
        .map(|object| (object.id, object.clone()))
        .collect();
    let pages = parse_pages(&objects);

    let page = if pages.is_empty() {
        PdfPage {
            media_box: [0.0, 0.0, 612.0, 792.0],
            content_refs: objects
                .iter()
                .filter(|object| object.stream.is_some())
                .map(|object| object.id)
                .collect(),
        }
    } else {
        pages
            .get(options.page)
            .cloned()
            .ok_or_else(|| format!("PDF page {} is out of range", options.page))?
    };

    let scale = (options.dpi / 72.0).max(0.01);
    let page_w_pt = (page.media_box[2] - page.media_box[0]).abs().max(1.0);
    let page_h_pt = (page.media_box[3] - page.media_box[1]).abs().max(1.0);
    let width = page_w_pt * scale;
    let height = page_h_pt * scale;

    let mut doc = SvgDocument::new();
    let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
        width: Some(SvgLength::px(width)),
        height: Some(SvgLength::px(height)),
        view_box: Some(ViewBox::new(0.0, 0.0, width, height)),
        preserve_aspect_ratio: None,
    }));
    doc.set_root(svg_id);

    let page_streams = page
        .content_refs
        .iter()
        .filter_map(|id| {
            object_map
                .get(id)
                .and_then(|object| object.stream.as_deref())
        })
        .collect::<Vec<_>>();

    for text in page_streams
        .iter()
        .flat_map(|stream| extract_text_runs(stream, scale, height))
    {
        let text_id = doc.add_node(SvgElement::Text(TextAttrs {
            x: vec![SvgLength::px(text.x)],
            y: vec![SvgLength::px(text.y)],
            font_size: Some(SvgLength::new(text.font_size, LengthUnit::Px)),
            text_content: text.text,
            ..Default::default()
        }));
        if let Some(node) = doc.get_node_mut(text_id) {
            node.presentation.fill = Some(Paint::Color(Color::BLACK));
        }
        doc.append_child(svg_id, text_id);
    }

    for vector in page_streams
        .iter()
        .flat_map(|stream| extract_vector_runs(stream, scale, height))
    {
        let path_id = doc.add_node(SvgElement::Path(PathAttrs {
            d: vector.path,
            inkscape_path_effect: None,
            inkscape_original_d: None,
        }));
        if let Some(node) = doc.get_node_mut(path_id) {
            node.presentation = PresentationAttrs {
                fill: vector.fill.map(Paint::Color),
                stroke: vector.stroke.map(Paint::Color),
                stroke_width: vector
                    .stroke
                    .map(|_| SvgLength::px(vector.stroke_width.max(0.1) * scale)),
                ..PresentationAttrs::default()
            };
        }
        doc.append_child(svg_id, path_id);
    }

    for rect in page_streams
        .iter()
        .flat_map(|stream| extract_rectangles(stream, scale, height))
    {
        let rect_id = doc.add_node(SvgElement::Rect(rect.0));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation = rect.1;
        }
        doc.append_child(svg_id, rect_id);
    }

    for image in extract_image_refs(&objects) {
        let image_id = doc.add_node(SvgElement::Image(ImageAttrs {
            href: Some(format!(
                "data:application/pdf-xobject;object={};base64,UNEXTRACTED",
                image.object_id
            )),
            x: Some(SvgLength::px(0.0)),
            y: Some(SvgLength::px(0.0)),
            width: Some(SvgLength::px(image.width.max(1.0) * scale)),
            height: Some(SvgLength::px(image.height.max(1.0) * scale)),
            preserve_aspect_ratio: None,
        }));
        doc.append_child(svg_id, image_id);
    }

    Ok(doc)
}

fn parse_pdf_objects(data: &[u8]) -> Vec<PdfObject> {
    let mut objects = Vec::new();
    let mut cursor = 0;

    while let Some(obj_pos) = find_bytes(&data[cursor..], b" obj").map(|pos| cursor + pos) {
        let header_start = data[..obj_pos]
            .iter()
            .rposition(|b| *b == b'\n' || *b == b'\r')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let header = String::from_utf8_lossy(&data[header_start..obj_pos]);
        let mut header_parts = header.split_whitespace();
        let Some(id) = header_parts
            .next()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            cursor = obj_pos + 4;
            continue;
        };

        let body_start = obj_pos + b" obj".len();
        let Some(end_rel) = find_bytes(&data[body_start..], b"endobj") else {
            break;
        };
        let body_end = body_start + end_rel;
        let body_bytes = trim_ascii(&data[body_start..body_end]);

        let (body, stream) = if let Some(stream_pos) = find_bytes(body_bytes, b"stream") {
            let dict_bytes = trim_ascii(&body_bytes[..stream_pos]);
            let mut stream_start = stream_pos + b"stream".len();
            if body_bytes.get(stream_start) == Some(&b'\r') {
                stream_start += 1;
            }
            if body_bytes.get(stream_start) == Some(&b'\n') {
                stream_start += 1;
            }
            let stream_end = find_bytes(&body_bytes[stream_start..], b"endstream")
                .map(|pos| stream_start + pos)
                .unwrap_or(body_bytes.len());
            let raw_stream = trim_ascii(&body_bytes[stream_start..stream_end]);
            let dict = String::from_utf8_lossy(dict_bytes).into_owned();
            let decoded = decode_stream(raw_stream, &dict);
            (dict, Some(decoded))
        } else {
            (String::from_utf8_lossy(body_bytes).into_owned(), None)
        };

        objects.push(PdfObject { id, body, stream });
        cursor = body_end + b"endobj".len();
    }

    objects
}

fn parse_pages(objects: &[PdfObject]) -> Vec<PdfPage> {
    objects
        .iter()
        .filter(|object| {
            object.body.contains("/Type /Page")
                || object.body.contains("/Type/Page")
                || object.body.contains("/Type\n/Page")
        })
        .filter(|object| !object.body.contains("/Type /Pages"))
        .map(|object| PdfPage {
            media_box: parse_media_box(&object.body).unwrap_or([0.0, 0.0, 612.0, 792.0]),
            content_refs: parse_refs_after_key(&object.body, "/Contents"),
        })
        .collect()
}

fn parse_media_box(body: &str) -> Option<[f64; 4]> {
    let start = body.find("/MediaBox")?;
    let after = &body[start..];
    let open = after.find('[')?;
    let close = after[open..].find(']')? + open;
    let values = after[open + 1..close]
        .split_whitespace()
        .filter_map(|value| value.parse::<f64>().ok())
        .collect::<Vec<_>>();
    (values.len() >= 4).then(|| [values[0], values[1], values[2], values[3]])
}

fn parse_refs_after_key(body: &str, key: &str) -> Vec<u32> {
    let Some(start) = body.find(key) else {
        return Vec::new();
    };
    let after = &body[start + key.len()..];
    let segment = if let Some(open) = after.find('[') {
        after[open + 1..]
            .find(']')
            .map(|close| &after[open + 1..open + 1 + close])
            .unwrap_or(after)
    } else {
        after
    };
    collect_indirect_refs(segment)
}

fn collect_indirect_refs(segment: &str) -> Vec<u32> {
    let tokens = segment.split_whitespace().collect::<Vec<_>>();
    let mut refs = Vec::new();
    for window in tokens.windows(3) {
        if window[2] == "R" {
            if let Ok(id) = window[0].parse::<u32>() {
                refs.push(id);
            }
        }
    }
    refs
}

fn decode_stream(raw: &[u8], dict: &str) -> Vec<u8> {
    if dict.contains("/FlateDecode") {
        let mut decoder = ZlibDecoder::new(raw);
        let mut decoded = Vec::new();
        if decoder.read_to_end(&mut decoded).is_ok() {
            return decoded;
        }
    }
    raw.to_vec()
}

fn extract_text_runs(stream: &[u8], scale: f64, page_height: f64) -> Vec<PdfTextRun> {
    let content = String::from_utf8_lossy(stream);
    let mut in_text = false;
    let mut x = 10.0;
    let mut y = page_height - 20.0;
    let mut font_size = 12.0;
    let mut runs = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("BT") {
            in_text = true;
        }
        if !in_text {
            continue;
        }

        if trimmed.contains("Tf") {
            if let Some(size) = numbers_before_operator(trimmed, "Tf").last() {
                font_size = *size * scale;
            }
        }
        if trimmed.contains("Td") {
            let nums = numbers_before_operator(trimmed, "Td");
            if nums.len() >= 2 {
                x = nums[nums.len() - 2] * scale;
                y = page_height - nums[nums.len() - 1] * scale;
            }
        }
        if trimmed.contains("Tm") {
            let nums = numbers_before_operator(trimmed, "Tm");
            if nums.len() >= 6 {
                x = nums[4] * scale;
                y = page_height - nums[5] * scale;
            }
        }
        if trimmed.contains("Tj") || trimmed.contains("TJ") || trimmed.ends_with('\'') {
            let text = extract_pdf_strings(trimmed).join("");
            if !text.is_empty() {
                runs.push(PdfTextRun {
                    text,
                    x,
                    y,
                    font_size,
                });
                y += font_size * 1.2;
            }
        }
        if trimmed.contains("ET") {
            in_text = false;
        }
    }

    if runs.is_empty() {
        let fallback = extract_pdf_strings(&content).join(" ");
        if !fallback.trim().is_empty() {
            runs.push(PdfTextRun {
                text: fallback,
                x: 10.0,
                y: 20.0,
                font_size: 12.0 * scale,
            });
        }
    }

    runs
}

fn extract_rectangles(
    stream: &[u8],
    scale: f64,
    page_height: f64,
) -> Vec<(RectAttrs, PresentationAttrs)> {
    let tokens = content_tokens(stream);
    let mut stack = Vec::<f64>::new();
    let mut fill = Color::BLACK;
    let mut stroke = Color::BLACK;
    let mut stroke_width = 1.0;
    let mut pending_rects = Vec::<RectAttrs>::new();
    let mut output = Vec::new();

    for token in tokens {
        if let Ok(value) = token.parse::<f64>() {
            stack.push(value);
            continue;
        }

        match token.as_str() {
            "rg" => {
                if stack.len() >= 3 {
                    fill = rgb_from_unit(
                        stack[stack.len() - 3],
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                    );
                }
                stack.clear();
            }
            "RG" => {
                if stack.len() >= 3 {
                    stroke = rgb_from_unit(
                        stack[stack.len() - 3],
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                    );
                }
                stack.clear();
            }
            "w" => {
                if let Some(value) = stack.last() {
                    stroke_width = *value;
                }
                stack.clear();
            }
            "re" => {
                if stack.len() >= 4 {
                    let x = stack[stack.len() - 4] * scale;
                    let y = stack[stack.len() - 3] * scale;
                    let w = stack[stack.len() - 2] * scale;
                    let h = stack[stack.len() - 1] * scale;
                    pending_rects.push(RectAttrs {
                        x: SvgLength::px(x),
                        y: SvgLength::px(page_height - y - h),
                        width: SvgLength::px(w.abs()),
                        height: SvgLength::px(h.abs()),
                        rx: SvgLength::zero(),
                        ry: SvgLength::zero(),
                    });
                }
                stack.clear();
            }
            "f" | "F" | "f*" => {
                output.extend(pending_rects.drain(..).map(|rect| {
                    (
                        rect,
                        PresentationAttrs {
                            fill: Some(Paint::Color(fill)),
                            ..PresentationAttrs::default()
                        },
                    )
                }));
                stack.clear();
            }
            "S" | "s" => {
                output.extend(pending_rects.drain(..).map(|rect| {
                    (
                        rect,
                        PresentationAttrs {
                            fill: Some(Paint::None),
                            stroke: Some(Paint::Color(stroke)),
                            stroke_width: Some(SvgLength::px(stroke_width.max(0.1) * scale)),
                            ..PresentationAttrs::default()
                        },
                    )
                }));
                stack.clear();
            }
            "B" | "B*" | "b" | "b*" => {
                output.extend(pending_rects.drain(..).map(|rect| {
                    (
                        rect,
                        PresentationAttrs {
                            fill: Some(Paint::Color(fill)),
                            stroke: Some(Paint::Color(stroke)),
                            stroke_width: Some(SvgLength::px(stroke_width.max(0.1) * scale)),
                            ..PresentationAttrs::default()
                        },
                    )
                }));
                stack.clear();
            }
            _ => {}
        }
    }

    output
}

fn extract_vector_runs(stream: &[u8], scale: f64, page_height: f64) -> Vec<PdfPathRun> {
    let tokens = content_tokens(stream);
    let mut stack = Vec::<f64>::new();
    let mut path = BezPath::new();
    let mut output = Vec::new();
    let mut fill = Color::BLACK;
    let mut stroke = Color::BLACK;
    let mut stroke_width = 1.0;

    for token in tokens {
        if let Ok(value) = token.parse::<f64>() {
            stack.push(value);
            continue;
        }

        match token.as_str() {
            "rg" => {
                if stack.len() >= 3 {
                    fill = rgb_from_unit(
                        stack[stack.len() - 3],
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                    );
                }
                stack.clear();
            }
            "RG" => {
                if stack.len() >= 3 {
                    stroke = rgb_from_unit(
                        stack[stack.len() - 3],
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                    );
                }
                stack.clear();
            }
            "w" => {
                if let Some(value) = stack.last() {
                    stroke_width = *value;
                }
                stack.clear();
            }
            "m" => {
                if stack.len() >= 2 {
                    path.move_to(pdf_point(
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                        scale,
                        page_height,
                    ));
                }
                stack.clear();
            }
            "l" => {
                if stack.len() >= 2 {
                    path.line_to(pdf_point(
                        stack[stack.len() - 2],
                        stack[stack.len() - 1],
                        scale,
                        page_height,
                    ));
                }
                stack.clear();
            }
            "c" => {
                if stack.len() >= 6 {
                    path.curve_to(
                        pdf_point(
                            stack[stack.len() - 6],
                            stack[stack.len() - 5],
                            scale,
                            page_height,
                        ),
                        pdf_point(
                            stack[stack.len() - 4],
                            stack[stack.len() - 3],
                            scale,
                            page_height,
                        ),
                        pdf_point(
                            stack[stack.len() - 2],
                            stack[stack.len() - 1],
                            scale,
                            page_height,
                        ),
                    );
                }
                stack.clear();
            }
            "h" => {
                path.close();
                stack.clear();
            }
            "f" | "F" | "f*" => {
                if !path.is_empty() {
                    output.push(PdfPathRun {
                        path: path.clone(),
                        fill: Some(fill),
                        stroke: None,
                        stroke_width,
                    });
                    path = BezPath::new();
                }
                stack.clear();
            }
            "S" | "s" => {
                if !path.is_empty() {
                    output.push(PdfPathRun {
                        path: path.clone(),
                        fill: None,
                        stroke: Some(stroke),
                        stroke_width,
                    });
                    path = BezPath::new();
                }
                stack.clear();
            }
            "B" | "B*" | "b" | "b*" => {
                if !path.is_empty() {
                    output.push(PdfPathRun {
                        path: path.clone(),
                        fill: Some(fill),
                        stroke: Some(stroke),
                        stroke_width,
                    });
                    path = BezPath::new();
                }
                stack.clear();
            }
            _ => {}
        }
    }

    output
}

fn extract_image_refs(objects: &[PdfObject]) -> Vec<PdfImageRef> {
    objects
        .iter()
        .filter(|object| {
            object.body.contains("/Subtype /Image") || object.body.contains("/Subtype/Image")
        })
        .map(|object| PdfImageRef {
            object_id: object.id,
            width: number_after_key(&object.body, "/Width").unwrap_or(1.0),
            height: number_after_key(&object.body, "/Height").unwrap_or(1.0),
        })
        .collect()
}

fn content_tokens(stream: &[u8]) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in String::from_utf8_lossy(stream).chars() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == ')' {
                in_string = false;
            }
            continue;
        }
        if ch == '(' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            in_string = true;
            continue;
        }
        if ch.is_whitespace() || matches!(ch, '[' | ']' | '<' | '>') {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn numbers_before_operator(line: &str, operator: &str) -> Vec<f64> {
    let Some(pos) = line.find(operator) else {
        return Vec::new();
    };
    line[..pos]
        .split_whitespace()
        .filter_map(|token| token.parse::<f64>().ok())
        .collect()
}

fn extract_pdf_strings(input: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '(' {
            continue;
        }
        let mut value = String::new();
        let mut escape = false;
        for next in chars.by_ref() {
            if escape {
                value.push(match next {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    other => other,
                });
                escape = false;
                continue;
            }
            match next {
                '\\' => escape = true,
                ')' => break,
                other => value.push(other),
            }
        }
        strings.push(value);
    }
    strings
}

fn number_after_key(body: &str, key: &str) -> Option<f64> {
    let start = body.find(key)?;
    body[start + key.len()..]
        .split_whitespace()
        .find_map(|token| token.parse::<f64>().ok())
}

fn rgb_from_unit(r: f64, g: f64, b: f64) -> Color {
    Color::rgb(
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn pdf_point(x: f64, y: f64, scale: f64, page_height: f64) -> Point {
    Point::new(x * scale, page_height - y * scale)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|idx| idx + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn empty_or_invalid_data_returns_error() {
        let empty_err = import_pdf(&[], &PdfImportOptions::default());
        assert!(empty_err.is_err());
        assert_eq!(empty_err.unwrap_err(), "PDF data is empty");

        let junk_err = import_pdf(b"not a pdf", &PdfImportOptions::default());
        assert!(junk_err.is_err());
        assert!(junk_err.unwrap_err().contains("missing %PDF header"));
    }

    #[test]
    fn imports_page_text_vectors_and_image_placeholders() {
        let pdf = br#"%PDF-1.4
1 0 obj
<< /Type /Page /MediaBox [0 0 200 100] /Contents [2 0 R 3 0 R] /Resources << /XObject << /Im1 4 0 R >> >> >>
endobj
2 0 obj
<< /Length 83 >>
stream
BT
/F1 12 Tf
20 80 Td
(Hello PDF) Tj
ET
0 0 1 rg
10 10 60 30 re
f
endstream
endobj
3 0 obj
<< /Length 28 >>
stream
0 1 0 RG
2 w
20 20 m
120 20 l
S
endstream
endobj
4 0 obj
<< /Type /XObject /Subtype /Image /Width 16 /Height 12 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 0 >>
stream
endstream
endobj
"#;

        let doc = import_pdf(pdf, &PdfImportOptions { page: 0, dpi: 72.0 }).unwrap();
        let root_id = doc.root().unwrap();
        let root = doc.get_node(root_id).unwrap();
        let SvgElement::Svg(svg) = &root.element else {
            panic!("expected svg root");
        };
        assert_eq!(svg.width.unwrap().to_px(), 200.0);
        assert_eq!(svg.height.unwrap().to_px(), 100.0);

        let mut has_text = false;
        let mut has_rect = false;
        let mut has_path = false;
        let mut has_image = false;
        for child in doc.children(root_id) {
            match &doc.get_node(*child).unwrap().element {
                SvgElement::Text(attrs) => {
                    has_text = attrs.text_content == "Hello PDF";
                }
                SvgElement::Rect(attrs) => {
                    has_rect = attrs.width.to_px() == 60.0 && attrs.height.to_px() == 30.0;
                }
                SvgElement::Path(attrs) => {
                    has_path = !attrs.d.is_empty();
                }
                SvgElement::Image(attrs) => {
                    has_image = attrs
                        .href
                        .as_deref()
                        .unwrap_or_default()
                        .contains("pdf-xobject;object=4");
                }
                _ => {}
            }
        }

        assert!(has_text, "text operator should import as SVG text");
        assert!(has_rect, "re/f operators should import as SVG rect");
        assert!(has_path, "m/l/S operators should import as SVG path");
        assert!(
            has_image,
            "image XObject should be preserved as placeholder"
        );
    }

    #[test]
    fn imports_flate_compressed_content_stream() {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"BT\n/F1 10 Tf\n10 40 Td\n(Compressed) Tj\nET\n")
            .unwrap();
        let compressed = encoder.finish().unwrap();

        let mut pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Page /MediaBox [0 0 100 50] /Contents 2 0 R >>\nendobj\n2 0 obj\n<< /Filter /FlateDecode /Length ".to_vec();
        pdf.extend_from_slice(compressed.len().to_string().as_bytes());
        pdf.extend_from_slice(b" >>\nstream\n");
        pdf.extend_from_slice(&compressed);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        let doc = import_pdf(&pdf, &PdfImportOptions { page: 0, dpi: 72.0 }).unwrap();
        let root_id = doc.root().unwrap();
        let text = doc.children(root_id).iter().find_map(|id| {
            let node = doc.get_node(*id)?;
            if let SvgElement::Text(attrs) = &node.element {
                Some(attrs.text_content.clone())
            } else {
                None
            }
        });
        assert_eq!(text.as_deref(), Some("Compressed"));
    }

    #[test]
    fn page_selection_is_bounds_checked() {
        let pdf = br#"%PDF-1.4
1 0 obj
<< /Type /Page /MediaBox [0 0 100 100] >>
endobj
"#;
        let err = import_pdf(pdf, &PdfImportOptions { page: 1, dpi: 96.0 }).unwrap_err();
        assert!(err.contains("out of range"));
    }
}
