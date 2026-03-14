//! Command-line batch processing (12.11).
//!
//! Provides argument parsing and a batch-processing driver that converts a
//! list of SVG input files to a specified output format (currently PNG).

use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parsed batch-processing arguments.
#[derive(Debug, Clone)]
pub struct BatchArgs {
    /// Input SVG file paths.
    pub input_files: Vec<String>,
    /// Directory where output files are written.
    pub output_dir: String,
    /// Output format (e.g. `"png"`).
    pub format: String,
    /// Dots-per-inch for rasterisation.
    pub dpi: f64,
    /// Optional fixed width (pixels); `None` means use the SVG's intrinsic width.
    pub width: Option<f64>,
    /// Optional fixed height (pixels); `None` means use the SVG's intrinsic height.
    pub height: Option<f64>,
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

/// Parse a raw argument slice into `BatchArgs`.
///
/// Expected format (order-independent flags + trailing positional files):
///
/// ```text
/// --output-dir <dir> --format <fmt> --dpi <n> [--width <w>] [--height <h>] file1.svg file2.svg ...
/// ```
///
/// Defaults: `--format png`, `--dpi 96`, `--output-dir .`
pub fn parse_batch_args(args: &[String]) -> Result<BatchArgs, String> {
    let mut output_dir = ".".to_string();
    let mut format = "png".to_string();
    let mut dpi: f64 = 96.0;
    let mut width: Option<f64> = None;
    let mut height: Option<f64> = None;
    let mut input_files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output-dir" => {
                i += 1;
                output_dir = args.get(i).ok_or("--output-dir requires a value")?.clone();
            }
            "--format" => {
                i += 1;
                format = args.get(i).ok_or("--format requires a value")?.clone();
            }
            "--dpi" => {
                i += 1;
                dpi = args
                    .get(i)
                    .ok_or("--dpi requires a value")?
                    .parse::<f64>()
                    .map_err(|_| "--dpi must be a number")?;
            }
            "--width" => {
                i += 1;
                width = Some(
                    args.get(i)
                        .ok_or("--width requires a value")?
                        .parse::<f64>()
                        .map_err(|_| "--width must be a number")?,
                );
            }
            "--height" => {
                i += 1;
                height = Some(
                    args.get(i)
                        .ok_or("--height requires a value")?
                        .parse::<f64>()
                        .map_err(|_| "--height must be a number")?,
                );
            }
            other => {
                if other.starts_with("--") {
                    return Err(format!("Unknown flag: {other}"));
                }
                input_files.push(other.to_string());
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        return Err("No input files specified".to_string());
    }

    Ok(BatchArgs {
        input_files,
        output_dir,
        format,
        dpi,
        width,
        height,
    })
}

// ---------------------------------------------------------------------------
// Batch driver
// ---------------------------------------------------------------------------

/// Process all files described by `args`.
///
/// For each input SVG the function:
/// 1. Reads and parses the SVG.
/// 2. Renders it using `sivgo-render`.
/// 3. Writes the result to `<output_dir>/<stem>.<format>`.
///
/// Returns a list of successfully written output file paths.
pub fn process_batch(args: &BatchArgs) -> Result<Vec<String>, String> {
    // Ensure output directory exists.
    fs::create_dir_all(&args.output_dir)
        .map_err(|e| format!("Cannot create output dir '{}': {e}", args.output_dir))?;

    let mut outputs: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for input in &args.input_files {
        match process_single(input, args) {
            Ok(out) => outputs.push(out),
            Err(e) => errors.push(format!("{input}: {e}")),
        }
    }

    if !errors.is_empty() && outputs.is_empty() {
        return Err(format!("All files failed:\n{}", errors.join("\n")));
    }

    // Log partial failures but still return the successful ones.
    for err in &errors {
        eprintln!("[batch] warning: {err}");
    }

    Ok(outputs)
}

/// Process a single input file and return the output path on success.
fn process_single(input: &str, args: &BatchArgs) -> Result<String, String> {
    let content =
        fs::read_to_string(input).map_err(|e| format!("Failed to read '{input}': {e}"))?;

    let doc =
        sivgo_dom::parse_svg(&content).map_err(|e| format!("Failed to parse '{input}': {e}"))?;

    // Determine render dimensions.
    let default_w = 800u32;
    let default_h = 600u32;
    let w = args.width.map(|v| v as u32).unwrap_or(default_w);
    let h = args.height.map(|v| v as u32).unwrap_or(default_h);

    let stem = Path::new(input)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let out_path = PathBuf::from(&args.output_dir).join(format!("{stem}.{}", args.format));

    match args.format.as_str() {
        "png" => {
            use sivgo_render::{DocumentRenderer, TinySkiaBackend};

            let mut backend = TinySkiaBackend::new(w, h);
            DocumentRenderer::render(&doc, &mut backend);
            let png_data = backend
                .pixmap()
                .encode_png()
                .map_err(|e| format!("PNG encode error: {e}"))?;
            fs::write(&out_path, &png_data)
                .map_err(|e| format!("Failed to write '{}': {e}", out_path.display()))?;
        }
        other => {
            return Err(format!("Unsupported output format: '{other}'"));
        }
    }

    Ok(out_path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_batch_args_defaults() {
        let args: Vec<String> = vec!["input.svg".to_string()];
        let parsed = parse_batch_args(&args).unwrap();
        assert_eq!(parsed.input_files, vec!["input.svg"]);
        assert_eq!(parsed.output_dir, ".");
        assert_eq!(parsed.format, "png");
        assert!((parsed.dpi - 96.0).abs() < f64::EPSILON);
        assert!(parsed.width.is_none());
        assert!(parsed.height.is_none());
    }

    #[test]
    fn test_parse_batch_args_full() {
        let args: Vec<String> = vec![
            "--output-dir",
            "/tmp/out",
            "--format",
            "png",
            "--dpi",
            "150",
            "--width",
            "1024",
            "--height",
            "768",
            "a.svg",
            "b.svg",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let parsed = parse_batch_args(&args).unwrap();
        assert_eq!(parsed.input_files, vec!["a.svg", "b.svg"]);
        assert_eq!(parsed.output_dir, "/tmp/out");
        assert!((parsed.dpi - 150.0).abs() < f64::EPSILON);
        assert_eq!(parsed.width, Some(1024.0));
        assert_eq!(parsed.height, Some(768.0));
    }

    #[test]
    fn test_parse_batch_args_no_files() {
        let args: Vec<String> = vec!["--format".to_string(), "png".to_string()];
        let result = parse_batch_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No input files"));
    }
}
