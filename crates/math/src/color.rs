//! Color management system for Sivgo.
//!
//! Provides a floating-point color type that can represent colors in multiple
//! color spaces, with conversions between them and interpolation support for
//! gradient rendering.
//!
//! # Supported color spaces
//!
//! | Space        | Components `[c0, c1, c2, alpha]`          |
//! |--------------|-------------------------------------------|
//! | **sRGB**     | red, green, blue (0..1 gamma-encoded)     |
//! | **LinearRGB**| red, green, blue (0..1 linear-light)      |
//! | **HSV**      | hue (0..360), saturation (0..1), value    |
//! | **HSL**      | hue (0..360), saturation (0..1), lightness|
//! | **Lab**      | L* (0..100), a*, b*                       |
//! | **LCH**      | L* (0..100), C* (chroma), h (0..360)      |
//! | **OkLab**    | L (0..1), a, b                            |
//! | **OkLCH**    | L (0..1), C (chroma), h (0..360)          |
//! | **CMYK**     | C (0..1), M (0..1), Y (0..1), K (0..1)    |
//!
//! Note: CMYK uses all four `components` slots for C, M, Y, K. Alpha is
//! stored separately when CMYK is active (always treated as 1.0).

// ---------------------------------------------------------------------------
// ColorSpace enum
// ---------------------------------------------------------------------------

/// Identifies the color space that a [`ColorF64`] value lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorSpace {
    /// sRGB with gamma-encoded components in 0..1.
    Srgb,
    /// Linear-light RGB (gamma removed).
    LinearRgb,
    /// Hue / Saturation / Value.
    Hsv,
    /// Hue / Saturation / Lightness.
    Hsl,
    /// CIE L*a*b* (D65 illuminant).
    Lab,
    /// CIE L*C*h (polar form of Lab).
    Lch,
    /// Bjorn Ottosson's perceptually uniform Lab.
    OkLab,
    /// Polar form of OkLab.
    OkLch,
    /// CMYK (Cyan, Magenta, Yellow, Key/Black).
    ///
    /// Components: `[C, M, Y, K]` all in 0..1.  The alpha channel is
    /// implicit (always 1.0) when in this space because all four component
    /// slots are used for CMYK channels.
    Cmyk,
}

// ---------------------------------------------------------------------------
// ColorF64
// ---------------------------------------------------------------------------

/// A color stored as four `f64` components whose meaning depends on the
/// associated [`ColorSpace`].  The fourth component is always alpha (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorF64 {
    /// Three color-space-dependent channels plus alpha.
    pub components: [f64; 4],
    /// Which color space the channels are expressed in.
    pub space: ColorSpace,
}

impl ColorF64 {
    /// Create a new color in the given space.
    #[inline]
    pub fn new(c0: f64, c1: f64, c2: f64, alpha: f64, space: ColorSpace) -> Self {
        Self {
            components: [c0, c1, c2, alpha],
            space,
        }
    }

    /// Shorthand for an sRGB color with components in 0..1.
    #[inline]
    pub fn srgb(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self::new(r, g, b, a, ColorSpace::Srgb)
    }

    /// Shorthand for an sRGB color from 0..255 integer components.
    #[inline]
    pub fn srgb_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::srgb(
            r as f64 / 255.0,
            g as f64 / 255.0,
            b as f64 / 255.0,
            a as f64 / 255.0,
        )
    }

    // -- accessors ----------------------------------------------------------

    #[inline]
    pub fn c0(&self) -> f64 {
        self.components[0]
    }
    #[inline]
    pub fn c1(&self) -> f64 {
        self.components[1]
    }
    #[inline]
    pub fn c2(&self) -> f64 {
        self.components[2]
    }
    #[inline]
    pub fn alpha(&self) -> f64 {
        self.components[3]
    }

    // -----------------------------------------------------------------------
    // Conversion to each target space
    // -----------------------------------------------------------------------

    /// Create a CMYK color.  All components are in 0..1.
    #[inline]
    pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Self {
        Self::new(c, m, y, k, ColorSpace::Cmyk)
    }

    /// Convert to sRGB.
    pub fn to_srgb(&self) -> Self {
        match self.space {
            ColorSpace::Srgb => *self,
            ColorSpace::LinearRgb => linear_to_srgb(self),
            ColorSpace::Hsv => hsv_to_srgb(self),
            ColorSpace::Hsl => hsl_to_srgb(self),
            ColorSpace::Lab => lab_to_srgb(self),
            ColorSpace::Lch => self.to_lab().to_srgb(),
            ColorSpace::OkLab => oklab_to_srgb(self),
            ColorSpace::OkLch => self.to_oklab().to_srgb(),
            ColorSpace::Cmyk => cmyk_to_srgb(self),
        }
    }

    /// Convert to linear RGB.
    pub fn to_linear_rgb(&self) -> Self {
        match self.space {
            ColorSpace::LinearRgb => *self,
            ColorSpace::Srgb => srgb_to_linear(self),
            _ => self.to_srgb().to_linear_rgb(),
        }
    }

    /// Convert to HSV.
    pub fn to_hsv(&self) -> Self {
        match self.space {
            ColorSpace::Hsv => *self,
            _ => {
                let srgb = self.to_srgb();
                srgb_to_hsv(&srgb)
            }
        }
    }

    /// Convert to HSL.
    pub fn to_hsl(&self) -> Self {
        match self.space {
            ColorSpace::Hsl => *self,
            _ => {
                let srgb = self.to_srgb();
                srgb_to_hsl(&srgb)
            }
        }
    }

    /// Convert to CIE L*a*b* (D65).
    pub fn to_lab(&self) -> Self {
        match self.space {
            ColorSpace::Lab => *self,
            ColorSpace::Lch => lch_to_lab(self),
            _ => {
                let linear = self.to_linear_rgb();
                linear_to_lab(&linear)
            }
        }
    }

    /// Convert to CIE L*C*h.
    pub fn to_lch(&self) -> Self {
        match self.space {
            ColorSpace::Lch => *self,
            _ => {
                let lab = self.to_lab();
                lab_to_lch(&lab)
            }
        }
    }

    /// Convert to OkLab.
    pub fn to_oklab(&self) -> Self {
        match self.space {
            ColorSpace::OkLab => *self,
            ColorSpace::OkLch => oklch_to_oklab(self),
            _ => {
                let linear = self.to_linear_rgb();
                linear_to_oklab(&linear)
            }
        }
    }

    /// Convert to OkLCH.
    pub fn to_oklch(&self) -> Self {
        match self.space {
            ColorSpace::OkLch => *self,
            _ => {
                let oklab = self.to_oklab();
                oklab_to_oklch(&oklab)
            }
        }
    }

    /// Convert to CMYK.
    ///
    /// Uses a simple formula without ICC profiles:
    /// K = min(1-R, 1-G, 1-B), then C=(1-R-K)/(1-K), etc.
    /// The alpha channel is stored in the K slot indirectly -- when in CMYK
    /// space the color is always fully opaque.
    pub fn to_cmyk(&self) -> Self {
        match self.space {
            ColorSpace::Cmyk => *self,
            _ => {
                let srgb = self.to_srgb();
                srgb_to_cmyk(&srgb)
            }
        }
    }

    /// Convert to an arbitrary target space.
    pub fn to_space(&self, target: ColorSpace) -> Self {
        match target {
            ColorSpace::Srgb => self.to_srgb(),
            ColorSpace::LinearRgb => self.to_linear_rgb(),
            ColorSpace::Hsv => self.to_hsv(),
            ColorSpace::Hsl => self.to_hsl(),
            ColorSpace::Lab => self.to_lab(),
            ColorSpace::Lch => self.to_lch(),
            ColorSpace::OkLab => self.to_oklab(),
            ColorSpace::OkLch => self.to_oklch(),
            ColorSpace::Cmyk => self.to_cmyk(),
        }
    }

    // -----------------------------------------------------------------------
    // Interpolation
    // -----------------------------------------------------------------------

    /// Interpolate between `self` and `other` at parameter `t` (0..1) in the
    /// given color space.  The result is returned in the same space.
    ///
    /// For polar spaces (HSV, HSL, LCH, OkLCH) the hue channel is
    /// interpolated along the shorter arc.
    ///
    /// For CMYK, all four component slots (C, M, Y, K) are interpolated
    /// linearly and the result alpha is 1.0.
    pub fn interpolate(&self, other: &Self, t: f64, space: ColorSpace) -> Self {
        let a = self.to_space(space);
        let b = other.to_space(space);

        // CMYK uses all four component slots for colour channels.
        if space == ColorSpace::Cmyk {
            return Self::new(
                lerp(a.c0(), b.c0(), t),
                lerp(a.c1(), b.c1(), t),
                lerp(a.c2(), b.c2(), t),
                lerp(a.components[3], b.components[3], t),
                ColorSpace::Cmyk,
            );
        }

        let is_polar = matches!(
            space,
            ColorSpace::Hsv | ColorSpace::Hsl | ColorSpace::Lch | ColorSpace::OkLch
        );

        let c0 = if is_polar {
            lerp_hue(a.c0(), b.c0(), t)
        } else {
            lerp(a.c0(), b.c0(), t)
        };
        // For LCH / OkLCH the hue is in component 2, not 0.
        // HSV/HSL have hue in component 0.
        let (c0, c1, c2) = if matches!(space, ColorSpace::Lch | ColorSpace::OkLch) {
            // L, C, H -- hue is c2
            (
                lerp(a.c0(), b.c0(), t),
                lerp(a.c1(), b.c1(), t),
                lerp_hue(a.c2(), b.c2(), t),
            )
        } else if is_polar {
            // H, S, V/L -- hue is c0
            (c0, lerp(a.c1(), b.c1(), t), lerp(a.c2(), b.c2(), t))
        } else {
            (c0, lerp(a.c1(), b.c1(), t), lerp(a.c2(), b.c2(), t))
        };

        let alpha = lerp(a.alpha(), b.alpha(), t);

        Self::new(c0, c1, c2, alpha, space)
    }

    // -----------------------------------------------------------------------
    // Gamut mapping / clipping  (16.9)
    // -----------------------------------------------------------------------

    /// Clamp out-of-gamut component values back to the valid range for the
    /// current color space.
    ///
    /// For most RGB-based spaces, this clamps each channel to 0..1.
    /// For HSV/HSL, hue wraps to 0..360 and others clamp to 0..1.
    /// For Lab/LCH, L* clamps to 0..100 and chroma/hue are adjusted.
    /// For OkLab/OkLCH, L clamps to 0..1.
    /// For CMYK, all channels clamp to 0..1.
    /// Alpha always clamps to 0..1.
    pub fn gamut_clip(&self) -> Self {
        let [c0, c1, c2, c3] = self.components;
        match self.space {
            ColorSpace::Srgb | ColorSpace::LinearRgb => Self::new(
                c0.clamp(0.0, 1.0),
                c1.clamp(0.0, 1.0),
                c2.clamp(0.0, 1.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::Hsv => Self::new(
                c0.rem_euclid(360.0),
                c1.clamp(0.0, 1.0),
                c2.clamp(0.0, 1.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::Hsl => Self::new(
                c0.rem_euclid(360.0),
                c1.clamp(0.0, 1.0),
                c2.clamp(0.0, 1.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::Lab => Self::new(
                c0.clamp(0.0, 100.0),
                c1, // a* is unbounded in theory but we leave it
                c2, // b* is unbounded
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::Lch => Self::new(
                c0.clamp(0.0, 100.0),
                c1.max(0.0), // chroma >= 0
                c2.rem_euclid(360.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::OkLab => Self::new(
                c0.clamp(0.0, 1.0),
                c1, // a is unbounded
                c2, // b is unbounded
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::OkLch => Self::new(
                c0.clamp(0.0, 1.0),
                c1.max(0.0), // chroma >= 0
                c2.rem_euclid(360.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
            ColorSpace::Cmyk => Self::new(
                c0.clamp(0.0, 1.0),
                c1.clamp(0.0, 1.0),
                c2.clamp(0.0, 1.0),
                c3.clamp(0.0, 1.0),
                self.space,
            ),
        }
    }
}

// ===========================================================================
// Private helpers
// ===========================================================================

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Interpolate two hue angles (in degrees) along the shorter arc.
fn lerp_hue(a: f64, b: f64, t: f64) -> f64 {
    let mut diff = b - a;
    if diff > 180.0 {
        diff -= 360.0;
    } else if diff < -180.0 {
        diff += 360.0;
    }
    let h = a + diff * t;
    h.rem_euclid(360.0)
}

// ---------------------------------------------------------------------------
// sRGB <-> Linear RGB  (precise sRGB transfer function, IEC 61966-2-1)
// ---------------------------------------------------------------------------

fn srgb_eotf(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_oetf(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear(c: &ColorF64) -> ColorF64 {
    ColorF64::new(
        srgb_eotf(c.c0()),
        srgb_eotf(c.c1()),
        srgb_eotf(c.c2()),
        c.alpha(),
        ColorSpace::LinearRgb,
    )
}

fn linear_to_srgb(c: &ColorF64) -> ColorF64 {
    ColorF64::new(
        srgb_oetf(c.c0()),
        srgb_oetf(c.c1()),
        srgb_oetf(c.c2()),
        c.alpha(),
        ColorSpace::Srgb,
    )
}

// ---------------------------------------------------------------------------
// sRGB <-> HSV
// ---------------------------------------------------------------------------

fn srgb_to_hsv(c: &ColorF64) -> ColorF64 {
    let r = c.c0();
    let g = c.c1();
    let b = c.c2();

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let h = h.rem_euclid(360.0);

    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;

    ColorF64::new(h, s, v, c.alpha(), ColorSpace::Hsv)
}

fn hsv_to_srgb(c: &ColorF64) -> ColorF64 {
    let h = c.c0().rem_euclid(360.0);
    let s = c.c1();
    let v = c.c2();

    let ch = v * s;
    let x = ch * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - ch;

    let (r1, g1, b1) = match (h / 60.0) as u32 {
        0 => (ch, x, 0.0),
        1 => (x, ch, 0.0),
        2 => (0.0, ch, x),
        3 => (0.0, x, ch),
        4 => (x, 0.0, ch),
        _ => (ch, 0.0, x),
    };

    ColorF64::new(r1 + m, g1 + m, b1 + m, c.alpha(), ColorSpace::Srgb)
}

// ---------------------------------------------------------------------------
// sRGB <-> HSL
// ---------------------------------------------------------------------------

fn srgb_to_hsl(c: &ColorF64) -> ColorF64 {
    let r = c.c0();
    let g = c.c1();
    let b = c.c2();

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = (max + min) / 2.0;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let h = h.rem_euclid(360.0);

    let s = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs())
    };

    ColorF64::new(h, s, l, c.alpha(), ColorSpace::Hsl)
}

fn hsl_to_srgb(c: &ColorF64) -> ColorF64 {
    let h = c.c0().rem_euclid(360.0);
    let s = c.c1();
    let l = c.c2();

    let ch = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = ch * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - ch / 2.0;

    let (r1, g1, b1) = match (h / 60.0) as u32 {
        0 => (ch, x, 0.0),
        1 => (x, ch, 0.0),
        2 => (0.0, ch, x),
        3 => (0.0, x, ch),
        4 => (x, 0.0, ch),
        _ => (ch, 0.0, x),
    };

    ColorF64::new(r1 + m, g1 + m, b1 + m, c.alpha(), ColorSpace::Srgb)
}

// ---------------------------------------------------------------------------
// Linear RGB <-> CIE XYZ (D65)  <-> CIE L*a*b*
// ---------------------------------------------------------------------------

// D65 reference white (CIE 1931 2-degree observer)
const D65_X: f64 = 0.95047;
const D65_Y: f64 = 1.00000;
const D65_Z: f64 = 1.08883;

/// Linear sRGB -> XYZ (D65).  Uses the standard IEC 61966-2-1 matrix.
fn linear_rgb_to_xyz(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
    (x, y, z)
}

/// XYZ (D65) -> Linear sRGB.
fn xyz_to_linear_rgb(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;
    (r, g, b)
}

fn lab_f(t: f64) -> f64 {
    const DELTA: f64 = 6.0 / 29.0;
    if t > DELTA * DELTA * DELTA {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

fn lab_f_inv(t: f64) -> f64 {
    const DELTA: f64 = 6.0 / 29.0;
    if t > DELTA {
        t * t * t
    } else {
        3.0 * DELTA * DELTA * (t - 4.0 / 29.0)
    }
}

fn linear_to_lab(c: &ColorF64) -> ColorF64 {
    let (x, y, z) = linear_rgb_to_xyz(c.c0(), c.c1(), c.c2());
    let fx = lab_f(x / D65_X);
    let fy = lab_f(y / D65_Y);
    let fz = lab_f(z / D65_Z);

    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);

    ColorF64::new(l, a, b, c.alpha(), ColorSpace::Lab)
}

fn lab_to_srgb(c: &ColorF64) -> ColorF64 {
    let l = c.c0();
    let a = c.c1();
    let b = c.c2();

    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;

    let x = D65_X * lab_f_inv(fx);
    let y = D65_Y * lab_f_inv(fy);
    let z = D65_Z * lab_f_inv(fz);

    let (r, g, b_lin) = xyz_to_linear_rgb(x, y, z);

    ColorF64::new(
        srgb_oetf(r),
        srgb_oetf(g),
        srgb_oetf(b_lin),
        c.alpha(),
        ColorSpace::Srgb,
    )
}

// ---------------------------------------------------------------------------
// Lab <-> LCH
// ---------------------------------------------------------------------------

fn lab_to_lch(c: &ColorF64) -> ColorF64 {
    let l = c.c0();
    let a = c.c1();
    let b = c.c2();

    let ch = (a * a + b * b).sqrt();
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);

    ColorF64::new(l, ch, h, c.alpha(), ColorSpace::Lch)
}

fn lch_to_lab(c: &ColorF64) -> ColorF64 {
    let l = c.c0();
    let ch = c.c1();
    let h = c.c2().to_radians();

    let a = ch * h.cos();
    let b = ch * h.sin();

    ColorF64::new(l, a, b, c.alpha(), ColorSpace::Lab)
}

// ---------------------------------------------------------------------------
// Linear RGB <-> OkLab  (Bjorn Ottosson, 2020)
// ---------------------------------------------------------------------------

fn linear_to_oklab(c: &ColorF64) -> ColorF64 {
    let r = c.c0();
    let g = c.c1();
    let b = c.c2();

    // Linear sRGB -> LMS (using Ottosson's M1 matrix)
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    // Non-linear LMS
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    // LMS' -> OkLab
    let ok_l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let ok_a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let ok_b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    ColorF64::new(ok_l, ok_a, ok_b, c.alpha(), ColorSpace::OkLab)
}

fn oklab_to_srgb(c: &ColorF64) -> ColorF64 {
    let ok_l = c.c0();
    let ok_a = c.c1();
    let ok_b = c.c2();

    // OkLab -> LMS'
    let l_ = ok_l + 0.3963377774 * ok_a + 0.2158037573 * ok_b;
    let m_ = ok_l - 0.1055613458 * ok_a - 0.0638541728 * ok_b;
    let s_ = ok_l - 0.0894841775 * ok_a - 1.2914855480 * ok_b;

    // Cube to undo the cbrt
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    // LMS -> linear sRGB (inverse of M1)
    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    ColorF64::new(
        srgb_oetf(r),
        srgb_oetf(g),
        srgb_oetf(b),
        c.alpha(),
        ColorSpace::Srgb,
    )
}

// ---------------------------------------------------------------------------
// OkLab <-> OkLCH
// ---------------------------------------------------------------------------

fn oklab_to_oklch(c: &ColorF64) -> ColorF64 {
    let l = c.c0();
    let a = c.c1();
    let b = c.c2();

    let ch = (a * a + b * b).sqrt();
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);

    ColorF64::new(l, ch, h, c.alpha(), ColorSpace::OkLch)
}

fn oklch_to_oklab(c: &ColorF64) -> ColorF64 {
    let l = c.c0();
    let ch = c.c1();
    let h = c.c2().to_radians();

    let a = ch * h.cos();
    let b = ch * h.sin();

    ColorF64::new(l, a, b, c.alpha(), ColorSpace::OkLab)
}

// ---------------------------------------------------------------------------
// sRGB <-> CMYK  (simple, no ICC profile)
// ---------------------------------------------------------------------------

/// Convert sRGB to CMYK using the standard non-ICC formula.
///
/// K = min(1-R, 1-G, 1-B)
/// C = (1-R-K) / (1-K)
/// M = (1-G-K) / (1-K)
/// Y = (1-B-K) / (1-K)
///
/// When K==1 (pure black), C=M=Y=0.
fn srgb_to_cmyk(c: &ColorF64) -> ColorF64 {
    let r = c.c0().clamp(0.0, 1.0);
    let g = c.c1().clamp(0.0, 1.0);
    let b = c.c2().clamp(0.0, 1.0);

    let k = 1.0 - r.max(g).max(b);
    if (1.0 - k).abs() < 1e-15 {
        // Pure black
        return ColorF64::new(0.0, 0.0, 0.0, 1.0, ColorSpace::Cmyk);
    }
    let c_val = (1.0 - r - k) / (1.0 - k);
    let m_val = (1.0 - g - k) / (1.0 - k);
    let y_val = (1.0 - b - k) / (1.0 - k);

    ColorF64::new(c_val, m_val, y_val, k, ColorSpace::Cmyk)
}

/// Convert CMYK to sRGB using the standard formula.
///
/// R = (1-C) * (1-K)
/// G = (1-M) * (1-K)
/// B = (1-Y) * (1-K)
fn cmyk_to_srgb(c: &ColorF64) -> ColorF64 {
    let cy = c.c0();
    let mg = c.c1();
    let ye = c.c2();
    let k = c.components[3]; // K is in the alpha slot for CMYK

    let r = (1.0 - cy) * (1.0 - k);
    let g = (1.0 - mg) * (1.0 - k);
    let b = (1.0 - ye) * (1.0 - k);

    ColorF64::new(r, g, b, 1.0, ColorSpace::Srgb)
}

// ---------------------------------------------------------------------------
// Soft proofing  (16.11)
// ---------------------------------------------------------------------------

/// Simulate how a color would appear when converted through a target color
/// space (simplified soft-proof: convert to `target` space, gamut-clip, then
/// convert back to sRGB).
pub fn soft_proof(color: &ColorF64, target: ColorSpace) -> ColorF64 {
    color.to_space(target).gamut_clip().to_srgb()
}

// ---------------------------------------------------------------------------
// Document color profile settings  (16.10)
// ---------------------------------------------------------------------------

/// Rendering intent as used in ICC profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderingIntent {
    /// Maintain relative colorimetric accuracy.
    Perceptual,
    /// Map in-gamut colors exactly; clip out-of-gamut.
    RelativeColorimetric,
    /// Preserve saturation at the expense of hue accuracy.
    Saturation,
    /// Map colors to the absolute white point of the destination.
    AbsoluteColorimetric,
}

/// Per-document color management settings.
#[derive(Debug, Clone)]
pub struct DocumentColorSettings {
    /// The working color space for this document.
    pub working_space: ColorSpace,
    /// The rendering intent to use for conversions.
    pub rendering_intent: RenderingIntent,
    /// An optional embedded ICC profile.
    pub embedded_profile: Option<crate::icc::IccProfile>,
}

impl Default for DocumentColorSettings {
    fn default() -> Self {
        Self {
            working_space: ColorSpace::Srgb,
            rendering_intent: RenderingIntent::Perceptual,
            embedded_profile: None,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Allowed absolute error for round-trip conversions.
    const EPS: f64 = 1e-10;
    /// Slightly larger tolerance for multi-step conversions (through XYZ/LMS).
    const EPS_LAB: f64 = 1e-5;

    fn assert_close(a: &ColorF64, b: &ColorF64, eps: f64, msg: &str) {
        for i in 0..4 {
            assert!(
                (a.components[i] - b.components[i]).abs() < eps,
                "{msg}: component {i} differs: {} vs {} (diff={})",
                a.components[i],
                b.components[i],
                (a.components[i] - b.components[i]).abs(),
            );
        }
    }

    // -- sRGB <-> Linear RGB round-trip -------------------------------------

    #[test]
    fn srgb_linear_roundtrip() {
        let colors = [
            ColorF64::srgb(0.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(1.0, 1.0, 1.0, 1.0),
            ColorF64::srgb(0.5, 0.2, 0.8, 0.75),
            ColorF64::srgb(0.04, 0.003, 0.9, 1.0),
        ];
        for c in &colors {
            let rt = c.to_linear_rgb().to_srgb();
            assert_close(c, &rt, EPS, "srgb->linear->srgb");
        }
    }

    #[test]
    fn srgb_eotf_known_values() {
        // 50% sRGB grey should be about 0.214 in linear
        let grey = ColorF64::srgb(0.5, 0.5, 0.5, 1.0).to_linear_rgb();
        assert!((grey.c0() - 0.214041).abs() < 1e-4);
    }

    // -- sRGB <-> HSV round-trip --------------------------------------------

    #[test]
    fn srgb_hsv_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 0.0, 1.0, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_hsv().to_srgb();
            assert_close(c, &rt, EPS, "srgb->hsv->srgb");
        }
    }

    #[test]
    fn hsv_known_red() {
        let red = ColorF64::srgb(1.0, 0.0, 0.0, 1.0).to_hsv();
        assert!((red.c0() - 0.0).abs() < EPS, "red hue should be 0");
        assert!((red.c1() - 1.0).abs() < EPS, "red sat should be 1");
        assert!((red.c2() - 1.0).abs() < EPS, "red val should be 1");
    }

    // -- sRGB <-> HSL round-trip --------------------------------------------

    #[test]
    fn srgb_hsl_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 0.0, 1.0, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_hsl().to_srgb();
            assert_close(c, &rt, EPS, "srgb->hsl->srgb");
        }
    }

    #[test]
    fn hsl_known_red() {
        let red = ColorF64::srgb(1.0, 0.0, 0.0, 1.0).to_hsl();
        assert!((red.c0() - 0.0).abs() < EPS, "red hue should be 0");
        assert!((red.c1() - 1.0).abs() < EPS, "red sat should be 1");
        assert!((red.c2() - 0.5).abs() < EPS, "red lightness should be 0.5");
    }

    // -- sRGB <-> Lab round-trip --------------------------------------------

    #[test]
    fn srgb_lab_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 0.0, 1.0, 1.0),
            ColorF64::srgb(0.5, 0.5, 0.5, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_lab().to_srgb();
            assert_close(c, &rt, EPS_LAB, "srgb->lab->srgb");
        }
    }

    #[test]
    fn lab_known_white() {
        let white = ColorF64::srgb(1.0, 1.0, 1.0, 1.0).to_lab();
        assert!(
            (white.c0() - 100.0).abs() < 1e-3,
            "white L* should be 100, got {}",
            white.c0()
        );
        assert!(white.c1().abs() < 1e-3, "white a* should be ~0");
        assert!(white.c2().abs() < 1e-3, "white b* should be ~0");
    }

    #[test]
    fn lab_known_black() {
        let black = ColorF64::srgb(0.0, 0.0, 0.0, 1.0).to_lab();
        assert!(
            black.c0().abs() < 1e-3,
            "black L* should be 0, got {}",
            black.c0()
        );
    }

    // -- sRGB <-> LCH round-trip --------------------------------------------

    #[test]
    fn srgb_lch_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_lch().to_srgb();
            assert_close(c, &rt, EPS_LAB, "srgb->lch->srgb");
        }
    }

    // -- sRGB <-> OkLab round-trip ------------------------------------------

    #[test]
    fn srgb_oklab_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 0.0, 1.0, 1.0),
            ColorF64::srgb(0.5, 0.5, 0.5, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_oklab().to_srgb();
            assert_close(c, &rt, EPS_LAB, "srgb->oklab->srgb");
        }
    }

    #[test]
    fn oklab_known_white() {
        let white = ColorF64::srgb(1.0, 1.0, 1.0, 1.0).to_oklab();
        assert!(
            (white.c0() - 1.0).abs() < 1e-4,
            "white OkL should be ~1, got {}",
            white.c0()
        );
        assert!(white.c1().abs() < 1e-4, "white Ok-a should be ~0");
        assert!(white.c2().abs() < 1e-4, "white Ok-b should be ~0");
    }

    // -- sRGB <-> OkLCH round-trip ------------------------------------------

    #[test]
    fn srgb_oklch_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 0.5),
        ];
        for c in &colors {
            let rt = c.to_oklch().to_srgb();
            assert_close(c, &rt, EPS_LAB, "srgb->oklch->srgb");
        }
    }

    // -- Interpolation ------------------------------------------------------

    #[test]
    fn interpolate_srgb_midpoint() {
        let black = ColorF64::srgb(0.0, 0.0, 0.0, 1.0);
        let white = ColorF64::srgb(1.0, 1.0, 1.0, 1.0);
        let mid = black.interpolate(&white, 0.5, ColorSpace::Srgb);
        assert_close(
            &mid,
            &ColorF64::srgb(0.5, 0.5, 0.5, 1.0),
            EPS,
            "srgb midpoint",
        );
    }

    #[test]
    fn interpolate_endpoints() {
        let a = ColorF64::srgb(0.2, 0.4, 0.6, 1.0);
        let b = ColorF64::srgb(0.8, 0.1, 0.3, 0.5);

        // t=0 should give a, t=1 should give b (both in the interpolation space)
        let at0 = a.interpolate(&b, 0.0, ColorSpace::OkLab);
        let at1 = a.interpolate(&b, 1.0, ColorSpace::OkLab);

        let a_ok = a.to_oklab();
        let b_ok = b.to_oklab();
        assert_close(&at0, &a_ok, EPS_LAB, "t=0 should be a");
        assert_close(&at1, &b_ok, EPS_LAB, "t=1 should be b");
    }

    #[test]
    fn interpolate_hue_short_arc() {
        // 350 deg and 10 deg -- short arc crosses 0
        let a = ColorF64::new(350.0, 1.0, 1.0, 1.0, ColorSpace::Hsv);
        let b = ColorF64::new(10.0, 1.0, 1.0, 1.0, ColorSpace::Hsv);
        let mid = a.interpolate(&b, 0.5, ColorSpace::Hsv);
        // Midpoint should be at 0 (or 360)
        let h = mid.c0();
        assert!(
            h < 1.0 || h > 359.0,
            "hue midpoint of 350..10 should be ~0/360, got {}",
            h,
        );
    }

    #[test]
    fn interpolate_alpha() {
        let a = ColorF64::srgb(1.0, 0.0, 0.0, 0.0);
        let b = ColorF64::srgb(0.0, 0.0, 1.0, 1.0);
        let mid = a.interpolate(&b, 0.5, ColorSpace::Srgb);
        assert!((mid.alpha() - 0.5).abs() < EPS, "alpha should be 0.5");
    }

    // -- to_space convenience -----------------------------------------------

    #[test]
    fn to_space_identity() {
        let c = ColorF64::srgb(0.3, 0.6, 0.9, 1.0);
        let c2 = c.to_space(ColorSpace::Srgb);
        assert_close(&c, &c2, EPS, "to_space identity");
    }

    // -- srgb_u8 convenience ------------------------------------------------

    #[test]
    fn srgb_u8_conversion() {
        let c = ColorF64::srgb_u8(255, 128, 0, 255);
        assert!((c.c0() - 1.0).abs() < EPS);
        assert!((c.c1() - 128.0 / 255.0).abs() < EPS);
        assert!((c.c2() - 0.0).abs() < EPS);
        assert!((c.alpha() - 1.0).abs() < EPS);
    }

    // -- Cross-space round trips --------------------------------------------

    #[test]
    fn full_roundtrip_through_all_spaces() {
        let original = ColorF64::srgb(0.4, 0.7, 0.2, 0.9);
        let spaces = [
            ColorSpace::LinearRgb,
            ColorSpace::Hsv,
            ColorSpace::Hsl,
            ColorSpace::Lab,
            ColorSpace::Lch,
            ColorSpace::OkLab,
            ColorSpace::OkLch,
        ];
        for &space in &spaces {
            let converted = original.to_space(space);
            assert_eq!(converted.space, space);
            let back = converted.to_srgb();
            assert_close(
                &original,
                &back,
                EPS_LAB,
                &format!("round-trip through {:?}", space),
            );
        }
    }

    // -- CMYK ---------------------------------------------------------------

    #[test]
    fn srgb_cmyk_roundtrip() {
        let colors = [
            ColorF64::srgb(1.0, 0.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 1.0, 0.0, 1.0),
            ColorF64::srgb(0.0, 0.0, 1.0, 1.0),
            ColorF64::srgb(0.5, 0.5, 0.5, 1.0),
            ColorF64::srgb(0.3, 0.6, 0.9, 1.0),
            ColorF64::srgb(1.0, 1.0, 1.0, 1.0),
        ];
        for c in &colors {
            let cmyk = c.to_cmyk();
            assert_eq!(cmyk.space, ColorSpace::Cmyk);
            let back = cmyk.to_srgb();
            // Alpha is always 1.0 for CMYK round-trip
            let expected = ColorF64::srgb(c.c0(), c.c1(), c.c2(), 1.0);
            assert_close(&expected, &back, EPS, "srgb->cmyk->srgb");
        }
    }

    #[test]
    fn cmyk_known_red() {
        let red = ColorF64::srgb(1.0, 0.0, 0.0, 1.0).to_cmyk();
        assert!((red.c0() - 0.0).abs() < EPS, "C should be 0 for red");
        assert!((red.c1() - 1.0).abs() < EPS, "M should be 1 for red");
        assert!((red.c2() - 1.0).abs() < EPS, "Y should be 1 for red");
        assert!(
            (red.components[3] - 0.0).abs() < EPS,
            "K should be 0 for red"
        );
    }

    #[test]
    fn cmyk_known_black() {
        let black = ColorF64::srgb(0.0, 0.0, 0.0, 1.0).to_cmyk();
        assert!((black.c0() - 0.0).abs() < EPS, "C should be 0 for black");
        assert!((black.c1() - 0.0).abs() < EPS, "M should be 0 for black");
        assert!((black.c2() - 0.0).abs() < EPS, "Y should be 0 for black");
        assert!(
            (black.components[3] - 1.0).abs() < EPS,
            "K should be 1 for black"
        );
    }

    #[test]
    fn cmyk_known_white() {
        let white = ColorF64::srgb(1.0, 1.0, 1.0, 1.0).to_cmyk();
        assert!((white.c0() - 0.0).abs() < EPS, "C should be 0 for white");
        assert!((white.c1() - 0.0).abs() < EPS, "M should be 0 for white");
        assert!((white.c2() - 0.0).abs() < EPS, "Y should be 0 for white");
        assert!(
            (white.components[3] - 0.0).abs() < EPS,
            "K should be 0 for white"
        );
    }

    #[test]
    fn cmyk_interpolation() {
        let a = ColorF64::cmyk(0.0, 0.0, 0.0, 0.0); // white
        let b = ColorF64::cmyk(1.0, 1.0, 1.0, 1.0); // max ink
        let mid = a.interpolate(&b, 0.5, ColorSpace::Cmyk);
        assert!((mid.c0() - 0.5).abs() < EPS, "C midpoint");
        assert!((mid.c1() - 0.5).abs() < EPS, "M midpoint");
        assert!((mid.c2() - 0.5).abs() < EPS, "Y midpoint");
        assert!((mid.components[3] - 0.5).abs() < EPS, "K midpoint");
    }

    #[test]
    fn cmyk_constructor() {
        let c = ColorF64::cmyk(0.2, 0.4, 0.6, 0.1);
        assert_eq!(c.space, ColorSpace::Cmyk);
        assert!((c.c0() - 0.2).abs() < EPS);
        assert!((c.c1() - 0.4).abs() < EPS);
        assert!((c.c2() - 0.6).abs() < EPS);
        assert!((c.components[3] - 0.1).abs() < EPS);
    }

    // -- Gamut clipping -----------------------------------------------------

    #[test]
    fn gamut_clip_srgb() {
        let c = ColorF64::new(1.5, -0.3, 0.5, 1.2, ColorSpace::Srgb);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 1.0).abs() < EPS);
        assert!((clipped.c1() - 0.0).abs() < EPS);
        assert!((clipped.c2() - 0.5).abs() < EPS);
        assert!((clipped.alpha() - 1.0).abs() < EPS);
    }

    #[test]
    fn gamut_clip_hsv() {
        let c = ColorF64::new(370.0, 1.5, -0.1, 0.5, ColorSpace::Hsv);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 10.0).abs() < EPS, "hue should wrap");
        assert!((clipped.c1() - 1.0).abs() < EPS, "sat should clamp");
        assert!((clipped.c2() - 0.0).abs() < EPS, "val should clamp");
    }

    #[test]
    fn gamut_clip_lab() {
        let c = ColorF64::new(110.0, 50.0, -30.0, 1.0, ColorSpace::Lab);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 100.0).abs() < EPS, "L* should clamp to 100");
        // a* and b* are unbounded, so they should pass through
        assert!((clipped.c1() - 50.0).abs() < EPS);
        assert!((clipped.c2() - -30.0).abs() < EPS);
    }

    #[test]
    fn gamut_clip_lch() {
        let c = ColorF64::new(110.0, -5.0, 400.0, 1.0, ColorSpace::Lch);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 100.0).abs() < EPS, "L should clamp");
        assert!((clipped.c1() - 0.0).abs() < EPS, "C should clamp to 0");
        assert!((clipped.c2() - 40.0).abs() < EPS, "h should wrap");
    }

    #[test]
    fn gamut_clip_cmyk() {
        let c = ColorF64::new(1.5, -0.1, 0.5, 1.2, ColorSpace::Cmyk);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 1.0).abs() < EPS);
        assert!((clipped.c1() - 0.0).abs() < EPS);
        assert!((clipped.c2() - 0.5).abs() < EPS);
        assert!((clipped.components[3] - 1.0).abs() < EPS);
    }

    #[test]
    fn gamut_clip_oklab() {
        let c = ColorF64::new(1.5, 0.3, -0.2, -0.5, ColorSpace::OkLab);
        let clipped = c.gamut_clip();
        assert!((clipped.c0() - 1.0).abs() < EPS, "L should clamp to 1");
        assert!((clipped.c1() - 0.3).abs() < EPS, "a passes through");
        assert!((clipped.c2() - -0.2).abs() < EPS, "b passes through");
        assert!((clipped.alpha() - 0.0).abs() < EPS, "alpha clamps to 0");
    }

    // -- Soft proofing ------------------------------------------------------

    #[test]
    fn soft_proof_identity() {
        // Soft proofing through sRGB should be close to identity for in-gamut colors
        let c = ColorF64::srgb(0.5, 0.3, 0.8, 1.0);
        let proofed = super::soft_proof(&c, ColorSpace::Srgb);
        assert_close(&c, &proofed, EPS, "soft proof through sRGB");
    }

    #[test]
    fn soft_proof_cmyk_roundtrip() {
        let c = ColorF64::srgb(0.5, 0.3, 0.8, 1.0);
        let proofed = super::soft_proof(&c, ColorSpace::Cmyk);
        // Result should be valid sRGB
        assert_eq!(proofed.space, ColorSpace::Srgb);
        assert!(proofed.c0() >= 0.0 && proofed.c0() <= 1.0);
        assert!(proofed.c1() >= 0.0 && proofed.c1() <= 1.0);
        assert!(proofed.c2() >= 0.0 && proofed.c2() <= 1.0);
    }

    // -- Document color settings --------------------------------------------

    #[test]
    fn document_color_settings_default() {
        let settings = super::DocumentColorSettings::default();
        assert_eq!(settings.working_space, ColorSpace::Srgb);
        assert_eq!(
            settings.rendering_intent,
            super::RenderingIntent::Perceptual
        );
        assert!(settings.embedded_profile.is_none());
    }
}
