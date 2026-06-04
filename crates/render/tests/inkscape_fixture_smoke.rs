use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap;

fn red_2x2_png_data_uri() -> String {
    use base64::Engine;

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(std::io::Cursor::new(&mut png_bytes), 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let data: [u8; 16] = [
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        writer.write_image_data(&data).unwrap();
    }

    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png_bytes)
    )
}

fn render_fixture(svg: &str) -> Vec<u8> {
    let doc = parse_svg(svg).expect("fixture should parse");
    let pixels = render_document_to_pixmap(&doc, 100, 100);
    assert_eq!(pixels.len(), 100 * 100 * 4);
    pixels
}

fn non_white_count(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count()
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let idx = (y * 100 + x) * 4;
    &pixels[idx..idx + 4]
}

#[test]
fn inkscape_fixture_classes_render_nonblank() {
    let red_png = red_2x2_png_data_uri();
    let imported_raster = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <image href="{red_png}" x="20" y="20" width="60" height="60"/>
</svg>"##
    );

    let fixtures = [
        (
            "markers",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <marker id="arrow" markerWidth="12" markerHeight="12" refX="10" refY="6" orient="auto">
      <path d="M 0 0 L 12 6 L 0 12 z" fill="red"/>
    </marker>
  </defs>
  <path d="M 15 50 L 82 50" fill="none" stroke="black" stroke-width="6" marker-end="url(#arrow)"/>
</svg>"##
                .to_string(),
        ),
        (
            "patterns",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <pattern id="hatch" patternUnits="userSpaceOnUse" width="12" height="12">
      <rect x="0" y="0" width="6" height="12" fill="black"/>
    </pattern>
  </defs>
  <rect x="10" y="10" width="80" height="80" fill="url(#hatch)"/>
</svg>"##
                .to_string(),
        ),
        (
            "text",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <text x="10" y="55" font-size="28" fill="black">Sivgo</text>
</svg>"##
                .to_string(),
        ),
        (
            "filters",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <filter id="blur"><feGaussianBlur stdDeviation="3"/></filter>
  </defs>
  <rect x="25" y="25" width="50" height="50" fill="red" filter="url(#blur)"/>
</svg>"##
                .to_string(),
        ),
        (
            "clips",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs><clipPath id="clip"><circle cx="50" cy="50" r="30"/></clipPath></defs>
  <rect x="0" y="0" width="100" height="100" fill="blue" clip-path="url(#clip)"/>
</svg>"##
                .to_string(),
        ),
        (
            "masks",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <mask id="spot">
      <rect x="0" y="0" width="100" height="100" fill="black"/>
      <circle cx="50" cy="50" r="30" fill="white"/>
    </mask>
  </defs>
  <rect x="0" y="0" width="100" height="100" fill="green" mask="url(#spot)"/>
</svg>"##
                .to_string(),
        ),
        (
            "symbols",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <symbol id="dot" viewBox="0 0 20 20">
      <circle cx="10" cy="10" r="8" fill="purple"/>
    </symbol>
  </defs>
  <use href="#dot" x="25" y="25" width="50" height="50"/>
</svg>"##
                .to_string(),
        ),
        (
            "pages",
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100" height="100">
  <sodipodi:namedview>
    <inkscape:page x="0" y="0" width="100" height="100" id="page1"/>
    <inkscape:page x="120" y="0" width="100" height="100" id="page2"/>
  </sodipodi:namedview>
  <rect x="10" y="10" width="35" height="35" fill="orange"/>
</svg>"##
                .to_string(),
        ),
        ("imported raster", imported_raster),
    ];

    for (name, svg) in fixtures {
        let pixels = render_fixture(&svg);
        assert!(
            non_white_count(&pixels) > 0,
            "{name} fixture should render non-white pixels"
        );
    }
}

#[test]
fn clip_mask_and_imported_raster_have_expected_key_pixels() {
    let clipped = render_fixture(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs><clipPath id="clip"><circle cx="50" cy="50" r="25"/></clipPath></defs>
  <rect x="0" y="0" width="100" height="100" fill="blue" clip-path="url(#clip)"/>
</svg>"##,
    );
    assert!(
        pixel(&clipped, 50, 50)[2] > 180,
        "clip center should be blue"
    );
    assert_eq!(pixel(&clipped, 5, 5), &[255, 255, 255, 255]);

    let red_png = red_2x2_png_data_uri();
    let imported = render_fixture(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <image href="{red_png}" x="20" y="20" width="60" height="60"/>
</svg>"##
    ));
    let center = pixel(&imported, 50, 50);
    assert!(
        center[0] > 180 && center[1] < 40 && center[2] < 40,
        "imported raster center should keep red channel"
    );
}
