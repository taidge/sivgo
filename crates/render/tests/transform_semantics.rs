//! Checks that nested coordinate systems compose the way SVG requires.
//!
//! Each case renders a document that uses nesting and compares it pixel for
//! pixel against a hand-flattened document that must look identical. The two
//! forms only agree when the transforms compose in the right order, and
//! because translation commutes with everything, a test built only from
//! translations would pass either way — every case here mixes in a scale or a
//! rotation.

use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap;

const W: u32 = 64;
const H: u32 = 64;

fn render(svg: &str) -> Vec<u8> {
    let doc = parse_svg(svg).expect("parse");
    render_document_to_pixmap(&doc, W, H)
}

fn wrap(body: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{W}" height="{H}">{body}</svg>"##
    )
}

/// Count pixels whose colour differs by more than a small tolerance.
fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len(), "buffers must be the same size");
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 2))
        .count()
}

#[track_caller]
fn assert_renders_same(label: &str, nested: &str, flattened: &str) {
    let a = render(&wrap(nested));
    let b = render(&wrap(flattened));
    let diff = differing_pixels(&a, &b);
    assert_eq!(
        diff, 0,
        "{label}: {diff} pixels differ between the nested and flattened forms"
    );
}

#[track_caller]
fn assert_renders_differently(label: &str, one: &str, other: &str) {
    let a = render(&wrap(one));
    let b = render(&wrap(other));
    assert!(
        differing_pixels(&a, &b) > 0,
        "{label}: the two forms should not render identically — the test cannot \
         tell right from wrong composition order"
    );
}

#[test]
fn child_transform_applies_inside_parent_scale() {
    // A child's transform is expressed in the parent's coordinate system, so
    // the parent's scale multiplies the child's translation as well.
    assert_renders_same(
        "group scale over child translate",
        r##"<g transform="scale(2)"><rect transform="translate(10,0)" width="10" height="10" fill="black"/></g>"##,
        r##"<rect x="20" y="0" width="20" height="20" fill="black"/>"##,
    );

    // Guard: the wrong order would put the rect at x=10 with the same size.
    assert_renders_differently(
        "correct vs reversed composition",
        r##"<rect x="20" y="0" width="20" height="20" fill="black"/>"##,
        r##"<rect x="10" y="0" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn three_levels_of_nesting_compose_in_order() {
    assert_renders_same(
        "scale then translate then scale",
        r##"<g transform="scale(2)"><g transform="translate(5,3)"><rect transform="scale(0.5)" width="20" height="20" fill="black"/></g></g>"##,
        r##"<rect x="10" y="6" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn rotation_in_a_parent_applies_to_child_offsets() {
    // rotate(90) maps (x,y) -> (-y,x); the child offset must be rotated too.
    assert_renders_same(
        "parent rotation over child translate",
        r##"<g transform="rotate(90)"><rect transform="translate(10,0)" width="8" height="8" fill="black"/></g>"##,
        r##"<rect transform="rotate(90) translate(10,0)" width="8" height="8" fill="black"/>"##,
    );
}

#[test]
fn transform_attribute_list_applies_left_to_right() {
    // `transform="a b"` means apply a, then b, to the element's coordinates,
    // i.e. the element is placed by a ∘ b.
    assert_renders_same(
        "translate then scale in one attribute",
        r##"<rect transform="translate(10,5) scale(2)" width="10" height="10" fill="black"/>"##,
        r##"<rect x="10" y="5" width="20" height="20" fill="black"/>"##,
    );

    assert_renders_same(
        "scale then translate in one attribute",
        r##"<rect transform="scale(2) translate(10,5)" width="10" height="10" fill="black"/>"##,
        r##"<rect x="20" y="10" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn use_offset_is_scaled_by_the_enclosing_transform() {
    assert_renders_same(
        "use x/y inside a scaled group",
        r##"<defs><rect id="r" width="10" height="10" fill="black"/></defs><g transform="scale(2)"><use href="#r" x="10" y="5"/></g>"##,
        r##"<rect x="20" y="10" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn use_own_transform_and_offset_combine() {
    assert_renders_same(
        "use with both transform and x/y",
        r##"<defs><rect id="r" width="10" height="10" fill="black"/></defs><use href="#r" transform="scale(2)" x="5" y="0"/>"##,
        r##"<rect x="10" y="0" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn symbol_viewbox_maps_inside_the_use_coordinate_system() {
    // The symbol's viewBox maps its 0..10 content onto the use's 20x20 box,
    // which then sits at the use's x/y.
    assert_renders_same(
        "symbol viewBox under use",
        r##"<defs><symbol id="s" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></symbol></defs><use href="#s" x="8" y="4" width="20" height="20"/>"##,
        r##"<rect x="8" y="4" width="20" height="20" fill="black"/>"##,
    );
}

#[test]
fn nested_group_opacity_does_not_change_geometry() {
    assert_renders_same(
        "opacity group keeps child placement",
        r##"<g transform="scale(2)" opacity="1"><rect transform="translate(4,4)" width="6" height="6" fill="black"/></g>"##,
        r##"<rect x="8" y="8" width="12" height="12" fill="black"/>"##,
    );
}
