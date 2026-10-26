use view_text::*;

fn doc(text: &str) -> RichDocument {
    RichDocument::new(
        vec![RichParagraph::plain(text)],
        RichDocumentConfig::default(),
    )
    .unwrap()
}
fn runs(text: &str) -> Vec<RichRun> {
    vec![RichRun::new(text, RichStyle::default())]
}
fn replace(paragraph: usize, range: std::ops::Range<usize>, text: &str) -> RichOperation {
    RichOperation::Replace {
        paragraph,
        range,
        runs: runs(text),
    }
}
fn remote(
    base_revision: TextRevision,
    paragraph: usize,
    range: std::ops::Range<usize>,
    text: &str,
) -> RichRemoteEdit {
    RichRemoteEdit {
        base_revision,
        paragraph,
        range,
        runs: runs(text),
    }
}
fn text(document: &RichDocument) -> String {
    document.paragraphs()[0].text()
}

#[test]
fn styled_transaction_splits_and_coalesces_runs_and_restores_all_properties() {
    let mut document = doc("ab😀cd");
    let bold = RichStyle {
        bold: true,
        ..RichStyle::default()
    };
    let format = ParagraphFormat {
        keep_together: true,
        space_before: 4.0,
        ..ParagraphFormat::default()
    };
    document
        .apply(
            document.revision(),
            vec![
                RichOperation::Style {
                    paragraph: 0,
                    range: 1..6,
                    style: bold,
                },
                replace(0, 6..7, "Z"),
                RichOperation::Format {
                    paragraph: 0,
                    format,
                },
                RichOperation::InsertParagraph {
                    index: 1,
                    paragraph: RichParagraph::plain("second"),
                },
            ],
        )
        .unwrap();
    assert_eq!(text(&document), "ab😀Zd");
    assert_eq!(document.paragraphs()[0].runs.len(), 3);
    assert_eq!(document.paragraphs()[0].runs[1].text, "b😀");
    assert_eq!(document.paragraphs()[0].runs[1].style, bold);
    assert_eq!(document.paragraphs()[0].format, format);
    let committed = document.paragraphs().to_vec();
    let first_revision = document.revision();
    assert!(document.undo(first_revision).unwrap());
    assert_eq!(text(&document), "ab😀cd");
    assert_eq!(document.paragraphs().len(), 1);
    assert_eq!(document.paragraphs()[0].format, ParagraphFormat::default());
    assert!(document.redo(document.revision()).unwrap());
    assert_eq!(document.paragraphs(), committed);
    assert!(document.revision() > first_revision);
}

#[test]
fn failed_late_operation_is_atomic_for_value_revision_and_redo() {
    let mut document = doc("abc");
    document
        .apply(document.revision(), vec![replace(0, 0..1, "A")])
        .unwrap();
    document.undo(document.revision()).unwrap();
    let base = document.revision();
    let bytes = document.history_bytes();
    assert_eq!(
        document.apply(base, vec![replace(0, 0..1, "Q"), replace(9, 0..0, "bad")]),
        Err(RichDocumentError::Text(TextError::InvalidIndex))
    );
    assert_eq!(text(&document), "abc");
    assert_eq!(document.revision(), base);
    assert_eq!(document.history_bytes(), bytes);
    assert!(document.can_redo());
    document.redo(base).unwrap();
    assert_eq!(text(&document), "Abc");
}

#[test]
fn unicode_edit_and_style_boundaries_are_graphemes_including_cross_run_clusters() {
    let mut document = doc("e\u{301}👩‍💻!");
    let bold = RichStyle {
        bold: true,
        ..RichStyle::default()
    };
    for operation in [
        replace(0, 1..1, "x"),
        replace(0, 4..4, "x"),
        RichOperation::Style {
            paragraph: 0,
            range: 0..1,
            style: bold,
        },
    ] {
        assert_eq!(
            document.apply(document.revision(), vec![operation]),
            Err(RichDocumentError::Text(TextError::InvalidIndex))
        );
    }
    assert_eq!(
        RichDocument::new(
            vec![RichParagraph {
                runs: vec![
                    RichRun::new("e", bold),
                    RichRun::new("\u{301}", RichStyle::default())
                ],
                ..RichParagraph::default()
            }],
            RichDocumentConfig::default()
        )
        .unwrap_err(),
        RichDocumentError::Text(TextError::InvalidIndex)
    );
    let end = "e\u{301}👩‍💻".len();
    document
        .apply(document.revision(), vec![replace(0, 3..end, "😀")])
        .unwrap();
    assert_eq!(text(&document), "e\u{301}😀!");
}

#[test]
fn empty_runs_merge_and_combining_insert_preserves_style_safety() {
    let mut document = doc("e");
    document
        .apply(document.revision(), vec![replace(0, 1..1, "\u{301}")])
        .unwrap();
    assert_eq!(text(&document), "e\u{301}");
    assert_eq!(document.paragraphs()[0].runs.len(), 1);
    let bold = RichStyle {
        bold: true,
        ..RichStyle::default()
    };
    let base = document.revision();
    assert!(
        document
            .apply(
                base,
                vec![RichOperation::Replace {
                    paragraph: 0,
                    range: 3..3,
                    runs: vec![RichRun::new("\u{301}", bold)]
                }]
            )
            .is_err()
    );
    assert_eq!(document.revision(), base);
    document.apply(base, vec![replace(0, 0..3, "")]).unwrap();
    assert!(document.paragraphs()[0].runs.is_empty());
}

#[test]
fn structure_and_metric_validation_and_removal_preserve_nonempty_document() {
    let mut document = doc("x");
    for operation in [
        replace(0, 0..0, "a\nb"),
        replace(0, 0..0, "\r"),
        RichOperation::RemoveParagraph { index: 0 },
        RichOperation::Format {
            paragraph: 0,
            format: ParagraphFormat {
                space_after: f64::NAN,
                ..ParagraphFormat::default()
            },
        },
        RichOperation::Style {
            paragraph: 0,
            range: 0..1,
            style: RichStyle {
                font_size_milli: 0,
                ..RichStyle::default()
            },
        },
    ] {
        assert!(
            document
                .apply(document.revision(), vec![operation])
                .is_err()
        );
        assert_eq!(text(&document), "x");
    }
    document
        .apply(
            document.revision(),
            vec![
                RichOperation::InsertParagraph {
                    index: 0,
                    paragraph: RichParagraph::plain("new"),
                },
                RichOperation::RemoveParagraph { index: 1 },
            ],
        )
        .unwrap();
    assert_eq!(text(&document), "new");
    assert_eq!(document.paragraphs().len(), 1);
}

#[test]
fn noops_do_not_consume_revision_or_undo_and_stale_history_requests_fail() {
    let mut document = doc("abc");
    let base = document.revision();
    document.apply(base, vec![replace(0, 0..1, "a")]).unwrap();
    document.apply(base, vec![]).unwrap();
    assert_eq!(document.revision(), base);
    assert!(!document.can_undo());
    document.apply(base, vec![replace(0, 0..1, "X")]).unwrap();
    assert_eq!(
        document.undo(base),
        Err(RichDocumentError::Text(TextError::StaleRevision))
    );
    assert_eq!(
        document.apply(base, vec![]),
        Err(RichDocumentError::Text(TextError::StaleRevision))
    );
}

#[test]
fn configured_data_run_operation_and_paragraph_limits_reject_atomically() {
    let config = RichDocumentConfig {
        max_bytes: 4,
        max_paragraphs: 1,
        max_runs: 2,
        max_operations: 2,
        ..RichDocumentConfig::default()
    };
    let mut document = RichDocument::new(vec![RichParagraph::plain("abcd")], config).unwrap();
    let bold = RichStyle {
        bold: true,
        ..RichStyle::default()
    };
    for operations in [
        vec![replace(0, 4..4, "x")],
        vec![RichOperation::InsertParagraph {
            index: 1,
            paragraph: RichParagraph::default(),
        }],
        vec![RichOperation::Style {
            paragraph: 0,
            range: 1..2,
            style: bold,
        }],
        vec![
            replace(0, 0..0, ""),
            replace(0, 0..0, ""),
            replace(0, 0..0, ""),
        ],
    ] {
        assert_eq!(
            document.apply(document.revision(), operations),
            Err(RichDocumentError::Text(TextError::LengthLimit))
        );
        assert_eq!(text(&document), "abcd");
        assert!(!document.can_undo());
    }
}

#[test]
fn history_count_and_bytes_bound_undo_redo_and_oversized_edit_is_a_barrier() {
    let config = RichDocumentConfig {
        history_limit: 1,
        history_bytes_limit: 1024,
        ..RichDocumentConfig::default()
    };
    let mut document = RichDocument::new(vec![RichParagraph::plain("abc")], config).unwrap();
    document
        .apply(document.revision(), vec![replace(0, 0..1, "A")])
        .unwrap();
    document
        .apply(document.revision(), vec![replace(0, 1..2, "B")])
        .unwrap();
    let retained = document.history_bytes();
    assert!(retained <= 1024);
    document.undo(document.revision()).unwrap();
    assert_eq!(text(&document), "Abc");
    assert_eq!(document.history_bytes(), retained);
    assert!(!document.undo(document.revision()).unwrap());
    document.redo(document.revision()).unwrap();
    assert_eq!(text(&document), "ABc");
    document
        .apply(
            document.revision(),
            vec![replace(0, 3..3, &"x".repeat(2048))],
        )
        .unwrap();
    assert!(!document.can_undo());
    assert!(!document.can_redo());
    assert_eq!(document.history_bytes(), 0);
    document
        .apply(document.revision(), vec![replace(0, 0..1, "Q")])
        .unwrap();
    assert!(!document.can_undo());
}

#[test]
fn disjoint_remote_rebases_over_sequential_insert_delete_and_style_transactions() {
    let mut document = doc("one two three");
    let base = document.revision();
    document
        .apply(base, vec![replace(0, 0..3, "ONE!"), replace(0, 5..8, "2")])
        .unwrap();
    document
        .apply(
            document.revision(),
            vec![RichOperation::Style {
                paragraph: 0,
                range: 0..4,
                style: RichStyle {
                    bold: true,
                    ..RichStyle::default()
                },
            }],
        )
        .unwrap();
    document
        .apply_remote(remote(base, 0, 8..13, "THREE"))
        .unwrap();
    assert_eq!(text(&document), "ONE! 2 THREE");
    assert!(document.paragraphs()[0].runs[0].style.bold);
    document.undo(document.revision()).unwrap();
    assert_eq!(text(&document), "ONE! 2 three");
}

#[test]
fn remote_edits_on_other_paragraphs_do_not_conflict_with_formatting() {
    let mut document = RichDocument::new(
        vec![RichParagraph::plain("a"), RichParagraph::plain("b")],
        RichDocumentConfig::default(),
    )
    .unwrap();
    let base = document.revision();
    document
        .apply(
            base,
            vec![RichOperation::Format {
                paragraph: 0,
                format: ParagraphFormat {
                    page_break_before: true,
                    ..ParagraphFormat::default()
                },
            }],
        )
        .unwrap();
    document.apply_remote(remote(base, 1, 0..1, "B")).unwrap();
    assert_eq!(document.paragraphs()[1].text(), "B");
}

#[test]
fn overlaps_same_offset_insertions_and_ambiguous_touching_boundaries_conflict() {
    for (local, incoming) in [(0..2, 1..3), (1..1, 1..1), (1..1, 1..2), (0..1, 1..1)] {
        let mut document = doc("abcd");
        let base = document.revision();
        document.apply(base, vec![replace(0, local, "X")]).unwrap();
        let before = document.paragraphs().to_vec();
        let revision = document.revision();
        assert_eq!(
            document.apply_remote(remote(base, 0, incoming, "Y")),
            Err(RichDocumentError::Conflict)
        );
        assert_eq!(document.paragraphs(), before);
        assert_eq!(document.revision(), revision);
    }
    // Adjacent nonempty replacements are unambiguous.
    let mut document = doc("abcd");
    let base = document.revision();
    document.apply(base, vec![replace(0, 0..2, "A")]).unwrap();
    document.apply_remote(remote(base, 0, 2..4, "B")).unwrap();
    assert_eq!(text(&document), "AB");
}

#[test]
fn formatting_structure_and_undo_redo_are_conservative_rebase_barriers() {
    for operation in [
        RichOperation::Format {
            paragraph: 0,
            format: ParagraphFormat {
                space_after: 2.0,
                ..ParagraphFormat::default()
            },
        },
        RichOperation::InsertParagraph {
            index: 1,
            paragraph: RichParagraph::default(),
        },
    ] {
        let mut document = doc("abc");
        let base = document.revision();
        document.apply(base, vec![operation]).unwrap();
        assert_eq!(
            document.apply_remote(remote(base, 0, 0..1, "X")),
            Err(RichDocumentError::Conflict)
        );
    }
    let mut document = doc("abc");
    document
        .apply(document.revision(), vec![replace(0, 0..1, "A")])
        .unwrap();
    let base = document.revision();
    document.undo(base).unwrap();
    assert_eq!(
        document.apply_remote(remote(base, 0, 2..3, "C")),
        Err(RichDocumentError::Conflict)
    );
    let before_redo = document.revision();
    document.redo(before_redo).unwrap();
    assert_eq!(
        document.apply_remote(remote(before_redo, 0, 2..3, "C")),
        Err(RichDocumentError::Conflict)
    );
}

#[test]
fn expired_rebase_history_and_invalid_remote_ranges_never_mutate() {
    let config = RichDocumentConfig {
        rebase_history_limit: 1,
        ..RichDocumentConfig::default()
    };
    let mut document = RichDocument::new(vec![RichParagraph::plain("a😀z")], config).unwrap();
    let base = document.revision();
    document.apply(base, vec![replace(0, 0..1, "A")]).unwrap();
    let retained_base = document.revision();
    document
        .apply(retained_base, vec![replace(0, 0..1, "AA")])
        .unwrap();
    assert_eq!(
        document.apply_remote(remote(base, 0, 5..6, "Z")),
        Err(RichDocumentError::HistoryUnavailable)
    );
    assert_eq!(
        document.apply_remote(remote(retained_base, 0, 2..3, "Z")),
        Err(RichDocumentError::Text(TextError::InvalidIndex))
    );
    assert_eq!(
        document.apply_remote(remote(retained_base, 0, 50..51, "Z")),
        Err(RichDocumentError::Text(TextError::InvalidRange))
    );
    assert_eq!(
        document.apply_remote(remote(
            document.revision().checked_next().unwrap(),
            0,
            0..0,
            "x"
        )),
        Err(RichDocumentError::Text(TextError::StaleRevision))
    );
    assert_eq!(text(&document), "AA😀z");
}

#[test]
fn local_edit_after_undo_discards_redo_and_remote_uses_same_undo_authority() {
    let mut document = doc("abc");
    document
        .apply(document.revision(), vec![replace(0, 0..1, "A")])
        .unwrap();
    document.undo(document.revision()).unwrap();
    document
        .apply_remote(remote(document.revision(), 0, 2..3, "C"))
        .unwrap();
    assert!(!document.can_redo());
    document.undo(document.revision()).unwrap();
    assert_eq!(text(&document), "abc");
}

#[test]
fn invalid_base_grapheme_boundary_cannot_become_valid_by_rebasing() {
    // Regional indicators pair from the start of the run. A leading RI changes
    // later grapheme pairing without overlapping the remote byte offset.
    let mut document = doc("🇦🇧🇨");
    let base = document.revision();
    assert!(!grapheme_boundaries(&text(&document)).contains(&4));
    document.apply(base, vec![replace(0, 0..0, "🇩")]).unwrap();
    assert!(grapheme_boundaries(&text(&document)).contains(&8));
    let revision = document.revision();
    let before = document.paragraphs().to_vec();
    assert_eq!(
        document.apply_remote(remote(base, 0, 4..4, "X")),
        Err(RichDocumentError::Text(TextError::InvalidIndex))
    );
    assert_eq!(document.revision(), revision);
    assert_eq!(document.paragraphs(), before);
}

#[test]
fn rebase_byte_budget_bounds_sources_and_oversized_transition_invalidates_chain() {
    let config = RichDocumentConfig {
        rebase_bytes_limit: 512,
        ..RichDocumentConfig::default()
    };
    let mut document = RichDocument::new(vec![RichParagraph::plain("abc")], config).unwrap();
    let oldest = document.revision();
    document.apply(oldest, vec![replace(0, 0..1, "A")]).unwrap();
    assert!(document.rebase_bytes() > 0);
    document
        .apply(document.revision(), vec![replace(0, 1..2, "B")])
        .unwrap();
    assert!(document.rebase_bytes() <= 512);
    // One large incoming edit can retain its small base, but the next transition
    // cannot retain that large source. It must clear all older chain records.
    document
        .apply(
            document.revision(),
            vec![replace(0, 3..3, &"x".repeat(1024))],
        )
        .unwrap();
    let large_base = document.revision();
    document
        .apply(large_base, vec![replace(0, 0..1, "Q")])
        .unwrap();
    assert_eq!(document.rebase_bytes(), 0);
    assert_eq!(
        document.apply_remote(remote(oldest, 0, 2..3, "C")),
        Err(RichDocumentError::HistoryUnavailable)
    );
    assert_eq!(
        document.apply_remote(remote(large_base, 0, 2..3, "C")),
        Err(RichDocumentError::HistoryUnavailable)
    );
    // Current-base commands do not require historical metadata.
    document
        .apply_remote(remote(document.revision(), 0, 2..3, "C"))
        .unwrap();
    assert!(text(&document).starts_with("QBC"));
}

#[test]
fn aggregate_constructor_limits_precede_paragraph_normalization_and_validation() {
    let config = RichDocumentConfig {
        max_bytes: 1,
        max_runs: 1,
        ..RichDocumentConfig::default()
    };
    // The malformed first run would produce InvalidMetrics if normalization
    // were attempted before the aggregate run/byte preflight.
    let paragraph = RichParagraph {
        runs: vec![RichRun::new(
            "a",
            RichStyle {
                font_size_milli: 0,
                ..RichStyle::default()
            },
        )],
        ..RichParagraph::default()
    };
    assert_eq!(
        RichDocument::new(vec![paragraph, RichParagraph::plain("b")], config).unwrap_err(),
        RichDocumentError::Text(TextError::LengthLimit)
    );
}

#[test]
fn independent_remote_replacements_produce_same_content_in_either_order() {
    let mut left = doc("one two three");
    let mut right = left.clone();
    let base = left.revision();
    left.apply_remote(remote(base, 0, 0..3, "1")).unwrap();
    left.apply_remote(remote(base, 0, 8..13, "3")).unwrap();
    right.apply_remote(remote(base, 0, 8..13, "3")).unwrap();
    right.apply_remote(remote(base, 0, 0..3, "1")).unwrap();
    assert_eq!(text(&left), "1 two 3");
    assert_eq!(left.paragraphs(), right.paragraphs());
}
