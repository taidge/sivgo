//! Malformed and hostile documents must fail a render, never the process.
//!
//! Every case here previously either aborted the process (a stack overflow
//! from a reference cycle or deep nesting, which unwinding cannot catch) or
//! panicked inside a filter primitive. All of them are reachable from the
//! public export endpoint, where the document is supplied by the caller.
//!
//! The assertion is deliberately weak — only that rendering returns — because
//! what the output looks like for nonsense input does not matter. What matters
//! is that the thread survives.

use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap_without_file_access;

/// Parse and render, returning false if the document was rejected at parse time.
fn survives(svg: &str) -> bool {
    match parse_svg(svg) {
        Ok(doc) => {
            let pixels = render_document_to_pixmap_without_file_access(&doc, 48, 48);
            assert_eq!(pixels.len(), 48 * 48 * 4, "buffer must be fully sized");
            true
        }
        // A clean parse error is a perfectly good outcome.
        Err(_) => false,
    }
}

fn wrap(body: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">{body}</svg>"##)
}

#[test]
fn reference_cycles_do_not_abort() {
    let cases = [
        // <use> pointing at its own ancestor, and at itself.
        r##"<g id="a"><rect width="8" height="8"/><use href="#a"/></g>"##,
        r##"<use id="u" href="#u"/>"##,
        // A pattern whose content paints with the same pattern.
        r##"<defs><pattern id="p" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="url(#p)"/></pattern></defs><rect width="40" height="40" fill="url(#p)"/>"##,
        // A mask whose content is masked by itself.
        r##"<defs><mask id="m"><rect width="40" height="40" fill="#fff" mask="url(#m)"/></mask></defs><rect width="40" height="40" mask="url(#m)"/>"##,
        // A marker whose content carries the same marker.
        r##"<defs><marker id="mk" markerWidth="4" markerHeight="4" refX="2" refY="2"><path d="M0 0 L4 4" marker-start="url(#mk)"/></marker></defs><path d="M0 0 L20 20" stroke="#000" marker-start="url(#mk)"/>"##,
        // Two clip paths referring to each other.
        r##"<defs><clipPath id="c1" clip-path="url(#c2)"><rect width="8" height="8"/></clipPath><clipPath id="c2" clip-path="url(#c1)"><rect width="8" height="8"/></clipPath></defs><rect width="40" height="40" clip-path="url(#c1)"/>"##,
        // A gradient chain that loops back on itself.
        r##"<defs><linearGradient id="g1" href="#g2"/><linearGradient id="g2" href="#g1"/></defs><rect width="40" height="40" fill="url(#g1)"/>"##,
    ];

    for case in cases {
        survives(&wrap(case));
    }
}

#[test]
fn deeply_nested_documents_do_not_abort() {
    for depth in [100, 300, 5000] {
        let svg = wrap(&format!(
            "{}<rect width=\"4\" height=\"4\"/>{}",
            "<g>".repeat(depth),
            "</g>".repeat(depth)
        ));
        // Deep-but-allowed renders; past the limit the parser rejects it.
        survives(&svg);
    }
}

#[test]
fn hostile_filter_parameters_do_not_abort() {
    let filters = [
        // An empty merge yields an empty buffer that later primitives consume.
        r##"<feMerge/><feComposite in2="SourceGraphic" operator="over"/>"##,
        r##"<feMerge/><feBlend in2="SourceGraphic" mode="multiply"/>"##,
        r##"<feMerge/><feDisplacementMap in2="SourceGraphic" scale="10"/>"##,
        // Radii and octave counts far beyond anything meaningful.
        r##"<feMorphology radius="3000000000" operator="dilate"/>"##,
        r##"<feMorphology radius="-5" operator="erode"/>"##,
        r##"<feTurbulence baseFrequency="0.05" numOctaves="2000000000"/>"##,
        r##"<feGaussianBlur stdDeviation="1e30"/>"##,
        r##"<feGaussianBlur stdDeviation="-4"/>"##,
        // A kernel whose length contradicts its declared order.
        r##"<feConvolveMatrix order="3" kernelMatrix="1 2 3"/>"##,
        r##"<feConvolveMatrix order="3" kernelMatrix="1 2 3 4 5 6 7 8 9" divisor="0"/>"##,
        // Non-finite and missing values.
        r##"<feOffset dx="1e400" dy="1e400"/>"##,
        r##"<feComposite operator="arithmetic" k1="nan" k2="inf" k3="1" k4="0"/>"##,
        r##"<feColorMatrix type="matrix" values="1 2 3"/>"##,
        r##"<feImage href="/etc/hostname"/>"##,
    ];

    for filter in filters {
        let svg = wrap(&format!(
            r##"<defs><filter id="f">{filter}</filter></defs><rect width="32" height="32" fill="red" filter="url(#f)"/>"##
        ));
        survives(&svg);
    }
}

#[test]
fn out_of_range_geometry_does_not_abort() {
    let cases = [
        r##"<rect x="1e308" y="-1e308" width="1e308" height="1e308" fill="#000"/>"##,
        r##"<rect width="nan" height="nan" fill="#000"/>"##,
        r##"<rect width="-10" height="-10" fill="#000"/>"##,
        r##"<circle cx="0" cy="0" r="-5" fill="#000"/>"##,
        r##"<ellipse cx="0" cy="0" rx="1e300" ry="1e300" fill="#000"/>"##,
        r##"<path d="M 0 0 C 1e308 1e308 -1e308 -1e308 5 5" fill="#000"/>"##,
        r##"<path d="M" fill="#000"/>"##,
        r##"<path d="M 0 0 L" fill="#000"/>"##,
        r##"<g transform="matrix(0,0,0,0,0,0)"><rect width="8" height="8"/></g>"##,
        r##"<g transform="scale(1e300)"><rect width="8" height="8"/></g>"##,
        r##"<polygon points="0,0 nan,nan 5,5" fill="#000"/>"##,
    ];

    for case in cases {
        survives(&wrap(case));
    }
}

#[test]
fn malformed_colours_and_lengths_do_not_abort() {
    // A byte-length dispatch used to slice a multi-byte character in half.
    let cases = [
        r##"<rect width="8" height="8" fill="#éa"/>"##,
        r##"<rect width="8" height="8" fill="#éé"/>"##,
        r##"<rect width="8" height="8" fill="#"/>"##,
        r##"<rect width="8" height="8" fill="rgb(é,é,é)"/>"##,
        r##"<rect width="8" height="8" style="fill:#ééééé"/>"##,
        r##"<rect width="8" height="8" stroke="#ab" stroke-width="é"/>"##,
        r##"<rect width="8" height="8" stroke="#000" stroke-dasharray="é,é"/>"##,
    ];

    for case in cases {
        survives(&wrap(case));
    }
}

#[test]
fn a_viewbox_of_zeros_or_infinities_does_not_abort() {
    for view_box in ["0 0 0 0", "0 0 inf inf", "nan nan nan nan", "0 0 -10 -10"] {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="{view_box}"><rect width="8" height="8" fill="#000"/></svg>"##
        );
        survives(&svg);
    }
}
