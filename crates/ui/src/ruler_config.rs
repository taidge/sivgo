//! Ruler rendering data model (11.1.2).
//!
//! Configuration, orientation, unit, and tick-mark computation for rulers.

/// Ruler orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Units displayed on the ruler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerUnit {
    Pixels,
    Millimeters,
    Inches,
    Points,
}

impl RulerUnit {
    /// Conversion factor: how many pixels per one unit.
    pub fn pixels_per_unit(&self) -> f64 {
        match self {
            RulerUnit::Pixels => 1.0,
            RulerUnit::Millimeters => 96.0 / 25.4, // CSS px per mm
            RulerUnit::Inches => 96.0,             // CSS px per inch
            RulerUnit::Points => 96.0 / 72.0,      // CSS px per point
        }
    }

    /// Short label for display.
    pub fn label(&self) -> &'static str {
        match self {
            RulerUnit::Pixels => "px",
            RulerUnit::Millimeters => "mm",
            RulerUnit::Inches => "in",
            RulerUnit::Points => "pt",
        }
    }
}

/// Configuration for computing ruler tick marks.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerConfig {
    /// Whether this is a horizontal or vertical ruler.
    pub orientation: Orientation,
    /// The unit system for labels.
    pub unit: RulerUnit,
    /// Current zoom factor (1.0 = 100%).
    pub zoom: f64,
    /// Current scroll/pan offset in screen pixels.
    pub pan: f64,
}

/// A single tick mark on the ruler.
#[derive(Debug, Clone, PartialEq)]
pub struct TickMark {
    /// Position along the ruler in screen pixels.
    pub position: f64,
    /// Optional label string (only for major ticks).
    pub label: Option<String>,
    /// Whether this is a major (labeled) tick or a minor one.
    pub major: bool,
}

/// Compute a "nice" tick interval in ruler units, aiming for roughly
/// 50-100 screen pixels between major ticks at the given zoom level.
fn nice_tick_interval(zoom: f64, pixels_per_unit: f64) -> f64 {
    let target_screen_px = 80.0;
    let raw = target_screen_px / (zoom * pixels_per_unit);

    // Snap to 1, 2, 5, 10, 20, 50, ...
    if raw <= 0.0 {
        return 1.0;
    }
    let magnitude = 10.0_f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    nice * magnitude
}

/// Compute the tick marks for a ruler of the given `length` (in screen pixels).
///
/// Returns a vector of [`TickMark`] values positioned in screen coordinates.
pub fn compute_tick_marks(config: &RulerConfig, length: f64) -> Vec<TickMark> {
    let ppu = config.unit.pixels_per_unit();
    let tick_interval = nice_tick_interval(config.zoom, ppu);
    let tick_screen = tick_interval * config.zoom * ppu;

    // First unit-space value aligned to tick_interval.
    let unit_start = -config.pan / (config.zoom * ppu);
    let first_tick = (unit_start / tick_interval).floor() * tick_interval;

    let mut ticks = Vec::new();
    let mut unit_val = first_tick;

    let major_every = 5; // every 5th tick is major
    let mut tick_index = 0i64;

    loop {
        let screen_pos = unit_val * config.zoom * ppu + config.pan;
        if screen_pos > length + tick_screen {
            break;
        }
        if screen_pos >= -tick_screen {
            // Determine if this is a major tick (every 5th).
            let major_val = unit_val / (tick_interval * major_every as f64);
            let is_major = major_val.fract().abs() < 0.01 || (1.0 - major_val.fract().abs()) < 0.01;

            let label = if is_major {
                Some(
                    format!("{:.4}", unit_val)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                )
            } else {
                None
            };

            ticks.push(TickMark {
                position: screen_pos,
                label,
                major: is_major,
            });
        }
        unit_val += tick_interval;
        tick_index += 1;

        // Safety: prevent infinite loops on degenerate input.
        if tick_index > 10_000 {
            break;
        }
    }

    ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_unit_factor() {
        assert!((RulerUnit::Pixels.pixels_per_unit() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_tick_marks_returns_ticks() {
        let config = RulerConfig {
            orientation: Orientation::Horizontal,
            unit: RulerUnit::Pixels,
            zoom: 1.0,
            pan: 0.0,
        };
        let ticks = compute_tick_marks(&config, 500.0);
        assert!(!ticks.is_empty());
        // All tick positions should be within a reasonable range.
        for t in &ticks {
            assert!(t.position >= -100.0 && t.position <= 600.0);
        }
    }

    #[test]
    fn major_ticks_have_labels() {
        let config = RulerConfig {
            orientation: Orientation::Vertical,
            unit: RulerUnit::Pixels,
            zoom: 1.0,
            pan: 0.0,
        };
        let ticks = compute_tick_marks(&config, 1000.0);
        let major_ticks: Vec<_> = ticks.iter().filter(|t| t.major).collect();
        assert!(!major_ticks.is_empty());
        for t in &major_ticks {
            assert!(t.label.is_some());
        }
    }
}
