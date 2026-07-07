//! Filter region and coordinate system computation.
//!
//! In SVG, the `<filter>` element defines a **filter region** via the attributes
//! `x`, `y`, `width`, `height` (default: x="-10%", y="-10%", width="120%",
//! height="120%" when `filterUnits="objectBoundingBox"`). Each filter primitive
//! can also specify its own **sub-region** that clips its output.
//!
//! This module provides the types and logic for computing these regions in
//! absolute (user-space pixel) coordinates, given the element's bounding box
//! and the coordinate-system settings from the `<filter>` element.

use crate::buffer::FilterBuffer;

/// Coordinate unit system for filter region attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterUnits {
    /// Coordinates are fractions/percentages of the element's bounding box.
    /// x=0, y=0 is the top-left of the bbox; width=1, height=1 covers it fully.
    #[default]
    ObjectBoundingBox,
    /// Coordinates are in the user coordinate system (absolute pixels).
    UserSpaceOnUse,
}

/// An axis-aligned rectangle in absolute pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsoluteRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl AbsoluteRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the right edge (x + width).
    pub fn x2(&self) -> f64 {
        self.x + self.width
    }

    /// Returns the bottom edge (y + height).
    pub fn y2(&self) -> f64 {
        self.y + self.height
    }

    /// Intersect this rect with another, returning the overlapping area.
    pub fn intersect(&self, other: &AbsoluteRect) -> AbsoluteRect {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.x2().min(other.x2());
        let y1 = self.y2().min(other.y2());
        let w = (x1 - x0).max(0.0);
        let h = (y1 - y0).max(0.0);
        AbsoluteRect::new(x0, y0, w, h)
    }
}

/// Length value that may be expressed as a percentage or an absolute value.
///
/// When used with `ObjectBoundingBox` units, percentage values (and bare
/// numbers) are relative to the bounding box dimensions. When used with
/// `UserSpaceOnUse`, values are absolute pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterLength {
    /// An absolute value in user-space pixels.
    Absolute(f64),
    /// A percentage value (e.g. 120 means 120%).
    Percent(f64),
}

impl FilterLength {
    /// Resolve this length to an absolute pixel value.
    ///
    /// - `bbox_origin`: the origin (x or y) of the element's bounding box
    /// - `bbox_extent`: the size (width or height) of the element's bounding box
    /// - `units`: the coordinate system in effect
    ///
    /// For position values (x, y):
    ///   - ObjectBoundingBox: value is a fraction of bbox_extent, offset from bbox_origin
    ///   - UserSpaceOnUse: value is absolute
    ///
    /// For size values (width, height):
    ///   - ObjectBoundingBox: value is a fraction of bbox_extent
    ///   - UserSpaceOnUse: value is absolute
    pub fn resolve_position(&self, bbox_origin: f64, bbox_extent: f64, units: FilterUnits) -> f64 {
        match units {
            FilterUnits::ObjectBoundingBox => {
                let frac = match self {
                    FilterLength::Absolute(v) => *v,
                    FilterLength::Percent(v) => *v / 100.0,
                };
                bbox_origin + frac * bbox_extent
            }
            FilterUnits::UserSpaceOnUse => match self {
                FilterLength::Absolute(v) => *v,
                FilterLength::Percent(v) => *v, // percentage in userSpaceOnUse: treat as-is
            },
        }
    }

    pub fn resolve_size(&self, bbox_extent: f64, units: FilterUnits) -> f64 {
        match units {
            FilterUnits::ObjectBoundingBox => {
                let frac = match self {
                    FilterLength::Absolute(v) => *v,
                    FilterLength::Percent(v) => *v / 100.0,
                };
                frac * bbox_extent
            }
            FilterUnits::UserSpaceOnUse => match self {
                FilterLength::Absolute(v) => *v,
                FilterLength::Percent(v) => *v,
            },
        }
    }
}

/// Parameters for computing the filter region from a `<filter>` element.
#[derive(Debug, Clone)]
pub struct FilterRegionParams {
    /// The x attribute of the filter element. Default: -10% for objectBoundingBox.
    pub x: Option<FilterLength>,
    /// The y attribute of the filter element. Default: -10% for objectBoundingBox.
    pub y: Option<FilterLength>,
    /// The width attribute of the filter element. Default: 120% for objectBoundingBox.
    pub width: Option<FilterLength>,
    /// The height attribute of the filter element. Default: 120% for objectBoundingBox.
    pub height: Option<FilterLength>,
    /// The filterUnits attribute. Default: objectBoundingBox.
    pub filter_units: FilterUnits,
    /// The primitiveUnits attribute. Default: userSpaceOnUse.
    pub primitive_units: FilterUnits,
}

impl Default for FilterRegionParams {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: None,
            height: None,
            filter_units: FilterUnits::ObjectBoundingBox,
            primitive_units: FilterUnits::UserSpaceOnUse,
        }
    }
}

/// Parameters for a single filter primitive's sub-region.
#[derive(Debug, Clone, Default)]
pub struct PrimitiveRegionParams {
    pub x: Option<FilterLength>,
    pub y: Option<FilterLength>,
    pub width: Option<FilterLength>,
    pub height: Option<FilterLength>,
}

/// The computed filter region in absolute pixel coordinates, plus the
/// primitive coordinate system for sub-region computation.
#[derive(Debug, Clone)]
pub struct ComputedFilterRegion {
    /// The overall filter region in absolute pixel coordinates.
    pub region: AbsoluteRect,
    /// The element's bounding box (needed for primitiveUnits=objectBoundingBox).
    pub element_bbox: AbsoluteRect,
    /// The primitiveUnits setting.
    pub primitive_units: FilterUnits,
}

impl ComputedFilterRegion {
    /// Compute a filter primitive's sub-region.
    ///
    /// If the primitive has explicit x/y/width/height, those are resolved
    /// using `primitiveUnits` against the element bbox. If not specified,
    /// the sub-region defaults to the overall filter region.
    ///
    /// The result is always intersected with the overall filter region.
    pub fn compute_primitive_subregion(&self, params: &PrimitiveRegionParams) -> AbsoluteRect {
        let units = self.primitive_units;
        let bbox = &self.element_bbox;

        let has_any = params.x.is_some()
            || params.y.is_some()
            || params.width.is_some()
            || params.height.is_some();

        if !has_any {
            // No sub-region specified: use the full filter region.
            return self.region;
        }

        let x = params
            .x
            .map(|l| l.resolve_position(bbox.x, bbox.width, units))
            .unwrap_or(self.region.x);
        let y = params
            .y
            .map(|l| l.resolve_position(bbox.y, bbox.height, units))
            .unwrap_or(self.region.y);
        let w = params
            .width
            .map(|l| l.resolve_size(bbox.width, units))
            .unwrap_or(self.region.width);
        let h = params
            .height
            .map(|l| l.resolve_size(bbox.height, units))
            .unwrap_or(self.region.height);

        let sub = AbsoluteRect::new(x, y, w, h);
        // Clip to the overall filter region.
        sub.intersect(&self.region)
    }
}

/// Compute the filter region from filter parameters and the element's bounding box.
///
/// The element bounding box should be in user-space pixel coordinates
/// (post-transform).
///
/// # SVG defaults
///
/// When `filterUnits="objectBoundingBox"` (the default):
///   - x defaults to -10%  (-0.1 as a fraction)
///   - y defaults to -10%
///   - width defaults to 120% (1.2 as a fraction)
///   - height defaults to 120%
///
/// This gives a 10% padding on all sides of the element's bounding box.
pub fn compute_filter_region(
    params: &FilterRegionParams,
    element_bbox: AbsoluteRect,
) -> ComputedFilterRegion {
    let units = params.filter_units;

    // SVG spec defaults for objectBoundingBox:
    let default_x = FilterLength::Percent(-10.0);
    let default_y = FilterLength::Percent(-10.0);
    let default_w = FilterLength::Percent(120.0);
    let default_h = FilterLength::Percent(120.0);

    let x =
        params
            .x
            .unwrap_or(default_x)
            .resolve_position(element_bbox.x, element_bbox.width, units);
    let y =
        params
            .y
            .unwrap_or(default_y)
            .resolve_position(element_bbox.y, element_bbox.height, units);
    let w = params
        .width
        .unwrap_or(default_w)
        .resolve_size(element_bbox.width, units);
    let h = params
        .height
        .unwrap_or(default_h)
        .resolve_size(element_bbox.height, units);

    ComputedFilterRegion {
        region: AbsoluteRect::new(x, y, w.max(0.0), h.max(0.0)),
        element_bbox,
        primitive_units: params.primitive_units,
    }
}

/// Clip a `FilterBuffer` to a sub-region within the canvas.
///
/// The source buffer represents the full canvas (canvas_width x canvas_height at
/// origin (0,0)). The sub-region is specified in absolute pixel coordinates.
/// Pixels outside the sub-region are set to transparent black.
///
/// This is used to apply the filter region or primitive sub-region: after a
/// primitive produces output, we clip it to the relevant region.
pub fn clip_buffer_to_region(buf: &FilterBuffer, region: &AbsoluteRect) -> FilterBuffer {
    let w = buf.width;
    let h = buf.height;
    let mut out = buf.clone();

    // Convert region to integer pixel bounds, clamped to buffer dimensions.
    let rx0 = (region.x.floor() as i64).max(0) as u32;
    let ry0 = (region.y.floor() as i64).max(0) as u32;
    let rx1 = (region.x2().ceil() as i64).max(0).min(w as i64) as u32;
    let ry1 = (region.y2().ceil() as i64).max(0).min(h as i64) as u32;

    for y in 0..h {
        for x in 0..w {
            if x < rx0 || x >= rx1 || y < ry0 || y >= ry1 {
                let idx = ((y as usize) * (w as usize) + (x as usize)) * 4;
                out.data[idx] = 0.0;
                out.data[idx + 1] = 0.0;
                out.data[idx + 2] = 0.0;
                out.data[idx + 3] = 0.0;
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_filter_region_object_bbox() {
        // Element bbox: 100x80 at (50, 60)
        let bbox = AbsoluteRect::new(50.0, 60.0, 100.0, 80.0);
        let params = FilterRegionParams::default();
        let computed = compute_filter_region(&params, bbox);

        // Default: -10% padding on each side.
        // x = 50 + (-0.10 * 100) = 40
        // y = 60 + (-0.10 * 80) = 52
        // w = 1.20 * 100 = 120
        // h = 1.20 * 80 = 96
        let r = &computed.region;
        assert!((r.x - 40.0).abs() < 1e-10, "x={}", r.x);
        assert!((r.y - 52.0).abs() < 1e-10, "y={}", r.y);
        assert!((r.width - 120.0).abs() < 1e-10, "w={}", r.width);
        assert!((r.height - 96.0).abs() < 1e-10, "h={}", r.height);
    }

    #[test]
    fn test_filter_region_user_space_on_use() {
        let bbox = AbsoluteRect::new(50.0, 60.0, 100.0, 80.0);
        let params = FilterRegionParams {
            x: Some(FilterLength::Absolute(10.0)),
            y: Some(FilterLength::Absolute(20.0)),
            width: Some(FilterLength::Absolute(200.0)),
            height: Some(FilterLength::Absolute(150.0)),
            filter_units: FilterUnits::UserSpaceOnUse,
            primitive_units: FilterUnits::UserSpaceOnUse,
        };
        let computed = compute_filter_region(&params, bbox);

        let r = &computed.region;
        assert!((r.x - 10.0).abs() < 1e-10);
        assert!((r.y - 20.0).abs() < 1e-10);
        assert!((r.width - 200.0).abs() < 1e-10);
        assert!((r.height - 150.0).abs() < 1e-10);
    }

    #[test]
    fn test_filter_region_explicit_object_bbox() {
        // Custom region: x="-20%", y="-20%", width="140%", height="140%"
        let bbox = AbsoluteRect::new(100.0, 100.0, 200.0, 200.0);
        let params = FilterRegionParams {
            x: Some(FilterLength::Percent(-20.0)),
            y: Some(FilterLength::Percent(-20.0)),
            width: Some(FilterLength::Percent(140.0)),
            height: Some(FilterLength::Percent(140.0)),
            filter_units: FilterUnits::ObjectBoundingBox,
            primitive_units: FilterUnits::UserSpaceOnUse,
        };
        let computed = compute_filter_region(&params, bbox);

        // x = 100 + (-0.20 * 200) = 60
        // y = 100 + (-0.20 * 200) = 60
        // w = 1.40 * 200 = 280
        // h = 1.40 * 200 = 280
        let r = &computed.region;
        assert!((r.x - 60.0).abs() < 1e-10, "x={}", r.x);
        assert!((r.y - 60.0).abs() < 1e-10, "y={}", r.y);
        assert!((r.width - 280.0).abs() < 1e-10, "w={}", r.width);
        assert!((r.height - 280.0).abs() < 1e-10, "h={}", r.height);
    }

    #[test]
    fn test_primitive_subregion_defaults_to_filter_region() {
        let bbox = AbsoluteRect::new(50.0, 50.0, 100.0, 100.0);
        let params = FilterRegionParams::default();
        let computed = compute_filter_region(&params, bbox);

        let prim_params = PrimitiveRegionParams::default();
        let sub = computed.compute_primitive_subregion(&prim_params);
        assert_eq!(sub.x, computed.region.x);
        assert_eq!(sub.y, computed.region.y);
        assert_eq!(sub.width, computed.region.width);
        assert_eq!(sub.height, computed.region.height);
    }

    #[test]
    fn test_primitive_subregion_explicit() {
        let bbox = AbsoluteRect::new(0.0, 0.0, 200.0, 200.0);
        let params = FilterRegionParams {
            filter_units: FilterUnits::ObjectBoundingBox,
            primitive_units: FilterUnits::UserSpaceOnUse,
            ..Default::default()
        };
        let computed = compute_filter_region(&params, bbox);

        let prim_params = PrimitiveRegionParams {
            x: Some(FilterLength::Absolute(10.0)),
            y: Some(FilterLength::Absolute(10.0)),
            width: Some(FilterLength::Absolute(100.0)),
            height: Some(FilterLength::Absolute(100.0)),
        };
        let sub = computed.compute_primitive_subregion(&prim_params);
        assert!((sub.x - 10.0).abs() < 1e-10);
        assert!((sub.y - 10.0).abs() < 1e-10);
        assert!((sub.width - 100.0).abs() < 1e-10);
        assert!((sub.height - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_primitive_subregion_clipped_to_filter_region() {
        let bbox = AbsoluteRect::new(50.0, 50.0, 100.0, 100.0);
        let params = FilterRegionParams::default();
        let computed = compute_filter_region(&params, bbox);
        // Filter region: x=40, y=40, w=120, h=120 => x2=160, y2=160

        // Primitive that extends beyond the filter region.
        let prim_params = PrimitiveRegionParams {
            x: Some(FilterLength::Absolute(0.0)),
            y: Some(FilterLength::Absolute(0.0)),
            width: Some(FilterLength::Absolute(500.0)),
            height: Some(FilterLength::Absolute(500.0)),
        };
        // primitiveUnits default = userSpaceOnUse
        let sub = computed.compute_primitive_subregion(&prim_params);
        // Should be clipped to the filter region.
        assert!((sub.x - computed.region.x).abs() < 1e-10);
        assert!((sub.y - computed.region.y).abs() < 1e-10);
        let expected_w = computed.region.x2() - computed.region.x;
        let expected_h = computed.region.y2() - computed.region.y;
        // The intersection of (0,0,500,500) and (40,40,120,120) => (40,40,120,120)
        assert!((sub.width - expected_w).abs() < 1e-10);
        assert!((sub.height - expected_h).abs() < 1e-10);
    }

    #[test]
    fn test_clip_buffer_to_region() {
        let mut buf = FilterBuffer::new(10, 10);
        buf.fill(1.0, 0.0, 0.0, 1.0); // Fill with opaque red.

        let region = AbsoluteRect::new(2.0, 3.0, 5.0, 4.0);
        let clipped = clip_buffer_to_region(&buf, &region);

        // Pixel inside the region should be unchanged.
        let inside = clipped.get_pixel(4, 5);
        assert_eq!(inside, [1.0, 0.0, 0.0, 1.0]);

        // Pixel outside the region should be transparent.
        let outside = clipped.get_pixel(0, 0);
        assert_eq!(outside, [0.0, 0.0, 0.0, 0.0]);

        let outside2 = clipped.get_pixel(8, 8);
        assert_eq!(outside2, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_absolute_rect_intersect() {
        let a = AbsoluteRect::new(0.0, 0.0, 10.0, 10.0);
        let b = AbsoluteRect::new(5.0, 5.0, 10.0, 10.0);
        let i = a.intersect(&b);
        assert!((i.x - 5.0).abs() < 1e-10);
        assert!((i.y - 5.0).abs() < 1e-10);
        assert!((i.width - 5.0).abs() < 1e-10);
        assert!((i.height - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_absolute_rect_no_overlap() {
        let a = AbsoluteRect::new(0.0, 0.0, 5.0, 5.0);
        let b = AbsoluteRect::new(10.0, 10.0, 5.0, 5.0);
        let i = a.intersect(&b);
        assert!(i.width <= 0.0 || i.height <= 0.0);
    }
}
