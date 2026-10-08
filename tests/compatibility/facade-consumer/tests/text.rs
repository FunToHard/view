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
