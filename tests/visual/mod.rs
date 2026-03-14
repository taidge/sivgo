//! Visual regression test framework.
//!
//! Renders SVG files to pixel buffers and compares them against reference
//! images. Differences are measured as mean absolute pixel error (MAE).
//! If no reference exists, the test creates one automatically.

use std::path::{Path, PathBuf};

/// Result of comparing two rendered images.
#[derive(Debug)]
pub struct ComparisonResult {
    /// Mean absolute error across all pixel channels (0.0 = identical).
    pub mae: f64,
    /// Maximum per-channel difference found.
    pub max_diff: u8,
    /// Number of pixels that differ above the threshold.
    pub diff_pixel_count: usize,
    /// Total number of pixels compared.
    pub total_pixels: usize,
}

impl ComparisonResult {
    /// Whether the images are considered equal within the given tolerance.
    pub fn is_match(&self, tolerance: f64) -> bool {
        self.mae <= tolerance
    }
}

/// Compare two RGBA pixel buffers of equal dimensions.
pub fn compare_pixels(
    actual: &[u8],
    reference: &[u8],
    width: u32,
    height: u32,
    threshold: u8,
) -> ComparisonResult {
    assert_eq!(actual.len(), reference.len());
    assert_eq!(actual.len(), (width * height * 4) as usize);

    let total = (width * height) as usize;
    let mut total_diff: u64 = 0;
    let mut max_diff: u8 = 0;
    let mut diff_count: usize = 0;

    for (a_chunk, r_chunk) in actual.chunks_exact(4).zip(reference.chunks_exact(4)) {
        let mut pixel_differs = false;
        for i in 0..4 {
            let d = (a_chunk[i] as i16 - r_chunk[i] as i16).unsigned_abs() as u8;
            total_diff += d as u64;
            if d > max_diff {
                max_diff = d;
            }
            if d > threshold {
                pixel_differs = true;
            }
        }
        if pixel_differs {
            diff_count += 1;
        }
    }

    let total_channels = total * 4;
    let mae = if total_channels > 0 {
        total_diff as f64 / total_channels as f64
    } else {
        0.0
    };

    ComparisonResult {
        mae,
        max_diff,
        diff_pixel_count: diff_count,
        total_pixels: total,
    }
}

/// Generate a difference image highlighting pixels that differ.
///
/// Returns an RGBA buffer where:
/// - Identical pixels are shown at 30% opacity
/// - Different pixels are highlighted in red
pub fn generate_diff_image(
    actual: &[u8],
    reference: &[u8],
    threshold: u8,
) -> Vec<u8> {
    let mut diff = Vec::with_capacity(actual.len());

    for (a_chunk, r_chunk) in actual.chunks_exact(4).zip(reference.chunks_exact(4)) {
        let max_ch_diff = (0..4)
            .map(|i| (a_chunk[i] as i16 - r_chunk[i] as i16).unsigned_abs() as u8)
            .max()
            .unwrap_or(0);

        if max_ch_diff > threshold {
            // Highlight difference in red
            diff.extend_from_slice(&[255, 0, 0, 255]);
        } else {
            // Show actual at reduced opacity
            diff.extend_from_slice(&[
                a_chunk[0],
                a_chunk[1],
                a_chunk[2],
                (a_chunk[3] as u16 * 77 / 255) as u8, // 30% opacity
            ]);
        }
    }

    diff
}

/// Get the path to the reference image directory.
pub fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("visual")
        .join("references")
}

/// Get the path to the example SVGs directory.
pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_images_zero_mae() {
        let pixels = vec![255u8, 0, 0, 255, 0, 255, 0, 255]; // 2 pixels
        let result = compare_pixels(&pixels, &pixels, 2, 1, 0);
        assert!((result.mae - 0.0).abs() < 1e-10);
        assert_eq!(result.max_diff, 0);
        assert_eq!(result.diff_pixel_count, 0);
    }

    #[test]
    fn different_images_nonzero_mae() {
        let actual = vec![255u8, 0, 0, 255, 0, 0, 0, 255]; // red, black
        let reference = vec![0u8, 0, 0, 255, 0, 0, 0, 255]; // black, black
        let result = compare_pixels(&actual, &reference, 2, 1, 0);
        assert!(result.mae > 0.0);
        assert_eq!(result.max_diff, 255);
        assert_eq!(result.diff_pixel_count, 1);
    }

    #[test]
    fn threshold_filters_small_diffs() {
        let actual = vec![128u8, 128, 128, 255];
        let reference = vec![130u8, 130, 130, 255];
        let result = compare_pixels(&actual, &reference, 1, 1, 5);
        // Differences are 2 per channel, below threshold of 5
        assert_eq!(result.diff_pixel_count, 0);
    }

    #[test]
    fn diff_image_highlights_changes() {
        let actual = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
        let reference = vec![0u8, 0, 0, 255, 0, 255, 0, 255];
        let diff = generate_diff_image(&actual, &reference, 0);
        // First pixel should be red (different)
        assert_eq!(diff[0], 255);
        assert_eq!(diff[1], 0);
        assert_eq!(diff[2], 0);
        // Second pixel should have reduced alpha (same)
        assert!(diff[7] < 255);
    }
}
