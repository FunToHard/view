use view_core::{Revision, ScaleFactor};
use view_text::*;

#[test]
fn utf16_round_trip_rejects_partial_surrogates_and_utf8_bytes() {
    let text = TextSnapshot::new("A😀e\u{301}אב", TextRevision::INITIAL);
    // Independent UTF-8 / UTF-16 offsets, including combining and RTL scalars.
    for (bytes, units) in [(0, 0), (1, 1), (5, 3), (6, 4), (8, 5), (10, 6), (12, 7)] {
        assert_eq!(text.to_utf16(TextIndex(bytes)), Ok(units));
        assert_eq!(text.from_utf16(units), Ok(TextIndex(bytes)));
    }
    assert_eq!(text.from_utf16(2), Err(TextError::InvalidIndex));
    assert_eq!(text.from_utf16(8), Err(TextError::InvalidIndex));
    for byte in [2, 3, 4, 7, 9, 11, 13, usize::MAX] {
        assert_eq!(text.to_utf16(TextIndex(byte)), Err(TextError::InvalidIndex));
    }
}

#[test]
fn selection_direction_is_preserved_and_validated() {
    let text = TextSnapshot::new("a😀b", TextRevision::INITIAL);
    let selection = Selection {
        anchor: TextIndex(6),
        focus: TextIndex(1),
    };
    assert_eq!(selection.range(), 1..6);
    assert_eq!(selection.anchor, TextIndex(6));
    assert_eq!(text.validate_selection(selection), Ok(()));
    assert_eq!(
        text.validate_selection(Selection::caret(TextIndex(2))),
        Err(TextError::InvalidIndex)
    );
}

#[test]
fn edit_validation_rejects_stale_reversed_and_partial_ranges() {
    let initial = TextRevision::INITIAL;
    let text = TextSnapshot::new("a😀b", initial.checked_next().unwrap());
    let mut edit = TextEdit {
        base: initial,
        range: 1..5,
        replacement: "x".into(),
    };
    assert_eq!(text.validate_edit(&edit), Err(TextError::StaleRevision));
    edit.base = text.revision();
    assert_eq!(text.validate_edit(&edit), Ok(()));
    edit.range = std::ops::Range { start: 5, end: 1 };
    assert_eq!(text.validate_edit(&edit), Err(TextError::InvalidRange));
    edit.range = 1..3;
    assert_eq!(text.validate_edit(&edit), Err(TextError::InvalidIndex));
    assert_eq!(text.text(), "a😀b");
}

#[test]
fn empty_text_and_invalid_layout_metrics_have_explicit_results() {
    let text = TextSnapshot::new("", TextRevision::INITIAL);
    assert_eq!(text.from_utf16(0), Ok(TextIndex(0)));
    assert_eq!(text.from_utf16(1), Err(TextError::InvalidIndex));
    let mut request = LayoutRequest {
        text,
        font_family: "sans-serif".into(),
        font_size: 16.0,
        line_height: 20.0,
        max_width: Some(0.0),
        wrap: Wrap::Word,
        scale: ScaleFactor::ONE,
        font_revision: Revision::INITIAL,
    };
    assert_eq!(request.validate(), Ok(()));
    for width in [f32::NAN, f32::INFINITY, -1.0] {
        request.max_width = Some(width);
        assert_eq!(request.validate(), Err(TextError::InvalidMetrics));
    }
    request.max_width = None;
    request.font_size = 0.0;
    assert_eq!(request.validate(), Err(TextError::InvalidMetrics));
}
