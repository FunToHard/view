#![cfg(feature = "shaping")]
use view_core::{LogicalPoint, LogicalSize, Revision, ScaleFactor};
use view_text::*;
fn fonts() -> Vec<Vec<u8>> {
    vec![
        include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec(),
        include_bytes!("../../../tests/fixtures/fonts/NotoSansHebrew.ttf").to_vec(),
        include_bytes!("../../../tests/fixtures/fonts/NotoSansArabic.ttf").to_vec(),
    ]
}
fn request(text: &str) -> LayoutRequest {
    LayoutRequest {
        text: TextSnapshot::new(text, TextRevision::INITIAL),
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        max_width: None,
        wrap: Wrap::None,
        scale: ScaleFactor::ONE,
        font_revision: Revision::INITIAL,
    }
}
#[test]
fn fallback_mixed_scripts_ligatures_emoji_and_empty_lines_have_valid_carets() {
    let mut backend = CosmicBackend::from_fonts(fonts(), 2).unwrap();
    for text in ["office", "aאב العربية", "e\u{301}👩‍💻🇯🇵", "", "a\n\nb\n"] {
        let layout = backend.shape(&request(text)).unwrap();
        assert!(layout.size().height > 0.0);
        for index in grapheme_boundaries(text) {
            let caret = Caret {
                index: TextIndex(index),
                affinity: Affinity::Downstream,
            };
            let rect = layout.caret_bounds(caret).unwrap();
            let hit = layout
                .hit_test(LogicalPoint::new(
                    rect.origin.x,
                    rect.origin.y + rect.size.height * 0.5,
                ))
                .unwrap();
            validate_caret(text, hit.index).unwrap();
        }
    }
    let layout = backend.shape(&request("aאב العربية")).unwrap();
    assert!(layout.font_count() >= 3);
    assert_eq!(layout.missing_glyphs(), 0);
}
#[test]
fn bidi_visual_movement_differs_from_logical_and_selection_has_geometry() {
    let mut backend = CosmicBackend::from_fonts(fonts(), 1).unwrap();
    let layout = backend.shape(&request("אבג")).unwrap();
    let first = Caret {
        index: TextIndex(0),
        affinity: Affinity::Downstream,
    };
    let next = layout.visual_neighbor(first, false).unwrap();
    assert_eq!(next.index, TextIndex(2));
    assert!(
        layout.caret_bounds(next).unwrap().origin.x < layout.caret_bounds(first).unwrap().origin.x
    );
    let rects = layout
        .selection_bounds(Selection {
            anchor: TextIndex(0),
            focus: TextIndex(6),
        })
        .unwrap();
    assert!(!rects.is_empty());
    assert!(rects.iter().all(|r| r.size.width > 0.0));
}
#[test]
fn cache_invalidates_on_content_dpi_fonts_width_and_keeps_old_layout_alive() {
    let mut backend = CosmicBackend::from_fonts(fonts(), 1).unwrap();
    let mut r = request("hello");
    let old = backend.shape(&r).unwrap();
    backend.shape(&r).unwrap();
    assert_eq!(backend.cache_hits(), 1);
    r.scale = ScaleFactor::new(2.0).unwrap();
    backend.shape(&r).unwrap();
    assert_eq!(backend.cache_hits(), 1);
    r.text = TextSnapshot::new("different", TextRevision::INITIAL);
    backend.shape(&r).unwrap();
    assert_eq!(backend.cache_hits(), 1);
    r.max_width = Some(30.0);
    r.wrap = Wrap::Word;
    backend.shape(&r).unwrap();
    assert_eq!(backend.cached_layouts(), 1);
    backend.reload_fonts(fonts()).unwrap();
    assert_eq!(backend.cached_layouts(), 0);
    assert!(matches!(backend.shape(&r), Err(TextError::StaleRevision)));
    r.font_revision = backend.font_revision();
    backend.shape(&r).unwrap();
    drop(backend);
    assert!(old.font_count() > 0);
    old.caret_bounds(Caret {
        index: TextIndex(0),
        affinity: Affinity::Downstream,
    })
    .unwrap();
}
#[test]
fn wrapped_vertical_pointer_selection_and_scroll_reveal() {
    let mut backend = CosmicBackend::from_fonts(fonts(), 1).unwrap();
    let mut e = PlainEditor::new(
        "one two three four five",
        EditorConfig {
            mode: LineMode::Multiline,
            ..Default::default()
        },
    )
    .unwrap();
    let mut r = request(e.snapshot().text.text());
    r.max_width = Some(60.0);
    r.wrap = Wrap::Word;
    let layout = backend.shape(&r).unwrap();
    assert!(layout.size().height > 26.0);
    e.move_vertical(&layout, true, false, 0).unwrap();
    assert!(e.snapshot().selection.focus.0 > 0);
    e.pointer_select(
        &layout,
        LogicalPoint::new(1000.0, 1000.0),
        true,
        LogicalSize::new(40.0, 26.0),
        0,
    )
    .unwrap();
    assert!(!e.snapshot().selection.range().is_empty());
    assert!(e.scroll_offset().y > 0.0);
    let caret = Caret {
        index: e.snapshot().selection.focus,
        affinity: Affinity::Downstream,
    };
    let candidate = e.candidate_rect(&layout, caret).unwrap();
    assert!(candidate.origin.y >= 0.0);
    assert!(candidate.origin.y + candidate.size.height <= 26.01);
    e.insert("x", EditKind::Typing, 0).unwrap();
    assert_eq!(
        e.move_vertical(&layout, false, false, 0),
        Err(TextError::StaleRevision)
    );
}

#[test]
fn scroll_clamps_and_preedit_scalar_cursor_maps_to_grapheme_geometry() {
    let mut backend = CosmicBackend::from_fonts(fonts(), 1).unwrap();
    let mut e = PlainEditor::new("abcdef", EditorConfig::default()).unwrap();
    let layout = backend.shape(&request("abcdef")).unwrap();
    e.set_scroll_offset(
        &layout,
        LogicalPoint::new(1000.0, 1000.0),
        LogicalSize::new(10.0, 26.0),
    )
    .unwrap();
    assert!((e.scroll_offset().x - (layout.size().width + 1.0 - 10.0)).abs() < 0.01);
    assert_eq!(e.scroll_offset().y, 0.0);
    let before = e.snapshot();
    assert_eq!(
        e.pointer_select(
            &layout,
            LogicalPoint::ZERO,
            false,
            LogicalSize::new(f32::NAN, 20.0),
            0
        ),
        Err(TextError::InvalidMetrics)
    );
    assert_eq!(e.snapshot(), before);
    let mut e = PlainEditor::new("", EditorConfig::default()).unwrap();
    e.preedit("e\u{301}", Selection::caret(TextIndex(1)))
        .unwrap();
    let display = e.display_snapshot().unwrap();
    let mut r = request("");
    r.text = display.text;
    let layout = backend.shape(&r).unwrap();
    let actual = e
        .candidate_rect(
            &layout,
            Caret {
                index: TextIndex(1),
                affinity: Affinity::Downstream,
            },
        )
        .unwrap();
    let expected = layout
        .caret_bounds(Caret {
            index: TextIndex(3),
            affinity: Affinity::Downstream,
        })
        .unwrap();
    assert_eq!(actual, expected);
}
