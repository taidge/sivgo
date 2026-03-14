//! Benchmarks for SVG parsing and rendering.
//!
//! Run with:
//!   cargo bench --bench render_bench
//!
//! These benchmarks use a simple timing harness compatible with
//! `#[bench]`-style layout. For more sophisticated statistical
//! analysis, consider adding criterion as a dev-dependency.

use std::time::Instant;

/// A minimal bench harness that runs `f` for `iterations` and prints
/// the average time per iteration.
fn bench(name: &str, iterations: u32, f: impl Fn()) {
    // Warm up
    for _ in 0..3 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations;
    println!(
        "{:>40}: {:>10.3?} / iter  ({} iters, {:.3?} total)",
        name, per_iter, iterations, elapsed
    );
}

// ---------------------------------------------------------------------------
// SVG test inputs
// ---------------------------------------------------------------------------

const SIMPLE_RECT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <rect x="20" y="20" width="160" height="120" fill="#3366cc" stroke="black" stroke-width="2"/>
</svg>"#;

const SHAPES_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
  <rect x="10" y="10" width="80" height="60" fill="red"/>
  <circle cx="200" cy="200" r="80" fill="blue"/>
  <ellipse cx="300" cy="100" rx="60" ry="30" fill="green"/>
  <line x1="0" y1="0" x2="400" y2="400" stroke="black" stroke-width="2"/>
  <polyline points="50,300 100,250 150,300 200,250" fill="none" stroke="purple" stroke-width="2"/>
  <polygon points="300,300 350,250 400,300" fill="orange"/>
  <path d="M 10 200 C 40 100 160 100 190 200" fill="none" stroke="brown" stroke-width="3"/>
</svg>"#;

const GRADIENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <defs>
    <linearGradient id="g1" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="red"/>
      <stop offset="50%" stop-color="green"/>
      <stop offset="100%" stop-color="blue"/>
    </linearGradient>
  </defs>
  <rect x="10" y="10" width="180" height="180" fill="url(#g1)"/>
</svg>"##;

const NESTED_GROUPS_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300">
  <g transform="translate(10,10)">
    <g transform="rotate(15)">
      <rect x="0" y="0" width="50" height="50" fill="red"/>
      <g transform="translate(60,0)">
        <circle cx="25" cy="25" r="20" fill="blue"/>
        <rect x="50" y="0" width="40" height="40" fill="green"/>
      </g>
    </g>
    <g transform="translate(0,100)">
      <ellipse cx="40" cy="20" rx="35" ry="18" fill="purple" opacity="0.6"/>
    </g>
  </g>
</svg>"#;

fn generate_many_rects(count: usize) -> String {
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
        count * 2,
        count * 2
    );
    for i in 0..count {
        svg.push_str(&format!(
            r#"<rect x="{}" y="{}" width="10" height="10" fill="#{:02x}{:02x}{:02x}"/>"#,
            (i * 12) % (count * 2),
            (i * 7) % (count * 2),
            (i * 37) % 256,
            (i * 73) % 256,
            (i * 113) % 256,
        ));
    }
    svg.push_str("</svg>");
    svg
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Sivgo Benchmarks ===\n");

    // --- Parsing benchmarks ---
    println!("--- SVG Parsing ---");

    bench("parse simple rect", 5000, || {
        let _ = sivgo_dom::parse_svg(SIMPLE_RECT_SVG).unwrap();
    });

    bench("parse 7 mixed shapes", 2000, || {
        let _ = sivgo_dom::parse_svg(SHAPES_SVG).unwrap();
    });

    bench("parse gradient + rect", 3000, || {
        let _ = sivgo_dom::parse_svg(GRADIENT_SVG).unwrap();
    });

    bench("parse nested groups", 2000, || {
        let _ = sivgo_dom::parse_svg(NESTED_GROUPS_SVG).unwrap();
    });

    let many_100 = generate_many_rects(100);
    bench("parse 100 rects", 500, || {
        let _ = sivgo_dom::parse_svg(&many_100).unwrap();
    });

    let many_1000 = generate_many_rects(1000);
    bench("parse 1000 rects", 50, || {
        let _ = sivgo_dom::parse_svg(&many_1000).unwrap();
    });

    // --- Serialization benchmarks ---
    println!("\n--- SVG Serialization ---");

    let doc_simple = sivgo_dom::parse_svg(SIMPLE_RECT_SVG).unwrap();
    bench("serialize simple rect", 5000, || {
        let _ = sivgo_dom::serialize_svg(&doc_simple);
    });

    let doc_shapes = sivgo_dom::parse_svg(SHAPES_SVG).unwrap();
    bench("serialize 7 mixed shapes", 2000, || {
        let _ = sivgo_dom::serialize_svg(&doc_shapes);
    });

    let doc_1000 = sivgo_dom::parse_svg(&many_1000).unwrap();
    bench("serialize 1000 rects", 50, || {
        let _ = sivgo_dom::serialize_svg(&doc_1000);
    });

    // --- Round-trip benchmarks ---
    println!("\n--- Round-trip (parse + serialize) ---");

    bench("roundtrip simple rect", 3000, || {
        let doc = sivgo_dom::parse_svg(SIMPLE_RECT_SVG).unwrap();
        let _ = sivgo_dom::serialize_svg(&doc);
    });

    bench("roundtrip 7 mixed shapes", 1000, || {
        let doc = sivgo_dom::parse_svg(SHAPES_SVG).unwrap();
        let _ = sivgo_dom::serialize_svg(&doc);
    });

    println!("\n=== Done ===");
}
