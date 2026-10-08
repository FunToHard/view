#![cfg(feature = "text")]

use view::text::{Selection, TextIndex, TextRevision, TextSnapshot};

#[test]
fn text_contracts_are_available_without_backend_dependencies() {
    let text = TextSnapshot::new("a😀", TextRevision::INITIAL);
    assert_eq!(text.from_utf16(3).unwrap(), TextIndex(5));
    assert!(
        text.validate_selection(Selection::caret(TextIndex(5)))
            .is_ok()
    );
}

#[test]
fn shared_editor_is_available_through_facade() {
    use view::text::{EditKind, EditorConfig, EditorSession, PlainEditor};
    let mut editor = PlainEditor::new("", EditorConfig::default()).unwrap();
    editor.insert("a👩‍💻", EditKind::Typing, 0).unwrap();
    editor.delete(true, false, 0).unwrap();
    assert_eq!(editor.snapshot().text.text(), "a");
}

#[cfg(feature = "text-shaping")]
#[test]
fn independent_shaping_has_no_workspace_feature_unification() {
    use view::text::{CosmicBackend, LayoutRequest, TextBackend, TextLayout, Wrap};
    let bytes = include_bytes!("../../../fixtures/fonts/NotoSans-Regular.ttf");
    let mut backend = CosmicBackend::from_fonts([bytes.to_vec()], 1).unwrap();
    let layout = backend
        .shape(&LayoutRequest {
            text: TextSnapshot::new("ffi", TextRevision::INITIAL),
            font_family: "Noto Sans".into(),
            font_size: 16.0,
            line_height: 20.0,
            max_width: None,
            wrap: Wrap::None,
            scale: view::ScaleFactor::ONE,
            font_revision: view::Revision::INITIAL,
        })
        .unwrap();
    assert!(layout.size().width > 0.0);
}
