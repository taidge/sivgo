//! Round-trip tests: `parse -> serialize -> parse` must not lose the document
//! or produce XML that the parser rejects.
//!
//! These exist because the serializer used to emit some attributes twice —
//! font properties (stored on both the text element and the presentation
//! attributes) and `inkscape:path-effect` / `inkscape:original-d` (stored on
//! the path element *and* in the extra-attribute map). Duplicate attributes
//! are not well-formed XML, so saving a document with text or a live path
//! effect produced a file the editor itself could not reopen.

use sivgo_dom::{parse_svg, serialize_svg};

/// Parse, serialize, and parse again; returns the serialized form.
#[track_caller]
fn roundtrip(src: &str) -> String {
    let doc = parse_svg(src).expect("first parse failed");
    let out = serialize_svg(&doc);
    parse_svg(&out).unwrap_or_else(|e| {
        panic!("re-parsing serialized output failed: {e}\n--- output ---\n{out}")
    });
    out
}

/// Every attribute name must appear at most once per element.
#[track_caller]
fn assert_no_duplicate_attrs(svg: &str) {
    for element in svg.split('<').skip(1) {
        let head = match element.find('>') {
            Some(i) => &element[..i],
            None => element,
        };
        let mut seen: Vec<&str> = Vec::new();
        let mut rest = head;
        while let Some(eq) = rest.find("=\"") {
            let name = rest[..eq].rsplit([' ', '\n', '\t']).next().unwrap_or("");
            let after = &rest[eq + 2..];
            let close = match after.find('"') {
                Some(i) => i,
                None => break,
            };
            if !name.is_empty() {
                assert!(
                    !seen.contains(&name),
                    "duplicate attribute `{name}` in element `{head}`"
                );
                seen.push(name);
            }
            rest = &after[close + 1..];
        }
    }
}

#[test]
fn text_with_font_attributes_roundtrips() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text x="5" y="5" font-family="Arial" font-size="12" font-weight="bold" font-style="italic" text-anchor="middle">hi</text></svg>"##;
    let out = roundtrip(src);
    assert_no_duplicate_attrs(&out);
    assert!(out.contains("font-family=\"Arial\""));
    assert_eq!(out.matches("font-family=").count(), 1);
    assert_eq!(out.matches("font-size=").count(), 1);
}

#[test]
fn tspan_with_font_attributes_roundtrips() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text x="1" y="2"><tspan font-family="Georgia" font-size="9">a</tspan></text></svg>"##;
    let out = roundtrip(src);
    assert_no_duplicate_attrs(&out);
    assert_eq!(out.matches("font-family=").count(), 1);
}

#[test]
fn path_with_live_path_effect_roundtrips() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100" height="100"><path d="M 0 0 L 10 10" inkscape:path-effect="sivgo:lpe-stack:id=lpe1,kind=offset,enabled=1,offset=5" inkscape:original-d="M 0 0 L 10 10"/></svg>"##;
    let out = roundtrip(src);
    assert_no_duplicate_attrs(&out);
    assert_eq!(out.matches("inkscape:path-effect=").count(), 1);
    assert!(out.contains("sivgo:lpe-stack:id=lpe1"));
    assert_eq!(out.matches("inkscape:original-d=").count(), 1);
}

#[test]
fn shapes_and_presentation_attributes_roundtrip() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><g transform="translate(5,5)" opacity="0.5"><rect x="1" y="2" width="3" height="4" rx="1" fill="#ff0000" stroke="#00ff00" stroke-width="2"/><circle cx="5" cy="5" r="3" fill-opacity="0.25"/><ellipse cx="1" cy="1" rx="2" ry="3"/><path d="M 0 0 C 1 1 2 2 3 3 Z" fill-rule="evenodd"/></g></svg>"##;
    let out = roundtrip(src);
    assert_no_duplicate_attrs(&out);
    assert!(out.contains("viewBox=\"0 0 100 100\""));
    assert!(out.contains("rx=\"1\""));
}

#[test]
fn gradients_and_defs_roundtrip() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#000000"/><stop offset="1" stop-color="#ffffff"/></linearGradient></defs><rect width="10" height="10" fill="url(#g)"/></svg>"##;
    let out = roundtrip(src);
    assert_no_duplicate_attrs(&out);
    assert!(out.contains("linearGradient"));
    assert!(out.contains("url(#g)"));
}

#[test]
fn every_example_file_roundtrips() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "svg"))
        .collect::<Vec<_>>();
    assert!(
        !entries.is_empty(),
        "no example SVGs found in {}",
        dir.display()
    );

    for entry in entries {
        let path = entry.path();
        let src = std::fs::read_to_string(&path).expect("read example");
        let doc =
            parse_svg(&src).unwrap_or_else(|e| panic!("{}: parse failed: {e}", path.display()));
        let out = serialize_svg(&doc);
        parse_svg(&out).unwrap_or_else(|e| panic!("{}: re-parse failed: {e}", path.display()));
        assert_no_duplicate_attrs(&out);
    }
}

#[test]
fn serializing_twice_is_stable() {
    let src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><text x="1" y="2" font-family="Arial" font-size="8">t</text><rect width="4" height="4" fill="#123456"/></svg>"##;
    let first = roundtrip(src);
    let second = serialize_svg(&parse_svg(&first).expect("parse first output"));
    assert_eq!(first, second, "serialization is not idempotent");
}

#[test]
fn deeply_nested_documents_do_not_abort() {
    // The XML layer recurses per nesting level, so before the depth pre-check
    // was added a document like this overflowed the stack and aborted the
    // process instead of returning an error.
    let nest = |depth: usize| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\">{}<rect width=\"1\" height=\"1\"/>{}</svg>",
            "<g>".repeat(depth),
            "</g>".repeat(depth)
        )
    };

    // Well within the limit: parses normally.
    assert!(parse_svg(&nest(20)).is_ok(), "shallow nesting must parse");

    // Deep but allowed: still parses, on a larger stack.
    assert!(parse_svg(&nest(200)).is_ok(), "200 levels must still parse");

    // Past the limit: a clean error, not a crash.
    let err = parse_svg(&nest(5000)).expect_err("5000 levels must be rejected");
    assert!(
        err.to_string().contains("nesting"),
        "unexpected error: {err}"
    );
}

#[test]
fn malformed_hex_colors_are_rejected_not_fatal() {
    // "#éa" is three bytes, so a byte-length dispatch used to slice it in the
    // middle of a character and panic.
    for value in ["#éa", "#éé", "#zz", "#", "#12345", "#ééééé"] {
        let src = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"1\" height=\"1\" fill=\"{value}\"/></svg>"
        );
        assert!(parse_svg(&src).is_ok(), "parsing must survive fill={value}");
    }
}
