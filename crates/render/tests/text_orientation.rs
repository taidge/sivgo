//! Raster regressions for font Y-up outlines in SVG's Y-down coordinate space.

use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap;

const SIDE: u32 = 128;
const ROW_BYTES: usize = SIDE as usize * 4;

fn ink_rows(body: &str) -> Vec<usize> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128"><g font-family="sans-serif" font-size="48" fill="black">{body}</g></svg>"#
    );
    let document = parse_svg(&svg).unwrap();
    let pixels = render_document_to_pixmap(&document, SIDE, SIDE);
    pixels
        .as_chunks::<ROW_BYTES>()
        .0
        .iter()
        .map(|row| row.as_chunks::<4>().0.iter().filter(|p| p[0] < 128).count())
        .collect()
}

#[test]
fn plain_positioned_and_path_text_stand_above_the_baseline() {
    for body in [
        r#"<text x="24" y="72">F</text>"#,
        r#"<text><tspan x="24" y="72">F</tspan></text>"#,
        r##"<defs><path id="baseline" d="M24 72 H120"/></defs><text><textPath href="#baseline">F</textPath></text>"##,
    ] {
        let rows = ink_rows(body);
        let top = rows
            .iter()
            .position(|count| *count > 0)
            .expect("outlined F");
        let bottom = rows.iter().rposition(|count| *count > 0).unwrap();
        assert!(
            top < 50,
            "capital must rise above y=72: ink rows {top}..{bottom}; {body}"
        );
        assert!(
            bottom <= 76,
            "capital must not descend below its baseline: ink rows {top}..{bottom}; {body}"
        );

        // An upright F has its wide horizontal bars at the top; merely moving
        // an upside-down glyph above the baseline must not satisfy this test.
        let third = (bottom - top + 1) / 3;
        let upper: usize = rows[top..top + third].iter().sum();
        let lower: usize = rows[bottom + 1 - third..=bottom].iter().sum();
        assert!(
            upper > lower,
            "F must be upright: upper={upper}, lower={lower}; {body}"
        );
    }
}

#[test]
fn descenders_extend_below_the_baseline_without_flipping_the_glyph() {
    let rows = ink_rows(r#"<text x="24" y="72">g</text>"#);
    let above: usize = rows[..72].iter().sum();
    let below: usize = rows[72..].iter().sum();
    assert!(above > 100, "the body of g must remain above its baseline");
    assert!(below > 20, "the descender must remain below its baseline");
    assert!(above > below, "most of g must be above its baseline");
}
