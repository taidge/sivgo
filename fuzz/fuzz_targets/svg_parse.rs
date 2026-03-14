// Fuzzing target for SVG parser
// Run with: cargo +nightly fuzz run svg_parse
//
// This target takes arbitrary bytes from the fuzzer, wraps them in a
// minimal SVG envelope, and feeds the result to `sivgo_dom::parse_svg`.
// The goal is to find panics, infinite loops, or other unexpected
// behaviour in the parser when given malformed input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Strategy 1: Try parsing raw bytes directly as SVG.
    // Most inputs will be rejected immediately by roxmltree, but any
    // that slip through exercise the DOM parser.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = sivgo_dom::parse_svg(s);
    }

    // Strategy 2: Wrap arbitrary bytes inside a valid SVG envelope so
    // the XML parser reaches the inner element parsing logic.
    if let Ok(inner) = std::str::from_utf8(data) {
        let wrapped = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">{}</svg>"#,
            inner
        );
        let _ = sivgo_dom::parse_svg(&wrapped);
    }

    // Strategy 3: Treat the bytes as a `d` attribute value for a path
    // element, exercising the path data parser specifically.
    if let Ok(d_attr) = std::str::from_utf8(data) {
        // Escape any double-quotes to keep the XML well-formed.
        let escaped = d_attr.replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;").replace('&', "&amp;");
        let path_svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{}"/></svg>"#,
            escaped
        );
        let _ = sivgo_dom::parse_svg(&path_svg);
    }
});
