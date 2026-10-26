use view_controls::view_text::{CosmicBackend, Selection, TextIndex, TextLayout};
use view_controls::*;
use view_core::{
    Description, Dirty, LogicalPoint, LogicalRect, LogicalSize, PointerButton, PointerEvent,
    PointerPhase, Runtime, ScaleFactor,
};

fn initial_layout() -> DockSnapshot {
    DockSnapshot {
        windows: vec![DockWindow {
            id: "main".into(),
            bounds: [0.0, 0.0, 1200.0, 800.0],
            root: DockNode::Split {
                axis: SplitAxis::Horizontal,
                ratio: 0.7,
                first: Box::new(DockNode::Stack {
                    id: "editor".into(),
                    tabs: vec!["doc-a".into(), "doc-b".into()],
                    active: "doc-a".into(),
                }),
                second: Box::new(DockNode::Stack {
                    id: "inspector".into(),
                    tabs: vec!["inspect".into()],
                    active: "inspect".into(),
                }),
            },
        }],
    }
}

#[test]
fn password_display_and_semantics_never_expose_value() {
    let mut runtime: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (_, id) = runtime
        .open_window(Description::new("password", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut field = PasswordField::new("Password").unwrap();
    field.set_value("p🔐ss").unwrap();
    assert_eq!(field.display_text(), "••••");
    assert_eq!(field.expose_value(), "p🔐ss");
    assert_eq!(
        field
            .semantics(
                id,
                LogicalRect::new(LogicalPoint::new(0.0, 0.0), LogicalSize::new(100.0, 30.0)),
            )
            .node
            .value,
        None
    );
    field.set_revealed(true).unwrap();
    assert_eq!(field.display_text(), "p🔐ss");
    field.set_modes(false, true);
    assert!(!field.revealed());
    assert!(field.set_value("other").is_err());
}

#[test]
fn password_masks_graphemes_and_blocks_clipboard_copy_cut_but_keeps_undo() {
    let mut field = PasswordField::new("Secret").unwrap();
    field.insert_text("e\u{301}👩‍👩‍👧‍👦x", 1).unwrap();
    assert_eq!(field.display_text(), "•••");
    assert_eq!(field.expose_value(), "e\u{301}👩‍👩‍👧‍👦x");
    let mut clipboard = view_text::MemoryClipboard("sentinel".into());
    field
        .command(TextCommand::SelectAll, &mut clipboard, 2)
        .unwrap();
    assert!(field.command(TextCommand::Copy, &mut clipboard, 3).is_err());
    assert!(field.command(TextCommand::Cut, &mut clipboard, 4).is_err());
    assert_eq!(clipboard.0, "sentinel");
    field
        .command(TextCommand::Delete, &mut clipboard, 5)
        .unwrap();
    assert_eq!(field.expose_value(), "");
    field.command(TextCommand::Undo, &mut clipboard, 6).unwrap();
    assert_eq!(field.expose_value(), "e\u{301}👩‍👩‍👧‍👦x");
}

#[test]
fn password_preparation_shapes_masks_and_maps_selection_and_pointer_to_raw_graphemes() {
    let mut field = PasswordField::new("Secret").unwrap();
    let secret = "e\u{301}👩‍👩‍👧‍👦x";
    field.set_value(secret).unwrap();
    let mut clipboard = view_text::MemoryClipboard::default();
    field
        .command(TextCommand::SelectAll, &mut clipboard, 1)
        .unwrap();
    field.set_focused(true);
    let mut backend = CosmicBackend::from_fonts(
        [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        32,
    )
    .unwrap();
    let presentation = field
        .prepare(
            &mut backend,
            &Style::default(),
            LogicalRect::new(LogicalPoint::new(0.0, 0.0), LogicalSize::new(220.0, 36.0)),
            ScaleFactor::ONE,
            2,
        )
        .unwrap();
    assert_eq!(presentation.layout.text().text(), "•••");
    assert_eq!(
        presentation.selection.unwrap(),
        Selection {
            anchor: TextIndex(0),
            focus: TextIndex("•••".len())
        }
    );

    // A click at the far end of the masked layout maps to the raw UTF-8 end.
    let content = presentation.clip;
    let x = content.right() - 2.0;
    field
        .pointer(
            PointerEvent {
                phase: PointerPhase::Down,
                position: LogicalPoint::new(x, content.center().y),
                button: Some(PointerButton::Primary),
                modifiers: Default::default(),
                timestamp: 3,
            },
            content,
        )
        .unwrap();
    field
        .pointer(
            PointerEvent {
                phase: PointerPhase::Up,
                position: LogicalPoint::new(x, content.center().y),
                button: Some(PointerButton::Primary),
                modifiers: Default::default(),
                timestamp: 4,
            },
            content,
        )
        .unwrap();
    assert_eq!(field.expose_value(), secret);
    let mut runtime: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (_, id) = runtime
        .open_window(Description::new("password", (), |_, _, _| Dirty::NONE))
        .unwrap();
    assert_eq!(
        field.semantics(id, content).selection.unwrap().focus,
        TextIndex(secret.len())
    );
}

#[test]
fn docking_moves_tears_off_and_round_trips_snapshot_atomically() {
    let mut layout = DockLayout::new(initial_layout()).unwrap();
    layout.move_tab("doc-b", "inspector").unwrap();
    assert_eq!(layout.snapshot(), layout.clone().snapshot());
    layout
        .tear_off("doc-b", "float-1", [50.0, 60.0, 640.0, 480.0])
        .unwrap();
    let saved = layout.snapshot();
    let restored = DockLayout::restore(saved.clone()).unwrap();
    assert_eq!(restored.snapshot(), saved);
    assert_eq!(restored.windows().len(), 2);
    assert!(
        layout
            .tear_off("missing", "float-2", [0.0, 0.0, 100.0, 100.0])
            .is_err()
    );
    assert_eq!(layout.snapshot(), saved);
}

#[test]
fn revealed_password_pointer_uses_unmasked_utf8_positions() {
    let mut field = PasswordField::new("Secret").unwrap();
    let secret = "éabcdef";
    field.set_value(secret).unwrap();
    field.set_revealed(true).unwrap();
    let mut backend = CosmicBackend::from_fonts(
        [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        8,
    )
    .unwrap();
    let presentation = field
        .prepare(
            &mut backend,
            &Style::default(),
            LogicalRect::new(LogicalPoint::ZERO, LogicalSize::new(220.0, 36.0)),
            ScaleFactor::ONE,
            0,
        )
        .unwrap();
    assert_eq!(presentation.layout.text().text(), secret);
    let content = presentation.clip;
    field
        .pointer(
            PointerEvent {
                phase: PointerPhase::Down,
                position: LogicalPoint::new(content.right() - 1.0, content.center().y),
                button: Some(PointerButton::Primary),
                modifiers: Default::default(),
                timestamp: 1,
            },
            content,
        )
        .unwrap();
    field.insert_text("!", 2).unwrap();
    assert_eq!(field.expose_value(), "éabcdef!");
}

#[test]
fn docking_rejects_duplicate_or_invalid_persisted_layouts() {
    let mut snapshot = initial_layout();
    snapshot.windows.push(snapshot.windows[0].clone());
    assert!(DockLayout::restore(snapshot).is_err());
    let invalid = DockSnapshot {
        windows: vec![DockWindow {
            id: "bad".into(),
            bounds: [0.0, 0.0, f32::NAN, 10.0],
            root: DockNode::Stack {
                id: "s".into(),
                tabs: vec![],
                active: String::new(),
            },
        }],
    };
    assert!(DockLayout::restore(invalid).is_err());
}

#[test]
fn docking_codec_round_trips_and_rejects_unknown_or_trailing_data() {
    let snapshot = initial_layout();
    let bytes = snapshot.encode().unwrap();
    assert_eq!(DockSnapshot::decode(&bytes).unwrap(), snapshot);
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(DockSnapshot::decode(&trailing).is_err());
    assert!(DockSnapshot::decode(b"NOPE").is_err());
    // A tiny untrusted input must not preallocate a million windows.
    let mut hostile = b"VDK1".to_vec();
    hostile.extend_from_slice(&1_000_000_u32.to_le_bytes());
    assert!(DockSnapshot::decode(&hostile).is_err());
}

#[test]
fn docking_reconciliation_suspends_inactive_panels_and_revokes_focus_and_capture() {
    let mut layout = DockLayout::new(initial_layout()).unwrap();
    let mut runtime: Runtime<(), ()> = Runtime::new(8).unwrap();
    let mut panels = std::collections::HashMap::new();
    let mut doc_a = None;
    for tab in ["doc-a", "doc-b", "inspect"] {
        let (window, root) = runtime
            .open_window(
                Description::new(tab, (), |_, _, _| Dirty::NONE).role(view_core::Role::Container),
            )
            .unwrap();
        panels.insert(tab.to_owned(), DockPanelHandle { window, root });
        if tab == "doc-a" {
            doc_a = Some((window, root));
        }
    }
    layout.reconcile_runtime(&mut runtime, &panels).unwrap();
    let (window, root) = doc_a.unwrap();
    runtime.set_focus(window, Some(root)).unwrap();
    runtime.capture_pointer(window, root).unwrap();
    layout.activate("editor", "doc-b").unwrap();
    let actions = layout.reconcile_runtime(&mut runtime, &panels).unwrap();
    assert!(!actions.is_empty());
    assert_eq!(runtime.focused(window).unwrap(), None);
    assert_eq!(runtime.captured(window).unwrap(), None);
    runtime.flush(&mut ()).unwrap();
    assert_eq!(
        runtime
            .snapshot()
            .nodes
            .iter()
            .find(|node| node.id == root)
            .unwrap()
            .visibility,
        view_core::Visibility::Suspended
    );
}

#[test]
fn docking_commands_validate_before_effects_and_reject_stale_plans() {
    let mut layout = DockLayout::new(initial_layout()).unwrap();
    let original = layout.snapshot();
    assert!(layout.move_tab("doc-a", "missing").is_err());
    assert!(
        layout
            .tear_off("doc-a", "main", [0.0, 0.0, 50.0, 50.0])
            .is_err()
    );
    assert!(
        layout
            .tear_off("doc-a", "new", [0.0, 0.0, f32::INFINITY, 50.0])
            .is_err()
    );
    assert_eq!(layout.snapshot(), original);
    let plan = layout
        .plan(DockCommand::TearOff {
            tab: "doc-a".into(),
            window: "detached".into(),
            bounds: [-600.0, 40.0, 500.0, 400.0],
        })
        .unwrap();
    assert_eq!(
        layout.snapshot(),
        original,
        "planning performs no model effects"
    );
    assert_eq!(
        plan.migrations(),
        &[DockPanelMigration {
            tab: "doc-a".into(),
            source_window: "main".into(),
            target_window: "detached".into(),
        }]
    );
    layout.activate("editor", "doc-b").unwrap();
    let changed = layout.snapshot();
    assert!(layout.commit(plan).is_err());
    assert_eq!(layout.snapshot(), changed);
    let plan = layout
        .plan(DockCommand::MoveTab {
            tab: "doc-a".into(),
            target_stack: "inspector".into(),
        })
        .unwrap();
    assert!(
        plan.migrations().is_empty(),
        "same native window needs no migration"
    );
    layout.commit(plan).unwrap();
    let plan = layout
        .plan(DockCommand::TearOff {
            tab: "doc-a".into(),
            window: "detached".into(),
            bounds: [50.0, 50.0, 500.0, 400.0],
        })
        .unwrap();
    layout.commit(plan).unwrap();
    let plan = layout
        .plan(DockCommand::MoveTab {
            tab: "doc-a".into(),
            target_stack: "editor".into(),
        })
        .unwrap();
    assert_eq!(plan.migrations()[0].source_window, "detached");
    assert_eq!(plan.migrations()[0].target_window, "main");
    layout.commit(plan).unwrap();
    assert_eq!(
        layout.windows().len(),
        2,
        "empty window IDs retained by model policy"
    );
}

#[test]
fn docking_restore_selects_monitor_clamps_outer_bounds_and_handles_removed_monitors() {
    let areas = [
        DockWorkArea {
            bounds: [-1920.0, -100.0, 1920.0, 1040.0],
        },
        DockWorkArea {
            bounds: [0.0, 40.0, 1600.0, 860.0],
        },
    ];
    let mut saved = initial_layout();
    saved.windows[0].bounds = [-1500.0, -500.0, 1300.0, 1500.0];
    let restored =
        DockLayout::restore_in_work_areas(saved.clone(), &areas, [360.0, 240.0]).unwrap();
    assert_eq!(
        restored.windows()[0].bounds,
        [-1500.0, -100.0, 1300.0, 1040.0]
    );
    saved.windows[0].bounds = [20_000.0, 20_000.0, 1200.0, 800.0];
    let restored =
        DockLayout::restore_in_work_areas(saved.clone(), &areas, [360.0, 240.0]).unwrap();
    assert_eq!(restored.windows()[0].bounds, [400.0, 100.0, 1200.0, 800.0]);
    saved.windows[0].bounds = [-2000.0, -100.0, 1.0, 1.0];
    let tiny = [DockWorkArea {
        bounds: [-100.0, 10.0, 200.0, 100.0],
    }];
    let restored = DockLayout::restore_in_work_areas(saved.clone(), &tiny, [360.0, 240.0]).unwrap();
    assert_eq!(restored.windows()[0].bounds, [-100.0, 10.0, 200.0, 100.0]);
    // Equal positive overlap follows monitor enumeration order even if the
    // second monitor's center is closer to the saved center.
    saved.windows[0].bounds = [50.0, 0.0, 100.0, 100.0];
    let tied = [
        DockWorkArea {
            bounds: [100.0, 0.0, 1000.0, 100.0],
        },
        DockWorkArea {
            bounds: [0.0, 0.0, 100.0, 100.0],
        },
    ];
    let restored = DockLayout::restore_in_work_areas(saved.clone(), &tied, [10.0, 10.0]).unwrap();
    assert_eq!(restored.windows()[0].bounds, [100.0, 0.0, 100.0, 100.0]);
    assert!(DockLayout::restore_in_work_areas(saved.clone(), &[], [360.0, 240.0]).is_err());
    assert!(DockLayout::restore_in_work_areas(saved.clone(), &areas, [0.0, 240.0]).is_err());
    for bounds in [
        [0.0, 0.0, f32::NAN, 100.0],
        [0.0, 0.0, 100.0, -1.0],
        [f32::MAX, 0.0, f32::MAX, 10.0],
    ] {
        assert!(
            DockLayout::restore_in_work_areas(
                saved.clone(),
                &[DockWorkArea { bounds }],
                [360.0, 240.0]
            )
            .is_err()
        );
    }
}

#[test]
fn docking_codec_rejects_every_truncation_and_duplicate_generated_stack_ids_atomically() {
    let bytes = initial_layout().encode().unwrap();
    for end in 0..bytes.len() {
        assert!(
            DockSnapshot::decode(&bytes[..end]).is_err(),
            "truncation at {end}"
        );
    }
    let mut saved = initial_layout();
    if let DockNode::Split { second, .. } = &mut saved.windows[0].root
        && let DockNode::Stack { id, .. } = &mut **second
    {
        *id = "stack:doc-a".into();
    }
    let mut layout = DockLayout::new(saved.clone()).unwrap();
    assert!(
        layout
            .tear_off("doc-a", "detached", [20.0, 20.0, 400.0, 300.0])
            .is_err()
    );
    assert_eq!(layout.snapshot(), saved);
}

#[test]
fn docking_reconciliation_prevalidates_wrong_window_stale_and_overlapping_handles() {
    use std::collections::HashMap;
    let mut runtime: Runtime<(), ()> = Runtime::new(8).unwrap();
    let (window, root) = runtime
        .open_window(Description::new("main", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let (other_window, other_root) = runtime
        .open_window(Description::new("other", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let a = runtime
        .mount(root, Description::new("a", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let b = runtime
        .mount(root, Description::new("b", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let inspect = runtime
        .mount(root, Description::new("inspect", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let panels = HashMap::from([
        ("doc-a".into(), DockPanelHandle { window, root: a }),
        ("doc-b".into(), DockPanelHandle { window, root: b }),
        (
            "inspect".into(),
            DockPanelHandle {
                window,
                root: inspect,
            },
        ),
    ]);
    let mut layout = DockLayout::new(initial_layout()).unwrap();
    layout.reconcile_runtime(&mut runtime, &panels).unwrap();
    runtime.set_focus(window, Some(a)).unwrap();
    runtime.capture_pointer(window, a).unwrap();
    runtime.flush(&mut ()).unwrap();
    let before = runtime.snapshot().clone();
    layout.activate("editor", "doc-b").unwrap();
    let mut wrong_window = panels.clone();
    wrong_window.get_mut("inspect").unwrap().window = other_window;
    let mut overlapping = panels.clone();
    overlapping.get_mut("inspect").unwrap().root = root;
    let mut stale = panels.clone();
    runtime.close_window(other_window).unwrap();
    stale.insert(
        "inspect".into(),
        DockPanelHandle {
            window: other_window,
            root: other_root,
        },
    );
    for invalid in [wrong_window, overlapping, stale] {
        assert!(layout.reconcile_runtime(&mut runtime, &invalid).is_err());
        assert_eq!(runtime.focused(window).unwrap(), Some(a));
        assert_eq!(runtime.captured(window).unwrap(), Some(a));
        assert_eq!(
            runtime.snapshot().nodes.iter().find(|n| n.id == a).unwrap(),
            before.nodes.iter().find(|n| n.id == a).unwrap()
        );
    }
    runtime.flush(&mut ()).unwrap();
    assert_eq!(
        runtime
            .snapshot()
            .nodes
            .iter()
            .find(|n| n.id == a)
            .unwrap()
            .visibility,
        view_core::Visibility::Visible
    );
}
