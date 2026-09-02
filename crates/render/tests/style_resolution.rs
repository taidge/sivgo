//! Paint has to come from the inline `style` attribute and from ancestors,
//! not only from an element's own presentation attributes.
//!
//! Inkscape and most other editors write paint as `style="fill:#3498db"` and
//! set shared paint once on a group. Reading only each element's own
//! attributes rendered both cases as default black, so most real-world files
//! came out as black silhouettes.

use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap;

const W: u32 = 32;
const H: u32 = 32;

fn wrap(body: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}">{body}</svg>"##)
}

/// Colour of the pixel at (x, y) as RGBA, un-premultiplied enough to compare
/// fully opaque paint.
fn pixel_at(svg: &str, x: u32, y: u32) -> [u8; 4] {
    let doc = parse_svg(svg).expect("parse");
    let pixels = render_document_to_pixmap(&doc, W, H);
    let idx = ((y * W + x) * 4) as usize;
    [
        pixels[idx],
        pixels[idx + 1],
        pixels[idx + 2],
        pixels[idx + 3],
    ]
}

#[track_caller]
fn assert_pixel_near(actual: [u8; 4], expected: [u8; 3], what: &str) {
    let close = actual[0].abs_diff(expected[0]) <= 3
        && actual[1].abs_diff(expected[1]) <= 3
        && actual[2].abs_diff(expected[2]) <= 3;
    assert!(
        close,
        "{what}: expected roughly rgb{expected:?}, got rgba{actual:?}"
    );
}

#[test]
fn inline_style_sets_fill() {
    let svg = wrap(r##"<rect width="32" height="32" style="fill:#3498db"/>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "inline fill");
}

#[test]
fn inline_style_outranks_the_presentation_attribute() {
    let svg = wrap(r##"<rect width="32" height="32" fill="#ff0000" style="fill:#00ff00"/>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "style beats attribute");
}

#[test]
fn inline_style_handles_spacing_and_trailing_semicolons() {
    let svg = wrap(r##"<rect width="32" height="32" style="  fill : #3498db ; "/>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "loose syntax");
}

#[test]
fn fill_is_inherited_from_an_ancestor_group() {
    let svg = wrap(r##"<g fill="#3498db"><rect width="32" height="32"/></g>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "inherited fill");
}

#[test]
fn inherited_fill_passes_through_several_levels() {
    let svg = wrap(r##"<g fill="#3498db"><g><g><rect width="32" height="32"/></g></g></g>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "deep inherit");
}

#[test]
fn a_child_overrides_the_inherited_value() {
    let svg = wrap(r##"<g fill="#ff0000"><rect width="32" height="32" fill="#00ff00"/></g>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "child wins");
}

#[test]
fn inheritance_works_through_an_inline_style_on_the_group() {
    let svg = wrap(r##"<g style="fill:#3498db"><rect width="32" height="32"/></g>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "group style");
}

#[test]
fn stroke_paint_and_width_are_inherited() {
    // A hairline default would leave the centre of the top edge unpainted.
    let svg = wrap(
        r##"<g stroke="#3498db" stroke-width="8"><line x1="0" y1="16" x2="32" y2="16"/></g>"##,
    );
    assert_pixel_near(
        pixel_at(&svg, 16, 16),
        [0x34, 0x98, 0xdb],
        "inherited stroke",
    );
}

#[test]
fn fill_none_is_not_treated_as_missing() {
    // `fill:none` must not fall back to the inherited red.
    let svg = wrap(r##"<g fill="#ff0000"><rect width="32" height="32" style="fill:none"/></g>"##);
    let px = pixel_at(&svg, 16, 16);
    assert!(
        px[0] < 250 || px[1] > 5 || px[2] > 5,
        "fill:none must not inherit the ancestor's red, got rgba{px:?}"
    );
}

#[test]
fn non_inherited_properties_do_not_leak_to_children() {
    // `opacity` is not an inherited property: a half-transparent group does
    // not make its child half-transparent twice over.
    let group = wrap(r##"<g opacity="0.5"><rect width="32" height="32" fill="#000000"/></g>"##);
    let direct = wrap(r##"<rect width="32" height="32" fill="#000000" opacity="0.5"/>"##);
    let a = pixel_at(&group, 16, 16);
    let b = pixel_at(&direct, 16, 16);
    assert!(
        a[0].abs_diff(b[0]) <= 3,
        "group opacity should not compound: {a:?} vs {b:?}"
    );
}

#[test]
fn fill_opacity_makes_paint_translucent() {
    // Half-opaque black over white reads as mid grey.
    let svg = wrap(r##"<rect width="32" height="32" fill="#000000" fill-opacity="0.5"/>"##);
    let px = pixel_at(&svg, 16, 16);
    assert!(
        (100..=160).contains(&px[0]),
        "fill-opacity 0.5 over white should be mid grey, got {px:?}"
    );
}

#[test]
fn fill_opacity_of_zero_hides_the_fill() {
    let svg = wrap(r##"<rect width="32" height="32" fill="#000000" fill-opacity="0"/>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [255, 255, 255], "fill-opacity 0");
}

#[test]
fn stroke_opacity_makes_the_stroke_translucent() {
    let opaque =
        wrap(r##"<line x1="0" y1="16" x2="32" y2="16" stroke="#000000" stroke-width="10"/>"##);
    let faded = wrap(
        r##"<line x1="0" y1="16" x2="32" y2="16" stroke="#000000" stroke-width="10" stroke-opacity="0.5"/>"##,
    );
    let a = pixel_at(&opaque, 16, 16);
    let b = pixel_at(&faded, 16, 16);
    assert!(
        b[0] > a[0] + 40,
        "stroke-opacity should lighten the stroke: opaque {a:?} vs faded {b:?}"
    );
}

#[test]
fn fill_opacity_is_inherited() {
    let svg = wrap(r##"<g fill-opacity="0.5"><rect width="32" height="32" fill="#000000"/></g>"##);
    let px = pixel_at(&svg, 16, 16);
    assert!(
        (100..=160).contains(&px[0]),
        "inherited fill-opacity should apply, got {px:?}"
    );
}

#[test]
fn rounded_rect_corners_are_cut_away() {
    // With rx=10 the top-left corner pixel falls outside the rounded shape.
    let square = wrap(r##"<rect width="32" height="32" fill="#000000"/>"##);
    let rounded = wrap(r##"<rect width="32" height="32" rx="10" fill="#000000"/>"##);

    assert_pixel_near(
        pixel_at(&square, 1, 1),
        [0, 0, 0],
        "square corner is filled",
    );

    let corner = pixel_at(&rounded, 1, 1);
    assert!(
        corner[0] > 200,
        "rounded corner should be background, got {corner:?}"
    );
    // The middle stays filled either way.
    assert_pixel_near(pixel_at(&rounded, 16, 16), [0, 0, 0], "rounded centre");
}

#[test]
fn a_single_radius_applies_to_both_axes() {
    let only_rx = wrap(r##"<rect width="32" height="32" rx="10" fill="#000000"/>"##);
    let only_ry = wrap(r##"<rect width="32" height="32" ry="10" fill="#000000"/>"##);
    assert_eq!(
        pixel_at(&only_rx, 1, 1),
        pixel_at(&only_ry, 1, 1),
        "a lone rx and a lone ry must round the same way"
    );
}

#[test]
fn odd_dash_arrays_still_produce_dashes() {
    // `stroke-dasharray="4"` means 4 on, 4 off. Rendering it as a solid line
    // means the gaps never appear.
    let solid =
        wrap(r##"<line x1="0" y1="16" x2="32" y2="16" stroke="#000000" stroke-width="6"/>"##);
    let dashed = wrap(
        r##"<line x1="0" y1="16" x2="32" y2="16" stroke="#000000" stroke-width="6" stroke-dasharray="4"/>"##,
    );

    let count_dark = |svg: &str| {
        let doc = sivgo_dom::parse_svg(svg).expect("parse");
        let pixels = sivgo_render::render_document_to_pixmap(&doc, W, H);
        (0..W)
            .filter(|x| {
                let idx = ((16 * W + x) * 4) as usize;
                pixels[idx] < 128
            })
            .count()
    };

    let solid_dark = count_dark(&solid);
    let dashed_dark = count_dark(&dashed);
    assert!(solid_dark > 25, "the solid line should cover the row");
    assert!(
        dashed_dark < solid_dark,
        "an odd dasharray must leave gaps: solid {solid_dark} vs dashed {dashed_dark}"
    );
}

#[test]
fn percentage_geometry_resolves_against_the_viewport() {
    // A rect at 50% of a 32x32 viewport covers the top-left quadrant. Treating
    // the percentage as a bare number would make it 50 user units and cover
    // everything.
    let svg = wrap(r##"<rect width="50%" height="50%" fill="#000000"/>"##);
    assert_pixel_near(pixel_at(&svg, 8, 8), [0, 0, 0], "inside the half-size rect");

    let outside = pixel_at(&svg, 24, 24);
    assert!(
        outside[0] > 200,
        "the far corner should stay background, got {outside:?}"
    );
}

#[test]
fn percentage_positions_resolve_against_the_viewport() {
    let percent = wrap(r##"<rect x="50%" y="50%" width="25%" height="25%" fill="#000000"/>"##);
    let absolute = wrap(r##"<rect x="16" y="16" width="8" height="8" fill="#000000"/>"##);
    assert_eq!(
        pixel_at(&percent, 20, 20),
        pixel_at(&absolute, 20, 20),
        "a percentage position must land where the equivalent absolute one does"
    );
    assert_pixel_near(pixel_at(&percent, 20, 20), [0, 0, 0], "inside");
}

#[test]
fn a_viewbox_redefines_what_percentages_refer_to() {
    // The viewBox makes user space 100 wide, so 50% is 50 user units, which
    // maps to half of the 32px surface.
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 100 100"><rect width="50%" height="50%" fill="#000000"/></svg>"##;
    assert_pixel_near(pixel_at(svg, 8, 8), [0, 0, 0], "inside");
    let outside = pixel_at(svg, 24, 24);
    assert!(
        outside[0] > 200,
        "outside should be background, got {outside:?}"
    );
}

#[test]
fn em_lengths_resolve_against_the_inherited_font_size() {
    // 2em with font-size 8 is 16 user units.
    let em = wrap(r##"<g font-size="8"><rect width="2em" height="2em" fill="#000000"/></g>"##);
    let absolute = wrap(r##"<rect width="16" height="16" fill="#000000"/>"##);

    for (x, y) in [(4, 4), (14, 14), (20, 20)] {
        assert_eq!(
            pixel_at(&em, x, y),
            pixel_at(&absolute, x, y),
            "em length should match the equivalent absolute size at ({x},{y})"
        );
    }
}

#[test]
fn a_circle_radius_in_percent_uses_the_normalised_diagonal() {
    // Whatever the exact reference, 25% of a 32x32 viewport must produce a
    // circle that covers the centre but not the corner.
    let svg = wrap(r##"<circle cx="50%" cy="50%" r="25%" fill="#000000"/>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 0, 0], "centre of the circle");
    let corner = pixel_at(&svg, 1, 1);
    assert!(
        corner[0] > 200,
        "corner should be background, got {corner:?}"
    );
}

#[test]
fn an_isolated_defs_element_does_not_swallow_the_rest_of_the_document() {
    // `<defs isolation="isolate">` pushed an offscreen layer that the defs
    // branch returned without popping, so everything drawn afterwards landed
    // in the orphaned buffer and never reached the canvas.
    let svg = wrap(
        r##"<defs isolation="isolate"><rect width="4" height="4"/></defs><rect width="32" height="32" fill="#000000"/>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 0, 0], "content after defs");
}

#[test]
fn an_isolated_gradient_definition_does_not_swallow_later_content() {
    let svg = wrap(
        r##"<linearGradient id="g" isolation="isolate"><stop offset="0" stop-color="#fff"/></linearGradient><rect width="32" height="32" fill="#000000"/>"##,
    );
    assert_pixel_near(
        pixel_at(&svg, 16, 16),
        [0, 0, 0],
        "content after a gradient",
    );
}

#[test]
fn switch_skips_a_branch_that_requires_unsupported_features() {
    // The first branch demands a feature nothing supports, so the fallback
    // must be drawn instead. Reading the condition from the wrong place made
    // `<switch>` always take its first child.
    let svg = wrap(
        r##"<switch><rect width="32" height="32" fill="#ff0000" requiredExtensions="http://example.com/ext"/><rect width="32" height="32" fill="#00ff00"/></switch>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "the fallback branch");
}

#[test]
fn switch_takes_the_first_usable_branch() {
    let svg = wrap(
        r##"<switch><rect width="32" height="32" fill="#00ff00"/><rect width="32" height="32" fill="#ff0000"/></switch>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "the first branch");
}

#[test]
fn unknown_elements_and_their_children_are_not_drawn() {
    // A `<foreignObject>` body is HTML, not SVG; drawing its contents as
    // shapes is wrong even when they happen to parse.
    let svg = wrap(
        r##"<foreignObject width="32" height="32"><rect width="32" height="32" fill="#ff0000"/></foreignObject>"##,
    );
    let px = pixel_at(&svg, 16, 16);
    assert!(
        px[0] > 200 && px[1] > 200,
        "foreign content must not be rendered, got {px:?}"
    );
}

#[test]
fn a_style_element_rule_paints_a_matching_class() {
    let svg = wrap(
        r##"<style>.brand { fill: #3498db; }</style><rect class="brand" width="32" height="32"/>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "class rule");
}

#[test]
fn a_style_element_rule_can_select_by_tag_and_id() {
    let by_tag = wrap(r##"<style>rect { fill: #3498db; }</style><rect width="32" height="32"/>"##);
    assert_pixel_near(pixel_at(&by_tag, 16, 16), [0x34, 0x98, 0xdb], "tag rule");

    let by_id = wrap(
        r##"<style>#target { fill: #3498db; }</style><rect id="target" width="32" height="32"/>"##,
    );
    assert_pixel_near(pixel_at(&by_id, 16, 16), [0x34, 0x98, 0xdb], "id rule");
}

#[test]
fn a_style_rule_outranks_a_presentation_attribute() {
    let svg = wrap(
        r##"<style>rect { fill: #00ff00; }</style><rect fill="#ff0000" width="32" height="32"/>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "rule beats attribute");
}

#[test]
fn an_inline_style_outranks_a_style_rule() {
    let svg = wrap(
        r##"<style>rect { fill: #ff0000; }</style><rect style="fill:#00ff00" width="32" height="32"/>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0, 255, 0], "inline beats rule");
}

#[test]
fn style_rules_are_inherited_by_descendants() {
    let svg = wrap(
        r##"<style>.wrap { fill: #3498db; }</style><g class="wrap"><rect width="32" height="32"/></g>"##,
    );
    assert_pixel_near(pixel_at(&svg, 16, 16), [0x34, 0x98, 0xdb], "inherited rule");
}

#[test]
fn a_descendant_selector_matches_through_the_tree() {
    let svg =
        wrap(r##"<style>g rect { fill: #3498db; }</style><g><rect width="32" height="32"/></g>"##);
    assert_pixel_near(
        pixel_at(&svg, 16, 16),
        [0x34, 0x98, 0xdb],
        "descendant rule",
    );
}

#[test]
fn the_style_element_itself_is_not_drawn() {
    let svg = wrap(r##"<style>rect { fill: #3498db; }</style>"##);
    assert_pixel_near(pixel_at(&svg, 16, 16), [255, 255, 255], "empty canvas");
}
