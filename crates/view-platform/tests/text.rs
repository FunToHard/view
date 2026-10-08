#![cfg(feature = "text")]
use view_platform::{ImeEvent, text::dispatch_ime};
use view_text::{EditorConfig, EditorSession, PlainEditor, Selection, TextIndex};
#[test]
fn normalized_ime_replaces_selection_once_and_cancel_preserves_value() {
    let mut e = PlainEditor::new("abc", EditorConfig::default()).unwrap();
    e.select(
        e.snapshot().text.revision(),
        Selection {
            anchor: TextIndex(1),
            focus: TextIndex(2),
        },
    )
    .unwrap();
    dispatch_ime(&mut e, &ImeEvent::Preedit("日本".into(), Some((6, 6))), 0).unwrap();
    assert_eq!(e.snapshot().text.text(), "abc");
    dispatch_ime(&mut e, &ImeEvent::Commit("日".into()), 1).unwrap();
    assert_eq!(e.snapshot().text.text(), "a日c");
    dispatch_ime(&mut e, &ImeEvent::Preedit("仮".into(), None), 2).unwrap();
    dispatch_ime(&mut e, &ImeEvent::Disabled, 3).unwrap();
    assert_eq!(e.snapshot().text.text(), "a日c");
    assert!(e.snapshot().composition.is_none());
    e.undo().unwrap();
    assert_eq!(e.snapshot().text.text(), "abc");
}
