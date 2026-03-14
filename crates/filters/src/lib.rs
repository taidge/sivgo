//! `sivgo-filters` -- SVG filter primitive execution engine.
//!
//! This crate provides pixel-buffer-based implementations of SVG filter primitives
//! (feGaussianBlur, feOffset, feFlood, feBlend, feComposite, feColorMatrix,
//! feComponentTransfer, feConvolveMatrix, feDiffuseLighting, feSpecularLighting,
//! feDisplacementMap, feTile, feImage, feMorphology, feTurbulence, feMerge,
//! feDropShadow) and a pipeline to chain them together using named intermediate buffers.

pub mod blend;
pub mod buffer;
pub mod color_matrix;
pub mod component_transfer;
pub mod composite;
pub mod convolve_matrix;
pub mod diffuse_lighting;
pub mod displacement_map;
pub mod drop_shadow;
pub mod flood;
pub mod gaussian_blur;
pub mod image;
pub mod merge;
pub mod morphology;
pub mod offset;
pub mod pipeline;
pub mod presets;
pub mod region;
pub mod specular_lighting;
pub mod tile;
pub mod turbulence;

// Re-export primary types at the crate root for convenience.
pub use blend::BlendMode;
pub use buffer::FilterBuffer;
pub use color_matrix::ColorMatrixType;
pub use component_transfer::{ComponentTransferFunctions, TransferFunction};
pub use composite::CompositeOp;
pub use convolve_matrix::EdgeMode;
pub use diffuse_lighting::LightSource;
pub use displacement_map::ChannelSelector;
pub use image::ImageFit;
pub use morphology::MorphologyOp;
pub use pipeline::{FilterPipeline, FilterPrimitive};
pub use region::{
    clip_buffer_to_region, compute_filter_region, AbsoluteRect, ComputedFilterRegion, FilterLength,
    FilterRegionParams, FilterUnits, PrimitiveRegionParams,
};
pub use turbulence::TurbulenceType;

// Preset library
pub use presets::{
    all_presets, preset_by_id, presets_by_category, FilterCategory, FilterPreset, PresetPrimitive,
};
