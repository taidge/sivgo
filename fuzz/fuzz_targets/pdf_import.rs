// Fuzzing target for the PDF importer.
// Run with: cargo +nightly fuzz run pdf_import
//
// The importer is a hand-written scanner over a binary format: it slices byte
// ranges, parses numbers, and inflates embedded streams, all driven by offsets
// taken from the file itself. That combination is exactly where malformed
// input tends to find panics.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = sivgo_io::import_pdf(data, &sivgo_io::PdfImportOptions::default());
});
