//! RGBA pixel buffer for filter processing.

/// An RGBA pixel buffer for filter processing.
/// Pixels are stored in straight (non-premultiplied) RGBA format, f32 per channel [0.0, 1.0].
#[derive(Debug, Clone)]
pub struct FilterBuffer {
    pub width: u32,
    pub height: u32,
    /// Flat array of RGBA f32 values. Length = width * height * 4.
    pub data: Vec<f32>,
}

impl FilterBuffer {
    /// Create a new buffer filled with transparent black.
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            data: vec![0.0; len],
        }
    }

    /// Convert from u8 RGBA data (0..255) to f32 RGBA data (0.0..1.0).
    pub fn from_rgba_u8(data: &[u8], width: u32, height: u32) -> Self {
        assert_eq!(
            data.len(),
            (width as usize) * (height as usize) * 4,
            "data length must match width * height * 4"
        );
        let float_data: Vec<f32> = data.iter().map(|&b| b as f32 / 255.0).collect();
        Self {
            width,
            height,
            data: float_data,
        }
    }

    /// Convert f32 RGBA data (0.0..1.0) back to u8 RGBA data (0..255).
    pub fn to_rgba_u8(&self) -> Vec<u8> {
        self.data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect()
    }

    /// Fill the entire buffer with a solid color.
    pub fn fill(&mut self, r: f32, g: f32, b: f32, a: f32) {
        for i in 0..(self.width as usize * self.height as usize) {
            let idx = i * 4;
            self.data[idx] = r;
            self.data[idx + 1] = g;
            self.data[idx + 2] = b;
            self.data[idx + 3] = a;
        }
    }

    /// Get the RGBA pixel at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> [f32; 4] {
        let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }

    /// Set the RGBA pixel at (x, y).
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [f32; 4]) {
        let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.data[idx] = rgba[0];
        self.data[idx + 1] = rgba[1];
        self.data[idx + 2] = rgba[2];
        self.data[idx + 3] = rgba[3];
    }

    /// Create a new buffer with the same dimensions, filled with transparent black.
    pub fn clone_empty(&self) -> Self {
        Self::new(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_transparent() {
        let buf = FilterBuffer::new(4, 4);
        assert_eq!(buf.data.len(), 64);
        for &v in &buf.data {
            assert_eq!(v, 0.0);
        }
    }

    #[test]
    fn test_fill() {
        let mut buf = FilterBuffer::new(2, 2);
        buf.fill(1.0, 0.5, 0.25, 0.75);
        for i in 0..4 {
            let idx = i * 4;
            assert_eq!(buf.data[idx], 1.0);
            assert_eq!(buf.data[idx + 1], 0.5);
            assert_eq!(buf.data[idx + 2], 0.25);
            assert_eq!(buf.data[idx + 3], 0.75);
        }
    }

    #[test]
    fn test_get_set_pixel() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.set_pixel(1, 2, [0.1, 0.2, 0.3, 0.4]);
        let px = buf.get_pixel(1, 2);
        assert_eq!(px, [0.1, 0.2, 0.3, 0.4]);
        // Other pixels should still be zero
        let px0 = buf.get_pixel(0, 0);
        assert_eq!(px0, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_from_rgba_u8() {
        let data = vec![255, 128, 0, 255];
        let buf = FilterBuffer::from_rgba_u8(&data, 1, 1);
        let px = buf.get_pixel(0, 0);
        assert!((px[0] - 1.0).abs() < 0.01);
        assert!((px[1] - 0.502).abs() < 0.01);
        assert_eq!(px[2], 0.0);
        assert!((px[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_to_rgba_u8() {
        let mut buf = FilterBuffer::new(1, 1);
        buf.set_pixel(0, 0, [1.0, 0.5, 0.0, 1.0]);
        let bytes = buf.to_rgba_u8();
        assert_eq!(bytes[0], 255);
        assert_eq!(bytes[1], 128);
        assert_eq!(bytes[2], 0);
        assert_eq!(bytes[3], 255);
    }

    #[test]
    fn test_clone_empty() {
        let mut buf = FilterBuffer::new(5, 5);
        buf.fill(1.0, 1.0, 1.0, 1.0);
        let empty = buf.clone_empty();
        assert_eq!(empty.width, 5);
        assert_eq!(empty.height, 5);
        for &v in &empty.data {
            assert_eq!(v, 0.0);
        }
    }
}
