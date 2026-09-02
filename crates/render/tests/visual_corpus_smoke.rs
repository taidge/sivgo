#[path = "../../../tests/visual/mod.rs"]
mod visual;

use std::fs;

use sivgo_dom::parse_svg;
use sivgo_render::render_document_to_pixmap;

const WIDTH: u32 = 200;
const HEIGHT: u32 = 200;

#[test]
fn visual_corpus_parses_renders_and_is_deterministic() {
    assert!(visual::examples_dir().is_dir());

    let corpus_dir = visual::corpus_dir();
    let mut fixtures = fs::read_dir(&corpus_dir)
        .expect("visual corpus directory should be readable")
        .map(|entry| {
            entry
                .expect("visual corpus entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .collect::<Vec<_>>();
    fixtures.sort();

    assert_eq!(
        fixtures.len(),
        10,
        "all committed corpus fixtures should run"
    );

    for fixture in fixtures {
        let name = fixture
            .file_name()
            .expect("fixture should have a file name")
            .to_string_lossy();
        let svg = fs::read_to_string(&fixture)
            .unwrap_or_else(|error| panic!("failed to read {name}: {error}"));
        let document =
            parse_svg(&svg).unwrap_or_else(|error| panic!("failed to parse {name}: {error}"));

        let actual = render_document_to_pixmap(&document, WIDTH, HEIGHT);
        let repeated = render_document_to_pixmap(&document, WIDTH, HEIGHT);
        let comparison = visual::compare_pixels(&actual, &repeated, WIDTH, HEIGHT, 0);

        assert!(
            comparison.is_match(0.0),
            "{name} rendered nondeterministically: {comparison:?}"
        );
        assert_eq!(comparison.max_diff, 0, "{name} changed between renders");
        assert_eq!(comparison.diff_pixel_count, 0);
        assert_eq!(comparison.total_pixels, (WIDTH * HEIGHT) as usize);

        let non_white_pixels = actual
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[..3] != [255, 255, 255])
            .count();
        assert!(
            non_white_pixels > 0,
            "{name} should render visible content instead of a blank canvas"
        );
    }

    // Keep this location part of the compiled contract even before the first
    // platform-specific reference set is committed.
    assert!(visual::reference_dir().ends_with("references"));
}
