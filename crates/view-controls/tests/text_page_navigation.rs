use view_controls::{Style, TextArea, TextField, TextInput, TextPresentation, TextResponse};
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalRect, LogicalSize, Modifiers, NamedKey,
    ScaleFactor,
};
use view_text::*;

fn fonts() -> CosmicBackend {
    CosmicBackend::from_fonts(
        [
            include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec(),
            include_bytes!("../fixtures/fonts/NotoSansHebrew.ttf").to_vec(),
        ],
        8,
    )
    .unwrap()
}

fn prepare(field: &mut TextField, width: f32, height: f32) -> TextPresentation {
    let style = Style {
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        padding: 8.0,
        ..Default::default()
    };
    field
        .prepare(
            &mut fonts(),
            &style,
            LogicalRect::new(
                LogicalPoint::ZERO,
                LogicalSize::new(width + 16.0, height + 16.0),
            ),
            ScaleFactor::ONE,
            0,
        )
        .unwrap()
}

fn key(
    field: &mut TextField,
    key: NamedKey,
    modifiers: Modifiers,
) -> Result<TextResponse, TextError> {
    field.key(
        &KeyboardEvent {
            key: Key::Named(key),
            phase: KeyPhase::Down,
            modifiers,
            timestamp: 20,
        },
        &mut MemoryClipboard::default(),
    )
}

fn select(field: &mut TextField, index: usize) {
    let revision = field.snapshot().text.revision();
    field
        .editor_mut()
        .select(revision, Selection::caret(TextIndex(index)))
        .unwrap();
}

fn caret_rect(field: &TextField, layout: &ShapedText) -> LogicalRect {
    layout
        .caret_bounds(Caret {
            index: field.snapshot().selection.focus,
            affinity: Affinity::Downstream,
        })
        .unwrap()
}

#[test]
fn pages_preserve_horizontal_intent_through_short_lines_and_shift_reversal() {
    let lines = [
        "abcdefghij",
        "abcdefghij",
        "abcdefghij",
        "a",
        "abcdefghij",
        "abcdefghij",
        "abcdefghij",
        "abcdefghij",
        "abcdefghij",
        "abcdefghij",
    ];
    let offsets: Vec<_> = lines
        .iter()
        .scan(0, |offset, line| {
            let result = *offset;
            *offset += line.len() + 1;
            Some(result)
        })
        .collect();
    let mut area = TextArea::new("Document", &lines.join("\n")).unwrap();
    area.0.set_focused(true, 0);
    select(&mut area.0, 6);
    let presentation = prepare(&mut area.0, 400.0, 78.0);
    let initial = area.0.snapshot().text;
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap(),
        TextResponse::Handled
    );
    assert_eq!(
        area.0.snapshot().selection,
        Selection::caret(TextIndex(offsets[3] + 1))
    );
    assert_eq!(area.0.editor().scroll_offset().y, 26.0);
    key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap();
    assert_eq!(
        area.0.snapshot().selection,
        Selection::caret(TextIndex(offsets[6] + 6))
    );
    assert_eq!(area.0.editor().scroll_offset().y, 104.0);
    let shift = Modifiers {
        shift: true,
        ..Default::default()
    };
    key(&mut area.0, NamedKey::PageUp, shift).unwrap();
    assert_eq!(
        area.0.snapshot().selection,
        Selection {
            anchor: TextIndex(offsets[6] + 6),
            focus: TextIndex(offsets[3] + 1)
        }
    );
    key(&mut area.0, NamedKey::PageUp, shift).unwrap();
    assert_eq!(
        area.0.snapshot().selection,
        Selection {
            anchor: TextIndex(offsets[6] + 6),
            focus: TextIndex(6)
        }
    );
    assert_eq!(area.0.editor().scroll_offset().y, 0.0);
    key(&mut area.0, NamedKey::ArrowDown, shift).unwrap();
    assert_eq!(area.0.snapshot().selection.focus, TextIndex(offsets[1] + 6));
    assert_eq!(area.0.snapshot().text, initial);
    assert_eq!(caret_rect(&area.0, &presentation.layout).origin.y, 26.0);
}

#[test]
fn pages_follow_wrapped_geometry_and_use_the_latest_content_viewport() {
    let mut area = TextArea::new("Wrapped", &"word ".repeat(80)).unwrap();
    area.0.set_focused(true, 0);
    select(&mut area.0, 2);
    let presentation = prepare(&mut area.0, 110.0, 78.0);
    let start = caret_rect(&area.0, &presentation.layout);
    assert!(presentation.layout.size().height > 260.0);
    key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap();
    let after = caret_rect(&area.0, &presentation.layout);
    assert_eq!(after.origin.y, start.origin.y + 78.0);
    assert!((after.origin.x - start.origin.x).abs() < 0.01);
    assert!(area.0.editor().scroll_offset().y > 0.0);
    let resized = prepare(&mut area.0, 110.0, 52.0);
    key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap();
    let after_resize = caret_rect(&area.0, &resized.layout);
    assert_eq!(after_resize.origin.y, after.origin.y + 52.0);
    assert!((after_resize.origin.x - start.origin.x).abs() < 0.01);
    assert!(after_resize.bottom() <= area.0.editor().scroll_offset().y + 52.0);
}

#[test]
fn repeats_clamp_at_visual_edges_and_small_viewports_still_make_progress() {
    let mut area = TextArea::new("Tiny", "abcdef\nabcdef\nabcdef\nabcdef\nabcdef").unwrap();
    area.0.set_focused(true, 0);
    select(&mut area.0, 3);
    let presentation = prepare(&mut area.0, 300.0, 8.0);
    key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap();
    assert_eq!(area.0.snapshot().selection.focus, TextIndex(10));
    for _ in 0..12 {
        let event = KeyboardEvent {
            key: Key::Named(NamedKey::PageDown),
            phase: KeyPhase::Repeat,
            modifiers: Modifiers::default(),
            timestamp: 30,
        };
        assert_eq!(
            area.0.key(&event, &mut MemoryClipboard::default()).unwrap(),
            TextResponse::Handled
        );
    }
    assert_eq!(area.0.snapshot().selection.focus, TextIndex(31));
    assert_eq!(caret_rect(&area.0, &presentation.layout).origin.y, 104.0);
    for _ in 0..12 {
        key(&mut area.0, NamedKey::PageUp, Modifiers::default()).unwrap();
    }
    assert_eq!(area.0.snapshot().selection, Selection::caret(TextIndex(3)));
    let up = KeyboardEvent {
        key: Key::Named(NamedKey::PageDown),
        phase: KeyPhase::Up,
        modifiers: Modifiers::default(),
        timestamp: 40,
    };
    assert_eq!(
        area.0.key(&up, &mut MemoryClipboard::default()).unwrap(),
        TextResponse::Ignored
    );
    assert_eq!(area.0.snapshot().selection.focus, TextIndex(3));
}

#[test]
fn page_shortcuts_single_line_composition_and_disabled_do_not_move() {
    let mut input = TextInput::new("One line", "abcdef").unwrap();
    input.0.set_focused(true, 0);
    for named in [NamedKey::PageUp, NamedKey::PageDown] {
        assert_eq!(
            key(&mut input.0, named, Modifiers::default()).unwrap(),
            TextResponse::Ignored
        );
    }
    let mut area = TextArea::new("Document", "abc\ndef\nghi\njkl").unwrap();
    area.0.set_focused(true, 0);
    // Modified chords must remain available even before initial layout.
    for modifiers in [
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
        Modifiers {
            alt: true,
            ..Default::default()
        },
        Modifiers {
            meta: true,
            ..Default::default()
        },
    ] {
        for named in [NamedKey::PageUp, NamedKey::PageDown] {
            assert_eq!(
                key(&mut area.0, named, modifiers).unwrap(),
                TextResponse::Ignored
            );
        }
    }
    prepare(&mut area.0, 300.0, 52.0);
    area.0.preedit("composing", None).unwrap();
    let before = area.0.snapshot();
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap(),
        TextResponse::Ignored
    );
    assert_eq!(area.0.snapshot(), before);
    area.0.set_modes(false, true);
    let before = area.0.snapshot();
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap(),
        TextResponse::Ignored
    );
    assert_eq!(area.0.snapshot(), before);
    area.0.set_modes(false, false);
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()).unwrap(),
        TextResponse::Ignored
    );
}

#[test]
fn readonly_unicode_bidi_pages_keep_grapheme_boundaries_and_selection_anchor() {
    let text = "ab אבג e\u{301} 👩‍💻 xyz\n".repeat(12);
    let mut area = TextArea::new("Unicode", &text).unwrap();
    area.0.set_focused(true, 0);
    select(&mut area.0, "ab א".len());
    area.0.set_modes(true, false);
    let presentation = prepare(&mut area.0, 400.0, 78.0);
    let start = caret_rect(&area.0, &presentation.layout);
    let anchor = area.0.snapshot().selection.anchor;
    for (i, named) in [NamedKey::PageDown, NamedKey::PageDown, NamedKey::PageUp]
        .into_iter()
        .enumerate()
    {
        key(
            &mut area.0,
            named,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        )
        .unwrap();
        let current = area.0.snapshot();
        assert_eq!(current.selection.anchor, anchor);
        validate_caret(&text, current.selection.focus).unwrap();
        let rect = caret_rect(&area.0, &presentation.layout);
        assert_eq!(rect.origin.y, if i == 1 { 156.0 } else { 78.0 });
        assert!((rect.origin.x - start.origin.x).abs() < 0.01);
        assert_eq!(current.text.text(), text);
    }
}

#[test]
fn absent_stale_and_invalid_page_geometry_reject_without_selection_effects() {
    let mut area = TextArea::new("Document", "abc\ndef\nghi\njkl").unwrap();
    area.0.set_focused(true, 0);
    let before = area.0.snapshot();
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()),
        Err(TextError::StaleRevision)
    );
    assert_eq!(area.0.snapshot(), before);
    let presentation = prepare(&mut area.0, 300.0, 52.0);
    for viewport in [
        LogicalSize::new(0.0, 52.0),
        LogicalSize::new(300.0, f32::NAN),
    ] {
        assert_eq!(
            area.0
                .editor_mut()
                .move_page(presentation.layout.as_ref(), true, true, viewport, 0),
            Err(TextError::InvalidMetrics)
        );
        assert_eq!(area.0.snapshot(), before);
    }
    area.0.insert_text("X", 0).unwrap();
    let changed = area.0.snapshot();
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()),
        Err(TextError::StaleRevision)
    );
    assert_eq!(area.0.snapshot(), changed);
    area.0.unmount(0);
    area.0.set_focused(true, 0);
    assert_eq!(
        key(&mut area.0, NamedKey::PageDown, Modifiers::default()),
        Err(TextError::StaleRevision)
    );
}
