use view_controls::{Style, TextArea, TextField, TextInput, TextPresentation};
use view_core::{
    Key, KeyPhase, KeyboardEvent, LogicalPoint, LogicalRect, LogicalSize, Modifiers, NamedKey,
    ScaleFactor,
};
use view_text::*;

fn fonts() -> CosmicBackend {
    CosmicBackend::from_fonts(
        [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        8,
    )
    .unwrap()
}

fn prepare(field: &mut TextField, width: f32, height: f32, password: bool) -> TextPresentation {
    let style = Style {
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        padding: 8.0,
        ..Default::default()
    };
    let mut backend = fonts();
    let bounds = LogicalRect::new(
        LogicalPoint::ZERO,
        LogicalSize::new(width + 16.0, height + 16.0),
    );
    if password {
        field
            .prepare_password(&mut backend, &style, bounds, ScaleFactor::ONE, 0)
            .unwrap()
    } else {
        field
            .prepare(&mut backend, &style, bounds, ScaleFactor::ONE, 0)
            .unwrap()
    }
}

fn end(field: &mut TextField) {
    let revision = field.snapshot().text.revision();
    let end = field.snapshot().text.text().len();
    field
        .editor_mut()
        .select(revision, Selection::caret(TextIndex(end)))
        .unwrap();
}

#[test]
fn focused_manual_scroll_survives_repeated_preparation() {
    let mut area = TextArea::new("Document", &"line\n".repeat(120)).unwrap();
    area.0.set_focused(true, 0);
    prepare(&mut area.0, 240.0, 78.0, false);
    assert!(
        area.0
            .scroll_by(LogicalPoint::new(0.0, 260.0), LogicalSize::new(240.0, 78.0))
            .unwrap()
    );
    let manual = area.0.editor().scroll_offset();
    assert!(manual.y > 0.0);

    for _ in 0..3 {
        let presentation = prepare(&mut area.0, 240.0, 78.0, false);
        assert_eq!(area.0.editor().scroll_offset(), manual);
        assert_eq!(presentation.origin.y, 8.0 - manual.y);
    }
}

#[test]
fn typing_and_navigation_request_caret_reveal() {
    let mut input = TextInput::new("Name", &"x".repeat(80)).unwrap();
    input.0.set_focused(true, 0);
    end(&mut input.0);
    prepare(&mut input.0, 80.0, 30.0, false);
    assert!(input.0.editor().scroll_offset().x > 0.0);

    input.0.insert_text("y", 1).unwrap();
    prepare(&mut input.0, 80.0, 30.0, false);
    assert!(input.0.editor().scroll_offset().x > 0.0);

    let revision = input.0.snapshot().text.revision();
    input
        .0
        .editor_mut()
        .select(revision, Selection::caret(TextIndex(0)))
        .unwrap();
    prepare(&mut input.0, 80.0, 30.0, false);
    assert_eq!(input.0.editor().scroll_offset().x, 0.0);
    let event = KeyboardEvent {
        key: Key::Named(NamedKey::End),
        phase: KeyPhase::Down,
        modifiers: Modifiers::default(),
        timestamp: 2,
    };
    input
        .0
        .key(&event, &mut MemoryClipboard::default())
        .unwrap();
    prepare(&mut input.0, 80.0, 30.0, false);
    assert!(input.0.editor().scroll_offset().x > 0.0);

    let home = KeyboardEvent {
        key: Key::Named(NamedKey::Home),
        phase: KeyPhase::Down,
        modifiers: Modifiers::default(),
        timestamp: 3,
    };
    input.0.key(&home, &mut MemoryClipboard::default()).unwrap();
    prepare(&mut input.0, 80.0, 30.0, false);
    assert_eq!(input.0.editor().scroll_offset().x, 0.0);

    input
        .0
        .scroll_by(LogicalPoint::new(100.0, 0.0), LogicalSize::new(80.0, 30.0))
        .unwrap();
    assert!(input.0.editor().scroll_offset().x > 0.0);
    input.0.key(&home, &mut MemoryClipboard::default()).unwrap();
    prepare(&mut input.0, 80.0, 30.0, false);
    assert_eq!(input.0.editor().scroll_offset().x, 0.0);
}

#[test]
fn resized_viewport_clamps_saved_scroll_without_revealing_old_caret() {
    let mut area = TextArea::new("Document", &"line\n".repeat(80)).unwrap();
    area.0.set_focused(true, 0);
    prepare(&mut area.0, 240.0, 78.0, false);
    area.0
        .scroll_by(
            LogicalPoint::new(0.0, 100_000.0),
            LogicalSize::new(240.0, 78.0),
        )
        .unwrap();
    let before = area.0.editor().scroll_offset().y;
    assert!(before > 100.0);

    let presentation = prepare(&mut area.0, 240.0, 400.0, false);
    let after = area.0.editor().scroll_offset().y;
    assert!(after < before);
    assert_eq!(after, (presentation.layout.size().height - 400.0).max(0.0));
}

#[test]
fn password_reveal_keeps_grapheme_mask_indices_and_raw_value() {
    let value = &"WiiiMMMM e\u{301} Wwiiii ".repeat(12);
    let mut input = TextInput::new("Password", value).unwrap();
    input.0.set_focused(true, 0);
    end(&mut input.0);
    let revealed = prepare(&mut input.0, 72.0, 30.0, false);
    assert_caret_visible(&revealed);

    input
        .0
        .scroll_by(LogicalPoint::new(-40.0, 0.0), LogicalSize::new(72.0, 30.0))
        .unwrap();
    let manual_revealed = input.0.editor().scroll_offset();
    prepare(&mut input.0, 72.0, 30.0, false);
    assert_eq!(input.0.editor().scroll_offset(), manual_revealed);

    let masked = prepare(&mut input.0, 72.0, 30.0, true);
    assert_caret_visible(&masked);
    assert_eq!(
        masked.layout.text().text(),
        "•".repeat(grapheme_boundaries(value).len() - 1)
    );
    assert_eq!(
        masked.caret.unwrap().index,
        TextIndex(masked.layout.text().text().len())
    );
    input
        .0
        .scroll_by(LogicalPoint::new(-40.0, 0.0), LogicalSize::new(72.0, 30.0))
        .unwrap();
    let manual_masked = input.0.editor().scroll_offset();
    prepare(&mut input.0, 72.0, 30.0, true);
    assert_eq!(input.0.editor().scroll_offset(), manual_masked);

    let revealed_again = prepare(&mut input.0, 72.0, 30.0, false);
    assert_caret_visible(&revealed_again);
    assert_eq!(input.0.snapshot().text.text(), value);
}

fn assert_caret_visible(presentation: &TextPresentation) {
    // Candidate rectangles are clamped for the OS even when a manually scrolled
    // caret is outside the clip. Check the actual painted caret geometry here.
    let mut caret = presentation
        .layout
        .caret_bounds(presentation.caret.unwrap())
        .unwrap();
    caret.origin = caret.origin + presentation.origin;
    assert!(caret.left() >= presentation.clip.left());
    assert!(caret.right() <= presentation.clip.right());
    assert!(caret.top() >= presentation.clip.top());
    assert!(caret.bottom() <= presentation.clip.bottom());
}
