//! Trace Bitmap dialog data model (11.4.15).
//!
//! Settings and state for converting raster images into vector paths.

/// Tracing algorithm / mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceMode {
    /// Threshold based on pixel brightness.
    Brightness,
    /// Edge detection.
    Edge,
    /// Colour quantisation (multiple passes).
    ColorQuantize,
    /// Centre-line (skeleton) tracing.
    Centerline,
}

impl TraceMode {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            TraceMode::Brightness => "Brightness Cutoff",
            TraceMode::Edge => "Edge Detection",
            TraceMode::ColorQuantize => "Color Quantization",
            TraceMode::Centerline => "Centerline (Autotrace)",
        }
    }

    /// All modes in display order.
    pub fn all() -> &'static [TraceMode] {
        &[
            TraceMode::Brightness,
            TraceMode::Edge,
            TraceMode::ColorQuantize,
            TraceMode::Centerline,
        ]
    }
}

/// Complete state for the Trace Bitmap dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceBitmapState {
    /// Active tracing mode.
    pub mode: TraceMode,
    /// Brightness / edge threshold in `[0.0, 1.0]`.
    pub threshold: f64,
    /// Apply curve smoothing to the result.
    pub smoothing: bool,
    /// Ignore speckles smaller than this many pixels.
    pub speckle_size: u32,
    /// Invert the source image before tracing.
    pub invert: bool,
    /// Whether the live preview is enabled.
    pub preview_enabled: bool,
    /// Number of colours for `ColorQuantize` mode.
    pub colors: u32,
    /// The resulting path data, if a trace has completed.
    /// Stored as an SVG `d` attribute string.
    pub result_path: Option<String>,
}

impl Default for TraceBitmapState {
    fn default() -> Self {
        Self {
            mode: TraceMode::Brightness,
            threshold: 0.5,
            smoothing: true,
            speckle_size: 2,
            invert: false,
            preview_enabled: true,
            colors: 8,
            result_path: None,
        }
    }
}

impl TraceBitmapState {
    /// Set the tracing mode, resetting mode-specific defaults.
    pub fn set_mode(&mut self, mode: TraceMode) {
        self.mode = mode;
        self.result_path = None;
        // Reset colours to sensible default only for colour mode.
        if mode == TraceMode::ColorQuantize {
            if self.colors < 2 {
                self.colors = 8;
            }
        }
    }

    /// Request a preview update.
    ///
    /// The actual rendering is performed externally; this method clears any
    /// stale result and returns `true` if preview is enabled.
    pub fn update_preview(&mut self) -> bool {
        if self.preview_enabled {
            self.result_path = None;
            true
        } else {
            false
        }
    }

    /// Execute the trace.
    ///
    /// In this data-model layer, the heavy lifting is done by the integration
    /// layer.  This method marks state as ready and returns `true`.
    pub fn run_trace(&mut self) -> bool {
        self.result_path = None;
        // The caller is expected to populate `result_path` after processing.
        true
    }

    /// Return the traced path data, if available.
    pub fn get_result(&self) -> Option<&str> {
        self.result_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = TraceBitmapState::default();
        assert_eq!(state.mode, TraceMode::Brightness);
        assert!((state.threshold - 0.5).abs() < f64::EPSILON);
        assert!(state.smoothing);
        assert_eq!(state.speckle_size, 2);
        assert!(!state.invert);
        assert!(state.preview_enabled);
        assert_eq!(state.colors, 8);
        assert!(state.result_path.is_none());
    }

    #[test]
    fn set_mode_clears_result() {
        let mut state = TraceBitmapState::default();
        state.result_path = Some("M 0 0 L 10 10".to_string());
        state.set_mode(TraceMode::Edge);
        assert_eq!(state.mode, TraceMode::Edge);
        assert!(state.result_path.is_none());
    }

    #[test]
    fn set_mode_color_quantize_defaults() {
        let mut state = TraceBitmapState::default();
        state.colors = 0;
        state.set_mode(TraceMode::ColorQuantize);
        assert_eq!(state.colors, 8);
    }

    #[test]
    fn update_preview_enabled() {
        let mut state = TraceBitmapState::default();
        state.result_path = Some("M 0 0".to_string());
        assert!(state.update_preview());
        assert!(state.result_path.is_none());
    }

    #[test]
    fn update_preview_disabled() {
        let mut state = TraceBitmapState::default();
        state.preview_enabled = false;
        state.result_path = Some("M 0 0".to_string());
        assert!(!state.update_preview());
        assert!(state.result_path.is_some()); // not cleared
    }

    #[test]
    fn run_trace() {
        let mut state = TraceBitmapState::default();
        assert!(state.run_trace());
        assert!(state.result_path.is_none());
    }

    #[test]
    fn get_result() {
        let mut state = TraceBitmapState::default();
        assert_eq!(state.get_result(), None);
        state.result_path = Some("M 0 0 L 100 100".to_string());
        assert_eq!(state.get_result(), Some("M 0 0 L 100 100"));
    }

    #[test]
    fn mode_labels() {
        assert_eq!(TraceMode::Brightness.label(), "Brightness Cutoff");
        assert_eq!(TraceMode::all().len(), 4);
    }
}
