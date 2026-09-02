// Fuzzing target for the rendering pipeline.
// Run with: cargo +nightly fuzz run svg_render
//
// Parsing is only half the exposure: the web export endpoint hands arbitrary
// documents straight to the renderer, which resolves references, builds filter
// pipelines and allocates buffers whose sizes come from the input. Reference
// cycles, empty filter results and out-of-range numbers all reached it before
// the guards were added, and each one aborted the process rather than failing
// a single request.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(inner) = std::str::from_utf8(data) else {
        return;
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">{inner}</svg>"#
    );

    let Ok(doc) = sivgo_dom::parse_svg(&svg) else {
        return;
    };

    // Small canvas: the interesting failures are structural, and a big one
    // only slows the fuzzer down. File access is disabled because the input is
    // untrusted, matching how the server renders.
    let _ = sivgo_render::render_document_to_pixmap_without_file_access(&doc, 64, 64);
});
