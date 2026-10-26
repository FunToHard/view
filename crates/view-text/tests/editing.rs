use view_text::*;

fn field(value: &str) -> PlainEditor {
    PlainEditor::new(value, EditorConfig::default()).unwrap()
}
fn select(e: &mut PlainEditor, anchor: usize, focus: usize) {
    EditorSession::select(
        e,
        e.snapshot().text.revision(),
        Selection {
            anchor: TextIndex(anchor),
            focus: TextIndex(focus),
        },
    )
    .unwrap();
}
fn value(e: &PlainEditor) -> String {
    e.snapshot().text.text().into()
}

#[test]
fn reaffirmed_focus_preserves_blink_and_one_typing_undo_group() {
    let mut editor = field("");
    editor.set_focused(true, 0);
    editor.insert("a", EditKind::Typing, 0).unwrap();
    editor.set_focused(true, 200);
    editor.set_focused(true, 450);
    assert!(!editor.caret_visible(500));
    editor.insert("b", EditKind::Typing, 600).unwrap();
    editor.set_focused(true, 700);
    assert_eq!(value(&editor), "ab");
    assert!(editor.undo().unwrap());
    assert_eq!(value(&editor), "");
    assert!(editor.redo().unwrap());
    assert_eq!(value(&editor), "ab");
}

#[test]
fn actual_focus_transitions_break_typing_and_blur_reaffirmation_cancels_preedit() {
    let mut editor = field("");
    editor.set_focused(true, 0);
    editor.insert("a", EditKind::Typing, 0).unwrap();
    editor.set_focused(false, 10);
    editor.set_focused(true, 20);
    assert!(editor.caret_visible(20));
    assert!(!editor.caret_visible(520));
    editor.insert("b", EditKind::Typing, 30).unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(value(&editor), "a");
    editor
        .preedit("temporary", Selection::caret(TextIndex(9)))
        .unwrap();
    editor.set_focused(false, 40);
    assert!(editor.snapshot().composition.is_none());
    editor
        .preedit("controlled", Selection::caret(TextIndex(10)))
        .unwrap();
    editor.set_focused(false, 50);
    assert!(editor.snapshot().composition.is_none());
    assert_eq!(value(&editor), "a");
    editor.insert("b", EditKind::Typing, 60).unwrap();
    editor.set_focused(false, 70);
    editor.insert("c", EditKind::Typing, 80).unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(value(&editor), "a");
    editor.set_focused(true, 90);
    editor.insert("d", EditKind::Typing, 100).unwrap();
    editor.break_group();
    editor.insert("e", EditKind::Typing, 110).unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(value(&editor), "ad");
}

#[test]
fn caret_deadlines_follow_phase_and_stop_for_selection_preedit_blur_and_overflow() {
    let mut editor = field("abc");
    assert_eq!(editor.next_caret_blink(0), None);
    editor.set_focused(true, 100);
    for (now, deadline, visible) in [
        (100, 600, true),
        (599, 600, true),
        (600, 1100, false),
        (1099, 1100, false),
        (1100, 1600, true),
    ] {
        assert_eq!(editor.next_caret_blink(now), Some(deadline));
        assert_eq!(editor.caret_visible(now), visible);
    }
    editor.set_focused(true, 700);
    assert_eq!(editor.next_caret_blink(700), Some(1100));
    select(&mut editor, 0, 2);
    assert_eq!(editor.next_caret_blink(700), None);
    select(&mut editor, 1, 1);
    editor
        .preedit("IME", Selection::caret(TextIndex(3)))
        .unwrap();
    assert!(editor.caret_visible(700));
    assert_eq!(editor.next_caret_blink(700), None);
    editor.cancel_composition();
    assert_eq!(editor.next_caret_blink(700), Some(1100));
    editor.set_focused(false, 800);
    assert_eq!(editor.next_caret_blink(800), None);
    editor.set_focused(true, 900);
    assert_eq!(editor.next_caret_blink(900), Some(1400));
    editor.set_modes(false, true);
    assert_eq!(editor.next_caret_blink(900), None);
    editor.set_modes(false, false);
    editor.set_focused(true, 0);
    assert_eq!(editor.next_caret_blink(u64::MAX), None);
    editor.set_focused(false, u64::MAX - 400);
    editor.set_focused(true, u64::MAX - 400);
    assert!(editor.caret_visible(u64::MAX - 400));
    assert_eq!(editor.next_caret_blink(u64::MAX - 400), None);
    editor.set_focused(false, u64::MAX - 500);
    editor.set_focused(true, u64::MAX - 500);
    assert_eq!(editor.next_caret_blink(u64::MAX - 500), Some(u64::MAX));
    assert_eq!(editor.next_caret_blink(u64::MAX), None);
}

#[test]
fn grapheme_deletion_and_word_boundaries_have_independent_expectations() {
    let mut e = field("a👩‍💻e\u{301}🇯🇵");
    e.move_caret(Movement::DocumentEnd, false, 0).unwrap();
    for expected in ["a👩‍💻e\u{301}", "a👩‍💻", "a", "", ""] {
        e.delete(true, false, 0).unwrap();
        assert_eq!(value(&e), expected);
    }
    let mut e = field("one, two three");
    assert_eq!(next_word(&value(&e), 0), 5);
    e.delete(false, true, 0).unwrap();
    assert_eq!(value(&e), "two three");
    e.move_caret(Movement::DocumentEnd, false, 0).unwrap();
    e.delete(true, true, 0).unwrap();
    assert_eq!(value(&e), "two ");
}

#[test]
fn insertion_snaps_after_combining_and_zwj_cluster_merges() {
    let mut combining = field("ae");
    combining
        .move_caret(Movement::DocumentStart, false, 0)
        .unwrap();
    combining.move_caret(Movement::Forward, false, 0).unwrap();
    combining.insert("\u{301}", EditKind::Typing, 1).unwrap();
    assert_eq!(value(&combining), "a\u{301}e");
    assert_eq!(
        combining.snapshot().selection,
        Selection::caret(TextIndex(3))
    );

    let mut zwj = field("👩💻");
    zwj.move_caret(Movement::DocumentStart, false, 0).unwrap();
    zwj.move_caret(Movement::Forward, false, 0).unwrap();
    zwj.insert("\u{200d}", EditKind::Typing, 1).unwrap();
    assert_eq!(value(&zwj), "👩‍💻");
    assert_eq!(
        zwj.snapshot().selection,
        Selection::caret(TextIndex("👩‍💻".len()))
    );
}

#[test]
fn replacement_direction_shift_selection_and_word_selection() {
    let mut e = field("abcd word");
    select(&mut e, 3, 1);
    e.insert("X", EditKind::Typing, 0).unwrap();
    assert_eq!(value(&e), "aXd word");
    e.move_caret(Movement::Forward, true, 0).unwrap();
    assert_eq!(
        e.snapshot().selection,
        Selection {
            anchor: TextIndex(2),
            focus: TextIndex(3)
        }
    );
    e.move_caret(Movement::Backward, false, 0).unwrap();
    assert_eq!(e.snapshot().selection.focus, TextIndex(2));
    e.select_word(TextIndex(5)).unwrap();
    assert_eq!(e.snapshot().selection.range(), 4..8);
}

#[test]
fn limits_validation_and_rejected_transactions_preserve_state() {
    let mut e = PlainEditor::new(
        "a",
        EditorConfig {
            max_graphemes: Some(2),
            ..Default::default()
        },
    )
    .unwrap();
    e.move_caret(Movement::End, false, 0).unwrap();
    e.insert("😀", EditKind::Typing, 0).unwrap();
    let before = e.snapshot();
    assert_eq!(
        e.insert("b", EditKind::Typing, 0),
        Err(TextError::LengthLimit)
    );
    assert_eq!(e.snapshot(), before);
    e.set_validator(|text| !text.contains('!')).unwrap();
    e.select_all().unwrap();
    let before = e.snapshot();
    assert_eq!(
        e.insert("!", EditKind::Typing, 0),
        Err(TextError::Validation)
    );
    assert_eq!(e.snapshot(), before);
    let mut e = field("a");
    e.move_caret(Movement::End, false, 0).unwrap();
    e.insert("\u{301}", EditKind::Typing, 0).unwrap();
    assert_eq!(e.snapshot().selection.focus, TextIndex(3));
    assert!(
        EditorSession::select(
            &mut e,
            TextRevision::INITIAL.checked_next().unwrap(),
            Selection::caret(TextIndex(1))
        )
        .is_err()
    );
}

#[test]
fn typing_groups_paste_and_composition_undo_separately() {
    let mut e = field("");
    e.insert("a", EditKind::Typing, 10).unwrap();
    e.insert("b", EditKind::Typing, 20).unwrap();
    e.insert("C", EditKind::Paste, 30).unwrap();
    e.preedit("仮", Selection::caret(TextIndex(3))).unwrap();
    e.commit_composition("名", 40).unwrap();
    for expected in ["abC", "ab", ""] {
        assert!(e.undo().unwrap());
        assert_eq!(value(&e), expected);
    }
    for expected in ["ab", "abC", "abC名"] {
        assert!(e.redo().unwrap());
        assert_eq!(value(&e), expected);
    }
    e.undo().unwrap();
    e.insert("D", EditKind::Typing, 50).unwrap();
    assert!(!e.redo().unwrap());
}

#[test]
fn typing_timeout_and_navigation_break_groups() {
    let mut e = field("");
    e.insert("a", EditKind::Typing, 0).unwrap();
    e.insert("b", EditKind::Typing, 1001).unwrap();
    e.undo().unwrap();
    assert_eq!(value(&e), "a");
    e.move_caret(Movement::Home, false, 1002).unwrap();
    e.insert("x", EditKind::Typing, 1003).unwrap();
    e.undo().unwrap();
    assert_eq!(value(&e), "a");
    assert_eq!(e.snapshot().selection.focus, TextIndex(0));
}

#[test]
fn controlled_echo_and_external_replacement_preserve_or_cancel_preedit_explicitly() {
    let mut e = field("abc");
    select(&mut e, 1, 2);
    e.preedit("日本", Selection::caret(TextIndex(6))).unwrap();
    let before = e.snapshot();
    assert_eq!(
        e.external_value(
            before.text.revision(),
            "abc",
            ExternalPolicy::ResetSelection
        ),
        Ok(ValueUpdate::Echo)
    );
    assert_eq!(e.snapshot(), before);
    assert_eq!(e.display_snapshot().unwrap().text.text(), "a日本c");
    e.commit_composition("日", 0).unwrap();
    let after = e.snapshot();
    assert_eq!(
        e.external_value(
            before.text.revision(),
            "abc",
            ExternalPolicy::ResetSelection
        ),
        Err(TextError::StaleRevision)
    );
    assert_eq!(e.snapshot(), after);
    e.external_value(
        after.text.revision(),
        "z",
        ExternalPolicy::PreserveSelection,
    )
    .unwrap();
    assert_eq!(e.snapshot().selection.focus, TextIndex(1));
    assert!(!e.undo().unwrap());
}

#[test]
fn mode_clipboard_newline_and_failed_clipboard_policies() {
    let mut e = field("abc");
    e.select_all().unwrap();
    e.set_modes(true, false);
    let mut clipboard = MemoryClipboard::default();
    assert!(e.copy(&mut clipboard).unwrap());
    assert_eq!(clipboard.0, "abc");
    assert_eq!(e.cut(&mut clipboard, 0), Err(TextError::ReadOnly));
    e.set_modes(false, true);
    assert_eq!(e.copy(&mut clipboard), Err(TextError::Disabled));
    e.set_modes(false, false);
    clipboard.0 = "a\r\nb\rc".into();
    e.paste(&mut clipboard, 0).unwrap();
    assert_eq!(value(&e), "a b c");
    struct Broken;
    impl Clipboard for Broken {
        fn read(&mut self) -> Result<String, TextError> {
            Err(TextError::Clipboard)
        }
        fn write(&mut self, _: &str) -> Result<(), TextError> {
            Err(TextError::Clipboard)
        }
    }
    e.select_all().unwrap();
    let before = e.snapshot();
    assert_eq!(e.cut(&mut Broken, 0), Err(TextError::Clipboard));
    assert_eq!(e.snapshot(), before);
}

#[test]
fn preedit_cancel_invalid_cursor_and_commit_validation_are_atomic() {
    let mut e = field("ab");
    select(&mut e, 1, 2);
    let before = e.snapshot();
    assert_eq!(
        e.preedit("😀", Selection::caret(TextIndex(2))),
        Err(TextError::InvalidIndex)
    );
    assert_eq!(e.snapshot(), before);
    e.preedit("😀", Selection::caret(TextIndex(4))).unwrap();
    e.cancel_composition();
    assert_eq!(e.snapshot(), before);
    e.set_validator(|s| !s.contains('!')).unwrap();
    e.preedit("!", Selection::caret(TextIndex(1))).unwrap();
    let before = e.snapshot();
    assert_eq!(e.commit_composition("!", 0), Err(TextError::Validation));
    assert_eq!(e.snapshot(), before);
    e.set_focused(false, 0);
    assert!(e.snapshot().composition.is_none());
}

#[test]
fn multiline_navigation_tab_newline_and_blink() {
    let mut e = PlainEditor::new(
        "ab\r\ncd",
        EditorConfig {
            mode: LineMode::Multiline,
            tab: TabPolicy::Insert,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(value(&e), "ab\ncd");
    e.move_caret(Movement::DocumentEnd, false, 0).unwrap();
    e.move_caret(Movement::Home, false, 0).unwrap();
    assert_eq!(e.snapshot().selection.focus, TextIndex(3));
    assert!(e.tab(0).unwrap());
    assert!(e.newline(0).unwrap());
    assert_eq!(value(&e), "ab\n\t\ncd");
    e.set_focused(true, 100);
    assert!(e.caret_visible(599));
    assert!(!e.caret_visible(600));
    assert!(e.caret_visible(1100));
    let mut single = field("");
    assert!(!single.tab(0).unwrap());
    assert!(!single.newline(0).unwrap());
}

#[test]
fn generated_edit_sequences_keep_carets_valid_and_undo_round_trips() {
    // Deterministic generated properties, no external property-testing dependency.
    for seed in 0..32u64 {
        let mut state = seed + 1;
        let mut e = field("");
        for step in 0..100 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let before = e.snapshot();
            match (state >> 32) % 5 {
                0 => e.insert("a", EditKind::Command, step).unwrap(),
                1 => e.insert("👩‍💻", EditKind::Command, step).unwrap(),
                2 => e.delete(true, false, step).unwrap(),
                3 => e.move_caret(Movement::Backward, true, step).unwrap(),
                _ => e.move_caret(Movement::Forward, false, step).unwrap(),
            }
            let after = e.snapshot();
            validate_caret(after.text.text(), after.selection.anchor).unwrap();
            validate_caret(after.text.text(), after.selection.focus).unwrap();
            if after.text.text() != before.text.text() {
                assert!(e.undo().unwrap());
                assert_eq!(e.snapshot().text.text(), before.text.text());
                assert_eq!(e.snapshot().selection, before.selection);
                assert!(e.redo().unwrap());
                assert_eq!(e.snapshot().text.text(), after.text.text());
                assert_eq!(e.snapshot().selection, after.selection);
            }
        }
    }
}

#[test]
fn reusable_profile_runner_reports_native_gaps() {
    for profile in [Profile::SingleLineV1, Profile::PlainMultilineV1] {
        let mut e = PlainEditor::new(
            "",
            EditorConfig {
                mode: if profile == Profile::PlainMultilineV1 {
                    LineMode::Multiline
                } else {
                    LineMode::SingleLine
                },
                ..Default::default()
            },
        )
        .unwrap();
        let report = run_profile(&mut e, profile, "PlainEditor 0.1", std::env::consts::OS);
        assert!(!report.certified());
        assert_eq!(
            report
                .cases
                .iter()
                .filter(|(_, r)| *r == CaseResult::Passed)
                .count(),
            if profile == Profile::PlainMultilineV1 {
                10
            } else {
                9
            }
        );
        assert!(
            !report
                .cases
                .iter()
                .any(|(_, r)| matches!(r, CaseResult::Failed(_)))
        );
    }
}

#[test]
fn placeholder_hidden_preedit_cursor_and_history_budget() {
    let mut e = PlainEditor::new(
        "",
        EditorConfig {
            placeholder: "Name".into(),
            history_limit: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(e.placeholder(), Some("Name"));
    e.set_focused(true, 0);
    e.preedit("仮", Selection::caret(TextIndex(3))).unwrap();
    assert_eq!(e.placeholder(), None);
    assert!(e.caret_visible(500));
    e.set_composition_cursor_visible(false);
    assert!(!e.caret_visible(0));
    e.cancel_composition();
    assert_eq!(e.placeholder(), Some("Name"));
    e.insert("a", EditKind::Command, 0).unwrap();
    e.insert("b", EditKind::Command, 0).unwrap();
    assert!(e.undo().unwrap());
    assert_eq!(value(&e), "a");
    assert!(!e.undo().unwrap());
    let mut reject = PlainEditor::new(
        "",
        EditorConfig {
            newline: NewlinePolicy::Reject,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        reject.insert("a\nb", EditKind::Paste, 0),
        Err(TextError::Validation)
    );
    assert_eq!(value(&reject), "");
}

#[test]
fn undo_history_respects_byte_budget() {
    let mut oversized = PlainEditor::new(
        "large",
        EditorConfig {
            history_bytes_limit: 4,
            ..Default::default()
        },
    )
    .unwrap();
    oversized
        .move_caret(Movement::DocumentEnd, false, 0)
        .unwrap();
    oversized.insert("!", EditKind::Command, 0).unwrap();
    assert_eq!(value(&oversized), "large!");
    assert!(!oversized.undo().unwrap());

    let mut bounded = PlainEditor::new(
        "a",
        EditorConfig {
            history_bytes_limit: 3,
            ..Default::default()
        },
    )
    .unwrap();
    bounded.move_caret(Movement::DocumentEnd, false, 0).unwrap();
    bounded.insert("b", EditKind::Command, 0).unwrap();
    bounded.insert("c", EditKind::Command, 0).unwrap();
    bounded.insert("d", EditKind::Command, 0).unwrap();
    assert!(bounded.undo().unwrap());
    assert_eq!(value(&bounded), "abc");
    assert!(!bounded.undo().unwrap());
}
