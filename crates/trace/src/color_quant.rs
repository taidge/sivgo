//! Color quantization tracing for multi-color bitmap-to-vector conversion.
//!
//! Pipeline:
//! 1. Quantize the RGBA image to N colors (median-cut algorithm).
//! 2. For each quantized color, create a binary mask.
//! 3. Trace each mask using the standard contour pipeline.
//! 4. Return `(Color, Vec<BezPath>)` pairs.

use sivgo_math::BezPath;

use crate::bitmap::GrayBitmap;
use crate::contour::{contour_area, find_contours};
use crate::smooth::smooth_contour;
use crate::threshold::{threshold, BinaryBitmap};

/// An RGB color from quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Format as CSS hex color.
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// A traced layer: one quantized color and its vector paths.
#[derive(Debug, Clone)]
pub struct ColorLayer {
    pub color: RgbColor,
    pub paths: Vec<BezPath>,
}

/// Options for color quantization tracing.
#[derive(Debug, Clone)]
pub struct ColorTraceOptions {
    /// Number of colors to quantize to. Default: 8.
    pub num_colors: usize,

    /// Smoothing factor for output curves. Default: 1.0.
    pub smoothing: f64,

    /// Minimum contour area in pixels. Default: 4.
    pub min_area: u32,
}

impl Default for ColorTraceOptions {
    fn default() -> Self {
        Self {
            num_colors: 8,
            smoothing: 1.0,
            min_area: 4,
        }
    }
}

/// Pixel in RGB space for the median-cut algorithm.
#[derive(Clone, Copy)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}

/// A "bucket" of pixels for median-cut.
struct Bucket {
    pixels: Vec<Pixel>,
}

impl Bucket {
    /// Find which channel (0=R, 1=G, 2=B) has the widest range.
    fn widest_channel(&self) -> u8 {
        let (mut min_r, mut max_r) = (255u8, 0u8);
        let (mut min_g, mut max_g) = (255u8, 0u8);
        let (mut min_b, mut max_b) = (255u8, 0u8);
        for p in &self.pixels {
            min_r = min_r.min(p.r);
            max_r = max_r.max(p.r);
            min_g = min_g.min(p.g);
            max_g = max_g.max(p.g);
            min_b = min_b.min(p.b);
            max_b = max_b.max(p.b);
        }
        let range_r = max_r - min_r;
        let range_g = max_g - min_g;
        let range_b = max_b - min_b;
        if range_r >= range_g && range_r >= range_b {
            0
        } else if range_g >= range_b {
            1
        } else {
            2
        }
    }

    /// Average color of all pixels in this bucket.
    fn average_color(&self) -> RgbColor {
        if self.pixels.is_empty() {
            return RgbColor::new(0, 0, 0);
        }
        let n = self.pixels.len() as u64;
        let (mut sr, mut sg, mut sb) = (0u64, 0u64, 0u64);
        for p in &self.pixels {
            sr += p.r as u64;
            sg += p.g as u64;
            sb += p.b as u64;
        }
        RgbColor::new((sr / n) as u8, (sg / n) as u8, (sb / n) as u8)
    }

    /// Split this bucket into two by sorting on the widest channel and
    /// cutting at the median.
    fn split(mut self) -> (Bucket, Bucket) {
        let ch = self.widest_channel();
        self.pixels.sort_by_key(|p| match ch {
            0 => p.r,
            1 => p.g,
            _ => p.b,
        });
        let mid = self.pixels.len() / 2;
        let right = self.pixels.split_off(mid);
        (
            Bucket {
                pixels: self.pixels,
            },
            Bucket { pixels: right },
        )
    }
}

/// Quantize RGBA pixel data to `n` colors using median-cut.
///
/// Returns a list of representative `RgbColor` values.
pub fn median_cut(data: &[u8], n: usize) -> Vec<RgbColor> {
    if n == 0 {
        return Vec::new();
    }

    let pixels: Vec<Pixel> = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| Pixel {
            r: c[0],
            g: c[1],
            b: c[2],
        })
        .collect();

    if pixels.is_empty() {
        return Vec::new();
    }

    let mut buckets = vec![Bucket { pixels }];

    // Repeatedly split the largest bucket until we have enough
    while buckets.len() < n {
        // Find the bucket with the most pixels
        let idx = buckets
            .iter()
            .enumerate()
            .max_by_key(|(_, b)| b.pixels.len())
            .map(|(i, _)| i)
            .unwrap();

        if buckets[idx].pixels.len() < 2 {
            break; // Can't split further
        }

        let bucket = buckets.remove(idx);
        let (a, b) = bucket.split();
        if !a.pixels.is_empty() {
            buckets.push(a);
        }
        if !b.pixels.is_empty() {
            buckets.push(b);
        }
    }

    buckets.iter().map(|b| b.average_color()).collect()
}

/// Map each pixel to the nearest quantized color (by Euclidean distance in RGB).
fn nearest_color(r: u8, g: u8, b: u8, palette: &[RgbColor]) -> usize {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| {
            let dr = r as i32 - c.r as i32;
            let dg = g as i32 - c.g as i32;
            let db = b as i32 - c.b as i32;
            (dr * dr + dg * dg + db * db) as u32
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Trace an RGBA image with color quantization.
///
/// Returns a `ColorLayer` for each quantized color, containing the color
/// and all vector paths for regions of that color.
pub fn trace_colors(
    data: &[u8],
    width: u32,
    height: u32,
    options: &ColorTraceOptions,
) -> Vec<ColorLayer> {
    let palette = median_cut(data, options.num_colors);
    if palette.is_empty() {
        return Vec::new();
    }

    let pixel_count = (width as usize) * (height as usize);

    // Classify each pixel
    let mut assignments = vec![0usize; pixel_count];
    for (i, chunk) in data.as_chunks::<4>().0.iter().enumerate() {
        assignments[i] = nearest_color(chunk[0], chunk[1], chunk[2], &palette);
    }

    let min_area = options.min_area as f64;

    // For each color, create a binary mask and trace it
    palette
        .iter()
        .enumerate()
        .filter_map(|(color_idx, &color)| {
            // Build grayscale bitmap: pixels matching this color = black (0),
            // others = white (255)
            let mut bmp = GrayBitmap::new(width, height);
            for (i, &assigned) in assignments.iter().enumerate() {
                if assigned == color_idx {
                    let x = (i % width as usize) as u32;
                    let y = (i / width as usize) as u32;
                    bmp.set(x, y, 0);
                }
            }

            let binary: BinaryBitmap = threshold(&bmp, 128);
            let contours = find_contours(&binary);

            let paths: Vec<BezPath> = contours
                .iter()
                .filter(|c| contour_area(c) >= min_area)
                .map(|c| smooth_contour(c, options.smoothing))
                .collect();

            if paths.is_empty() {
                None
            } else {
                Some(ColorLayer { color, paths })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_cut_two_colors() {
        // Half red, half blue image
        let mut data = Vec::new();
        for _ in 0..50 {
            data.extend_from_slice(&[255, 0, 0, 255]); // red
        }
        for _ in 0..50 {
            data.extend_from_slice(&[0, 0, 255, 255]); // blue
        }
        let colors = median_cut(&data, 2);
        assert_eq!(colors.len(), 2);

        // One should be reddish, one bluish
        let has_red = colors.iter().any(|c| c.r > 200 && c.b < 50);
        let has_blue = colors.iter().any(|c| c.b > 200 && c.r < 50);
        assert!(has_red, "Should have a red color");
        assert!(has_blue, "Should have a blue color");
    }

    #[test]
    fn median_cut_single_color() {
        let data: Vec<u8> = (0..10).flat_map(|_| [100u8, 150, 200, 255]).collect();
        let colors = median_cut(&data, 4);
        // With only one unique color, we should get exactly 1 bucket
        assert!(!colors.is_empty());
        assert!(colors[0].r.abs_diff(100) <= 1);
    }

    #[test]
    fn trace_colors_two_halves() {
        // 10x10 image: left half red, right half blue
        let mut data = Vec::new();
        for _y in 0..10 {
            for x in 0..10 {
                if x < 5 {
                    data.extend_from_slice(&[255, 0, 0, 255]);
                } else {
                    data.extend_from_slice(&[0, 0, 255, 255]);
                }
            }
        }
        let opts = ColorTraceOptions {
            num_colors: 2,
            smoothing: 0.0,
            min_area: 1,
        };
        let layers = trace_colors(&data, 10, 10, &opts);
        assert!(
            !layers.is_empty(),
            "Should produce at least one color layer"
        );
        assert!(layers.len() <= 2, "Should produce at most 2 color layers");
        for layer in &layers {
            assert!(!layer.paths.is_empty(), "Each layer should have paths");
        }
    }

    #[test]
    fn rgb_color_to_hex() {
        let c = RgbColor::new(255, 128, 0);
        assert_eq!(c.to_hex(), "#ff8000");
    }
}
