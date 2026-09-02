//! Server-side SVG export handler.
//!
//! Accepts SVG content, parses it with `sivgo-dom`, renders it with
//! `sivgo-render`, and returns the result as PNG bytes or as cleaned SVG.

use std::io::BufWriter;

use salvo::prelude::*;

use crate::audit::{self, AuditAction};
use crate::errors::ApiError;
use crate::models::*;

/// `POST /api/v1/export`
///
/// Renders SVG content to the requested format and returns the result as
/// a binary response. The request body is JSON:
///
/// ```json
/// {
///   "svg_content": "<svg ...>...</svg>",
///   "format": "Png",
///   "width": 800,
///   "height": 600
/// }
/// ```
#[handler]
pub async fn export_document(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let body: ExportRequest = match req.parse_json().await {
        Ok(b) => b,
        Err(e) => {
            ApiError::bad_request(format!("Invalid request body: {e}")).write_to(res);
            return;
        }
    };

    // This endpoint is unauthenticated; bound the input to limit DoS surface.
    const MAX_SVG_BYTES: usize = 8 * 1024 * 1024; // 8 MiB
    if body.svg_content.len() > MAX_SVG_BYTES {
        ApiError::bad_request("svg_content exceeds 8 MiB limit").write_to(res);
        return;
    }

    // Parse the SVG content using sivgo-dom.
    let doc = match sivgo_dom::parse_svg(&body.svg_content) {
        Ok(d) => d,
        Err(e) => {
            ApiError::bad_request(format!("Invalid SVG: {e}")).write_to(res);
            return;
        }
    };
    let svg_bytes = body.svg_content.len();
    let format_name = format!("{:?}", body.format);

    match body.format {
        ExportFormat::Svg => {
            // Re-serialize the parsed SVG (this normalises it).
            let xml = sivgo_dom::serialize_svg(&doc);
            res.headers_mut()
                .insert("content-type", "image/svg+xml".parse().unwrap());
            res.headers_mut().insert(
                "content-disposition",
                "attachment; filename=\"export.svg\"".parse().unwrap(),
            );
            audit::record_if_available(
                depot,
                AuditAction::ExportRequested,
                None,
                Some("export"),
                None,
                serde_json::json!({
                    "format": format_name,
                    "svg_bytes": svg_bytes,
                }),
            );
            res.render(xml);
        }
        ExportFormat::Png => {
            let width = body.width.unwrap_or(800);
            let height = body.height.unwrap_or(600);

            if width == 0 || height == 0 || width > 8192 || height > 8192 {
                ApiError::bad_request("width and height must be 1..8192").write_to(res);
                return;
            }

            // Cap total pixel area so a single request can't drive a huge
            // (e.g. 8192x8192 ~ 256 MiB RGBA) allocation on this public route.
            const MAX_PIXELS: u64 = 16 * 1024 * 1024; // 16 MP (~64 MiB RGBA)
            if width as u64 * height as u64 > MAX_PIXELS {
                ApiError::bad_request("width*height exceeds 16 megapixel limit").write_to(res);
                return;
            }

            // Render to premultiplied RGBA.
            //
            // The document came from an unauthenticated request, so file-system
            // image references are refused: an `<image href>` is attacker
            // controlled and would otherwise read server-local files.
            let premultiplied =
                sivgo_render::render_document_to_pixmap_without_file_access(&doc, width, height);

            // Un-premultiply alpha for PNG.
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
            }

            // Encode as PNG into an in-memory buffer.
            let mut png_buf: Vec<u8> = Vec::new();
            {
                let buf_writer = BufWriter::new(&mut png_buf);
                let mut encoder = png::Encoder::new(buf_writer, width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_compression(png::Compression::Balanced);

                let mut writer = match encoder.write_header() {
                    Ok(w) => w,
                    Err(e) => {
                        ApiError::internal(format!("PNG encode error: {e}")).write_to(res);
                        return;
                    }
                };

                if let Err(e) = writer.write_image_data(&pixels) {
                    ApiError::internal(format!("PNG write error: {e}")).write_to(res);
                    return;
                }
            }

            res.headers_mut()
                .insert("content-type", "image/png".parse().unwrap());
            res.headers_mut().insert(
                "content-disposition",
                "attachment; filename=\"export.png\"".parse().unwrap(),
            );
            audit::record_if_available(
                depot,
                AuditAction::ExportRequested,
                None,
                Some("export"),
                None,
                serde_json::json!({
                    "format": format_name,
                    "svg_bytes": svg_bytes,
                    "width": width,
                    "height": height,
                }),
            );
            res.body(png_buf);
        }
    }
}
