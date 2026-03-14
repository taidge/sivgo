use crate::bitmap::GrayBitmap;

/// A binary bitmap (one bit per pixel).
///
/// `true` means "foreground" (black), `false` means "background" (white).
#[derive(Debug, Clone)]
pub struct BinaryBitmap {
    pub width: u32,
    pub height: u32,
    bits: Vec<bool>,
}

impl BinaryBitmap {
    /// Create a new all-background (all false) binary bitmap.
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize);
        BinaryBitmap {
            width,
            height,
            bits: vec![false; len],
        }
    }

    /// Get the pixel at (x, y). Returns `false` (background) for out-of-bounds coordinates.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        self.bits[(y as usize) * (self.width as usize) + (x as usize)]
    }

    /// Set the pixel at (x, y). Panics if out of bounds.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, val: bool) {
        debug_assert!(x < self.width && y < self.height);
        self.bits[(y as usize) * (self.width as usize) + (x as usize)] = val;
    }
}

/// Apply a brightness threshold to create a binary bitmap.
///
/// Pixels with grayscale value < `level` become foreground (true/black).
/// Pixels with value >= `level` become background (false/white).
pub fn threshold(bitmap: &GrayBitmap, level: u8) -> BinaryBitmap {
    let mut result = BinaryBitmap::new(bitmap.width, bitmap.height);
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            if bitmap.get(x, y) < level {
                result.set(x, y, true);
            }
        }
    }
    result
}

/// Compute an automatic threshold using Otsu's method.
///
/// Otsu's method finds the threshold that minimizes the weighted intra-class variance
/// of a bimodal histogram. Returns a threshold value in 0..=255.
pub fn otsu_threshold(bitmap: &GrayBitmap) -> u8 {
    if bitmap.pixels.is_empty() {
        return 128;
    }

    // Build histogram
    let mut histogram = [0u64; 256];
    for &p in &bitmap.pixels {
        histogram[p as usize] += 1;
    }

    let total = bitmap.pixels.len() as f64;

    // Compute total mean
    let mut sum_total: f64 = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += (i as f64) * (count as f64);
    }

    let mut sum_bg: f64 = 0.0;
    let mut weight_bg: f64 = 0.0;
    let mut max_variance: f64 = 0.0;
    let mut best_threshold: u8 = 0;

    for t in 0..256u32 {
        weight_bg += histogram[t as usize] as f64;
        if weight_bg == 0.0 {
            continue;
        }

        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }

        sum_bg += (t as f64) * (histogram[t as usize] as f64);

        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_total - sum_bg) / weight_fg;

        // Between-class variance
        let variance = weight_bg * weight_fg * (mean_bg - mean_fg) * (mean_bg - mean_fg);

        if variance > max_variance {
            max_variance = variance;
            best_threshold = t as u8;
        }
    }

    // The threshold returned is the value below which pixels are foreground.
    // Otsu gives us the optimal split point; we add 1 so that the returned value
    // can be used as: pixel < threshold → foreground.
    best_threshold.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_bitmap_new() {
        let bmp = BinaryBitmap::new(5, 5);
        assert_eq!(bmp.width, 5);
        assert_eq!(bmp.height, 5);
        assert!(!bmp.get(0, 0));
        assert!(!bmp.get(4, 4));
    }

    #[test]
    fn test_binary_bitmap_out_of_bounds() {
        let bmp = BinaryBitmap::new(5, 5);
        assert!(!bmp.get(-1, 0));
        assert!(!bmp.get(0, -1));
        assert!(!bmp.get(5, 0));
        assert!(!bmp.get(0, 5));
        assert!(!bmp.get(100, 100));
    }

    #[test]
    fn test_binary_bitmap_set_get() {
        let mut bmp = BinaryBitmap::new(5, 5);
        bmp.set(2, 3, true);
        assert!(bmp.get(2, 3));
        assert!(!bmp.get(3, 2));
    }

    #[test]
    fn test_threshold_simple() {
        let mut gray = GrayBitmap::new(4, 4);
        // Set some pixels to dark (below threshold)
        gray.set(0, 0, 10);
        gray.set(1, 0, 20);
        gray.set(2, 0, 200);
        gray.set(3, 0, 250);

        let bin = threshold(&gray, 128);
        assert!(bin.get(0, 0)); // 10 < 128 → foreground
        assert!(bin.get(1, 0)); // 20 < 128 → foreground
        assert!(!bin.get(2, 0)); // 200 >= 128 → background
        assert!(!bin.get(3, 0)); // 250 >= 128 → background
    }

    #[test]
    fn test_threshold_boundary() {
        let mut gray = GrayBitmap::new(2, 1);
        gray.set(0, 0, 127);
        gray.set(1, 0, 128);

        let bin = threshold(&gray, 128);
        assert!(bin.get(0, 0)); // 127 < 128 → foreground
        assert!(!bin.get(1, 0)); // 128 >= 128 → background
    }

    #[test]
    fn test_otsu_bimodal() {
        // Create a bimodal image: half pixels at 50, half at 200
        let mut gray = GrayBitmap::new(100, 1);
        for x in 0..50 {
            gray.set(x, 0, 50);
        }
        for x in 50..100 {
            gray.set(x, 0, 200);
        }

        let thresh = otsu_threshold(&gray);
        // Otsu should find a threshold somewhere between 50 and 200
        assert!(
            thresh > 50 && thresh <= 200,
            "Otsu threshold {} should be between 50 and 200",
            thresh
        );
    }

    #[test]
    fn test_otsu_all_same() {
        // All pixels the same value
        let mut gray = GrayBitmap::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                gray.set(x, y, 100);
            }
        }

        // Should not panic
        let _thresh = otsu_threshold(&gray);
    }
}
