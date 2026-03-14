//! WASM build configuration and runtime state.
//!
//! Provides data models for configuring the WebAssembly build of the Sivgo
//! editor and tracking the runtime state of the WASM canvas.

/// Build-time configuration for the WASM target.
#[derive(Debug, Clone)]
pub struct WasmBuildConfig {
    /// Whether this is a debug build (includes extra logging, source maps).
    pub debug: bool,
    /// Optimisation level for `wasm-opt` (0 = none, 3 = maximum).
    pub optimization_level: u8,
    /// Cargo features to enable for the WASM build.
    pub features: Vec<String>,
}

impl Default for WasmBuildConfig {
    fn default() -> Self {
        Self {
            debug: false,
            optimization_level: 2,
            features: Vec::new(),
        }
    }
}

impl WasmBuildConfig {
    /// Create a debug build configuration (no optimisation, debug enabled).
    pub fn debug() -> Self {
        Self {
            debug: true,
            optimization_level: 0,
            features: Vec::new(),
        }
    }

    /// Create a release build configuration with maximum optimisation.
    pub fn release() -> Self {
        Self {
            debug: false,
            optimization_level: 3,
            features: Vec::new(),
        }
    }

    /// Add a cargo feature to the build.
    pub fn with_feature(mut self, feature: impl Into<String>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Validate the configuration.
    ///
    /// Returns an error message if the optimisation level is out of range.
    pub fn validate(&self) -> Result<(), String> {
        if self.optimization_level > 3 {
            return Err(format!(
                "optimization_level must be 0-3, got {}",
                self.optimization_level
            ));
        }
        Ok(())
    }
}

/// Runtime state of the WASM module and its rendering canvas.
#[derive(Debug, Clone)]
pub struct WasmRuntime {
    /// Whether the WASM runtime has been initialised.
    pub initialized: bool,
    /// The DOM id of the `<canvas>` element used for rendering.
    pub canvas_id: String,
    /// Canvas width in CSS pixels.
    pub width: u32,
    /// Canvas height in CSS pixels.
    pub height: u32,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            canvas_id: String::new(),
            width: 0,
            height: 0,
        }
    }
}

impl WasmRuntime {
    /// Initialise the runtime with the given canvas element id.
    ///
    /// Sets `initialized` to `true` and records the canvas id.
    pub fn init(canvas_id: impl Into<String>) -> Self {
        Self {
            initialized: true,
            canvas_id: canvas_id.into(),
            width: 0,
            height: 0,
        }
    }

    /// Resize the canvas to the given dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Returns `true` if the runtime has been initialised and has a
    /// non-zero canvas size.
    pub fn is_ready(&self) -> bool {
        self.initialized && self.width > 0 && self.height > 0
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_build_config() {
        let cfg = WasmBuildConfig::default();
        assert!(!cfg.debug);
        assert_eq!(cfg.optimization_level, 2);
        assert!(cfg.features.is_empty());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn debug_build_config() {
        let cfg = WasmBuildConfig::debug();
        assert!(cfg.debug);
        assert_eq!(cfg.optimization_level, 0);
    }

    #[test]
    fn release_build_config() {
        let cfg = WasmBuildConfig::release();
        assert!(!cfg.debug);
        assert_eq!(cfg.optimization_level, 3);
    }

    #[test]
    fn build_config_with_features() {
        let cfg = WasmBuildConfig::default()
            .with_feature("svg-filters")
            .with_feature("webgl");
        assert_eq!(cfg.features.len(), 2);
        assert_eq!(cfg.features[0], "svg-filters");
        assert_eq!(cfg.features[1], "webgl");
    }

    #[test]
    fn invalid_optimization_level() {
        let cfg = WasmBuildConfig {
            optimization_level: 5,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn runtime_default_is_not_ready() {
        let rt = WasmRuntime::default();
        assert!(!rt.initialized);
        assert!(!rt.is_ready());
    }

    #[test]
    fn runtime_init_and_resize() {
        let mut rt = WasmRuntime::init("sivgo-canvas");
        assert!(rt.initialized);
        assert_eq!(rt.canvas_id, "sivgo-canvas");
        assert!(!rt.is_ready()); // no size yet

        rt.resize(1920, 1080);
        assert!(rt.is_ready());
        assert_eq!(rt.width, 1920);
        assert_eq!(rt.height, 1080);
    }

    #[test]
    fn runtime_zero_size_is_not_ready() {
        let mut rt = WasmRuntime::init("canvas");
        rt.resize(0, 0);
        assert!(!rt.is_ready());
    }
}
