use remarkable_render::{
    Document, DocumentSource, Error, PlannerKind, Rect, index_planner_dates, render_planner_page,
};
use std::{
    io::{self, Cursor, Read},
    path::Path,
};

mod common;

#[test]
fn original_planner_fixtures_preserve_links_and_rendered_handwriting() {
    let doc = Document::from_bytes(include_bytes!("fixtures/linked-pages.pdf").as_slice()).unwrap();
    let links = doc.links(0).unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_page, None);
    assert_eq!(links[0].uri, b"#page=2");
    assert_eq!(links[0].text, "Se");
    assert_eq!(
        links[0].rect,
        Rect {
            left: 10.,
            bottom: f64::from(545.2756_f32),
            right: 50.,
            top: f64::from(585.2756_f32)
        }
    );
    let png = render_planner_page(
        include_bytes!("fixtures/oct1-notes-template.pdf").as_slice(),
        0,
        include_bytes!("fixtures/oct1-notes-strokes.rm"),
        200,
    )
    .unwrap();
    let actual = image::load_from_memory(&png).unwrap().to_rgba8();
    let expected = image::load_from_memory(include_bytes!("fixtures/oct1-notes-golden.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(actual.dimensions(), expected.dimensions());
    let mut delta = 0u64;
    let mut samples = 0u64;
    for y in (0..actual.height()).step_by(2) {
        for x in (0..actual.width()).step_by(2) {
            for channel in 0..3 {
                delta += u64::from(
                    actual.get_pixel(x, y)[channel].abs_diff(expected.get_pixel(x, y)[channel]),
                );
                samples += 1;
            }
        }
    }
    let mean_error = delta as f64 / samples as f64;
    eprintln!(
        "Original planner golden: dimensions={:?}, sampled RGB mean error={mean_error:.6}/255",
        actual.dimensions()
    );
    assert!(
        mean_error <= 8.,
        "planner differs from original golden: {mean_error}"
    );
    let go = image::load_from_memory(include_bytes!("fixtures/oct1-notes-go.png"))
        .unwrap()
        .to_rgba8();
    let delta: u64 = actual
        .as_raw()
        .iter()
        .zip(go.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    let mean = delta as f64 / actual.as_raw().len() as f64;
    eprintln!("Original planner Go comparison: mean channel error={mean:.6}/255");
    assert!(mean < 2.);
}

#[test]
fn rendering_matches_go_baselines() {
    for (rm, golden) in [
        (
            &[][..],
            include_bytes!("fixtures/background-go.png").as_slice(),
        ),
        (RM, include_bytes!("fixtures/overlay-go.png").as_slice()),
    ] {
        let expected = image::load_from_memory(golden).unwrap().to_rgba8();
        let actual = image::load_from_memory(&render_planner_page(PDF, 0, rm, 72).unwrap())
            .unwrap()
            .to_rgba8();
        let differences = actual
            .pixels()
            .zip(expected.pixels())
            .filter(|(a, b)| a != b)
            .count();
        let sum: u64 = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        let max_error = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        eprintln!(
            "Go parity rm={} differing_pixels={differences} total_abs_channel_error={sum} max_channel_error={max_error}",
            rm.len()
        );
        if rm.is_empty() {
            assert_eq!(differences, 0);
        } else {
            // Updated rasterizer/compositor versions may quantize 8-bit alpha
            // differently; geometry and all channels must remain within 2/255.
            assert!(max_error <= 2);
            assert!((differences as f64 / f64::from(actual.width() * actual.height())) < 0.002);
        }
    }
}

#[test]
fn transparency_remains_straight_rgba_in_png() {
    let pdf = common::transparency_pdf();
    let background =
        image::load_from_memory(&render_planner_page(pdf.as_slice(), 0, &[], 72).unwrap())
            .unwrap()
            .to_rgba8();
    assert_eq!(background.get_pixel(150, 50).0, [0; 4]);
    let red = background.get_pixel(50, 150).0;
    assert_eq!(red[0], 255);
    assert_eq!(red[1], 0);
    assert_eq!(red[2], 0);
    assert!((126..=128).contains(&red[3]), "{red:?}");
    let composite =
        image::load_from_memory(&render_planner_page(pdf.as_slice(), 0, RM, 72).unwrap())
            .unwrap()
            .to_rgba8();
    assert_eq!(composite.get_pixel(50, 150).0, red);
    assert!(composite.pixels().any(|p| p[3] > 0 && p[0] > 0 && p[1] > 0));
}

#[test]
fn native_action_precedence_and_default_user_space_are_preserved() {
    let pdf = common::pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [100 200 300 400] /CropBox [110 210 290 390] /Rotate 90 /Annots [5 0 R 6 0 R 7 0 R 8 0 R 9 0 R] >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [121 232 153 264] /Dest [4 0 R /Fit] /A << /S /Named /N /NextPage >> >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [121 232 153 264] /Dest [4 0 R /Fit] /A << /S /GoTo /D [1 0 R /Fit] >> >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [121 232 153 264] /Dest [4 0 R /Fit] /A << /S /URI /URI (#page=2) >> >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [121 232 153 264] /Dest [4 0 R /Fit] >>".into(),
        "<< /Type /Annot /Subtype /Link /Rect [121 232 153] /Dest [4 0 R /Fit] >>".into(),
    ]);
    let doc = Document::from_bytes(pdf).unwrap();
    let links = doc.links(0).unwrap();
    assert_eq!(links.len(), 5);
    assert_eq!(
        links.iter().map(|l| l.target_page).collect::<Vec<_>>(),
        [None, None, None, Some(1), Some(1)]
    );
    assert_eq!(links[2].uri, b"#page=2");
    assert_eq!(links[3].uri, b"#page=2");
    for link in &links[..4] {
        assert_eq!(
            link.rect,
            Rect {
                left: 121.,
                bottom: 232.,
                right: 153.,
                top: 264.
            }
        );
    }
    assert_eq!(links[4].rect, Rect::default());
}

#[test]
fn document_operations_serialize_across_threads_and_remain_reusable() {
    let doc = std::sync::Arc::new(Document::from_bytes(PDF).unwrap());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let doc = doc.clone();
            std::thread::spawn(move || {
                for _ in 0..3 {
                    assert!(doc.text(0).unwrap().contains("Thursday"));
                    assert!(doc.links(0).unwrap().is_empty());
                }
                assert!(
                    !render_planner_page(doc.as_ref(), 0, &[], 12)
                        .unwrap()
                        .is_empty()
                );
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(doc.num_pages(), 19);
}

const PDF: &[u8] = include_bytes!("fixtures/planner.pdf");
const RM: &[u8] = include_bytes!("fixtures/sample.rm");

#[test]
fn opens_owned_sources_and_borrows_without_closing() {
    let doc = Document::from_bytes(PDF).unwrap();
    assert_eq!(doc.num_pages(), 19);
    assert!(doc.text(0).unwrap().contains("Jan 1 Thursday Notes"));
    assert_eq!(doc.text(5).unwrap(), "");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/planner.pdf");
    let file = Document::from_path(&path).unwrap();
    let reader = Document::from_reader(Cursor::new(PDF)).unwrap();
    assert_eq!(file.text(2).unwrap(), reader.text(2).unwrap());
    for source in [
        DocumentSource::from(&doc),
        DocumentSource::from(PDF),
        DocumentSource::from(path.as_path()),
    ] {
        assert!(!render_planner_page(source, 0, &[], 24).unwrap().is_empty());
    }
    let mut reader = Cursor::new(PDF);
    assert!(
        !render_planner_page(DocumentSource::Reader(&mut reader), 0, &[], 24)
            .unwrap()
            .is_empty()
    );
    assert!(!doc.text(0).unwrap().is_empty());
    let bytes = PDF.to_vec();
    assert!(!render_planner_page(&bytes, 0, &[], 12).unwrap().is_empty());
    assert!(
        !render_planner_page(path.to_str().unwrap(), 0, &[], 12)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn links_preserve_raw_uri_actions_rectangles_and_bounded_utf16() {
    for (bytes, base) in [
        (
            include_bytes!("fixtures/links-base.pdf").as_slice(),
            "https://example.com/docs/index.html",
        ),
        (include_bytes!("fixtures/links.pdf").as_slice(), ""),
    ] {
        let doc = Document::from_bytes(bytes).unwrap();
        let links = doc.links(0).unwrap();
        assert_eq!(links.len(), 2);
        for (i, link) in links.iter().enumerate() {
            assert_eq!(link.index, i);
            assert_eq!(link.target_page, Some(1));
            assert_eq!(link.uri, b"#page=2");
            assert_eq!(link.text, "");
        }
        assert!(doc.links(1).unwrap().is_empty());
        let links = doc.links(2).unwrap();
        assert_eq!(links.len(), 11);
        let expected = [
            "https://example.com/path?q=1&r=café#frag"
                .as_bytes()
                .to_vec(),
            format!("{base}guide.html").into_bytes(),
            vec![],
            vec![],
            vec![],
            vec![],
            base.as_bytes().to_vec(),
            vec![],
            b"guide.html?t=10:30".to_vec(),
            format!("{base}:note").into_bytes(),
            b"https://example.com/\xff".to_vec(),
        ];
        for (i, (link, uri)) in links.iter().zip(expected).enumerate() {
            assert_eq!(link.index, i);
            assert_eq!(link.target_page, None);
            assert_eq!(link.uri, uri);
            assert_eq!(link.text, "");
        }
        let links = doc.links(3).unwrap();
        assert_eq!(
            links
                .iter()
                .map(|link| link.text.as_str())
                .collect::<Vec<_>>(),
            ["Notes", "Standup", "Pa", "Mon", "Year 2026", "Back", "😀B"]
        );
        assert_eq!(
            links[5].rect,
            Rect {
                left: 95.75,
                bottom: 45.,
                right: 150.25,
                top: 65.5
            }
        );
        assert_eq!(links[4].target_page, Some(0));
        assert_eq!(links[2].uri, format!("{base}guide.html").as_bytes());
        let links = doc.links(4).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].rect, Rect::default());
        assert_eq!(links[0].text, "");
    }
}

#[test]
fn planner_indexes_intended_views_and_rejects_impossible_dates() {
    let index = index_planner_dates(PDF, 2026).unwrap();
    assert_eq!(index.len(), 5);
    assert_eq!(index["2026-01-01"][&PlannerKind::Day], 18);
    assert_eq!(index["2026-01-01"][&PlannerKind::Notes], 1);
    assert_eq!(index["2026-01-15"][&PlannerKind::Day], 2);
    assert_eq!(index["2026-02-28"][&PlannerKind::Day], 3);
    assert_eq!(index["2026-12-31"][&PlannerKind::Notes], 4);
    assert_eq!(index["2026-01-04"][&PlannerKind::Day], 17);
    assert_eq!(
        index["2026-01-01"]
            .keys()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>(),
        ["day", "notes"]
    );
    for year in [2024, 2000, 2400] {
        assert_eq!(
            index_planner_dates(PDF, year).unwrap()[&format!("{year:04}-02-29")][&PlannerKind::Day],
            7
        );
    }
    for year in [2026, 1900, 2100] {
        assert!(
            !index_planner_dates(PDF, year)
                .unwrap()
                .contains_key(&format!("{year:04}-02-29"))
        );
    }
    assert!(
        index_planner_dates(PDF, 1)
            .unwrap()
            .contains_key("0001-01-01")
    );
    assert!(
        index_planner_dates(PDF, 10000)
            .unwrap()
            .contains_key("10000-01-01")
    );
}

#[test]
fn renders_pdf_then_composites_real_rm_at_exact_pixel_dimensions() {
    let doc = Document::from_bytes(PDF).unwrap();
    for (dpi, expected) in [
        (72, (448, 596)),
        (150, (934, 1241)),
        (0, (1245, 1654)),
        (-1, (1245, 1654)),
    ] {
        let png = render_planner_page(&doc, 0, &[], dpi).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), expected);
    }
    let background = image::load_from_memory(&render_planner_page(&doc, 0, &[], 72).unwrap())
        .unwrap()
        .to_rgba8();
    let composite = image::load_from_memory(&render_planner_page(&doc, 0, RM, 72).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(background.dimensions(), composite.dimensions());
    assert!(
        background
            .pixels()
            .zip(composite.pixels())
            .filter(|(a, b)| a != b)
            .count()
            > 100
    );
    let yellow = composite
        .pixels()
        .filter(|p| p.0[0] > 220 && p.0[1] > 220 && p.0[2] < 220 && p.0[3] == 255)
        .count();
    assert!(yellow > 100, "highlighter was not composited: {yellow}");
    assert_eq!(background.get_pixel(0, 0).0, [255; 4]);
}

#[test]
fn failures_are_typed_and_leave_borrowed_document_usable() {
    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("read failed"))
        }
    }
    assert!(matches!(
        Document::from_reader(BrokenReader),
        Err(Error::Io(_))
    ));
    assert!(matches!(
        Document::from_path("tests/fixtures/missing.pdf"),
        Err(Error::Io(_))
    ));
    assert!(matches!(
        Document::from_bytes(b"bad PDF".as_slice()),
        Err(Error::Pdfium { .. })
    ));
    let doc = Document::from_bytes(PDF).unwrap();
    for page in [-1, 19, isize::MAX] {
        assert!(matches!(doc.text(page), Err(Error::PageOutOfBounds { .. })));
        assert!(matches!(
            doc.links(page),
            Err(Error::PageOutOfBounds { .. })
        ));
        assert!(matches!(
            render_planner_page(&doc, page, &[], 72),
            Err(Error::PageOutOfBounds { .. })
        ));
    }
    for year in [0, -1] {
        assert!(matches!(
            index_planner_dates(b"bad".as_slice(), year),
            Err(Error::InvalidYear)
        ));
    }
    assert!(matches!(
        render_planner_page(&doc, 0, b"bad RM", 72),
        Err(Error::Strokes(_))
    ));
    assert!(matches!(
        render_planner_page(&doc, 0, &[], i32::MAX),
        Err(Error::Dimensions)
    ));
    assert!(doc.text(0).unwrap().contains("Jan 1"));
}
