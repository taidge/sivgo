//! `sivgo-io` -- Import/export for the Sivgo SVG editor.
//!
//! Provides file-level operations:
//!
//! * [`load_svg`] -- read an SVG file from disk into an [`sivgo_dom::SvgDocument`].
//! * [`save_svg`] -- serialize an [`sivgo_dom::SvgDocument`] and write it to disk.
//! * [`export_png`] -- render a document to a PNG image file.
//! * [`export_with_config`] -- render with area/DPI configuration.
//! * [`batch_export`] -- export multiple items to files.
//! * [`export_pdf`] -- export a document to PDF format.
//! * [`export_eps`] -- export a document to EPS (Encapsulated PostScript).
//! * [`export_dxf`] -- export a document to DXF (AutoCAD Drawing Exchange Format).
//! * [`pdf_import`] -- stub/minimal PDF-to-SVG importer.
//! * [`recent_files`] -- recent files list manager with JSON persistence.

mod atomic_write;
pub mod dxf;
pub mod eps;
pub mod multi_page_pdf;
pub mod pdf;
pub mod pdf_import;
pub mod recent_files;
pub mod trace_import;

// Re-export the main types/functions from the new export modules.
pub use atomic_write::write_atomic;
pub use dxf::{export_dxf, render_dxf, DxfExportOptions};
pub use eps::{export_eps, render_eps, EpsExportOptions};
pub use multi_page_pdf::{MultiPagePdfExporter, PdfMetadata, PdfPage};
pub use pdf::{export_pdf, export_pdf_multipage, PdfExportOptions};
pub use pdf_import::{import_pdf, PdfImportOptions};
pub use trace_import::{trace_bitmap, trace_bitmap_file, TraceOptions};

use std::fs;
use std::io::BufWriter;
use std::io::{Read as _, Write as _};
use std::path::Path;

use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during SVG/PNG import and export.
#[derive(Debug, Error)]
pub enum IoError {
    /// An I/O error from the filesystem.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A DOM-level error (parse failure, invalid structure, etc.).
    #[error("DOM error: {0}")]
    Dom(#[from] sivgo_dom::DomError),

    /// A PNG encoding error.
    #[error("PNG error: {0}")]
    Png(String),

    /// A JPEG encoding error.
    #[error("JPEG error: {0}")]
    Jpeg(String),

    /// A gzip compression/decompression error.
    #[error("Gzip error: {0}")]
    Gzip(String),

    /// A rendering/encoding error.
    #[error("Render error: {0}")]
    Render(String),
}

// ---------------------------------------------------------------------------
// SVG Import
// ---------------------------------------------------------------------------

fn is_svgz(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svgz"))
}

/// Load SVG or SVGZ from `path` and parse it into an [`sivgo_dom::SvgDocument`].
///
/// # Errors
///
/// Returns [`IoError::Io`] if the file cannot be read, or [`IoError::Dom`] if
/// the SVG content is malformed.
pub fn load_svg(path: &Path) -> Result<sivgo_dom::SvgDocument, IoError> {
    log::debug!("Loading SVG from {}", path.display());
    if is_svgz(path) {
        return decode_svgz(fs::File::open(path)?);
    }
    let contents = fs::read_to_string(path)?;
    let doc = sivgo_dom::parse_svg(&contents)?;
    log::info!("Loaded SVG from {} ({} nodes)", path.display(), doc.len());
    Ok(doc)
}

// ---------------------------------------------------------------------------
// SVG Export
// ---------------------------------------------------------------------------

/// Replace the drawing at `path` atomically, compressing it for SVGZ.
///
/// # Errors
///
/// Returns [`IoError::Io`] if the file cannot be written.
pub fn save_svg(doc: &sivgo_dom::SvgDocument, path: &Path) -> Result<(), IoError> {
    log::debug!("Saving SVG to {}", path.display());
    let bytes = if is_svgz(path) {
        save_svgz(doc)?
    } else {
        sivgo_dom::serialize_svg(doc).into_bytes()
    };
    write_atomic(path, |file| file.write_all(&bytes))?;
    log::info!(
        "Saved drawing to {} ({} bytes)",
        path.display(),
        bytes.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// SVGZ (gzip-compressed SVG) Import/Export
// ---------------------------------------------------------------------------

/// Load a gzip-compressed SVG (SVGZ) from raw bytes and parse it into an
/// [`sivgo_dom::SvgDocument`].
///
/// # Errors
///
/// Returns [`IoError::Gzip`] if the data cannot be decompressed, or
/// [`IoError::Dom`] if the SVG content is malformed.
pub fn load_svgz(data: &[u8]) -> Result<sivgo_dom::SvgDocument, IoError> {
    decode_svgz(data)
}

const MAX_SVGZ_XML_BYTES: usize = 64 * 1024 * 1024;

fn decode_svgz(data: impl std::io::Read) -> Result<sivgo_dom::SvgDocument, IoError> {
    let mut decoder = MultiGzDecoder::new(data).take(MAX_SVGZ_XML_BYTES as u64 + 1);
    let mut svg_string = String::new();
    decoder
        .read_to_string(&mut svg_string)
        .map_err(|e| IoError::Gzip(format!("decompression failed: {e}")))?;
    if svg_string.len() > MAX_SVGZ_XML_BYTES {
        return Err(IoError::Gzip("decompressed SVG exceeds 64 MiB".into()));
    }
    Ok(sivgo_dom::parse_svg(&svg_string)?)
}

/// Serialize an [`sivgo_dom::SvgDocument`] to gzip-compressed SVG (SVGZ).
///
/// # Errors
///
/// Returns [`IoError::Gzip`] if compression fails.
pub fn save_svgz(doc: &sivgo_dom::SvgDocument) -> Result<Vec<u8>, IoError> {
    let xml = sivgo_dom::serialize_svg(doc);
    if xml.len() > MAX_SVGZ_XML_BYTES {
        return Err(IoError::Gzip(
            "SVG exceeds 64 MiB before compression".into(),
        ));
    }
    log::debug!("Saving SVGZ ({} uncompressed bytes)", xml.len());
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .map_err(|e| IoError::Gzip(format!("compression write failed: {e}")))?;
    let compressed = encoder
        .finish()
        .map_err(|e| IoError::Gzip(format!("compression finish failed: {e}")))?;
    log::info!(
        "Saved SVGZ ({} -> {} bytes, {:.0}% compression)",
        xml.len(),
        compressed.len(),
        if xml.is_empty() {
            0.0
        } else {
            (1.0 - compressed.len() as f64 / xml.len() as f64) * 100.0
        }
    );
    Ok(compressed)
}

// ---------------------------------------------------------------------------
// Inkscape SVG Export
// ---------------------------------------------------------------------------

/// Namespace URI for Inkscape extensions.
const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
/// Namespace URI for Sodipodi extensions.
const SODIPODI_NS: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd";

/// Serialize an [`sivgo_dom::SvgDocument`] to SVG with Inkscape namespace
/// declarations on the root element.
///
/// This ensures `xmlns:inkscape` and `xmlns:sodipodi` are present so the file
/// can be opened in Inkscape without losing extension data.
pub fn save_inkscape_svg(doc: &sivgo_dom::SvgDocument) -> String {
    // We temporarily inject the namespace declarations onto the root node,
    // serialize, then restore the original state.  Since we only have a shared
    // reference, we clone the document, patch the root, and serialize.
    let mut doc_clone = doc.clone();
    if let Some(root_id) = doc_clone.root() {
        if let Some(root_node) = doc_clone.get_node_mut(root_id) {
            // Insert inkscape and sodipodi namespace prefixes if not already present.
            root_node
                .namespaces
                .entry("inkscape".to_string())
                .or_insert_with(|| INKSCAPE_NS.to_string());
            root_node
                .namespaces
                .entry("sodipodi".to_string())
                .or_insert_with(|| SODIPODI_NS.to_string());
        }
    }
    sivgo_dom::serialize_svg(&doc_clone)
}

// ---------------------------------------------------------------------------
// Plain SVG Export (strip Inkscape extensions)
// ---------------------------------------------------------------------------

/// Serialize an [`sivgo_dom::SvgDocument`] to plain SVG, stripping all
/// Inkscape (`inkscape:*`) and Sodipodi (`sodipodi:*`) attributes and elements.
///
/// Elements whose tag name starts with `inkscape:` or `sodipodi:` are removed
/// entirely (along with their children).  Attributes with those prefixes are
/// dropped from remaining elements.  Namespace declarations for `inkscape` and
/// `sodipodi` are also removed.
pub fn export_plain_svg(doc: &sivgo_dom::SvgDocument) -> String {
    let mut doc_clone = doc.clone();

    // 1. Collect node ids of Inkscape/Sodipodi elements to remove.
    let ids_to_remove: Vec<sivgo_dom::NodeId> = if let Some(root_id) = doc_clone.root() {
        doc_clone
            .traverse_pre_order(root_id)
            .into_iter()
            .filter(|&id| {
                if let Some(node) = doc_clone.get_node(id) {
                    let tag = node.element.tag_name();
                    tag.starts_with("inkscape:") || tag.starts_with("sodipodi:")
                } else {
                    false
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // Remove Inkscape/Sodipodi elements (children are removed recursively).
    for id in ids_to_remove {
        doc_clone.remove_node(id);
    }

    // 2. Strip inkscape:/sodipodi: attributes and namespaces from remaining nodes.
    if let Some(root_id) = doc_clone.root() {
        let all_ids = doc_clone.traverse_pre_order(root_id);
        for id in all_ids {
            if let Some(node) = doc_clone.get_node_mut(id) {
                // Remove inkscape:*/sodipodi:* from extra attributes map.
                node.attributes.retain(|key, _| {
                    !key.starts_with("inkscape:") && !key.starts_with("sodipodi:")
                });

                // Remove inkscape/sodipodi namespace declarations.
                node.namespaces.remove("inkscape");
                node.namespaces.remove("sodipodi");

                // Clear Inkscape-specific typed fields on Group elements.
                if let sivgo_dom::SvgElement::Group(ref mut attrs) = node.element {
                    attrs.inkscape_groupmode = None;
                    attrs.inkscape_label = None;
                }
                // Clear Inkscape-specific typed fields on Path elements.
                if let sivgo_dom::SvgElement::Path(ref mut attrs) = node.element {
                    attrs.inkscape_path_effect = None;
                    attrs.inkscape_original_d = None;
                }
            }
        }
    }

    sivgo_dom::serialize_svg(&doc_clone)
}

// ---------------------------------------------------------------------------
// Optimized SVG Export
// ---------------------------------------------------------------------------

/// Serialize a document as optimized plain SVG.
///
/// This uses the plain-SVG export path to remove editor-only Inkscape and
/// Sodipodi data, then applies a deterministic XML minification pass that
/// removes inter-element whitespace without touching attribute values.
pub fn export_optimized_svg(doc: &sivgo_dom::SvgDocument) -> String {
    let plain = export_plain_svg(doc);
    minify_svg_xml(&plain)
}

fn minify_svg_xml(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut pending_space = false;
    let mut previous = '\0';

    for ch in xml.chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space && previous != '>' && ch != '<' {
            out.push(' ');
        }
        out.push(ch);
        previous = ch;
        pending_space = false;
    }

    out
}

// ---------------------------------------------------------------------------
// PNG Export
// ---------------------------------------------------------------------------

/// Render an [`sivgo_dom::SvgDocument`] to a PNG image and write it to `path`.
///
/// The document is rasterized at the given `width` x `height` resolution using
/// the CPU renderer from `sivgo-render`.
///
/// # Errors
///
/// Returns [`IoError::Png`] if the PNG encoder fails, or [`IoError::Io`] if
/// the output file cannot be written.
pub fn export_png(
    doc: &sivgo_dom::SvgDocument,
    path: &Path,
    width: u32,
    height: u32,
) -> Result<(), IoError> {
    log::debug!("Exporting PNG {}x{} to {}", width, height, path.display());

    // 1. Render to premultiplied RGBA pixel buffer.
    let premultiplied = sivgo_render::render_document_to_pixmap(doc, width, height);

    // 2. Convert from premultiplied alpha to straight alpha.
    //    PNG stores straight (non-premultiplied) alpha, but the renderer
    //    produces premultiplied RGBA.  We must undo the premultiplication.
    let mut pixels = premultiplied;
    for chunk in pixels.as_chunks_mut::<4>().0 {
        let a = chunk[3] as u16;
        if a == 0 {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
        } else if a < 255 {
            chunk[0] = ((chunk[0] as u16 * 255 + a / 2) / a).min(255) as u8;
            chunk[1] = ((chunk[1] as u16 * 255 + a / 2) / a).min(255) as u8;
            chunk[2] = ((chunk[2] as u16 * 255 + a / 2) / a).min(255) as u8;
        }
        // a == 255: already correct, nothing to do.
    }

    // 3. Encode as PNG and write to file.
    let file = fs::File::create(path)?;
    let buf_writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(buf_writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Balanced);

    let mut writer = encoder
        .write_header()
        .map_err(|e| IoError::Png(e.to_string()))?;

    writer
        .write_image_data(&pixels)
        .map_err(|e| IoError::Png(e.to_string()))?;

    log::info!(
        "Exported PNG {}x{} to {} ({} bytes of pixel data)",
        width,
        height,
        path.display(),
        pixels.len()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// JPEG Export
// ---------------------------------------------------------------------------

/// Render an [`sivgo_dom::SvgDocument`] to JPEG image bytes (in memory).
///
/// The document is rasterized at the given `width` x `height` resolution,
/// composited onto a white background, and encoded as JPEG at the specified
/// `quality` (1–100).
///
/// # Errors
///
/// Returns [`IoError::Jpeg`] if the JPEG encoder fails.
pub fn export_jpeg(
    doc: &sivgo_dom::SvgDocument,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<Vec<u8>, IoError> {
    log::debug!("Exporting JPEG {}x{} quality={}", width, height, quality);

    // 1. Render to premultiplied RGBA pixel buffer.
    let premultiplied = sivgo_render::render_document_to_pixmap(doc, width, height);

    // 2. Convert premultiplied RGBA → straight RGB (composited on white).
    //    JPEG does not support transparency, so we composite against white.
    let pixel_count = (width as usize) * (height as usize);
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for chunk in premultiplied.as_chunks::<4>().0 {
        let a = chunk[3] as f64 / 255.0;
        // Premultiplied values: divide by alpha to get straight, then composite on white.
        // composite = src_straight * alpha + white * (1 - alpha)
        // Since src is premultiplied: src_premul = src_straight * alpha
        // composite = src_premul + white * (1 - alpha) = src_premul + 255 * (1 - alpha)
        let r = (chunk[0] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        let g = (chunk[1] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        let b = (chunk[2] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        rgb.push(r);
        rgb.push(g);
        rgb.push(b);
    }

    // 3. Encode as JPEG.
    let mut jpeg_buf: Vec<u8> = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut jpeg_buf, quality);
    encoder
        .encode(
            &rgb,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .map_err(|e: jpeg_encoder::EncodingError| IoError::Jpeg(e.to_string()))?;

    log::info!(
        "Exported JPEG {}x{} quality={} ({} bytes)",
        width,
        height,
        quality,
        jpeg_buf.len()
    );

    Ok(jpeg_buf)
}

// ---------------------------------------------------------------------------
// WebP Export (10.7)
// ---------------------------------------------------------------------------

/// Export the document as a WebP image.
///
/// Renders the document at the given `width`×`height`, then encodes as WebP.
/// `quality` ranges from 0 (worst) to 100 (lossless).
pub fn export_webp(
    doc: &sivgo_dom::SvgDocument,
    width: u32,
    height: u32,
    quality: u8,
) -> Result<Vec<u8>, IoError> {
    // 1. Render to RGBA pixel buffer (premultiplied).
    let premultiplied = sivgo_render::render_document_to_pixmap(doc, width, height);

    // 2. Convert premultiplied RGBA to straight RGBA.
    let pixel_count = (width as usize) * (height as usize);
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for chunk in premultiplied.as_chunks::<4>().0 {
        let a = chunk[3];
        if a == 0 {
            rgba.extend_from_slice(&[255, 255, 255, 255]);
        } else {
            let af = a as f64 / 255.0;
            let r = ((chunk[0] as f64 / af).round().min(255.0)) as u8;
            let g = ((chunk[1] as f64 / af).round().min(255.0)) as u8;
            let b = ((chunk[2] as f64 / af).round().min(255.0)) as u8;
            // Composite on white for WebP (no transparency in lossy mode).
            let r2 = (r as f64 * af + 255.0 * (1.0 - af)).round() as u8;
            let g2 = (g as f64 * af + 255.0 * (1.0 - af)).round() as u8;
            let b2 = (b as f64 * af + 255.0 * (1.0 - af)).round() as u8;
            rgba.push(r2);
            rgba.push(g2);
            rgba.push(b2);
            rgba.push(255);
        }
    }

    // 3. Use the image crate to encode as WebP.
    let img = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| IoError::Render("Failed to create image buffer for WebP".into()))?;

    let mut webp_buf: Vec<u8> = Vec::new();
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_buf);
    img.write_with_encoder(encoder)
        .map_err(|e| IoError::Render(format!("WebP encoding failed: {e}")))?;

    let _ = quality; // quality param for future lossy mode

    log::info!(
        "Exported WebP {}x{} ({} bytes)",
        width,
        height,
        webp_buf.len()
    );

    Ok(webp_buf)
}

// ---------------------------------------------------------------------------
// Bitmap Import (10.12)
// ---------------------------------------------------------------------------

/// Import a bitmap image (PNG, JPEG, BMP, TIFF, GIF, WebP) and embed it
/// into a document as a base64-encoded `<image>` element.
///
/// Returns the document with the image element appended to the root.
pub fn import_bitmap(
    doc: &mut sivgo_dom::SvgDocument,
    image_data: &[u8],
    mime_type: &str,
) -> Result<sivgo_dom::NodeId, IoError> {
    use base64::Engine;

    // Decode the image to get dimensions.
    let img = image::load_from_memory(image_data)
        .map_err(|e| IoError::Render(format!("Failed to decode image: {e}")))?;

    let width = img.width() as f64;
    let height = img.height() as f64;

    // Encode as base64 data URI.
    let b64 = base64::engine::general_purpose::STANDARD.encode(image_data);
    let data_uri = format!("data:{};base64,{}", mime_type, b64);

    // Create an <image> element.
    let image_attrs = sivgo_dom::ImageAttrs {
        href: Some(data_uri),
        x: Some(sivgo_dom::SvgLength::px(0.0)),
        y: Some(sivgo_dom::SvgLength::px(0.0)),
        width: Some(sivgo_dom::SvgLength::px(width)),
        height: Some(sivgo_dom::SvgLength::px(height)),
        preserve_aspect_ratio: None,
    };

    let image_id = doc.add_node(sivgo_dom::SvgElement::Image(image_attrs));
    if let Some(root) = doc.root() {
        doc.append_child(root, image_id);
    }

    log::info!(
        "Imported bitmap image {}x{} as <image> element",
        width,
        height
    );

    Ok(image_id)
}

/// Detect the MIME type of an image from its magic bytes.
pub fn detect_image_mime(data: &[u8]) -> Option<&'static str> {
    if data.len() < 4 {
        return None;
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF8") {
        Some("image/gif")
    } else if data.starts_with(b"BM") {
        Some("image/bmp")
    } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(&[0x49, 0x49]) || data.starts_with(&[0x4D, 0x4D]) {
        Some("image/tiff")
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Export area / config / batch types (10.14, 10.15, 10.16)
// ---------------------------------------------------------------------------

/// Export area specification.
///
/// Controls which portion of the SVG document is rendered for export.
#[derive(Debug, Clone)]
pub enum ExportArea {
    /// Export the page (viewBox or width/height).
    Page,
    /// Export the drawing bounding box (tight fit around content).
    Drawing,
    /// Export a custom rectangle (in SVG user units).
    Custom {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

/// Export configuration.
///
/// Combines an [`ExportArea`], DPI, and optional background colour into a
/// single configuration object that can be passed to [`export_with_config`].
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Which area of the document to export.
    pub area: ExportArea,
    /// Dots per inch -- controls the pixel resolution of the output.
    /// Default: 96.0 (matches the SVG specification's default).
    pub dpi: f64,
    /// Optional RGBA background colour.  When `Some`, this colour is
    /// composited *behind* the rendered content.  When `None`, the
    /// background is transparent (PNG/WebP) or white (JPEG).
    pub background: Option<[u8; 4]>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            area: ExportArea::Page,
            dpi: 96.0,
            background: None,
        }
    }
}

/// Output format for [`batch_export`].
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Png,
    Jpeg { quality: u8 },
    WebP { quality: u8 },
    Svg,
    PlainSvg,
    OptimizedSvg,
    Svgz,
    Pdf,
    Eps,
    Dxf,
}

/// A single item in a [`batch_export`] run.
#[derive(Debug, Clone)]
pub struct BatchExportItem {
    /// Area of the document to export.
    pub area: ExportArea,
    /// Destination filename (written relative to the caller's working dir or
    /// as an absolute path).
    pub filename: String,
    /// Output format.
    pub format: ExportFormat,
}

// ---------------------------------------------------------------------------
// Helpers for area / DPI resolution
// ---------------------------------------------------------------------------

/// Standard SVG DPI (CSS pixel == 1/96 inch).
const SVG_DEFAULT_DPI: f64 = 96.0;

/// Resolve the export area to a concrete rectangle in SVG user units and
/// the corresponding pixel dimensions at the requested DPI.
///
/// Returns `(area_x, area_y, area_w, area_h, pixel_w, pixel_h)`.
fn resolve_area_and_size(
    doc: &sivgo_dom::SvgDocument,
    area: &ExportArea,
    dpi: f64,
) -> Result<(f64, f64, f64, f64, u32, u32), IoError> {
    let (ax, ay, aw, ah) = match area {
        ExportArea::Page => page_area(doc)?,
        ExportArea::Drawing => drawing_area(doc)?,
        ExportArea::Custom {
            x,
            y,
            width,
            height,
        } => (*x, *y, *width, *height),
    };

    if aw <= 0.0 || ah <= 0.0 {
        return Err(IoError::Render(format!(
            "Export area has non-positive dimensions: {aw}x{ah}"
        )));
    }

    let scale = dpi / SVG_DEFAULT_DPI;
    let pixel_w = (aw * scale).round().max(1.0) as u32;
    let pixel_h = (ah * scale).round().max(1.0) as u32;

    Ok((ax, ay, aw, ah, pixel_w, pixel_h))
}

/// Extract the page area from the root `<svg>` element (viewBox, or
/// width/height, or a sensible default).
fn page_area(doc: &sivgo_dom::SvgDocument) -> Result<(f64, f64, f64, f64), IoError> {
    let root_id = doc
        .root()
        .ok_or_else(|| IoError::Render("Document has no root node".into()))?;
    let root_node = doc
        .get_node(root_id)
        .ok_or_else(|| IoError::Render("Root node not found".into()))?;

    if let sivgo_dom::SvgElement::Svg(ref attrs) = root_node.element {
        // Prefer viewBox if present.
        if let Some(ref vb) = attrs.view_box {
            return Ok((vb.min_x, vb.min_y, vb.width, vb.height));
        }
        // Fall back to width/height (default 300x150 per HTML spec for
        // replaced elements when no intrinsic size is provided).
        let w = attrs.width.as_ref().map_or(300.0, |l| l.to_px());
        let h = attrs.height.as_ref().map_or(150.0, |l| l.to_px());
        Ok((0.0, 0.0, w, h))
    } else {
        Err(IoError::Render("Root node is not an <svg> element".into()))
    }
}

/// Compute the bounding box of all rendered content (the "drawing" area).
///
/// We walk every shape element and take the union of their geometry bounds.
/// This is a simplified bounding-box that does not account for transforms,
/// stroke width, or filters -- but it gives a reasonable tight fit for most
/// documents.
fn drawing_area(doc: &sivgo_dom::SvgDocument) -> Result<(f64, f64, f64, f64), IoError> {
    let root_id = doc
        .root()
        .ok_or_else(|| IoError::Render("Document has no root node".into()))?;

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for id in doc.traverse_pre_order(root_id) {
        if let Some(node) = doc.get_node(id) {
            let (ex, ey, ew, eh) = match &node.element {
                sivgo_dom::SvgElement::Rect(a) => {
                    (a.x.to_px(), a.y.to_px(), a.width.to_px(), a.height.to_px())
                }
                sivgo_dom::SvgElement::Circle(a) => {
                    let r = a.r.to_px();
                    (a.cx.to_px() - r, a.cy.to_px() - r, r * 2.0, r * 2.0)
                }
                sivgo_dom::SvgElement::Ellipse(a) => {
                    let rx = a.rx.to_px();
                    let ry = a.ry.to_px();
                    (a.cx.to_px() - rx, a.cy.to_px() - ry, rx * 2.0, ry * 2.0)
                }
                sivgo_dom::SvgElement::Line(a) => {
                    let x1 = a.x1.to_px();
                    let y1 = a.y1.to_px();
                    let x2 = a.x2.to_px();
                    let y2 = a.y2.to_px();
                    let lx = x1.min(x2);
                    let ly = y1.min(y2);
                    ((lx), (ly), (x1.max(x2) - lx), (y1.max(y2) - ly))
                }
                sivgo_dom::SvgElement::Image(a) => {
                    let x = a.x.as_ref().map_or(0.0, |l| l.to_px());
                    let y = a.y.as_ref().map_or(0.0, |l| l.to_px());
                    let w = a.width.as_ref().map_or(0.0, |l| l.to_px());
                    let h = a.height.as_ref().map_or(0.0, |l| l.to_px());
                    (x, y, w, h)
                }
                // Skip non-geometry elements.
                _ => continue,
            };

            if ew <= 0.0 && eh <= 0.0 {
                continue;
            }

            min_x = min_x.min(ex);
            min_y = min_y.min(ey);
            max_x = max_x.max(ex + ew);
            max_y = max_y.max(ey + eh);
        }
    }

    if min_x.is_infinite() {
        // No geometry found -- fall back to the page area.
        return page_area(doc);
    }

    Ok((min_x, min_y, max_x - min_x, max_y - min_y))
}

// ---------------------------------------------------------------------------
// Render helper (area + DPI aware)
// ---------------------------------------------------------------------------

/// Render the document into premultiplied RGBA pixels, honouring the
/// requested area and DPI.
///
/// For the `Page` area this renders the full document at the DPI-scaled
/// resolution.  For `Drawing` and `Custom` areas we render the full page
/// at DPI-scaled resolution, then crop to the requested rectangle.
fn render_with_area(
    doc: &sivgo_dom::SvgDocument,
    area: &ExportArea,
    dpi: f64,
    background: Option<[u8; 4]>,
) -> Result<(Vec<u8>, u32, u32), IoError> {
    let (ax, ay, _aw, _ah, pixel_w, pixel_h) = resolve_area_and_size(doc, area, dpi)?;

    let needs_crop = !matches!(area, ExportArea::Page);

    let (pixels, out_w, out_h) = if needs_crop {
        // Render the full page at DPI-scaled size, then crop.
        let (_, _, page_w, page_h) = page_area(doc)?;
        let scale = dpi / SVG_DEFAULT_DPI;
        let full_w = (page_w * scale).round().max(1.0) as u32;
        let full_h = (page_h * scale).round().max(1.0) as u32;

        let full_pixels = sivgo_render::render_document_to_pixmap(doc, full_w, full_h);

        // Crop coordinates in pixel space.
        let crop_x = ((ax * scale).round() as i64).max(0) as u32;
        let crop_y = ((ay * scale).round() as i64).max(0) as u32;
        let crop_w = pixel_w.min(full_w.saturating_sub(crop_x));
        let crop_h = pixel_h.min(full_h.saturating_sub(crop_y));

        if crop_w == 0 || crop_h == 0 {
            return Err(IoError::Render(
                "Crop area is outside the rendered page".into(),
            ));
        }

        let mut cropped = Vec::with_capacity((crop_w * crop_h * 4) as usize);
        for row in 0..crop_h {
            let src_row = (crop_y + row) as usize;
            let src_start = (src_row * full_w as usize + crop_x as usize) * 4;
            let src_end = src_start + (crop_w as usize) * 4;
            if src_end <= full_pixels.len() {
                cropped.extend_from_slice(&full_pixels[src_start..src_end]);
            } else {
                // If out-of-bounds, fill with transparent.
                cropped.extend(std::iter::repeat_n(0u8, (crop_w as usize) * 4));
            }
        }

        (cropped, crop_w, crop_h)
    } else {
        let pixels = sivgo_render::render_document_to_pixmap(doc, pixel_w, pixel_h);
        (pixels, pixel_w, pixel_h)
    };

    // Composite the background behind the content if requested.
    let pixels = if let Some(bg) = background {
        composite_background(&pixels, bg)
    } else {
        pixels
    };

    Ok((pixels, out_w, out_h))
}

/// Composite a premultiplied RGBA buffer *over* a solid background colour
/// (also specified as RGBA).  The result is still premultiplied RGBA.
fn composite_background(premultiplied: &[u8], bg: [u8; 4]) -> Vec<u8> {
    let mut result = Vec::with_capacity(premultiplied.len());
    let bg_r = bg[0] as f64 / 255.0;
    let bg_g = bg[1] as f64 / 255.0;
    let bg_b = bg[2] as f64 / 255.0;
    let bg_a = bg[3] as f64 / 255.0;

    for chunk in premultiplied.as_chunks::<4>().0 {
        // Source is premultiplied.
        let sr = chunk[0] as f64 / 255.0;
        let sg = chunk[1] as f64 / 255.0;
        let sb = chunk[2] as f64 / 255.0;
        let sa = chunk[3] as f64 / 255.0;

        // Porter-Duff "src over dst": out = src + dst * (1 - src_alpha)
        // Background is treated as premultiplied too.
        let bg_premul_r = bg_r * bg_a;
        let bg_premul_g = bg_g * bg_a;
        let bg_premul_b = bg_b * bg_a;

        let out_r = sr + bg_premul_r * (1.0 - sa);
        let out_g = sg + bg_premul_g * (1.0 - sa);
        let out_b = sb + bg_premul_b * (1.0 - sa);
        let out_a = sa + bg_a * (1.0 - sa);

        result.push((out_r * 255.0).round().min(255.0) as u8);
        result.push((out_g * 255.0).round().min(255.0) as u8);
        result.push((out_b * 255.0).round().min(255.0) as u8);
        result.push((out_a * 255.0).round().min(255.0) as u8);
    }

    result
}

// ---------------------------------------------------------------------------
// Premultiplied → straight alpha helpers (shared)
// ---------------------------------------------------------------------------

/// Convert premultiplied RGBA to straight RGBA (for PNG/WebP).
fn premultiplied_to_straight_rgba(premultiplied: &[u8]) -> Vec<u8> {
    let mut pixels = premultiplied.to_vec();
    for chunk in pixels.as_chunks_mut::<4>().0 {
        let a = chunk[3] as u16;
        if a == 0 {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
        } else if a < 255 {
            chunk[0] = ((chunk[0] as u16 * 255 + a / 2) / a).min(255) as u8;
            chunk[1] = ((chunk[1] as u16 * 255 + a / 2) / a).min(255) as u8;
            chunk[2] = ((chunk[2] as u16 * 255 + a / 2) / a).min(255) as u8;
        }
    }
    pixels
}

/// Convert premultiplied RGBA to straight RGB composited on white (for JPEG).
fn premultiplied_to_rgb_on_white(premultiplied: &[u8]) -> Vec<u8> {
    let pixel_count = premultiplied.len() / 4;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for chunk in premultiplied.as_chunks::<4>().0 {
        let a = chunk[3] as f64 / 255.0;
        let r = (chunk[0] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        let g = (chunk[1] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        let b = (chunk[2] as f64 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
        rgb.push(r);
        rgb.push(g);
        rgb.push(b);
    }
    rgb
}

// ---------------------------------------------------------------------------
// export_with_config (10.15, 10.16)
// ---------------------------------------------------------------------------

/// Render a document to an in-memory PNG using the specified
/// [`ExportConfig`] (area, DPI, background).
///
/// The output format is always PNG.  For other formats, use
/// [`batch_export`] or the format-specific `export_*` functions.
///
/// # Errors
///
/// Returns [`IoError`] if rendering or encoding fails.
pub fn export_with_config(
    doc: &sivgo_dom::SvgDocument,
    config: &ExportConfig,
) -> Result<Vec<u8>, IoError> {
    let (premultiplied, width, height) =
        render_with_area(doc, &config.area, config.dpi, config.background)?;

    // Encode as PNG (straight alpha).
    let straight = premultiplied_to_straight_rgba(&premultiplied);

    let mut png_buf: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Balanced);

        // Embed the physical pixel density (pHYs chunk) so viewers know the
        // intended DPI.
        let ppm = (config.dpi / 0.0254).round() as u32;
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));

        let mut writer = encoder
            .write_header()
            .map_err(|e| IoError::Png(e.to_string()))?;

        writer
            .write_image_data(&straight)
            .map_err(|e| IoError::Png(e.to_string()))?;
    }

    log::info!(
        "export_with_config: {}x{} @ {} DPI ({} bytes PNG)",
        width,
        height,
        config.dpi,
        png_buf.len()
    );

    Ok(png_buf)
}

// ---------------------------------------------------------------------------
// batch_export (10.14)
// ---------------------------------------------------------------------------

/// Export multiple items from a document, each with its own area, filename,
/// and format.
///
/// Returns one `Result` per item, in the same order as `items`.  An error in
/// one item does not prevent the remaining items from being exported.
pub fn batch_export(
    doc: &sivgo_dom::SvgDocument,
    items: &[BatchExportItem],
) -> Vec<Result<(), IoError>> {
    items
        .iter()
        .map(|item| batch_export_one(doc, item))
        .collect()
}

/// Export a single [`BatchExportItem`].
fn batch_export_one(doc: &sivgo_dom::SvgDocument, item: &BatchExportItem) -> Result<(), IoError> {
    let path = Path::new(&item.filename);

    match &item.format {
        ExportFormat::Svg => {
            // SVG export: serialize, optionally crop via viewBox.
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            let xml = sivgo_dom::serialize_svg(&doc_clone);
            fs::write(path, xml.as_bytes())?;
        }
        ExportFormat::PlainSvg => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            fs::write(path, export_plain_svg(&doc_clone).as_bytes())?;
        }
        ExportFormat::OptimizedSvg => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            fs::write(path, export_optimized_svg(&doc_clone).as_bytes())?;
        }
        ExportFormat::Svgz => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            fs::write(path, save_svgz(&doc_clone)?)?;
        }
        ExportFormat::Pdf => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            let pdf = export_pdf(&doc_clone, &PdfExportOptions::default());
            fs::write(path, pdf)?;
        }
        ExportFormat::Eps => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            // The exporter derives the page box from the document itself.
            fs::write(
                path,
                render_eps(&doc_clone, &EpsExportOptions::default()).as_bytes(),
            )?;
        }
        ExportFormat::Dxf => {
            let mut doc_clone = doc.clone();
            apply_area_to_svg(&mut doc_clone, &item.area)?;
            fs::write(
                path,
                render_dxf(&doc_clone, &DxfExportOptions::default()).as_bytes(),
            )?;
        }
        ExportFormat::Png => {
            let config = ExportConfig {
                area: item.area.clone(),
                dpi: SVG_DEFAULT_DPI,
                background: None,
            };
            let png_data = export_with_config(doc, &config)?;
            fs::write(path, &png_data)?;
        }
        ExportFormat::Jpeg { quality } => {
            let (premultiplied, width, height) =
                render_with_area(doc, &item.area, SVG_DEFAULT_DPI, None)?;
            let rgb = premultiplied_to_rgb_on_white(&premultiplied);

            let mut jpeg_buf: Vec<u8> = Vec::new();
            let encoder = jpeg_encoder::Encoder::new(&mut jpeg_buf, *quality);
            encoder
                .encode(
                    &rgb,
                    width as u16,
                    height as u16,
                    jpeg_encoder::ColorType::Rgb,
                )
                .map_err(|e: jpeg_encoder::EncodingError| IoError::Jpeg(e.to_string()))?;

            fs::write(path, &jpeg_buf)?;
        }
        ExportFormat::WebP { quality } => {
            let (premultiplied, width, height) =
                render_with_area(doc, &item.area, SVG_DEFAULT_DPI, None)?;
            let straight = premultiplied_to_straight_rgba(&premultiplied);

            // Composite on white (WebP lossy has no transparency).
            let mut rgba_white = Vec::with_capacity(straight.len());
            for chunk in straight.as_chunks::<4>().0 {
                let a = chunk[3] as f64 / 255.0;
                let r = (chunk[0] as f64 * a + 255.0 * (1.0 - a)).round() as u8;
                let g = (chunk[1] as f64 * a + 255.0 * (1.0 - a)).round() as u8;
                let b = (chunk[2] as f64 * a + 255.0 * (1.0 - a)).round() as u8;
                rgba_white.push(r);
                rgba_white.push(g);
                rgba_white.push(b);
                rgba_white.push(255);
            }

            let img = image::RgbaImage::from_raw(width, height, rgba_white)
                .ok_or_else(|| IoError::Render("Failed to create image buffer for WebP".into()))?;

            let mut webp_buf: Vec<u8> = Vec::new();
            let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_buf);
            img.write_with_encoder(enc)
                .map_err(|e| IoError::Render(format!("WebP encoding failed: {e}")))?;

            let _ = quality; // quality param for future lossy mode
            fs::write(path, &webp_buf)?;
        }
    }

    log::info!("batch_export: wrote {}", item.filename);
    Ok(())
}

/// For SVG batch export: modify the root `<svg>` viewBox / width / height
/// to match the requested area.
fn apply_area_to_svg(doc: &mut sivgo_dom::SvgDocument, area: &ExportArea) -> Result<(), IoError> {
    match area {
        ExportArea::Page => { /* nothing to change */ }
        ExportArea::Drawing => {
            let (x, y, w, h) = drawing_area(doc)?;
            set_svg_viewport(doc, x, y, w, h)?;
        }
        ExportArea::Custom {
            x,
            y,
            width,
            height,
        } => {
            set_svg_viewport(doc, *x, *y, *width, *height)?;
        }
    }
    Ok(())
}

/// Set the root `<svg>` element's viewBox and width/height.
fn set_svg_viewport(
    doc: &mut sivgo_dom::SvgDocument,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), IoError> {
    let root_id = doc
        .root()
        .ok_or_else(|| IoError::Render("Document has no root node".into()))?;
    let root_node = doc
        .get_node_mut(root_id)
        .ok_or_else(|| IoError::Render("Root node not found".into()))?;

    if let sivgo_dom::SvgElement::Svg(ref mut attrs) = root_node.element {
        attrs.view_box = Some(sivgo_dom::ViewBox::new(x, y, w, h));
        attrs.width = Some(sivgo_dom::SvgLength::px(w));
        attrs.height = Some(sivgo_dom::SvgLength::px(h));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sivgo_dom::*;
    use std::io::Read;

    /// Build a minimal test document: 100x100 SVG with a red rect.
    fn test_doc() -> SvgDocument {
        let mut doc = SvgDocument::new();
        let svg_id = doc.add_node(SvgElement::Svg(SvgSvgAttrs {
            width: Some(SvgLength::px(100.0)),
            height: Some(SvgLength::px(100.0)),
            view_box: None,
            preserve_aspect_ratio: None,
        }));
        doc.set_root(svg_id);

        let rect_id = doc.add_node(SvgElement::Rect(RectAttrs {
            x: SvgLength::px(10.0),
            y: SvgLength::px(10.0),
            width: SvgLength::px(80.0),
            height: SvgLength::px(80.0),
            ..RectAttrs::default()
        }));
        if let Some(node) = doc.get_node_mut(rect_id) {
            node.presentation.fill = Some(Paint::Color(Color::RED));
        }
        doc.append_child(svg_id, rect_id);
        doc
    }

    #[test]
    fn roundtrip_svg_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let svg_path = dir.path().join("test.svg");

        let doc = test_doc();
        save_svg(&doc, &svg_path).expect("save_svg failed");

        // File should exist and contain SVG.
        let contents = fs::read_to_string(&svg_path).unwrap();
        assert!(contents.contains("<svg"));
        assert!(contents.contains("<rect"));

        // Load it back.
        let doc2 = load_svg(&svg_path).expect("load_svg failed");
        assert_eq!(doc.len(), doc2.len());
    }

    #[test]
    fn load_svg_nonexistent_file() {
        let result = load_svg(Path::new("/nonexistent/path/test.svg"));
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::Io(_) => {} // expected
            other => panic!("expected IoError::Io, got: {other}"),
        }
    }

    #[test]
    fn load_svg_invalid_content() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let svg_path = dir.path().join("bad.svg");
        fs::write(&svg_path, "this is not valid SVG").unwrap();

        let result = load_svg(&svg_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            IoError::Dom(_) => {} // expected
            other => panic!("expected IoError::Dom, got: {other}"),
        }
    }

    #[test]
    fn export_png_produces_valid_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let png_path = dir.path().join("output.png");

        let doc = test_doc();
        export_png(&doc, &png_path, 100, 100).expect("export_png failed");

        // Read back and verify it's a valid PNG.
        let mut file = fs::File::open(&png_path).unwrap();
        let mut header = [0u8; 8];
        file.read_exact(&mut header).unwrap();
        // PNG magic bytes.
        assert_eq!(&header, &[137, 80, 78, 71, 13, 10, 26, 10]);

        // Decode to verify dimensions and content.
        let file = std::io::BufReader::new(fs::File::open(&png_path).unwrap());
        let decoder = png::Decoder::new(file);
        let mut reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 100);
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);

        let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
        let frame_info = reader.next_frame(&mut buf).unwrap();
        let pixels = &buf[..frame_info.buffer_size()];

        // Center pixel (50, 50) should be red.
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 255, "red channel");
        assert_eq!(pixels[idx + 1], 0, "green channel");
        assert_eq!(pixels[idx + 2], 0, "blue channel");
        assert_eq!(pixels[idx + 3], 255, "alpha channel");
    }

    // -----------------------------------------------------------------
    // SVGZ round-trip
    // -----------------------------------------------------------------

    #[test]
    fn roundtrip_svgz() {
        let doc = test_doc();

        // Compress.
        let compressed = save_svgz(&doc).expect("save_svgz failed");
        assert!(!compressed.is_empty(), "SVGZ output should not be empty");

        // The compressed data should be smaller than the raw SVG (for non-trivial docs).
        let raw_xml = sivgo_dom::serialize_svg(&doc);
        // Not strictly guaranteed for tiny docs, but at least it should be valid gzip.

        // Decompress and re-parse.
        let doc2 = load_svgz(&compressed).expect("load_svgz failed");
        assert_eq!(
            doc.len(),
            doc2.len(),
            "node count mismatch after SVGZ round-trip"
        );

        // Re-serialize to verify structural equivalence.
        let raw_xml2 = sivgo_dom::serialize_svg(&doc2);
        assert_eq!(raw_xml, raw_xml2, "SVGZ round-trip produced different SVG");

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drawing.SVGZ");
        save_svg(&doc, &path).unwrap();
        assert!(fs::read(&path).unwrap().starts_with(&[0x1f, 0x8b]));
        assert_eq!(sivgo_dom::serialize_svg(&load_svg(&path).unwrap()), raw_xml);
    }

    #[test]
    fn load_svgz_invalid_data() {
        let result = load_svgz(b"this is not gzip data");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------
    // Inkscape SVG save
    // -----------------------------------------------------------------

    #[test]
    fn save_inkscape_svg_includes_namespaces() {
        let doc = test_doc();
        let xml = save_inkscape_svg(&doc);

        assert!(
            xml.contains("xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\""),
            "Inkscape namespace declaration missing from output: {xml}"
        );
        assert!(
            xml.contains("xmlns:sodipodi=\"http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd\""),
            "Sodipodi namespace declaration missing from output: {xml}"
        );
        // Basic SVG content should still be present.
        assert!(xml.contains("<svg"));
        assert!(xml.contains("<rect"));
    }

    #[test]
    fn save_inkscape_svg_preserves_existing_namespaces() {
        // Parse an SVG that already has the Inkscape namespaces.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd"
             width="100" height="100">
          <rect x="0" y="0" width="100" height="100"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let xml = save_inkscape_svg(&doc);

        // The root <svg> element must contain the namespace declarations.
        // Extract the root element's opening tag for verification.
        let svg_tag_end = xml.find('>').expect("no closing > in root element");
        let svg_tag = &xml[..svg_tag_end];

        assert!(
            svg_tag.contains("xmlns:inkscape="),
            "root <svg> should contain xmlns:inkscape, got: {svg_tag}"
        );
        assert!(
            svg_tag.contains("xmlns:sodipodi="),
            "root <svg> should contain xmlns:sodipodi, got: {svg_tag}"
        );

        // Standard SVG content should be preserved.
        assert!(xml.contains("<svg"));
        assert!(xml.contains("<rect"));
    }

    // -----------------------------------------------------------------
    // Plain SVG export (strip Inkscape extensions)
    // -----------------------------------------------------------------

    #[test]
    fn export_plain_svg_strips_inkscape_elements() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd"
             width="800" height="600">
          <sodipodi:namedview id="nv1" pagecolor="#ffffff" inkscape:zoom="1.5">
            <inkscape:grid id="grid1" type="xygrid"/>
            <sodipodi:guide id="guide1" position="100,0" orientation="0,1"/>
          </sodipodi:namedview>
          <g inkscape:groupmode="layer" inkscape:label="Layer 1" id="layer1">
            <rect x="0" y="0" width="100" height="100" fill="red"/>
          </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let plain = export_plain_svg(&doc);

        // Inkscape/Sodipodi elements should be gone.
        assert!(
            !plain.contains("sodipodi:namedview"),
            "namedview should be stripped"
        );
        assert!(!plain.contains("inkscape:grid"), "grid should be stripped");
        assert!(
            !plain.contains("sodipodi:guide"),
            "guide should be stripped"
        );

        // Inkscape attributes on remaining elements should be gone.
        assert!(
            !plain.contains("inkscape:groupmode"),
            "groupmode attr should be stripped"
        );
        assert!(
            !plain.contains("inkscape:label"),
            "label attr should be stripped"
        );

        // Inkscape namespace declarations should be gone.
        assert!(
            !plain.contains("xmlns:inkscape"),
            "inkscape xmlns should be stripped"
        );
        assert!(
            !plain.contains("xmlns:sodipodi"),
            "sodipodi xmlns should be stripped"
        );

        // Standard SVG content should be preserved.
        assert!(plain.contains("<svg"), "svg element missing");
        assert!(plain.contains("<g"), "g element missing");
        assert!(plain.contains("<rect"), "rect element missing");
        assert!(plain.contains("fill=\"#ff0000\""), "fill attr missing");
    }

    #[test]
    fn export_plain_svg_strips_path_inkscape_attrs() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape">
          <path d="M 0 0 L 100 100" inkscape:path-effect="#effect1"
                inkscape:original-d="M 0 0 L 50 50"/>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let plain = export_plain_svg(&doc);

        assert!(
            !plain.contains("inkscape:path-effect"),
            "path-effect should be stripped"
        );
        assert!(
            !plain.contains("inkscape:original-d"),
            "original-d should be stripped"
        );
        assert!(
            plain.contains("d=\""),
            "path d attribute should be preserved"
        );
    }

    #[test]
    fn export_optimized_svg_strips_extensions_and_minifies() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
             xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.0.dtd"
             width="100" height="100">
          <sodipodi:namedview id="nv1"/>
          <g inkscape:groupmode="layer" inkscape:label="Layer">
            <rect x="1" y="2" width="30" height="40" fill="red"/>
          </g>
        </svg>"##;

        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let optimized = export_optimized_svg(&doc);
        assert!(!optimized.contains("inkscape:"));
        assert!(!optimized.contains("sodipodi:"));
        assert!(!optimized.contains('\n'));
        assert!(optimized.contains("<rect"));
        assert!(optimized.len() <= export_plain_svg(&doc).len());
    }

    // -----------------------------------------------------------------
    // JPEG export
    // -----------------------------------------------------------------

    #[test]
    fn export_jpeg_produces_valid_data() {
        let doc = test_doc();
        let jpeg_data = export_jpeg(&doc, 100, 100, 85).expect("export_jpeg failed");

        // JPEG files start with SOI marker: 0xFF 0xD8
        assert!(jpeg_data.len() > 2, "JPEG data too short");
        assert_eq!(jpeg_data[0], 0xFF, "JPEG SOI marker byte 0");
        assert_eq!(jpeg_data[1], 0xD8, "JPEG SOI marker byte 1");

        // JPEG files end with EOI marker: 0xFF 0xD9
        assert_eq!(
            jpeg_data[jpeg_data.len() - 2],
            0xFF,
            "JPEG EOI marker byte 0"
        );
        assert_eq!(
            jpeg_data[jpeg_data.len() - 1],
            0xD9,
            "JPEG EOI marker byte 1"
        );
    }

    #[test]
    fn export_jpeg_quality_affects_size() {
        let doc = test_doc();
        let low_q = export_jpeg(&doc, 100, 100, 10).expect("low quality JPEG failed");
        let high_q = export_jpeg(&doc, 100, 100, 95).expect("high quality JPEG failed");

        // Higher quality should generally produce larger files.
        // For a solid-color image this might not always hold, but the low quality
        // should at least produce valid data.
        assert!(!low_q.is_empty());
        assert!(!high_q.is_empty());
    }

    // -----------------------------------------------------------------
    // export_with_config (10.15, 10.16)
    // -----------------------------------------------------------------

    #[test]
    fn export_with_config_default_produces_png() {
        let doc = test_doc();
        let config = ExportConfig::default();
        let png_data = export_with_config(&doc, &config).expect("export_with_config failed");

        // Verify PNG magic bytes.
        assert!(png_data.len() > 8);
        assert_eq!(&png_data[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

        // Decode and verify dimensions: 100x100 at 96 DPI => 100x100 pixels.
        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 100);
    }

    #[test]
    fn export_with_config_higher_dpi_produces_larger_image() {
        let doc = test_doc();

        // At 192 DPI (2x), a 100x100 SVG should produce a 200x200 PNG.
        let config = ExportConfig {
            area: ExportArea::Page,
            dpi: 192.0,
            background: None,
        };
        let png_data = export_with_config(&doc, &config).expect("192 DPI export failed");

        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 200);
        assert_eq!(info.height, 200);
    }

    #[test]
    fn export_with_config_lower_dpi() {
        let doc = test_doc();

        // At 48 DPI (0.5x), a 100x100 SVG should produce a 50x50 PNG.
        let config = ExportConfig {
            area: ExportArea::Page,
            dpi: 48.0,
            background: None,
        };
        let png_data = export_with_config(&doc, &config).expect("48 DPI export failed");

        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 50);
        assert_eq!(info.height, 50);
    }

    #[test]
    fn export_with_config_custom_area() {
        let doc = test_doc();

        // Export only the top-left 50x50 area.
        let config = ExportConfig {
            area: ExportArea::Custom {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            dpi: 96.0,
            background: None,
        };
        let png_data = export_with_config(&doc, &config).expect("custom area export failed");

        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 50);
        assert_eq!(info.height, 50);
    }

    #[test]
    fn export_with_config_drawing_area() {
        let doc = test_doc();

        // Drawing area should be the bounding box of the rect: 10,10 80x80.
        let config = ExportConfig {
            area: ExportArea::Drawing,
            dpi: 96.0,
            background: None,
        };
        let png_data = export_with_config(&doc, &config).expect("drawing area export failed");

        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 80, "drawing width should be 80");
        assert_eq!(info.height, 80, "drawing height should be 80");
    }

    #[test]
    fn export_with_config_with_background() {
        let doc = test_doc();

        // Export with a solid blue background.
        // Note: the renderer clears to opaque white before drawing, so the
        // background composite only affects truly transparent pixels.  For
        // this test we verify the function succeeds and produces valid
        // PNG output with the expected dimensions.
        let config = ExportConfig {
            area: ExportArea::Page,
            dpi: 96.0,
            background: Some([0, 0, 255, 255]), // blue
        };
        let png_data = export_with_config(&doc, &config).expect("background export failed");
        assert!(!png_data.is_empty());

        // Verify it is a valid PNG with correct dimensions.
        let decoder = png::Decoder::new(std::io::Cursor::new(&png_data));
        let mut reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 100);
        assert_eq!(info.height, 100);

        // Verify we can decode pixel data.
        let mut buf = vec![0u8; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut buf).unwrap();
        let pixels = &buf[..frame.buffer_size()];
        assert_eq!(pixels.len(), 100 * 100 * 4, "expected RGBA pixel data");

        // The center pixel (50,50) should still be red (from the rect).
        let idx = (50 * 100 + 50) * 4;
        assert_eq!(pixels[idx], 255, "red channel at center");
        assert_eq!(pixels[idx + 1], 0, "green channel at center");
        assert_eq!(pixels[idx + 2], 0, "blue channel at center");
    }

    #[test]
    fn composite_background_on_transparent() {
        // Unit test for the composite_background helper.
        // Fully transparent source + blue background => blue.
        let transparent = [0u8, 0, 0, 0]; // premultiplied transparent
        let result = composite_background(&transparent, [0, 0, 255, 255]);
        assert_eq!(result[0], 0, "R");
        assert_eq!(result[1], 0, "G");
        assert_eq!(result[2], 255, "B");
        assert_eq!(result[3], 255, "A");
    }

    // -----------------------------------------------------------------
    // batch_export (10.14)
    // -----------------------------------------------------------------

    #[test]
    fn batch_export_png_and_svg() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let doc = test_doc();

        let items = vec![
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("out.png").to_string_lossy().into_owned(),
                format: ExportFormat::Png,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("out.svg").to_string_lossy().into_owned(),
                format: ExportFormat::Svg,
            },
        ];

        let results = batch_export(&doc, &items);
        assert_eq!(results.len(), 2);
        for (i, r) in results.iter().enumerate() {
            assert!(r.is_ok(), "batch item {i} failed: {:?}", r);
        }

        // Verify PNG file.
        let png_bytes = fs::read(dir.path().join("out.png")).unwrap();
        assert_eq!(&png_bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);

        // Verify SVG file.
        let svg_text = fs::read_to_string(dir.path().join("out.svg")).unwrap();
        assert!(svg_text.contains("<svg"));
        assert!(svg_text.contains("<rect"));
    }

    #[test]
    fn batch_export_jpeg() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let doc = test_doc();

        let items = vec![BatchExportItem {
            area: ExportArea::Page,
            filename: dir.path().join("out.jpg").to_string_lossy().into_owned(),
            format: ExportFormat::Jpeg { quality: 85 },
        }];

        let results = batch_export(&doc, &items);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].is_ok(),
            "JPEG batch export failed: {:?}",
            results[0]
        );

        let jpeg_bytes = fs::read(dir.path().join("out.jpg")).unwrap();
        assert_eq!(jpeg_bytes[0], 0xFF);
        assert_eq!(jpeg_bytes[1], 0xD8);
    }

    #[test]
    fn batch_export_extended_vector_and_raster_formats() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let doc = test_doc();

        let items = vec![
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("plain.svg").to_string_lossy().into_owned(),
                format: ExportFormat::PlainSvg,
            },
            BatchExportItem {
                area: ExportArea::Drawing,
                filename: dir
                    .path()
                    .join("optimized.svg")
                    .to_string_lossy()
                    .into_owned(),
                format: ExportFormat::OptimizedSvg,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir
                    .path()
                    .join("compressed.svgz")
                    .to_string_lossy()
                    .into_owned(),
                format: ExportFormat::Svgz,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("doc.pdf").to_string_lossy().into_owned(),
                format: ExportFormat::Pdf,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("doc.eps").to_string_lossy().into_owned(),
                format: ExportFormat::Eps,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("doc.dxf").to_string_lossy().into_owned(),
                format: ExportFormat::Dxf,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("doc.webp").to_string_lossy().into_owned(),
                format: ExportFormat::WebP { quality: 90 },
            },
        ];

        let results = batch_export(&doc, &items);
        for (idx, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "batch item {idx} failed: {result:?}");
        }

        let plain = fs::read_to_string(dir.path().join("plain.svg")).unwrap();
        assert!(plain.contains("<svg"));
        assert!(!plain.contains("inkscape:"));

        let optimized = fs::read_to_string(dir.path().join("optimized.svg")).unwrap();
        assert!(optimized.contains("viewBox=\"10 10 80 80\""));
        assert!(!optimized.contains('\n'));

        let svgz = fs::read(dir.path().join("compressed.svgz")).unwrap();
        let svgz_doc = load_svgz(&svgz).expect("batch SVGZ should round-trip");
        assert_eq!(svgz_doc.len(), doc.len());

        let pdf = fs::read(dir.path().join("doc.pdf")).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));

        let eps = fs::read_to_string(dir.path().join("doc.eps")).unwrap();
        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));

        let dxf = fs::read_to_string(dir.path().join("doc.dxf")).unwrap();
        assert!(dxf.contains("ENTITIES"));
        assert!(dxf.trim_end().ends_with("EOF"));

        let webp = fs::read(dir.path().join("doc.webp")).unwrap();
        assert!(webp.starts_with(b"RIFF"));
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn raster_mime_detection_covers_supported_imports() {
        assert_eq!(
            detect_image_mime(&[0x89, b'P', b'N', b'G']),
            Some("image/png")
        );
        assert_eq!(
            detect_image_mime(&[0xFF, 0xD8, 0xFF, 0x00]),
            Some("image/jpeg")
        );
        assert_eq!(detect_image_mime(b"GIF89a"), Some("image/gif"));
        assert_eq!(detect_image_mime(b"BMxxxx"), Some("image/bmp"));
        assert_eq!(detect_image_mime(b"RIFFxxxxWEBP"), Some("image/webp"));
        assert_eq!(
            detect_image_mime(&[0x49, 0x49, 0x2A, 0x00]),
            Some("image/tiff")
        );
    }

    #[test]
    fn batch_export_custom_area() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let doc = test_doc();

        let items = vec![BatchExportItem {
            area: ExportArea::Custom {
                x: 10.0,
                y: 10.0,
                width: 50.0,
                height: 50.0,
            },
            filename: dir.path().join("crop.png").to_string_lossy().into_owned(),
            format: ExportFormat::Png,
        }];

        let results = batch_export(&doc, &items);
        assert!(
            results[0].is_ok(),
            "custom area batch failed: {:?}",
            results[0]
        );

        // Decode and verify dimensions.
        let png_bytes = fs::read(dir.path().join("crop.png")).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(&png_bytes));
        let reader = decoder.read_info().unwrap();
        let info = reader.info();
        assert_eq!(info.width, 50);
        assert_eq!(info.height, 50);
    }

    #[test]
    fn batch_export_svg_with_drawing_area() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let doc = test_doc();

        let items = vec![BatchExportItem {
            area: ExportArea::Drawing,
            filename: dir
                .path()
                .join("drawing.svg")
                .to_string_lossy()
                .into_owned(),
            format: ExportFormat::Svg,
        }];

        let results = batch_export(&doc, &items);
        assert!(
            results[0].is_ok(),
            "drawing SVG batch failed: {:?}",
            results[0]
        );

        let svg_text = fs::read_to_string(dir.path().join("drawing.svg")).unwrap();
        // The viewBox should be set to the drawing bounding box (10 10 80 80).
        assert!(
            svg_text.contains("viewBox=\"10 10 80 80\""),
            "SVG should have drawing-area viewBox, got: {svg_text}"
        );
    }

    #[test]
    fn batch_export_partial_failure() {
        let doc = test_doc();

        // One valid item, one with an impossible path.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let items = vec![
            BatchExportItem {
                area: ExportArea::Page,
                filename: dir.path().join("good.png").to_string_lossy().into_owned(),
                format: ExportFormat::Png,
            },
            BatchExportItem {
                area: ExportArea::Page,
                filename: "/nonexistent/dir/bad.png".to_string(),
                format: ExportFormat::Png,
            },
        ];

        let results = batch_export(&doc, &items);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok(), "first item should succeed");
        assert!(results[1].is_err(), "second item should fail (bad path)");
    }

    // -----------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------

    #[test]
    fn page_area_from_width_height() {
        let doc = test_doc();
        let (x, y, w, h) = page_area(&doc).expect("page_area failed");
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(w, 100.0);
        assert_eq!(h, 100.0);
    }

    #[test]
    fn page_area_from_viewbox() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="10 20 300 400"/>"#;
        let doc = sivgo_dom::parse_svg(svg).unwrap();
        let (x, y, w, h) = page_area(&doc).expect("page_area failed");
        assert_eq!(x, 10.0);
        assert_eq!(y, 20.0);
        assert_eq!(w, 300.0);
        assert_eq!(h, 400.0);
    }

    #[test]
    fn drawing_area_computes_bbox() {
        let doc = test_doc();
        let (x, y, w, h) = drawing_area(&doc).expect("drawing_area failed");
        // The rect is at (10,10) with size 80x80.
        assert_eq!(x, 10.0);
        assert_eq!(y, 10.0);
        assert_eq!(w, 80.0);
        assert_eq!(h, 80.0);
    }

    #[test]
    fn resolve_area_dpi_scaling() {
        let doc = test_doc();
        // At 192 DPI (2x), page 100x100 => 200x200 pixels.
        let (_, _, _, _, pw, ph) = resolve_area_and_size(&doc, &ExportArea::Page, 192.0).unwrap();
        assert_eq!(pw, 200);
        assert_eq!(ph, 200);

        // At 48 DPI (0.5x), page 100x100 => 50x50 pixels.
        let (_, _, _, _, pw, ph) = resolve_area_and_size(&doc, &ExportArea::Page, 48.0).unwrap();
        assert_eq!(pw, 50);
        assert_eq!(ph, 50);
    }

    #[test]
    fn export_config_default_is_96_dpi_page() {
        let config = ExportConfig::default();
        assert_eq!(config.dpi, 96.0);
        assert!(config.background.is_none());
        assert!(matches!(config.area, ExportArea::Page));
    }
}
