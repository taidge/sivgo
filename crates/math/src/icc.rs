//! Basic ICC profile parsing and metadata extraction.
//!
//! This module provides a lightweight [`IccProfile`] struct that can parse
//! the 128-byte ICC profile header and extract useful metadata such as
//! profile description, color space, white point, version, device class,
//! and rendering intent.  It does **not** implement full color management
//! (no LUT evaluation, no lcms2-style transforms).
//!
//! Reference: ICC.1:2004-10 (ICC Profile Format Specification).

/// An ICC profile loaded from raw bytes.
///
/// Stores the raw profile data and caches the parsed header fields.
#[derive(Debug, Clone, PartialEq)]
pub struct IccProfile {
    /// Raw ICC profile data.
    data: Vec<u8>,
    /// Parsed header information.
    pub header: IccHeader,
}

/// Parsed fields from the 128-byte ICC profile header.
#[derive(Debug, Clone, PartialEq)]
pub struct IccHeader {
    /// Profile size in bytes (from header offset 0..4).
    pub profile_size: u32,
    /// Preferred CMM type signature (offset 4..8).
    pub cmm_type: [u8; 4],
    /// Profile version (major, minor, bugfix).
    pub version: (u8, u8, u8),
    /// Device class / profile class signature (offset 12..16).
    pub device_class: ProfileClass,
    /// Color space of the profile's input data (offset 16..20).
    pub color_space: IccColorSpace,
    /// Profile connection space (offset 20..24).
    pub pcs: IccColorSpace,
    /// Rendering intent (offset 64..68, only low 2 bytes matter).
    pub rendering_intent: u32,
    /// PCS illuminant (X, Y, Z) as s15Fixed16Number values divided by 65536.
    /// For a D50-based profile this should be approximately (0.9505, 1.0, 1.0890).
    pub pcs_illuminant: (f64, f64, f64),
}

/// ICC profile / device class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileClass {
    Input,
    Display,
    Output,
    Link,
    Abstract,
    ColorSpace,
    NamedColor,
    Unknown([u8; 4]),
}

/// Color space signatures used in ICC profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IccColorSpace {
    Xyz,
    Lab,
    Luv,
    YCbCr,
    Yxy,
    Rgb,
    Gray,
    Hsv,
    Hls,
    Cmyk,
    Cmy,
    /// A multi-channel space (2-15 channels).
    MultiChannel(u8),
    Unknown([u8; 4]),
}

/// Errors that can occur when parsing an ICC profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IccError {
    /// Data is too short to contain a valid 128-byte header.
    TooShort,
    /// The profile signature at bytes 36..40 is not `'acsp'`.
    BadSignature,
    /// The declared profile size exceeds the provided data length.
    SizeMismatch,
}

impl std::fmt::Display for IccError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IccError::TooShort => write!(f, "ICC data too short (need at least 128 bytes)"),
            IccError::BadSignature => write!(f, "ICC signature mismatch (expected 'acsp')"),
            IccError::SizeMismatch => write!(f, "ICC profile size exceeds data length"),
        }
    }
}

impl std::error::Error for IccError {}

impl IccProfile {
    /// Parse an ICC profile from raw bytes.
    ///
    /// Validates the 128-byte header and extracts metadata fields.
    pub fn from_bytes(data: &[u8]) -> Result<Self, IccError> {
        if data.len() < 128 {
            return Err(IccError::TooShort);
        }

        // Check the 'acsp' signature at offset 36..40.
        if &data[36..40] != b"acsp" {
            return Err(IccError::BadSignature);
        }

        let profile_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if (profile_size as usize) > data.len() {
            return Err(IccError::SizeMismatch);
        }

        let cmm_type = [data[4], data[5], data[6], data[7]];

        // Version: major at byte 8, minor/bugfix packed in byte 9.
        let version_major = data[8];
        let version_minor = (data[9] >> 4) & 0x0F;
        let version_bugfix = data[9] & 0x0F;

        let device_class = parse_profile_class(&data[12..16]);
        let color_space = parse_color_space(&data[16..20]);
        let pcs = parse_color_space(&data[20..24]);

        let rendering_intent = u32::from_be_bytes([data[64], data[65], data[66], data[67]]);

        // PCS illuminant at offset 68..80 (three s15Fixed16Number values).
        let ill_x = s15fixed16(&data[68..72]);
        let ill_y = s15fixed16(&data[72..76]);
        let ill_z = s15fixed16(&data[76..80]);

        let header = IccHeader {
            profile_size,
            cmm_type,
            version: (version_major, version_minor, version_bugfix),
            device_class,
            color_space,
            pcs,
            rendering_intent,
            pcs_illuminant: (ill_x, ill_y, ill_z),
        };

        Ok(Self {
            data: data.to_vec(),
            header,
        })
    }

    /// Return the raw profile data bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Return the profile size in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Return the color space signature string (e.g. "RGB", "CMYK").
    pub fn color_space(&self) -> &str {
        match self.header.color_space {
            IccColorSpace::Xyz => "XYZ",
            IccColorSpace::Lab => "Lab",
            IccColorSpace::Luv => "Luv",
            IccColorSpace::YCbCr => "YCbCr",
            IccColorSpace::Yxy => "Yxy",
            IccColorSpace::Rgb => "RGB",
            IccColorSpace::Gray => "GRAY",
            IccColorSpace::Hsv => "HSV",
            IccColorSpace::Hls => "HLS",
            IccColorSpace::Cmyk => "CMYK",
            IccColorSpace::Cmy => "CMY",
            IccColorSpace::MultiChannel(_) => "MultiChannel",
            IccColorSpace::Unknown(_) => "Unknown",
        }
    }

    /// The white point from the PCS illuminant header field (X, Y, Z).
    pub fn white_point(&self) -> (f64, f64, f64) {
        self.header.pcs_illuminant
    }

    /// Attempt to extract the profile description string.
    ///
    /// Scans the tag table for a `desc` tag and reads the ASCII portion.
    /// Returns `None` if the tag is absent or cannot be parsed.
    pub fn description(&self) -> Option<String> {
        let tag_count = self.tag_count()?;
        for i in 0..tag_count {
            let base = 132 + i * 12;
            if base + 12 > self.data.len() {
                return None;
            }
            let sig = &self.data[base..base + 4];
            if sig == b"desc" {
                let offset = u32::from_be_bytes([
                    self.data[base + 4],
                    self.data[base + 5],
                    self.data[base + 6],
                    self.data[base + 7],
                ]) as usize;
                let size = u32::from_be_bytes([
                    self.data[base + 8],
                    self.data[base + 9],
                    self.data[base + 10],
                    self.data[base + 11],
                ]) as usize;
                return self.read_desc_tag(offset, size);
            }
        }
        None
    }

    /// The number of tags in the tag table.
    fn tag_count(&self) -> Option<usize> {
        if self.data.len() < 132 {
            return None;
        }
        Some(u32::from_be_bytes([
            self.data[128],
            self.data[129],
            self.data[130],
            self.data[131],
        ]) as usize)
    }

    /// Try to read a `desc` (textDescriptionType) or `mluc` tag.
    fn read_desc_tag(&self, offset: usize, size: usize) -> Option<String> {
        if offset + size > self.data.len() || size < 12 {
            return None;
        }
        let tag_type = &self.data[offset..offset + 4];
        if tag_type == b"desc" {
            // textDescriptionType: offset+8 = ASCII count, then ASCII string.
            let ascii_len = u32::from_be_bytes([
                self.data[offset + 8],
                self.data[offset + 9],
                self.data[offset + 10],
                self.data[offset + 11],
            ]) as usize;
            let str_start = offset + 12;
            let str_end = (str_start + ascii_len).min(offset + size);
            if str_end > self.data.len() {
                return None;
            }
            let bytes = &self.data[str_start..str_end];
            // Trim trailing null bytes.
            let trimmed: Vec<u8> = bytes.iter().copied().take_while(|&b| b != 0).collect();
            String::from_utf8(trimmed).ok()
        } else if tag_type == b"mluc" {
            // multiLocalizedUnicodeType: simplified extraction of first record.
            if size < 16 {
                return None;
            }
            let record_count = u32::from_be_bytes([
                self.data[offset + 8],
                self.data[offset + 9],
                self.data[offset + 10],
                self.data[offset + 11],
            ]);
            if record_count == 0 || size < 28 {
                return None;
            }
            // First record at offset+16: language(2) + country(2) + length(4) + offset(4)
            let str_len = u32::from_be_bytes([
                self.data[offset + 20],
                self.data[offset + 21],
                self.data[offset + 22],
                self.data[offset + 23],
            ]) as usize;
            let str_off = u32::from_be_bytes([
                self.data[offset + 24],
                self.data[offset + 25],
                self.data[offset + 26],
                self.data[offset + 27],
            ]) as usize;
            let abs_start = offset + str_off;
            let abs_end = abs_start + str_len;
            if abs_end > self.data.len() {
                return None;
            }
            // It's UTF-16BE.
            let u16_vals: Vec<u16> = self.data[abs_start..abs_end]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|chunk| u16::from_be_bytes(*chunk))
                .collect();
            String::from_utf16(&u16_vals).ok()
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn s15fixed16(bytes: &[u8]) -> f64 {
    let raw = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    raw as f64 / 65536.0
}

fn parse_profile_class(sig: &[u8]) -> ProfileClass {
    match sig {
        b"scnr" => ProfileClass::Input,
        b"mntr" => ProfileClass::Display,
        b"prtr" => ProfileClass::Output,
        b"link" => ProfileClass::Link,
        b"abst" => ProfileClass::Abstract,
        b"spac" => ProfileClass::ColorSpace,
        b"nmcl" => ProfileClass::NamedColor,
        _ => ProfileClass::Unknown([sig[0], sig[1], sig[2], sig[3]]),
    }
}

fn parse_color_space(sig: &[u8]) -> IccColorSpace {
    match sig {
        b"XYZ " => IccColorSpace::Xyz,
        b"Lab " => IccColorSpace::Lab,
        b"Luv " => IccColorSpace::Luv,
        b"YCbr" => IccColorSpace::YCbCr,
        b"Yxy " => IccColorSpace::Yxy,
        b"RGB " => IccColorSpace::Rgb,
        b"GRAY" => IccColorSpace::Gray,
        b"HSV " => IccColorSpace::Hsv,
        b"HLS " => IccColorSpace::Hls,
        b"CMYK" => IccColorSpace::Cmyk,
        b"CMY " => IccColorSpace::Cmy,
        _ => {
            // Check for multi-channel signatures like "2CLR" .. "FCLR"
            if sig.len() == 4 && &sig[1..4] == b"CLR" {
                let ch = match sig[0] {
                    b'2'..=b'9' => sig[0] - b'0',
                    b'A'..=b'F' => sig[0] - b'A' + 10,
                    _ => 0,
                };
                if ch >= 2 {
                    return IccColorSpace::MultiChannel(ch);
                }
            }
            IccColorSpace::Unknown([sig[0], sig[1], sig[2], sig[3]])
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid ICC profile header (128 bytes) with a tag table.
    fn make_minimal_profile(color_space: &[u8; 4], device_class: &[u8; 4]) -> Vec<u8> {
        let mut buf = vec![0u8; 128 + 4]; // header + tag count (0)

        // Profile size.
        let size = buf.len() as u32;
        buf[0..4].copy_from_slice(&size.to_be_bytes());

        // Profile version 2.4.0
        buf[8] = 2;
        buf[9] = 0x40;

        // Device class
        buf[12..16].copy_from_slice(device_class);

        // Color space
        buf[16..20].copy_from_slice(color_space);

        // PCS (XYZ)
        buf[20..24].copy_from_slice(b"XYZ ");

        // 'acsp' signature at offset 36
        buf[36..40].copy_from_slice(b"acsp");

        // PCS illuminant = D50: X=0.9505, Y=1.0000, Z=1.0890
        let ill_x = (0.9505 * 65536.0) as i32;
        let ill_y = (1.0000 * 65536.0) as i32;
        let ill_z = (1.0890 * 65536.0) as i32;
        buf[68..72].copy_from_slice(&ill_x.to_be_bytes());
        buf[72..76].copy_from_slice(&ill_y.to_be_bytes());
        buf[76..80].copy_from_slice(&ill_z.to_be_bytes());

        // Tag count = 0 at offset 128
        buf[128..132].copy_from_slice(&0u32.to_be_bytes());

        buf
    }

    #[test]
    fn parse_minimal_rgb_profile() {
        let data = make_minimal_profile(b"RGB ", b"mntr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        assert_eq!(profile.header.color_space, IccColorSpace::Rgb);
        assert_eq!(profile.header.device_class, ProfileClass::Display);
        assert_eq!(profile.header.version, (2, 4, 0));
        assert_eq!(profile.color_space(), "RGB");

        let (x, y, z) = profile.white_point();
        assert!((x - 0.9505).abs() < 0.001);
        assert!((y - 1.0).abs() < 0.001);
        assert!((z - 1.0890).abs() < 0.001);
    }

    #[test]
    fn parse_cmyk_profile() {
        let data = make_minimal_profile(b"CMYK", b"prtr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        assert_eq!(profile.header.color_space, IccColorSpace::Cmyk);
        assert_eq!(profile.header.device_class, ProfileClass::Output);
        assert_eq!(profile.color_space(), "CMYK");
    }

    #[test]
    fn too_short_data() {
        assert_eq!(IccProfile::from_bytes(&[0; 64]), Err(IccError::TooShort));
    }

    #[test]
    fn bad_signature() {
        let mut data = vec![0u8; 132];
        let size = 132u32;
        data[0..4].copy_from_slice(&size.to_be_bytes());
        // Don't set 'acsp' -> should fail
        assert_eq!(IccProfile::from_bytes(&data), Err(IccError::BadSignature));
    }

    #[test]
    fn size_mismatch() {
        let mut data = make_minimal_profile(b"RGB ", b"mntr");
        let fake_size = (data.len() + 1000) as u32;
        data[0..4].copy_from_slice(&fake_size.to_be_bytes());
        assert_eq!(IccProfile::from_bytes(&data), Err(IccError::SizeMismatch));
    }

    #[test]
    fn description_with_desc_tag() {
        let desc_text = b"Test Profile";
        let mut buf = make_minimal_profile(b"RGB ", b"mntr");

        // Tag count = 1
        buf[128..132].copy_from_slice(&1u32.to_be_bytes());

        // Tag table entry at offset 132: sig(4) + offset(4) + size(4)
        let tag_data_offset = 132 + 12; // right after the tag table
        let ascii_count = desc_text.len() as u32 + 1; // include null terminator
        let tag_data_size = 12 + ascii_count as usize;

        buf.resize(tag_data_offset + tag_data_size, 0);
        buf[132..136].copy_from_slice(b"desc");
        buf[136..140].copy_from_slice(&(tag_data_offset as u32).to_be_bytes());
        buf[140..144].copy_from_slice(&(tag_data_size as u32).to_be_bytes());

        // Tag data: type='desc', reserved=0, ascii_count, then ascii
        buf[tag_data_offset..tag_data_offset + 4].copy_from_slice(b"desc");
        buf[tag_data_offset + 8..tag_data_offset + 12].copy_from_slice(&ascii_count.to_be_bytes());
        buf[tag_data_offset + 12..tag_data_offset + 12 + desc_text.len()]
            .copy_from_slice(desc_text);

        // Update profile size
        let total = buf.len() as u32;
        buf[0..4].copy_from_slice(&total.to_be_bytes());

        let profile = IccProfile::from_bytes(&buf).unwrap();
        assert_eq!(profile.description(), Some("Test Profile".to_string()));
    }

    #[test]
    fn description_missing() {
        let data = make_minimal_profile(b"RGB ", b"mntr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        assert_eq!(profile.description(), None);
    }

    #[test]
    fn raw_data_preserved() {
        let data = make_minimal_profile(b"RGB ", b"mntr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        assert_eq!(profile.data(), &data[..]);
        assert_eq!(profile.size(), data.len());
    }

    #[test]
    fn profile_classes() {
        assert_eq!(parse_profile_class(b"scnr"), ProfileClass::Input);
        assert_eq!(parse_profile_class(b"mntr"), ProfileClass::Display);
        assert_eq!(parse_profile_class(b"prtr"), ProfileClass::Output);
        assert_eq!(parse_profile_class(b"link"), ProfileClass::Link);
        assert_eq!(parse_profile_class(b"abst"), ProfileClass::Abstract);
        assert_eq!(parse_profile_class(b"spac"), ProfileClass::ColorSpace);
        assert_eq!(parse_profile_class(b"nmcl"), ProfileClass::NamedColor);
        assert_eq!(
            parse_profile_class(b"????"),
            ProfileClass::Unknown(*b"????")
        );
    }

    #[test]
    fn color_spaces() {
        assert_eq!(parse_color_space(b"XYZ "), IccColorSpace::Xyz);
        assert_eq!(parse_color_space(b"Lab "), IccColorSpace::Lab);
        assert_eq!(parse_color_space(b"RGB "), IccColorSpace::Rgb);
        assert_eq!(parse_color_space(b"GRAY"), IccColorSpace::Gray);
        assert_eq!(parse_color_space(b"CMYK"), IccColorSpace::Cmyk);
        assert_eq!(parse_color_space(b"CMY "), IccColorSpace::Cmy);
        assert_eq!(parse_color_space(b"HSV "), IccColorSpace::Hsv);
        assert_eq!(parse_color_space(b"HLS "), IccColorSpace::Hls);
        assert_eq!(parse_color_space(b"Luv "), IccColorSpace::Luv);
        assert_eq!(parse_color_space(b"YCbr"), IccColorSpace::YCbCr);
        assert_eq!(parse_color_space(b"Yxy "), IccColorSpace::Yxy);
        assert_eq!(parse_color_space(b"3CLR"), IccColorSpace::MultiChannel(3));
        assert_eq!(parse_color_space(b"FCLR"), IccColorSpace::MultiChannel(15));
        assert_eq!(parse_color_space(b"ZZZZ"), IccColorSpace::Unknown(*b"ZZZZ"));
    }

    #[test]
    fn white_point_d50() {
        let data = make_minimal_profile(b"RGB ", b"mntr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        let (x, y, z) = profile.white_point();
        // D50 standard values
        assert!((x - 0.9505).abs() < 0.001);
        assert!((y - 1.0).abs() < 0.001);
        assert!((z - 1.089).abs() < 0.001);
    }

    #[test]
    fn rendering_intent_default() {
        let data = make_minimal_profile(b"RGB ", b"mntr");
        let profile = IccProfile::from_bytes(&data).unwrap();
        // Default is 0 (Perceptual)
        assert_eq!(profile.header.rendering_intent, 0);
    }
}
