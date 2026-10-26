use view_text::*;

fn document(text: &str) -> LargeDocument {
    LargeDocument::new(text, LargeDocumentConfig::default()).unwrap()
}

#[test]
fn indexed_lines_exclude_newlines_and_reject_stale_or_missing_rows() {
    let doc = document("zero\né😀\nlast\n");
    let rev = doc.revision();
    assert_eq!(doc.line_count(), 4);
    assert_eq!(doc.line(rev, 0), Ok("zero"));
    assert_eq!(doc.line(rev, 1), Ok("é😀"));
    assert_eq!(doc.line_range(rev, 1), Ok(5..11));
    assert_eq!(doc.line(rev, 3), Ok(""));
    assert_eq!(doc.line(rev, 4), Err(TextError::InvalidIndex));
    assert_eq!(
        doc.line(TextRevision::INITIAL.checked_next().unwrap(), 0),
        Err(TextError::StaleRevision)
    );
}

#[test]
fn batch_uses_one_base_revision_and_is_atomic_on_invalid_or_overlapping_ranges() {
    let mut doc = document("a😀c\nsecond");
    let rev = doc.revision();
    let next = doc
        .apply_batch(
            rev,
            vec![
                LargeEdit::new(0..1, "A"),
                LargeEdit::new(5..6, "C"),
                LargeEdit::new(7..13, "2"),
            ],
        )
        .unwrap();
    assert_eq!(doc.text(), "A😀C\n2");
    assert_eq!(next.get(), rev.get() + 1);

    let before = doc.text().to_owned();
    let current = doc.revision();
    assert_eq!(
        doc.apply_batch(current, vec![LargeEdit::new(2..3, "x")]),
        Err(TextError::InvalidIndex)
    );
    assert_eq!(
        doc.apply_batch(
            current,
            vec![LargeEdit::new(0..5, "x"), LargeEdit::new(1..6, "y")]
        ),
        Err(TextError::InvalidRange)
    );
    assert_eq!(
        doc.apply_batch(rev, vec![LargeEdit::new(0..1, "x")]),
        Err(TextError::StaleRevision)
    );
    assert_eq!(doc.text(), before);
    assert_eq!(doc.revision(), current);
}

#[test]
fn batch_undo_and_redo_preserve_unicode_and_advance_revision() {
    let mut doc = document("a👩‍💻e\u{301}");
    let rev = doc.revision();
    let after = doc
        .apply_batch(
            rev,
            vec![
                LargeEdit::new(1.."a👩‍💻".len(), "🌱"),
                LargeEdit::new("a👩‍💻e".len().."a👩‍💻e\u{301}".len(), ""),
            ],
        )
        .unwrap();
    assert_eq!(doc.text(), "a🌱e");
    assert!(doc.undo(after).unwrap());
    let undone = doc.revision();
    assert_eq!(doc.text(), "a👩‍💻e\u{301}");
    assert!(undone.get() > after.get());
    assert!(doc.redo(undone).unwrap());
    assert_eq!(doc.text(), "a🌱e");
    assert!(doc.undo(doc.revision()).unwrap());
    assert_eq!(doc.line(doc.revision(), 0), Ok("a👩‍💻e\u{301}"));
}

#[test]
fn search_is_revision_checked_unicode_safe_and_result_bounded() {
    let config = LargeDocumentConfig {
        max_search_results: 2,
        ..Default::default()
    };
    let doc = LargeDocument::new("😀 x 😀 x 😀", config).unwrap();
    let result = doc.search(doc.revision(), "😀", 0).unwrap();
    assert_eq!(result.matches, vec![0..4, 7..11]);
    assert!(result.truncated);
    assert_eq!(
        doc.search(doc.revision(), "", 0),
        Err(TextError::InvalidRange)
    );
    assert_eq!(
        doc.search(doc.revision(), "x", 1),
        Err(TextError::InvalidIndex)
    );
}

#[test]
fn document_and_history_limits_reject_without_partial_mutation() {
    let config = LargeDocumentConfig {
        max_bytes: 4,
        max_lines: 2,
        history_bytes_limit: 4,
        ..Default::default()
    };
    let mut doc = LargeDocument::new("a\nb", config).unwrap();
    let revision = doc.revision();
    assert_eq!(
        doc.apply_batch(revision, vec![LargeEdit::new(0..1, "long")]),
        Err(TextError::LengthLimit)
    );
    assert_eq!(
        doc.apply_batch(revision, vec![LargeEdit::new(1..1, "\n")]),
        Err(TextError::LengthLimit)
    );
    assert_eq!(doc.text(), "a\nb");
    assert_eq!(doc.revision(), revision);

    doc.apply_batch(revision, vec![LargeEdit::new(0..1, "A")])
        .unwrap();
    assert!(doc.undo(doc.revision()).unwrap());
    assert_eq!(doc.text(), "a\nb");
}

#[test]
fn line_limit_is_enforced_while_building_index_and_byte_limit_precedes_indexing() {
    let config = LargeDocumentConfig {
        max_bytes: 2,
        max_lines: 1_000_000,
        ..Default::default()
    };
    assert!(matches!(
        LargeDocument::new("a\n\nb\n", config),
        Err(TextError::LengthLimit)
    ));

    let config = LargeDocumentConfig {
        max_lines: 2,
        ..Default::default()
    };
    assert!(matches!(
        LargeDocument::new("a\nb\nc", config),
        Err(TextError::LengthLimit)
    ));
}

#[test]
fn unrecordable_edit_creates_history_barrier_instead_of_replaying_stale_ranges() {
    let config = LargeDocumentConfig {
        history_bytes_limit: 2,
        ..Default::default()
    };
    let mut doc = LargeDocument::new("abcde", config).unwrap();
    let rev = doc.revision();
    let rev = doc
        .apply_batch(rev, vec![LargeEdit::new(0..1, "x")])
        .unwrap();
    assert!(doc.undo(rev).unwrap());
    let rev = doc.redo(doc.revision()).unwrap();
    assert!(rev);
    let rev = doc.revision();
    let rev = doc
        .apply_batch(rev, vec![LargeEdit::new(0..5, "123456")])
        .unwrap();
    assert_eq!(doc.text(), "123456");
    assert!(!doc.undo(rev).unwrap());
    assert_eq!(doc.text(), "123456");
}

#[test]
fn batch_edit_count_and_duplicate_insertions_are_bounded_and_atomic() {
    let config = LargeDocumentConfig {
        max_batch_edits: 1,
        ..Default::default()
    };
    let mut doc = LargeDocument::new("abcd", config).unwrap();
    let rev = doc.revision();
    assert_eq!(
        doc.apply_batch(
            rev,
            vec![LargeEdit::new(0..0, "x"), LargeEdit::new(2..2, "y")]
        ),
        Err(TextError::LengthLimit)
    );
    assert_eq!(doc.text(), "abcd");
    assert_eq!(doc.revision(), rev);

    let mut doc = document("abcd");
    let rev = doc.revision();
    assert_eq!(
        doc.apply_batch(
            rev,
            vec![LargeEdit::new(1..1, "x"), LargeEdit::new(1..1, "y")]
        ),
        Err(TextError::InvalidRange)
    );
    assert_eq!(doc.text(), "abcd");
    assert_eq!(doc.revision(), rev);
}

#[test]
fn undo_groups_adjacent_replacements_whose_output_ranges_collapse_or_touch() {
    let mut doc = document("abcd");
    let rev = doc.revision();
    let rev = doc
        .apply_batch(
            rev,
            vec![
                LargeEdit::new(0..1, ""),
                LargeEdit::new(1..2, ""),
                LargeEdit::new(2..3, "X"),
            ],
        )
        .unwrap();
    assert_eq!(doc.text(), "Xd");
    assert!(doc.undo(rev).unwrap());
    let rev = doc.revision();
    assert_eq!(doc.text(), "abcd");
    assert!(doc.redo(rev).unwrap());
    assert_eq!(doc.text(), "Xd");
}
