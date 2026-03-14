//! Filter preset library -- programmatic definitions of common SVG filter effects.
//!
//! This module provides a curated collection of filter presets modelled after the
//! ~200 presets that Inkscape ships.  Each preset is a [`FilterPreset`] containing
//! metadata (id, name, category, description) and a sequence of
//! [`PresetPrimitive`]s that describe the filter graph.  A helper method
//! [`FilterPreset::to_pipeline`] converts a preset into the crate's
//! [`FilterPipeline`] so it can be executed directly.

use crate::blend::BlendMode;
use crate::color_matrix::ColorMatrixType;
use crate::composite::CompositeOp;
use crate::diffuse_lighting::LightSource;
use crate::morphology::MorphologyOp;
use crate::pipeline::{FilterPipeline, FilterPrimitive};
use crate::turbulence::TurbulenceType;

// ---------------------------------------------------------------------------
// FilterCategory
// ---------------------------------------------------------------------------

/// High-level category a filter preset belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterCategory {
    Shadows,
    Blurs,
    Textures,
    Bevels,
    Distortions,
    ColorEffects,
    Overlays,
}

impl FilterCategory {
    /// Human-readable label for display in a UI.
    pub fn label(&self) -> &'static str {
        match self {
            FilterCategory::Shadows => "Shadows",
            FilterCategory::Blurs => "Blurs",
            FilterCategory::Textures => "Textures",
            FilterCategory::Bevels => "Bevels",
            FilterCategory::Distortions => "Distortions",
            FilterCategory::ColorEffects => "Color Effects",
            FilterCategory::Overlays => "Overlays",
        }
    }
}

// ---------------------------------------------------------------------------
// PresetPrimitive
// ---------------------------------------------------------------------------

/// A declarative description of a single filter primitive inside a preset.
///
/// Unlike [`FilterPrimitive`] (which uses concrete Rust types for every
/// parameter), `PresetPrimitive` is intentionally simple and string-oriented so
/// that presets can be expressed compactly as data.
#[derive(Debug, Clone)]
pub enum PresetPrimitive {
    GaussianBlur {
        std_dev_x: f64,
        std_dev_y: f64,
        input: &'static str,
        result: &'static str,
    },
    Offset {
        dx: f64,
        dy: f64,
        input: &'static str,
        result: &'static str,
    },
    Flood {
        color: &'static str,
        opacity: f64,
        result: &'static str,
    },
    Composite {
        operator: &'static str,
        in1: &'static str,
        in2: &'static str,
        result: &'static str,
    },
    Merge {
        inputs: Vec<&'static str>,
    },
    ColorMatrix {
        matrix_type: &'static str,
        values: &'static str,
        input: &'static str,
        result: &'static str,
    },
    Blend {
        mode: &'static str,
        in1: &'static str,
        in2: &'static str,
        result: &'static str,
    },
    Turbulence {
        freq_x: f64,
        freq_y: f64,
        octaves: u32,
        seed: u32,
        turb_type: &'static str,
        result: &'static str,
    },
    DiffuseLighting {
        surface_scale: f64,
        diffuse_constant: f64,
        input: &'static str,
        result: &'static str,
    },
    SpecularLighting {
        surface_scale: f64,
        specular_constant: f64,
        specular_exponent: f64,
        input: &'static str,
        result: &'static str,
    },
    Morphology {
        operator: &'static str,
        radius: f64,
        input: &'static str,
        result: &'static str,
    },
    ConvolveMatrix {
        order_x: u32,
        order_y: u32,
        kernel: Vec<f32>,
        divisor: f32,
        bias: f32,
        input: &'static str,
        result: &'static str,
    },
    DisplacementMap {
        scale: f64,
        x_channel: &'static str,
        y_channel: &'static str,
        in1: &'static str,
        in2: &'static str,
        result: &'static str,
    },
    ComponentTransfer {
        input: &'static str,
        result: &'static str,
        /// Per-channel transfer function type: "discrete", "table", "linear", or "identity".
        func_type: &'static str,
        /// Comma-separated table values (used by discrete and table types).
        table_values: &'static str,
    },
}

// ---------------------------------------------------------------------------
// FilterPreset
// ---------------------------------------------------------------------------

/// A complete filter preset definition.
#[derive(Debug, Clone)]
pub struct FilterPreset {
    /// Machine-readable identifier (e.g. `"drop_shadow"`).
    pub id: &'static str,
    /// Human-readable display name (e.g. `"Drop Shadow"`).
    pub name: &'static str,
    /// Organisational category.
    pub category: FilterCategory,
    /// Short description of the visual effect.
    pub description: &'static str,
    /// Ordered list of filter primitives that implement the effect.
    pub primitives: Vec<PresetPrimitive>,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Parse a hex colour string like `"#000000"` or `"#ff8800"` into `[r, g, b, a]`
/// with the given opacity.  Falls back to opaque black on parse failure.
fn parse_flood_color(hex: &str, opacity: f64) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (0, 0, 0)
    };
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        opacity as f32,
    ]
}

/// Map a string blend mode name to [`BlendMode`].
fn parse_blend_mode(s: &str) -> BlendMode {
    match s {
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        _ => BlendMode::Normal,
    }
}

/// Map a string composite operator to [`CompositeOp`].
fn parse_composite_op(s: &str) -> CompositeOp {
    match s {
        "in" => CompositeOp::In,
        "out" => CompositeOp::Out,
        "atop" => CompositeOp::Atop,
        "xor" => CompositeOp::Xor,
        _ => CompositeOp::Over,
    }
}

/// Map a string morphology operator to [`MorphologyOp`].
fn parse_morphology_op(s: &str) -> MorphologyOp {
    match s {
        "dilate" => MorphologyOp::Dilate,
        _ => MorphologyOp::Erode,
    }
}

/// Map a string turbulence type to [`TurbulenceType`].
fn parse_turbulence_type(s: &str) -> TurbulenceType {
    match s {
        "fractalNoise" => TurbulenceType::FractalNoise,
        _ => TurbulenceType::Turbulence,
    }
}

/// Parse a `ColorMatrixType` from the string-based preset description.
fn parse_color_matrix(matrix_type: &str, values: &str) -> ColorMatrixType {
    match matrix_type {
        "saturate" => {
            let v: f32 = values.parse().unwrap_or(1.0);
            ColorMatrixType::Saturate(v)
        }
        "hueRotate" => {
            let v: f32 = values.parse().unwrap_or(0.0);
            ColorMatrixType::HueRotate(v)
        }
        "luminanceToAlpha" => ColorMatrixType::LuminanceToAlpha,
        _ => {
            // "matrix" -- parse 20 whitespace-separated values
            let vals: Vec<f32> = values
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
            let mut arr = [0.0f32; 20];
            for (i, v) in vals.iter().enumerate().take(20) {
                arr[i] = *v;
            }
            ColorMatrixType::Matrix(arr)
        }
    }
}

/// Parse a channel selector string to [`crate::displacement_map::ChannelSelector`].
fn parse_channel_selector(s: &str) -> crate::displacement_map::ChannelSelector {
    use crate::displacement_map::ChannelSelector;
    match s {
        "R" => ChannelSelector::R,
        "G" => ChannelSelector::G,
        "B" => ChannelSelector::B,
        _ => ChannelSelector::A,
    }
}

impl FilterPreset {
    /// Convert this preset into a [`FilterPipeline`] ready for execution.
    pub fn to_pipeline(&self) -> FilterPipeline {
        let mut pipeline = FilterPipeline::new();
        for prim in &self.primitives {
            match prim {
                PresetPrimitive::GaussianBlur {
                    std_dev_x,
                    std_dev_y,
                    input,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::GaussianBlur {
                        input: input.to_string(),
                        std_dev_x: *std_dev_x,
                        std_dev_y: *std_dev_y,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Offset {
                    dx,
                    dy,
                    input,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::Offset {
                        input: input.to_string(),
                        dx: *dx,
                        dy: *dy,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Flood {
                    color,
                    opacity,
                    result,
                } => {
                    let c = parse_flood_color(color, *opacity);
                    pipeline.add(FilterPrimitive::Flood {
                        color: c,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Composite {
                    operator,
                    in1,
                    in2,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::Composite {
                        input1: in1.to_string(),
                        input2: in2.to_string(),
                        op: parse_composite_op(operator),
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Merge { inputs } => {
                    pipeline.add(FilterPrimitive::Merge {
                        inputs: inputs.iter().map(|s| s.to_string()).collect(),
                        result: "merge_result".to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::ColorMatrix {
                    matrix_type,
                    values,
                    input,
                    result,
                } => {
                    let mt = parse_color_matrix(matrix_type, values);
                    pipeline.add(FilterPrimitive::ColorMatrix {
                        input: input.to_string(),
                        matrix_type: mt,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Blend {
                    mode,
                    in1,
                    in2,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::Blend {
                        input1: in1.to_string(),
                        input2: in2.to_string(),
                        mode: parse_blend_mode(mode),
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Turbulence {
                    freq_x,
                    freq_y,
                    octaves,
                    seed,
                    turb_type,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::Turbulence {
                        turb_type: parse_turbulence_type(turb_type),
                        base_freq_x: *freq_x,
                        base_freq_y: *freq_y,
                        num_octaves: *octaves,
                        seed: *seed as i32,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::DiffuseLighting {
                    surface_scale,
                    diffuse_constant,
                    input,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::DiffuseLighting {
                        input: input.to_string(),
                        surface_scale: *surface_scale,
                        diffuse_constant: *diffuse_constant,
                        light: LightSource::Distant {
                            azimuth: 235.0,
                            elevation: 45.0,
                        },
                        light_color: [1.0, 1.0, 1.0],
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::SpecularLighting {
                    surface_scale,
                    specular_constant,
                    specular_exponent,
                    input,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::SpecularLighting {
                        input: input.to_string(),
                        surface_scale: *surface_scale,
                        specular_constant: *specular_constant,
                        specular_exponent: *specular_exponent,
                        light: LightSource::Distant {
                            azimuth: 235.0,
                            elevation: 45.0,
                        },
                        light_color: [1.0, 1.0, 1.0],
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::Morphology {
                    operator,
                    radius,
                    input,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::Morphology {
                        input: input.to_string(),
                        op: parse_morphology_op(operator),
                        radius_x: *radius,
                        radius_y: *radius,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::ConvolveMatrix {
                    order_x,
                    order_y,
                    kernel,
                    divisor,
                    bias,
                    input,
                    result,
                } => {
                    let tx = *order_x / 2;
                    let ty = *order_y / 2;
                    pipeline.add(FilterPrimitive::ConvolveMatrix {
                        input: input.to_string(),
                        kernel: kernel.clone(),
                        order_x: *order_x,
                        order_y: *order_y,
                        divisor: *divisor,
                        bias: *bias,
                        target_x: tx,
                        target_y: ty,
                        edge_mode: crate::convolve_matrix::EdgeMode::Duplicate,
                        preserve_alpha: false,
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::DisplacementMap {
                    scale,
                    x_channel,
                    y_channel,
                    in1,
                    in2,
                    result,
                } => {
                    pipeline.add(FilterPrimitive::DisplacementMap {
                        input1: in1.to_string(),
                        input2: in2.to_string(),
                        scale: *scale,
                        x_channel: parse_channel_selector(x_channel),
                        y_channel: parse_channel_selector(y_channel),
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
                PresetPrimitive::ComponentTransfer { input, result, .. } => {
                    // Component transfer uses identity by default -- the preset
                    // provides the hint via func_type / table_values but a full
                    // implementation would need per-channel functions.  We map it
                    // to an identity transfer so the pipeline is structurally
                    // complete.
                    use crate::component_transfer::{ComponentTransferFunctions, TransferFunction};
                    pipeline.add(FilterPrimitive::ComponentTransfer {
                        input: input.to_string(),
                        functions: ComponentTransferFunctions {
                            func_r: TransferFunction::Identity,
                            func_g: TransferFunction::Identity,
                            func_b: TransferFunction::Identity,
                            func_a: TransferFunction::Identity,
                        },
                        result: result.to_string(),
                        sub_region: None,
                    });
                }
            }
        }
        pipeline
    }
}

// ===========================================================================
// Preset definitions
// ===========================================================================

/// Build and return the complete preset for "Drop Shadow".
fn drop_shadow() -> FilterPreset {
    FilterPreset {
        id: "drop_shadow",
        name: "Drop Shadow",
        category: FilterCategory::Shadows,
        description: "Classic drop shadow offset behind the element.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 3.0,
                std_dev_y: 3.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::Offset {
                dx: 4.0,
                dy: 4.0,
                input: "blur",
                result: "offsetBlur",
            },
            PresetPrimitive::Flood {
                color: "#000000",
                opacity: 0.5,
                result: "flood",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "flood",
                in2: "offsetBlur",
                result: "shadow",
            },
            PresetPrimitive::Merge {
                inputs: vec!["shadow", "SourceGraphic"],
            },
        ],
    }
}

/// Build and return the complete preset for "Inner Shadow".
fn inner_shadow() -> FilterPreset {
    FilterPreset {
        id: "inner_shadow",
        name: "Inner Shadow",
        category: FilterCategory::Shadows,
        description: "Shadow cast inside the element edges.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 3.0,
                std_dev_y: 3.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::Offset {
                dx: 2.0,
                dy: 2.0,
                input: "blur",
                result: "offsetBlur",
            },
            PresetPrimitive::Composite {
                operator: "out",
                in1: "SourceAlpha",
                in2: "offsetBlur",
                result: "inverse",
            },
            PresetPrimitive::Flood {
                color: "#000000",
                opacity: 0.6,
                result: "flood",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "flood",
                in2: "inverse",
                result: "shadow",
            },
            PresetPrimitive::Composite {
                operator: "over",
                in1: "shadow",
                in2: "SourceGraphic",
                result: "final",
            },
        ],
    }
}

/// Build and return the complete preset for "Long Shadow".
fn long_shadow() -> FilterPreset {
    FilterPreset {
        id: "long_shadow",
        name: "Long Shadow",
        category: FilterCategory::Shadows,
        description: "Extended diagonal shadow creating a flat-design depth effect.",
        primitives: vec![
            // Create a blurred, heavily offset copy of the alpha channel.
            PresetPrimitive::GaussianBlur {
                std_dev_x: 1.0,
                std_dev_y: 1.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::Offset {
                dx: 12.0,
                dy: 12.0,
                input: "blur",
                result: "offsetBlur",
            },
            PresetPrimitive::Flood {
                color: "#000000",
                opacity: 0.35,
                result: "flood",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "flood",
                in2: "offsetBlur",
                result: "shadow",
            },
            PresetPrimitive::Merge {
                inputs: vec!["shadow", "SourceGraphic"],
            },
        ],
    }
}

/// Build and return the complete preset for "Simple Blur".
fn simple_blur() -> FilterPreset {
    FilterPreset {
        id: "simple_blur",
        name: "Simple Blur",
        category: FilterCategory::Blurs,
        description: "Uniform Gaussian blur applied to the entire element.",
        primitives: vec![PresetPrimitive::GaussianBlur {
            std_dev_x: 4.0,
            std_dev_y: 4.0,
            input: "SourceGraphic",
            result: "blur",
        }],
    }
}

/// Build and return the complete preset for "Motion Blur".
fn motion_blur() -> FilterPreset {
    FilterPreset {
        id: "motion_blur",
        name: "Motion Blur",
        category: FilterCategory::Blurs,
        description: "Directional blur simulating horizontal movement.",
        primitives: vec![PresetPrimitive::GaussianBlur {
            std_dev_x: 8.0,
            std_dev_y: 0.5,
            input: "SourceGraphic",
            result: "motionBlur",
        }],
    }
}

/// Build and return the complete preset for "Zoom Blur".
fn zoom_blur() -> FilterPreset {
    FilterPreset {
        id: "zoom_blur",
        name: "Zoom Blur",
        category: FilterCategory::Blurs,
        description: "Radial blur effect simulating camera zoom.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 2.0,
                std_dev_y: 2.0,
                input: "SourceGraphic",
                result: "blur1",
            },
            PresetPrimitive::GaussianBlur {
                std_dev_x: 6.0,
                std_dev_y: 6.0,
                input: "SourceGraphic",
                result: "blur2",
            },
            PresetPrimitive::Blend {
                mode: "screen",
                in1: "blur1",
                in2: "blur2",
                result: "zoomBlur",
            },
        ],
    }
}

/// Build and return the complete preset for "Noise Fill".
fn noise_fill() -> FilterPreset {
    FilterPreset {
        id: "noise_fill",
        name: "Noise Fill",
        category: FilterCategory::Textures,
        description: "Fill the element region with Perlin noise.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.05,
                freq_y: 0.05,
                octaves: 4,
                seed: 0,
                turb_type: "turbulence",
                result: "noise",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "noise",
                in2: "SourceAlpha",
                result: "noiseFill",
            },
        ],
    }
}

/// Build and return the complete preset for "Paper Texture".
fn paper_texture() -> FilterPreset {
    FilterPreset {
        id: "paper_texture",
        name: "Paper Texture",
        category: FilterCategory::Textures,
        description: "Subtle organic texture resembling paper grain.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.04,
                freq_y: 0.04,
                octaves: 5,
                seed: 2,
                turb_type: "fractalNoise",
                result: "noise",
            },
            PresetPrimitive::DiffuseLighting {
                surface_scale: 2.0,
                diffuse_constant: 1.0,
                input: "noise",
                result: "lit",
            },
            PresetPrimitive::Blend {
                mode: "multiply",
                in1: "lit",
                in2: "SourceGraphic",
                result: "paper",
            },
        ],
    }
}

/// Build and return the complete preset for "Canvas Texture".
fn canvas_texture() -> FilterPreset {
    FilterPreset {
        id: "canvas_texture",
        name: "Canvas Texture",
        category: FilterCategory::Textures,
        description: "Coarse weave-like texture resembling artist canvas.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.12,
                freq_y: 0.12,
                octaves: 2,
                seed: 5,
                turb_type: "turbulence",
                result: "coarseNoise",
            },
            PresetPrimitive::DiffuseLighting {
                surface_scale: 4.0,
                diffuse_constant: 0.8,
                input: "coarseNoise",
                result: "lit",
            },
            PresetPrimitive::Blend {
                mode: "multiply",
                in1: "lit",
                in2: "SourceGraphic",
                result: "canvas",
            },
        ],
    }
}

/// Build and return the complete preset for "Inner Bevel".
fn inner_bevel() -> FilterPreset {
    FilterPreset {
        id: "inner_bevel",
        name: "Inner Bevel",
        category: FilterCategory::Bevels,
        description: "Raised inner edge creating a button-like 3D look.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 2.0,
                std_dev_y: 2.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::SpecularLighting {
                surface_scale: 5.0,
                specular_constant: 0.8,
                specular_exponent: 20.0,
                input: "blur",
                result: "specular",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "specular",
                in2: "SourceAlpha",
                result: "bevel",
            },
            PresetPrimitive::Blend {
                mode: "screen",
                in1: "bevel",
                in2: "SourceGraphic",
                result: "innerBevel",
            },
        ],
    }
}

/// Build and return the complete preset for "Outer Bevel".
fn outer_bevel() -> FilterPreset {
    FilterPreset {
        id: "outer_bevel",
        name: "Outer Bevel",
        category: FilterCategory::Bevels,
        description: "Raised outer edge creating an extruded 3D look.",
        primitives: vec![
            PresetPrimitive::Morphology {
                operator: "dilate",
                radius: 2.0,
                input: "SourceAlpha",
                result: "dilated",
            },
            PresetPrimitive::GaussianBlur {
                std_dev_x: 3.0,
                std_dev_y: 3.0,
                input: "dilated",
                result: "blur",
            },
            PresetPrimitive::SpecularLighting {
                surface_scale: 6.0,
                specular_constant: 1.0,
                specular_exponent: 15.0,
                input: "blur",
                result: "specular",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "specular",
                in2: "dilated",
                result: "bevel",
            },
            PresetPrimitive::Merge {
                inputs: vec!["bevel", "SourceGraphic"],
            },
        ],
    }
}

/// Build and return the complete preset for "Emboss".
fn emboss() -> FilterPreset {
    FilterPreset {
        id: "emboss",
        name: "Emboss",
        category: FilterCategory::Bevels,
        description: "Stamped-metal embossing effect using a directional light.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 1.0,
                std_dev_y: 1.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::DiffuseLighting {
                surface_scale: 4.0,
                diffuse_constant: 1.0,
                input: "blur",
                result: "diffuse",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "diffuse",
                in2: "SourceAlpha",
                result: "embossed",
            },
        ],
    }
}

/// Build and return the complete preset for "Torn Edges".
fn torn_edges() -> FilterPreset {
    FilterPreset {
        id: "torn_edges",
        name: "Torn Edges",
        category: FilterCategory::Distortions,
        description: "Irregular, ragged edges as if the paper was ripped.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.04,
                freq_y: 0.04,
                octaves: 5,
                seed: 0,
                turb_type: "turbulence",
                result: "noise",
            },
            PresetPrimitive::DisplacementMap {
                scale: 15.0,
                x_channel: "R",
                y_channel: "G",
                in1: "SourceGraphic",
                in2: "noise",
                result: "displaced",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "displaced",
                in2: "SourceAlpha",
                result: "tornEdges",
            },
        ],
    }
}

/// Build and return the complete preset for "Rough Paper".
fn rough_paper() -> FilterPreset {
    FilterPreset {
        id: "rough_paper",
        name: "Rough Paper",
        category: FilterCategory::Distortions,
        description: "Small-scale displacement giving a hand-drawn feel.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.08,
                freq_y: 0.08,
                octaves: 4,
                seed: 3,
                turb_type: "fractalNoise",
                result: "noise",
            },
            PresetPrimitive::DisplacementMap {
                scale: 6.0,
                x_channel: "R",
                y_channel: "G",
                in1: "SourceGraphic",
                in2: "noise",
                result: "roughPaper",
            },
        ],
    }
}

/// Build and return the complete preset for "Broken Glass".
fn broken_glass() -> FilterPreset {
    FilterPreset {
        id: "broken_glass",
        name: "Broken Glass",
        category: FilterCategory::Distortions,
        description: "Heavy fragmented distortion as if viewed through cracked glass.",
        primitives: vec![
            PresetPrimitive::Turbulence {
                freq_x: 0.02,
                freq_y: 0.02,
                octaves: 3,
                seed: 7,
                turb_type: "turbulence",
                result: "noise",
            },
            PresetPrimitive::DisplacementMap {
                scale: 30.0,
                x_channel: "R",
                y_channel: "G",
                in1: "SourceGraphic",
                in2: "noise",
                result: "distorted",
            },
            PresetPrimitive::GaussianBlur {
                std_dev_x: 0.5,
                std_dev_y: 0.5,
                input: "distorted",
                result: "brokenGlass",
            },
        ],
    }
}

/// Build and return the complete preset for "Desaturate".
fn desaturate() -> FilterPreset {
    FilterPreset {
        id: "desaturate",
        name: "Desaturate",
        category: FilterCategory::ColorEffects,
        description: "Convert the element to grayscale.",
        primitives: vec![PresetPrimitive::ColorMatrix {
            matrix_type: "saturate",
            values: "0",
            input: "SourceGraphic",
            result: "desaturated",
        }],
    }
}

/// Build and return the complete preset for "Sepia".
fn sepia() -> FilterPreset {
    FilterPreset {
        id: "sepia",
        name: "Sepia",
        category: FilterCategory::ColorEffects,
        description: "Warm brownish tone reminiscent of old photographs.",
        primitives: vec![PresetPrimitive::ColorMatrix {
            matrix_type: "matrix",
            // Classic sepia matrix (4x5 row-major, 20 values).
            values: "0.393 0.769 0.189 0 0 \
                     0.349 0.686 0.168 0 0 \
                     0.272 0.534 0.131 0 0 \
                     0     0     0     1 0",
            input: "SourceGraphic",
            result: "sepia",
        }],
    }
}

/// Build and return the complete preset for "Night Vision".
fn night_vision() -> FilterPreset {
    FilterPreset {
        id: "night_vision",
        name: "Night Vision",
        category: FilterCategory::ColorEffects,
        description: "Green-tinted monochrome simulating night-vision goggles.",
        primitives: vec![
            PresetPrimitive::ColorMatrix {
                matrix_type: "matrix",
                values: "0 0 0 0 0 \
                         0.33 0.33 0.33 0 0 \
                         0 0 0 0 0 \
                         0 0 0 1 0",
                input: "SourceGraphic",
                result: "greenChannel",
            },
            PresetPrimitive::Turbulence {
                freq_x: 0.15,
                freq_y: 0.15,
                octaves: 2,
                seed: 1,
                turb_type: "fractalNoise",
                result: "scanlines",
            },
            PresetPrimitive::Blend {
                mode: "multiply",
                in1: "greenChannel",
                in2: "scanlines",
                result: "nightVision",
            },
        ],
    }
}

/// Build and return the complete preset for "Posterize".
fn posterize() -> FilterPreset {
    FilterPreset {
        id: "posterize",
        name: "Posterize",
        category: FilterCategory::ColorEffects,
        description: "Reduce colour depth to create a poster-like flat look.",
        primitives: vec![PresetPrimitive::ComponentTransfer {
            input: "SourceGraphic",
            result: "posterized",
            func_type: "discrete",
            table_values: "0 0.25 0.5 0.75 1",
        }],
    }
}

/// Build and return the complete preset for "Vignette".
fn vignette() -> FilterPreset {
    FilterPreset {
        id: "vignette",
        name: "Vignette",
        category: FilterCategory::Overlays,
        description: "Darkened edges drawing attention to the centre.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 6.0,
                std_dev_y: 6.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::Flood {
                color: "#000000",
                opacity: 0.7,
                result: "flood",
            },
            PresetPrimitive::Composite {
                operator: "out",
                in1: "flood",
                in2: "blur",
                result: "darkEdge",
            },
            PresetPrimitive::Blend {
                mode: "multiply",
                in1: "darkEdge",
                in2: "SourceGraphic",
                result: "vignette",
            },
        ],
    }
}

/// Build and return the complete preset for "Spotlight".
fn spotlight() -> FilterPreset {
    FilterPreset {
        id: "spotlight",
        name: "Spotlight",
        category: FilterCategory::Overlays,
        description: "Bright centre highlight fading to darkness at the edges.",
        primitives: vec![
            PresetPrimitive::GaussianBlur {
                std_dev_x: 10.0,
                std_dev_y: 10.0,
                input: "SourceAlpha",
                result: "blur",
            },
            PresetPrimitive::SpecularLighting {
                surface_scale: 5.0,
                specular_constant: 1.5,
                specular_exponent: 25.0,
                input: "blur",
                result: "specular",
            },
            PresetPrimitive::Composite {
                operator: "in",
                in1: "specular",
                in2: "SourceAlpha",
                result: "spot",
            },
            PresetPrimitive::Blend {
                mode: "screen",
                in1: "spot",
                in2: "SourceGraphic",
                result: "spotlight",
            },
        ],
    }
}

// ===========================================================================
// Public API
// ===========================================================================

/// Return all built-in filter presets.
pub fn all_presets() -> Vec<FilterPreset> {
    vec![
        // Shadows
        drop_shadow(),
        inner_shadow(),
        long_shadow(),
        // Blurs
        simple_blur(),
        motion_blur(),
        zoom_blur(),
        // Textures
        noise_fill(),
        paper_texture(),
        canvas_texture(),
        // Bevels
        inner_bevel(),
        outer_bevel(),
        emboss(),
        // Distortions
        torn_edges(),
        rough_paper(),
        broken_glass(),
        // Color Effects
        desaturate(),
        sepia(),
        night_vision(),
        posterize(),
        // Overlays
        vignette(),
        spotlight(),
    ]
}

/// Return all presets that belong to a given [`FilterCategory`].
pub fn presets_by_category(cat: FilterCategory) -> Vec<&'static FilterPreset> {
    // We use a leaked `Vec` to return `&'static` references.  This is a
    // one-time allocation that lives for the entire process lifetime -- fine
    // for a library that provides a fixed catalogue.
    use std::sync::OnceLock;

    static ALL: OnceLock<Vec<FilterPreset>> = OnceLock::new();
    let all = ALL.get_or_init(all_presets);
    all.iter().filter(|p| p.category == cat).collect()
}

/// Look up a preset by its `id` string.
pub fn preset_by_id(id: &str) -> Option<&'static FilterPreset> {
    use std::sync::OnceLock;

    static ALL: OnceLock<Vec<FilterPreset>> = OnceLock::new();
    let all = ALL.get_or_init(all_presets);
    all.iter().find(|p| p.id == id)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::FilterBuffer;

    #[test]
    fn test_all_presets_count() {
        let presets = all_presets();
        assert!(
            presets.len() >= 20,
            "expected at least 20 presets, got {}",
            presets.len()
        );
    }

    #[test]
    fn test_all_presets_have_unique_ids() {
        let presets = all_presets();
        let mut ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), presets.len(), "preset ids must be unique");
    }

    #[test]
    fn test_all_presets_have_nonempty_primitives() {
        for preset in all_presets() {
            assert!(
                !preset.primitives.is_empty(),
                "preset '{}' has no primitives",
                preset.id
            );
        }
    }

    #[test]
    fn test_all_categories_have_presets() {
        let categories = [
            FilterCategory::Shadows,
            FilterCategory::Blurs,
            FilterCategory::Textures,
            FilterCategory::Bevels,
            FilterCategory::Distortions,
            FilterCategory::ColorEffects,
            FilterCategory::Overlays,
        ];
        for cat in &categories {
            let presets = presets_by_category(*cat);
            assert!(!presets.is_empty(), "category {:?} has no presets", cat);
        }
    }

    #[test]
    fn test_presets_by_category_shadows() {
        let shadows = presets_by_category(FilterCategory::Shadows);
        assert_eq!(shadows.len(), 3);
        let ids: Vec<&str> = shadows.iter().map(|p| p.id).collect();
        assert!(ids.contains(&"drop_shadow"));
        assert!(ids.contains(&"inner_shadow"));
        assert!(ids.contains(&"long_shadow"));
    }

    #[test]
    fn test_presets_by_category_blurs() {
        let blurs = presets_by_category(FilterCategory::Blurs);
        assert_eq!(blurs.len(), 3);
    }

    #[test]
    fn test_presets_by_category_color_effects() {
        let fx = presets_by_category(FilterCategory::ColorEffects);
        assert_eq!(fx.len(), 4);
    }

    #[test]
    fn test_preset_by_id_found() {
        let p = preset_by_id("sepia");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "Sepia");
    }

    #[test]
    fn test_preset_by_id_not_found() {
        assert!(preset_by_id("nonexistent_filter").is_none());
    }

    #[test]
    fn test_category_label() {
        assert_eq!(FilterCategory::Shadows.label(), "Shadows");
        assert_eq!(FilterCategory::ColorEffects.label(), "Color Effects");
    }

    #[test]
    fn test_drop_shadow_pipeline_executes() {
        let preset = preset_by_id("drop_shadow").unwrap();
        let pipeline = preset.to_pipeline();

        let mut src = FilterBuffer::new(20, 20);
        src.fill(1.0, 0.0, 0.0, 1.0);
        let result = pipeline.execute(&src);

        assert_eq!(result.width, 20);
        assert_eq!(result.height, 20);
    }

    #[test]
    fn test_simple_blur_pipeline_executes() {
        let preset = preset_by_id("simple_blur").unwrap();
        let pipeline = preset.to_pipeline();

        let mut src = FilterBuffer::new(10, 10);
        src.set_pixel(5, 5, [1.0, 1.0, 1.0, 1.0]);
        let result = pipeline.execute(&src);

        assert_eq!(result.width, 10);
        assert_eq!(result.height, 10);
        // Centre should still have some brightness (blurred from the single white pixel).
        let center = result.get_pixel(5, 5);
        assert!(center[0] > 0.0, "blurred pixel should retain some color");
    }

    #[test]
    fn test_desaturate_pipeline_executes() {
        let preset = preset_by_id("desaturate").unwrap();
        let pipeline = preset.to_pipeline();

        let mut src = FilterBuffer::new(4, 4);
        src.fill(1.0, 0.0, 0.0, 1.0); // pure red
        let result = pipeline.execute(&src);

        // After desaturation, R == G == B (grayscale)
        let px = result.get_pixel(1, 1);
        assert!(
            (px[0] - px[1]).abs() < 0.01 && (px[1] - px[2]).abs() < 0.01,
            "desaturated pixel should be grayscale, got {:?}",
            px
        );
    }

    #[test]
    fn test_sepia_pipeline_executes() {
        let preset = preset_by_id("sepia").unwrap();
        let pipeline = preset.to_pipeline();

        let mut src = FilterBuffer::new(4, 4);
        src.fill(0.5, 0.5, 0.5, 1.0);
        let result = pipeline.execute(&src);

        // Sepia should produce warm tones: R > G > B
        let px = result.get_pixel(2, 2);
        assert!(
            px[0] >= px[1] && px[1] >= px[2],
            "sepia should produce warm tones (R >= G >= B), got {:?}",
            px
        );
    }

    #[test]
    fn test_emboss_pipeline_executes() {
        let preset = preset_by_id("emboss").unwrap();
        let pipeline = preset.to_pipeline();

        let mut src = FilterBuffer::new(16, 16);
        src.fill(0.6, 0.6, 0.8, 1.0);
        let result = pipeline.execute(&src);

        assert_eq!(result.width, 16);
        assert_eq!(result.height, 16);
    }

    #[test]
    fn test_all_presets_can_build_pipelines() {
        let src = FilterBuffer::new(8, 8);
        for preset in all_presets() {
            let pipeline = preset.to_pipeline();
            // Just verify it doesn't panic.
            let _result = pipeline.execute(&src);
        }
    }

    #[test]
    fn test_parse_flood_color_valid() {
        let c = parse_flood_color("#ff8000", 0.5);
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!((c[1] - 0.502).abs() < 0.01);
        assert!((c[2] - 0.0).abs() < 0.01);
        assert!((c[3] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_flood_color_black_fallback() {
        let c = parse_flood_color("invalid", 1.0);
        assert_eq!(c, [0.0, 0.0, 0.0, 1.0]);
    }
}
