use super::*;

#[test]
fn native_failures_release_pages_and_do_not_invalidate_the_document() {
    let document =
        NativeDocument::open(include_bytes!("../fixtures/planner.pdf").to_vec()).unwrap();
    assert!(matches!(
        document.text(999),
        Err(Error::Pdfium {
            operation: "load page",
            ..
        })
    ));
    assert!(matches!(
        document.links(-1),
        Err(Error::Pdfium {
            operation: "load page",
            ..
        })
    ));
    assert!(matches!(
        document.render(999, 72),
        Err(Error::Pdfium {
            operation: "load page",
            ..
        })
    ));
    assert!(document.text(0).unwrap().contains("Jan 1"));
    assert!(document.links(0).unwrap().is_empty());
}

#[test]
fn verifies_library_content_and_rejects_changed_bytes() {
    let pin = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    verify_library(b"abc", pin).unwrap();
    assert!(matches!(
        verify_library(b"abd", pin),
        Err(Error::LibraryDigest)
    ));
}

#[test]
fn dimensions_round_up_and_check_native_bitmap_limits() {
    assert_eq!(pixel_size(447.874, 595.275, 200).unwrap(), (1245, 1654));
    for (w, h, dpi) in [
        (0., 1., 72),
        (-1., 1., 72),
        (1., 0., 72),
        (f64::NAN, 1., 72),
        (1., f64::INFINITY, 72),
        (1., 1., 0),
        (f64::from(i32::MAX), 1., 72),
        (1., f64::from(i32::MAX) + 1., 72),
        (32768., 32768., 72),
    ] {
        assert!(matches!(pixel_size(w, h, dpi), Err(Error::Dimensions)));
    }
}

#[test]
fn utf16_decoder_preserves_astral_characters_and_embedded_nuls() {
    assert_eq!(decode_text(&[0xd83d, 0xde00, 0x42, 0], 4).unwrap(), "😀B");
    assert_eq!(decode_text(&[0x41, 0, 0x42, 0], 4).unwrap(), "A\0B");
    assert_eq!(decode_text(&[0x41, 0xd800], 2).unwrap(), "A�");
    for count in [-1, 0, 5] {
        assert!(matches!(
            decode_text(&[0; 4], count),
            Err(Error::Pdfium {
                operation: "read text",
                ..
            })
        ));
    }
}
