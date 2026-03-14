/// A grayscale bitmap for tracing.
///
/// Each pixel is stored as a single byte: 0 = black, 255 = white.
#[derive(Debug, Clone)]
pub struct GrayBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GrayBitmap {
    /// Create a new all-white bitmap with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize);
        GrayBitmap {
            width,
            height,
            pixels: vec![255; len],
        }
    }

    /// Create from RGBA pixel data, converting to grayscale using ITU-R BT.601 luminance.
    ///
    /// `data` must contain exactly `width * height * 4` bytes (R, G, B, A per pixel).
    /// The alpha channel is used to blend against a white background.
    pub fn from_rgba(data: &[u8], width: u32, height: u32) -> Self {
        let expected = (width as usize) * (height as usize) * 4;
        assert_eq!(
            data.len(),
            expected,
            "RGBA data length {} does not match {}x{}x4 = {}",
            data.len(),
            width,
            height,
            expected
        );

        let pixel_count = (width as usize) * (height as usize);
        let mut pixels = Vec::with_capacity(pixel_count);

        for chunk in data.chunks_exact(4) {
            let r = chunk[0] as f64;
            let g = chunk[1] as f64;
            let b = chunk[2] as f64;
            let a = chunk[3] as f64 / 255.0;

            // Luminance formula: ITU-R BT.601
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            // Blend against white background using alpha
            let blended = lum * a + 255.0 * (1.0 - a);
            pixels.push(blended.round().min(255.0).max(0.0) as u8);
        }

        GrayBitmap {
            width,
            height,
            pixels,
        }
    }

    /// Get the pixel value at (x, y). Panics if out of bounds.
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        debug_assert!(x < self.width && y < self.height);
        self.pixels[(y as usize) * (self.width as usize) + (x as usize)]
    }

    /// Set the pixel value at (x, y). Panics if out of bounds.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, val: u8) {
        debug_assert!(x < self.width && y < self.height);
        self.pixels[(y as usize) * (self.width as usize) + (x as usize)] = val;
    }

    /// Invert the bitmap (each pixel becomes 255 - pixel).
    pub fn invert(&mut self) {
        for p in &mut self.pixels {
            *p = 255 - *p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bitmap() {
        let bmp = GrayBitmap::new(10, 5);
        assert_eq!(bmp.width, 10);
        assert_eq!(bmp.height, 5);
        assert_eq!(bmp.pixels.len(), 50);
        assert!(bmp.pixels.iter().all(|&p| p == 255));
    }

    #[test]
    fn test_get_set() {
        let mut bmp = GrayBitmap::new(10, 10);
        bmp.set(3, 4, 42);
        assert_eq!(bmp.get(3, 4), 42);
        assert_eq!(bmp.get(0, 0), 255);
    }

    #[test]
    fn test_invert() {
        let mut bmp = GrayBitmap::new(2, 2);
        bmp.set(0, 0, 0);
        bmp.set(1, 0, 100);
        bmp.set(0, 1, 200);
        bmp.set(1, 1, 255);
        bmp.invert();
        assert_eq!(bmp.get(0, 0), 255);
        assert_eq!(bmp.get(1, 0), 155);
        assert_eq!(bmp.get(0, 1), 55);
        assert_eq!(bmp.get(1, 1), 0);
    }

    #[test]
    fn test_from_rgba() {
        // Pure red pixel, fully opaque
        let data: Vec<u8> = vec![255, 0, 0, 255];
        let bmp = GrayBitmap::from_rgba(&data, 1, 1);
        // 0.299*255 + 0.587*0 + 0.114*0 = 76.245 → 76
        assert_eq!(bmp.get(0, 0), 76);
    }

    #[test]
    fn test_from_rgba_white() {
        // Pure white pixel, fully opaque
        let data: Vec<u8> = vec![255, 255, 255, 255];
        let bmp = GrayBitmap::from_rgba(&data, 1, 1);
        assert_eq!(bmp.get(0, 0), 255);
    }

    #[test]
    fn test_from_rgba_transparent() {
        // Black pixel but fully transparent → blends to white
        let data: Vec<u8> = vec![0, 0, 0, 0];
        let bmp = GrayBitmap::from_rgba(&data, 1, 1);
        assert_eq!(bmp.get(0, 0), 255);
    }

    #[test]
    fn test_from_rgba_semi_transparent() {
        // Black pixel, half transparent → blends to ~128
        let data: Vec<u8> = vec![0, 0, 0, 128];
        let bmp = GrayBitmap::from_rgba(&data, 1, 1);
        // 0.0 * (128/255) + 255.0 * (1 - 128/255) ≈ 127.0
        let val = bmp.get(0, 0);
        assert!(val >= 126 && val <= 128, "got {}", val);
    }
}
