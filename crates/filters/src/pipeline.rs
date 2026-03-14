//! Filter pipeline -- chain of filter primitives operating on named buffers.

use std::collections::HashMap;

use crate::blend::{self, BlendMode};
use crate::buffer::FilterBuffer;
use crate::color_matrix::{self, ColorMatrixType};
use crate::component_transfer::{self, ComponentTransferFunctions};
use crate::composite::{self, CompositeOp};
use crate::convolve_matrix::{self, EdgeMode};
use crate::diffuse_lighting::{self, LightSource};
use crate::displacement_map::{self, ChannelSelector};
use crate::drop_shadow;
use crate::flood;
use crate::gaussian_blur;
use crate::image::{self, ImageFit};
use crate::merge;
use crate::morphology::{self, MorphologyOp};
use crate::offset;
use crate::region::{clip_buffer_to_region, ComputedFilterRegion, PrimitiveRegionParams};
use crate::specular_lighting;
use crate::tile;
use crate::turbulence::{self, TurbulenceType};

/// A single filter primitive in the pipeline.
#[derive(Debug, Clone)]
pub enum FilterPrimitive {
    GaussianBlur {
        input: String,
        std_dev_x: f64,
        std_dev_y: f64,
        result: String,
        /// Optional sub-region parameters for this primitive.
        sub_region: Option<PrimitiveRegionParams>,
    },
    Offset {
        input: String,
        dx: f64,
        dy: f64,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Flood {
        color: [f32; 4],
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Blend {
        input1: String,
        input2: String,
        mode: BlendMode,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Composite {
        input1: String,
        input2: String,
        op: CompositeOp,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    ColorMatrix {
        input: String,
        matrix_type: ColorMatrixType,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Merge {
        inputs: Vec<String>,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Morphology {
        input: String,
        op: MorphologyOp,
        radius_x: f64,
        radius_y: f64,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Turbulence {
        turb_type: TurbulenceType,
        base_freq_x: f64,
        base_freq_y: f64,
        num_octaves: u32,
        seed: i32,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    DropShadow {
        input: String,
        dx: f64,
        dy: f64,
        std_dev: f64,
        color: [f32; 4],
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    ComponentTransfer {
        input: String,
        functions: ComponentTransferFunctions,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    ConvolveMatrix {
        input: String,
        kernel: Vec<f32>,
        order_x: u32,
        order_y: u32,
        divisor: f32,
        bias: f32,
        target_x: u32,
        target_y: u32,
        edge_mode: EdgeMode,
        preserve_alpha: bool,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    DisplacementMap {
        input1: String,
        input2: String,
        scale: f64,
        x_channel: ChannelSelector,
        y_channel: ChannelSelector,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Tile {
        input: String,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    DiffuseLighting {
        input: String,
        surface_scale: f64,
        diffuse_constant: f64,
        light: LightSource,
        light_color: [f32; 3],
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    SpecularLighting {
        input: String,
        surface_scale: f64,
        specular_constant: f64,
        specular_exponent: f64,
        light: LightSource,
        light_color: [f32; 3],
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
    Image {
        /// Pre-decoded image buffer. The higher-level code resolves the href and
        /// passes the pixel data here. If `None`, a transparent buffer is produced.
        image_data: Option<FilterBuffer>,
        fit: ImageFit,
        result: String,
        sub_region: Option<PrimitiveRegionParams>,
    },
}

impl FilterPrimitive {
    /// Returns the sub-region parameters for this primitive, if any.
    fn sub_region(&self) -> Option<&PrimitiveRegionParams> {
        match self {
            FilterPrimitive::GaussianBlur { sub_region, .. }
            | FilterPrimitive::Offset { sub_region, .. }
            | FilterPrimitive::Flood { sub_region, .. }
            | FilterPrimitive::Blend { sub_region, .. }
            | FilterPrimitive::Composite { sub_region, .. }
            | FilterPrimitive::ColorMatrix { sub_region, .. }
            | FilterPrimitive::Merge { sub_region, .. }
            | FilterPrimitive::Morphology { sub_region, .. }
            | FilterPrimitive::Turbulence { sub_region, .. }
            | FilterPrimitive::DropShadow { sub_region, .. }
            | FilterPrimitive::ComponentTransfer { sub_region, .. }
            | FilterPrimitive::ConvolveMatrix { sub_region, .. }
            | FilterPrimitive::DisplacementMap { sub_region, .. }
            | FilterPrimitive::Tile { sub_region, .. }
            | FilterPrimitive::DiffuseLighting { sub_region, .. }
            | FilterPrimitive::SpecularLighting { sub_region, .. }
            | FilterPrimitive::Image { sub_region, .. } => sub_region.as_ref(),
        }
    }
}

/// A pipeline of filter primitives that operate on named buffers.
pub struct FilterPipeline {
    pub primitives: Vec<FilterPrimitive>,
}

impl FilterPipeline {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self {
            primitives: Vec::new(),
        }
    }

    /// Add a primitive to the pipeline.
    pub fn add(&mut self, primitive: FilterPrimitive) {
        self.primitives.push(primitive);
    }

    /// Execute the filter pipeline without filter region clipping.
    ///
    /// `source_graphic` is the rendered content of the element being filtered.
    /// Returns the final output buffer.
    pub fn execute(&self, source_graphic: &FilterBuffer) -> FilterBuffer {
        self.execute_impl(source_graphic, None)
    }

    /// Execute the filter pipeline with filter region clipping.
    ///
    /// `source_graphic` is the rendered content of the element being filtered.
    /// `filter_region` provides the computed filter region and primitive sub-region
    /// coordinate system. Each primitive's output is clipped to its computed
    /// sub-region, and the final result is clipped to the overall filter region.
    pub fn execute_with_region(
        &self,
        source_graphic: &FilterBuffer,
        filter_region: &ComputedFilterRegion,
    ) -> FilterBuffer {
        self.execute_impl(source_graphic, Some(filter_region))
    }

    /// Internal execution implementation supporting optional region clipping.
    fn execute_impl(
        &self,
        source_graphic: &FilterBuffer,
        filter_region: Option<&ComputedFilterRegion>,
    ) -> FilterBuffer {
        let mut named_buffers: HashMap<String, FilterBuffer> = HashMap::new();
        let w = source_graphic.width;
        let h = source_graphic.height;

        // SourceGraphic: the original rendered element
        named_buffers.insert("SourceGraphic".to_string(), source_graphic.clone());

        // SourceAlpha: same dimensions, RGB zeroed, alpha preserved
        let mut source_alpha = source_graphic.clone();
        let pixel_count = (w as usize) * (h as usize);
        for i in 0..pixel_count {
            let idx = i * 4;
            source_alpha.data[idx] = 0.0;
            source_alpha.data[idx + 1] = 0.0;
            source_alpha.data[idx + 2] = 0.0;
            // alpha unchanged
        }
        named_buffers.insert("SourceAlpha".to_string(), source_alpha);

        let mut last_result = String::from("SourceGraphic");

        for prim in &self.primitives {
            let (result_name, mut output) =
                self.execute_primitive(prim, &named_buffers, source_graphic, &last_result, w, h);

            // If a filter region is provided, clip the primitive output to its
            // computed sub-region (or the overall filter region if no sub-region
            // is specified on the primitive).
            if let Some(region) = filter_region {
                let sub_params = prim.sub_region().cloned().unwrap_or_default();
                let sub_rect = region.compute_primitive_subregion(&sub_params);
                output = clip_buffer_to_region(&output, &sub_rect);
            }

            last_result = result_name.clone();
            named_buffers.insert(result_name, output);
        }

        let mut final_buf = named_buffers
            .remove(&last_result)
            .unwrap_or_else(|| source_graphic.clone());

        // Clip the final result to the overall filter region.
        if let Some(region) = filter_region {
            final_buf = clip_buffer_to_region(&final_buf, &region.region);
        }

        final_buf
    }

    /// Execute a single filter primitive and return (result_name, output_buffer).
    fn execute_primitive(
        &self,
        prim: &FilterPrimitive,
        named_buffers: &HashMap<String, FilterBuffer>,
        source_graphic: &FilterBuffer,
        last_result: &str,
        w: u32,
        h: u32,
    ) -> (String, FilterBuffer) {
        match prim {
            FilterPrimitive::GaussianBlur {
                input,
                std_dev_x,
                std_dev_y,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = gaussian_blur::gaussian_blur(&input_buf, *std_dev_x, *std_dev_y);
                (result.clone(), output)
            }
            FilterPrimitive::Offset {
                input,
                dx,
                dy,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = offset::offset(&input_buf, *dx, *dy);
                (result.clone(), output)
            }
            FilterPrimitive::Flood { color, result, .. } => {
                let output = flood::flood(w, h, color[0], color[1], color[2], color[3]);
                (result.clone(), output)
            }
            FilterPrimitive::Blend {
                input1,
                input2,
                mode,
                result,
                ..
            } => {
                let name1 = resolve_input(input1, last_result);
                let name2 = resolve_input(input2, last_result);
                let buf1 = get_buffer(named_buffers, &name1, source_graphic);
                let buf2 = get_buffer(named_buffers, &name2, source_graphic);
                let output = blend::blend(&buf1, &buf2, *mode);
                (result.clone(), output)
            }
            FilterPrimitive::Composite {
                input1,
                input2,
                op,
                result,
                ..
            } => {
                let name1 = resolve_input(input1, last_result);
                let name2 = resolve_input(input2, last_result);
                let buf1 = get_buffer(named_buffers, &name1, source_graphic);
                let buf2 = get_buffer(named_buffers, &name2, source_graphic);
                let output = composite::composite(&buf1, &buf2, op.clone());
                (result.clone(), output)
            }
            FilterPrimitive::ColorMatrix {
                input,
                matrix_type,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = color_matrix::color_matrix(&input_buf, matrix_type.clone());
                (result.clone(), output)
            }
            FilterPrimitive::Merge { inputs, result, .. } => {
                let bufs: Vec<FilterBuffer> = inputs
                    .iter()
                    .map(|name| {
                        let resolved = resolve_input(name, last_result);
                        get_buffer(named_buffers, &resolved, source_graphic)
                    })
                    .collect();
                let refs: Vec<&FilterBuffer> = bufs.iter().collect();
                let output = merge::merge(&refs);
                (result.clone(), output)
            }
            FilterPrimitive::Morphology {
                input,
                op,
                radius_x,
                radius_y,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = morphology::morphology(&input_buf, *op, *radius_x, *radius_y);
                (result.clone(), output)
            }
            FilterPrimitive::Turbulence {
                turb_type,
                base_freq_x,
                base_freq_y,
                num_octaves,
                seed,
                result,
                ..
            } => {
                let output = turbulence::turbulence(
                    w,
                    h,
                    *base_freq_x,
                    *base_freq_y,
                    *num_octaves,
                    *seed,
                    *turb_type,
                    false,
                );
                (result.clone(), output)
            }
            FilterPrimitive::DropShadow {
                input,
                dx,
                dy,
                std_dev,
                color,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = drop_shadow::drop_shadow(&input_buf, *dx, *dy, *std_dev, *color);
                (result.clone(), output)
            }
            FilterPrimitive::ComponentTransfer {
                input,
                functions,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = component_transfer::component_transfer(&input_buf, functions);
                (result.clone(), output)
            }
            FilterPrimitive::ConvolveMatrix {
                input,
                kernel,
                order_x,
                order_y,
                divisor,
                bias,
                target_x,
                target_y,
                edge_mode,
                preserve_alpha,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = convolve_matrix::convolve_matrix(
                    &input_buf,
                    kernel,
                    *order_x,
                    *order_y,
                    *divisor,
                    *bias,
                    *target_x,
                    *target_y,
                    *edge_mode,
                    *preserve_alpha,
                );
                (result.clone(), output)
            }
            FilterPrimitive::DisplacementMap {
                input1,
                input2,
                scale,
                x_channel,
                y_channel,
                result,
                ..
            } => {
                let name1 = resolve_input(input1, last_result);
                let name2 = resolve_input(input2, last_result);
                let buf1 = get_buffer(named_buffers, &name1, source_graphic);
                let buf2 = get_buffer(named_buffers, &name2, source_graphic);
                let output = displacement_map::displacement_map(
                    &buf1, &buf2, *scale, *x_channel, *y_channel,
                );
                (result.clone(), output)
            }
            FilterPrimitive::Tile { input, result, .. } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = tile::tile(&input_buf, w, h);
                (result.clone(), output)
            }
            FilterPrimitive::DiffuseLighting {
                input,
                surface_scale,
                diffuse_constant,
                light,
                light_color,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = diffuse_lighting::diffuse_lighting(
                    &input_buf,
                    *surface_scale,
                    *diffuse_constant,
                    light,
                    *light_color,
                );
                (result.clone(), output)
            }
            FilterPrimitive::SpecularLighting {
                input,
                surface_scale,
                specular_constant,
                specular_exponent,
                light,
                light_color,
                result,
                ..
            } => {
                let input_name = resolve_input(input, last_result);
                let input_buf = get_buffer(named_buffers, &input_name, source_graphic);
                let output = specular_lighting::specular_lighting(
                    &input_buf,
                    *surface_scale,
                    *specular_constant,
                    *specular_exponent,
                    light,
                    *light_color,
                );
                (result.clone(), output)
            }
            FilterPrimitive::Image {
                image_data,
                fit,
                result,
                ..
            } => {
                let output = match image_data {
                    Some(src) => image::image_from_buffer(src, w, h, *fit),
                    None => FilterBuffer::new(w, h),
                };
                (result.clone(), output)
            }
        }
    }
}

impl Default for FilterPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve an input name: if empty, use the last result name.
fn resolve_input(input: &str, last_result: &str) -> String {
    if input.is_empty() {
        last_result.to_string()
    } else {
        input.to_string()
    }
}

/// Get a named buffer, falling back to source_graphic if not found.
fn get_buffer(
    named: &HashMap<String, FilterBuffer>,
    name: &str,
    fallback: &FilterBuffer,
) -> FilterBuffer {
    named.get(name).cloned().unwrap_or_else(|| {
        log::warn!("Filter buffer '{}' not found, using SourceGraphic", name);
        fallback.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pipeline_returns_source() {
        let mut buf = FilterBuffer::new(3, 3);
        buf.fill(0.5, 0.5, 0.5, 1.0);

        let pipeline = FilterPipeline::new();
        let result = pipeline.execute(&buf);

        assert_eq!(result.get_pixel(1, 1), [0.5, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn test_pipeline_blur_then_offset() {
        let mut buf = FilterBuffer::new(9, 9);
        buf.set_pixel(4, 4, [1.0, 0.0, 0.0, 1.0]);

        let mut pipeline = FilterPipeline::new();
        pipeline.add(FilterPrimitive::GaussianBlur {
            input: "SourceGraphic".to_string(),
            std_dev_x: 1.0,
            std_dev_y: 1.0,
            result: "blurred".to_string(),
            sub_region: None,
        });
        pipeline.add(FilterPrimitive::Offset {
            input: "blurred".to_string(),
            dx: 2.0,
            dy: 0.0,
            result: "shifted".to_string(),
            sub_region: None,
        });

        let result = pipeline.execute(&buf);

        // After blur, the center area has some color spread.
        // After offset by +2, the brightest area should be shifted right.
        // Pixel at (4,4) should be dimmer, pixel at (6,4) should be brighter.
        let at_original = result.get_pixel(4, 4);
        let at_shifted = result.get_pixel(6, 4);

        // The shifted result's peak should be near (6,4)
        assert!(at_shifted[0] > 0.0, "shifted peak should have some red");
        // The original location is 2 pixels left of the shifted peak
        // so it should be dimmer (or zero depending on blur spread)
        assert!(
            at_shifted[0] >= at_original[0],
            "shifted peak ({}) should be >= original location ({})",
            at_shifted[0],
            at_original[0]
        );
    }

    #[test]
    fn test_pipeline_flood_and_composite() {
        let mut src = FilterBuffer::new(3, 3);
        src.set_pixel(1, 1, [1.0, 1.0, 1.0, 1.0]); // white center

        let mut pipeline = FilterPipeline::new();

        // Flood with semi-transparent red
        pipeline.add(FilterPrimitive::Flood {
            color: [1.0, 0.0, 0.0, 0.5],
            result: "red".to_string(),
            sub_region: None,
        });

        // Composite SourceGraphic over the red flood
        pipeline.add(FilterPrimitive::Composite {
            input1: "SourceGraphic".to_string(),
            input2: "red".to_string(),
            op: CompositeOp::Over,
            result: "final".to_string(),
            sub_region: None,
        });

        let result = pipeline.execute(&src);

        // Center pixel: opaque white over semi-transparent red = white
        let center = result.get_pixel(1, 1);
        assert!((center[0] - 1.0).abs() < 0.01);
        assert!((center[3] - 1.0).abs() < 0.01);

        // Corner pixel: transparent over semi-transparent red = red at 0.5 alpha
        let corner = result.get_pixel(0, 0);
        assert!((corner[0] - 1.0).abs() < 0.01); // red channel
        assert!((corner[3] - 0.5).abs() < 0.01); // alpha
    }

    #[test]
    fn test_pipeline_uses_implicit_input() {
        // When input is empty, should use the last result
        let mut buf = FilterBuffer::new(5, 5);
        buf.set_pixel(2, 2, [1.0, 0.0, 0.0, 1.0]);

        let mut pipeline = FilterPipeline::new();

        // First operation: blur SourceGraphic
        pipeline.add(FilterPrimitive::GaussianBlur {
            input: "SourceGraphic".to_string(),
            std_dev_x: 1.0,
            std_dev_y: 1.0,
            result: "step1".to_string(),
            sub_region: None,
        });

        // Second operation: use implicit input (should be "step1")
        pipeline.add(FilterPrimitive::Offset {
            input: "".to_string(),
            dx: 1.0,
            dy: 0.0,
            result: "step2".to_string(),
            sub_region: None,
        });

        let result = pipeline.execute(&buf);

        // Should have blurred and then shifted right
        let px = result.get_pixel(3, 2);
        assert!(px[0] > 0.0, "implicit chaining should work");
    }

    #[test]
    fn test_pipeline_drop_shadow() {
        let mut buf = FilterBuffer::new(11, 11);
        buf.set_pixel(5, 5, [1.0, 1.0, 1.0, 1.0]);

        let mut pipeline = FilterPipeline::new();
        pipeline.add(FilterPrimitive::DropShadow {
            input: "SourceGraphic".to_string(),
            dx: 2.0,
            dy: 2.0,
            std_dev: 1.0,
            color: [0.0, 0.0, 0.0, 1.0],
            result: "shadow".to_string(),
            sub_region: None,
        });

        let result = pipeline.execute(&buf);

        // Original pixel should be visible
        let center = result.get_pixel(5, 5);
        assert!(center[3] > 0.5);

        // Shadow region should have something
        let shadow = result.get_pixel(7, 7);
        assert!(shadow[3] > 0.0, "shadow area should have opacity");
    }
}
