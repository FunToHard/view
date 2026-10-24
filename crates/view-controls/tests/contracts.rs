use view_controls::*;
use view_core::*;
use view_text::*;

fn key(key: NamedKey, phase: KeyPhase) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(key),
        phase,
        modifiers: Modifiers::default(),
        timestamp: 20,
    }
}
fn pointer(phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        phase,
        position: LogicalPoint::new(x, y),
        button: Some(PointerButton::Primary),
        modifiers: Modifiers::default(),
        timestamp: 20,
    }
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
}
fn fonts() -> CosmicBackend {
    CosmicBackend::from_fonts(
        [include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        32,
    )
    .unwrap()
}
fn spec() -> RangeSpec {
    RangeSpec {
        min: 0.0,
        max: 10.0,
        step: 0.1,
        page: 1.0,
        decimals: 1,
    }
}
#[derive(Default)]
struct Clip(String);
impl Clipboard for Clip {
    fn read(&mut self) -> Result<String, TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, value: &str) -> Result<(), TextError> {
        self.0 = value.into();
        Ok(())
    }
}

#[test]
fn press_cancel_repeat_and_disabled_do_not_activate() {
    let mut button = Button::new("Save");
    button.behavior.set_focused(true);
    assert!(button.key(&key(NamedKey::Enter, KeyPhase::Down)));
    assert!(!button.key(&key(NamedKey::Enter, KeyPhase::Repeat)));
    assert!(!button.key(&key(NamedKey::Space, KeyPhase::Down)));
    button.behavior.set_focused(false);
    assert!(!button.key(&key(NamedKey::Space, KeyPhase::Up)));
    button.pointer(pointer(PointerPhase::Down, 0.0, 0.0), true);
    button.pointer(pointer(PointerPhase::Move, 100.0, 0.0), false);
    assert!(!button.pointer(pointer(PointerPhase::Up, 100.0, 0.0), false));
    button.pointer(pointer(PointerPhase::Down, 0.0, 0.0), true);
    button.behavior.set_disabled(true);
    assert!(!button.pointer(pointer(PointerPhase::Up, 0.0, 0.0), true));
    assert_eq!(button.activate(), Err(ControlError::Disabled));
}

#[test]
fn multiline_text_scroll_uses_prepared_layout_and_clamps_to_content() {
    let text = (0..40)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut area = TextArea::new("Document", &text).unwrap();
    let bounds = rect(0.0, 0.0, 240.0, 100.0);
    area.0
        .prepare(&mut fonts(), &Style::default(), bounds, ScaleFactor::ONE, 0)
        .unwrap();
    assert!(
        area.0
            .scroll_by(LogicalPoint::new(0.0, 48.0), bounds.size)
            .unwrap()
    );
    assert_eq!(area.0.editor().scroll_offset().y, 48.0);
    assert!(
        area.0
            .scroll_by(LogicalPoint::new(0.0, 100_000.0), bounds.size)
            .unwrap()
    );
    assert!(
        !area
            .0
            .scroll_by(LogicalPoint::new(0.0, 100_000.0), bounds.size)
            .unwrap()
    );
    assert!(area.0.editor().scroll_offset().y > 0.0);
}

#[test]
fn mixed_checkbox_and_radio_exclusivity() {
    let mut check = Checkbox::new("Use defaults", CheckState::Mixed, true).unwrap();
    assert_eq!(check.activate().unwrap(), CheckState::Off);
    assert_eq!(check.activate().unwrap(), CheckState::On);
    check.button.behavior.set_disabled(true);
    assert!(check.activate().is_err());
    let mut group = RadioGroup::new(
        vec![
            RadioOption {
                key: "a".into(),
                label: "A".into(),
                disabled: false,
            },
            RadioOption {
                key: "b".into(),
                label: "B".into(),
                disabled: true,
            },
            RadioOption {
                key: "c".into(),
                label: "C".into(),
                disabled: false,
            },
        ],
        None,
        true,
    )
    .unwrap();
    group
        .key(&key(NamedKey::ArrowRight, KeyPhase::Down))
        .unwrap();
    assert_eq!(group.selected(), Some("c"));
    group
        .key(&key(NamedKey::ArrowRight, KeyPhase::Down))
        .unwrap();
    assert_eq!(group.selected(), Some("a"));
    assert!(group.select("b").is_err());
    assert!(group.clear().is_err());
}
#[test]
fn inherited_style_and_reload_invalidate_only_resolved_changes() {
    let mut theme = Theme::new(Style::default()).unwrap();
    let initial = theme.revision();
    let local = StyleOverride {
        foreground: Some(Rgba([1.0, 0.0, 0.0, 1.0])),
        ..Default::default()
    };
    let before = local.resolve(theme.style()).unwrap();
    let mut next = theme.style().clone();
    next.foreground = Rgba([0.0, 1.0, 0.0, 1.0]);
    assert_eq!(theme.reload(next).unwrap(), Dirty::PAINT);
    assert!(theme.revision() > initial);
    assert_eq!(
        Theme::difference(&before, &local.resolve(theme.style()).unwrap()),
        Dirty::NONE
    );
    let mut next = theme.style().clone();
    next.font_size += 2.0;
    assert!(theme.reload(next).unwrap().contains(Dirty::LAYOUT));
    assert!(
        StyleOverride {
            font_size: Some(f32::NAN),
            ..Default::default()
        }
        .resolve(theme.style())
        .is_err()
    );
}
#[test]
fn numeric_intermediate_commit_cancel_and_read_only() {
    let mut number = NumericField::new("Opacity", spec(), 3.0).unwrap();
    let mut clip = Clip::default();
    number
        .field
        .command(TextCommand::SelectAll, &mut clip, 0)
        .unwrap();
    number.field.insert_text("-", 1).unwrap();
    assert!(!number.validate_pending());
    assert!(number.commit().is_err());
    assert_eq!(number.value(), 3.0);
    number.cancel().unwrap();
    assert_eq!(number.field.snapshot().text.text(), "3.0");
    number
        .field
        .command(TextCommand::SelectAll, &mut clip, 2)
        .unwrap();
    number.field.insert_text("4.26", 3).unwrap();
    number.commit().unwrap();
    assert_eq!(number.value(), 4.3);
    number.field.set_modes(true, false);
    assert!(number.commit().is_err());
    let mut slider = Slider::new(spec(), 0.0).unwrap();
    slider.focused = true;
    slider.key(&key(NamedKey::End, KeyPhase::Down)).unwrap();
    assert_eq!(slider.range.value(), 10.0);
    slider
        .pointer(
            pointer(PointerPhase::Down, 25.0, 5.0),
            rect(0.0, 0.0, 100.0, 20.0),
        )
        .unwrap();
    assert_eq!(slider.range.value(), 2.5);
    slider.cancel();
    assert!(
        !slider
            .pointer(
                pointer(PointerPhase::Up, 90.0, 5.0),
                rect(0.0, 0.0, 100.0, 20.0)
            )
            .unwrap()
    );
}
#[test]
fn virtualization_preserves_keys_and_reveals_offscreen_focus() {
    let mut list = VirtualList::new(
        (0..10000).map(|i| format!("row-{i}")).collect(),
        20.0,
        LogicalSize::new(200.0, 100.0),
        8,
    )
    .unwrap();
    list.key(&key(NamedKey::End, KeyPhase::Down)).unwrap();
    assert_eq!(list.selection.focused(), Some("row-9999"));
    assert_eq!(list.realized().unwrap(), 9995..10000);
    let mut reordered = list.selection.keys().to_vec();
    reordered.reverse();
    list.replace(reordered).unwrap();
    assert_eq!(list.selection.focused(), Some("row-9999"));
    assert_eq!(list.realized().unwrap(), 0..5);
    list.replace(vec!["replacement".into()]).unwrap();
    assert_eq!(list.selection.focused(), Some("replacement"));
    assert!(list.selection.selected().is_empty());
    assert!(list.replace(vec!["x".into(), "x".into()]).is_err());
    let mut tree = Tree::new(
        vec![
            TreeItem {
                key: "parent".into(),
                parent: None,
                label: "Parent".into(),
            },
            TreeItem {
                key: "child".into(),
                parent: Some("parent".into()),
                label: "Child".into(),
            },
        ],
        LogicalSize::new(200.0, 100.0),
    )
    .unwrap();
    tree.set_expanded("parent", true).unwrap();
    tree.list.selection.select("child", false, false).unwrap();
    tree.set_expanded("parent", false).unwrap();
    assert_eq!(tree.list.selection.focused(), Some("parent"));
    assert_eq!(tree.list.selection.keys(), &["parent"]);
}
#[test]
fn scrollbar_and_split_cancel_boundaries() {
    let mut scroll = ScrollState::new(
        LogicalSize::new(100.0, 1000.0),
        LogicalSize::new(100.0, 100.0),
    );
    let track = rect(0.0, 0.0, 12.0, 100.0);
    let mut bar = Scrollbar::new();
    bar.pointer(pointer(PointerPhase::Down, 4.0, 3.0), &mut scroll, track)
        .unwrap();
    bar.pointer(pointer(PointerPhase::Move, 4.0, 103.0), &mut scroll, track)
        .unwrap();
    assert_eq!(scroll.offset.y, 900.0);
    bar.pointer(
        pointer(PointerPhase::Cancel, 4.0, 103.0),
        &mut scroll,
        track,
    )
    .unwrap();
    bar.pointer(pointer(PointerPhase::Move, 4.0, 0.0), &mut scroll, track)
        .unwrap();
    assert_eq!(scroll.offset.y, 900.0);
    let mut split = SplitPane::new(100.0, 50.0, 40.0).unwrap();
    split.begin(100.0).unwrap();
    split.drag(900.0, 300.0).unwrap();
    assert_eq!(split.first, 260.0);
    split.cancel();
    assert_eq!(split.first, 100.0);
    split.begin(100.0).unwrap();
    assert!(split.drag(200.0, 10.0).is_err());
    assert_eq!(split.first, 100.0);
}
#[test]
fn popup_nesting_edges_and_live_focus_return() {
    let mut runtime: Runtime<(), ()> = Runtime::new(16).unwrap();
    let (_, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let owner = runtime
        .mount(root, Description::new("owner", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let content = runtime
        .mount(root, Description::new("popup", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let bounds = PopupStack::placement(
        rect(290.0, 190.0, 10.0, 10.0),
        LogicalSize::new(150.0, 100.0),
        rect(0.0, 0.0, 300.0, 200.0),
    )
    .unwrap();
    assert_eq!(bounds, rect(150.0, 90.0, 150.0, 100.0));
    let mut stack = PopupStack::default();
    stack
        .open(Popup {
            owner,
            content,
            return_focus: Some(owner),
            bounds,
            modal: true,
        })
        .unwrap();
    assert!(!stack.focus_allowed(root, |_, _| false));
    runtime.unmount(owner).unwrap();
    assert_eq!(stack.escape(|id| runtime.state::<()>(id).is_ok()), None);
    let mut palette = CommandPalette::new(vec![
        MenuItem {
            label: "Open file".into(),
            action: 1,
            enabled: true,
        },
        MenuItem {
            label: "Open recent".into(),
            action: 2,
            enabled: false,
        },
    ]);
    palette.query("OPEN");
    assert_eq!(palette.menu.activate(), Some(1));
    palette.query("absent");
    assert_eq!(palette.menu.activate(), None);
    palette.query("open");
    assert_eq!(
        palette.menu.key(&key(NamedKey::Enter, KeyPhase::Repeat)),
        MenuResponse::Ignored
    );
    assert_eq!(
        palette.menu.key(&key(NamedKey::Enter, KeyPhase::Down)),
        MenuResponse::Activate(1)
    );
    assert_eq!(
        palette.menu.key(&key(NamedKey::Escape, KeyPhase::Down)),
        MenuResponse::Dismiss
    );
    stack
        .open(Popup {
            owner: root,
            content,
            return_focus: Some(root),
            bounds,
            modal: true,
        })
        .unwrap();
    let nested = runtime
        .mount(root, Description::new("nested", (), |_, _, _| Dirty::NONE))
        .unwrap();
    stack
        .open(Popup {
            owner: content,
            content: nested,
            return_focus: Some(content),
            bounds,
            modal: false,
        })
        .unwrap();
    assert_eq!(
        stack.escape(|id| runtime.state::<()>(id).is_ok()),
        Some(content)
    );
    assert_eq!(stack.top().unwrap().content, content);
    stack.remove_owner(root);
    assert!(stack.top().is_none());
}
#[test]
fn standard_and_distinct_presentations_share_mandatory_profiles() {
    for mode in [LineMode::SingleLine, LineMode::Multiline] {
        let profile = if mode == LineMode::SingleLine {
            Profile::SingleLineV1
        } else {
            Profile::PlainMultilineV1
        };
        let mut standard = TextField::new(
            "Standard",
            "",
            EditorConfig {
                mode,
                ..Default::default()
            },
        )
        .unwrap();
        let mut distinct = TextField::new(
            "Underlined",
            "",
            EditorConfig {
                mode,
                ..Default::default()
            },
        )
        .unwrap();
        let report = run_profile(&mut standard, profile, "standard", std::env::consts::OS);
        let other = run_profile(&mut distinct, profile, "underlined", std::env::consts::OS);
        assert_eq!(report.cases, other.cases);
        assert!(
            !report
                .cases
                .iter()
                .any(|(_, result)| matches!(result, CaseResult::Failed(_)))
        );
        assert!(!report.certified());
        let mut backend = fonts();
        let style = Style {
            padding: 12.0,
            border_width: 3.0,
            foreground: Rgba([0.2, 0.8, 0.5, 1.0]),
            ..Default::default()
        };
        distinct
            .prepare(
                &mut backend,
                &style,
                rect(0.0, 0.0, 240.0, 100.0),
                ScaleFactor::ONE,
                0,
            )
            .unwrap();
    }
}
#[test]
fn readonly_copy_preedit_external_unmount_and_scaled_clipping() {
    let mut field = TextField::new(
        "Name",
        "long value long value long value",
        EditorConfig::default(),
    )
    .unwrap();
    let mut clip = Clip::default();
    field.set_focused(true, 0);
    field.command(TextCommand::SelectAll, &mut clip, 0).unwrap();
    field.set_modes(true, false);
    field.command(TextCommand::Copy, &mut clip, 0).unwrap();
    assert!(clip.0.contains("long value"));
    assert!(field.command(TextCommand::Cut, &mut clip, 0).is_err());
    field.set_modes(false, false);
    field
        .preedit("composition long composition", Some((28, 28)))
        .unwrap();
    let mut backend = fonts();
    for scale in [1.0, 1.5, 2.0] {
        let p = field
            .prepare(
                &mut backend,
                &Style::default(),
                rect(40.0, 50.0, 110.0, 36.0),
                ScaleFactor::new(scale).unwrap(),
                10,
            )
            .unwrap();
        assert!(p.candidate.left() >= p.clip.left());
        assert!(p.candidate.right() <= p.clip.right());
        assert!(p.candidate.bottom() <= p.clip.bottom());
        assert!(p.preedit.is_some());
    }
    let snapshot = field.snapshot();
    field
        .external(
            snapshot.text.revision(),
            snapshot.text.text(),
            ExternalPolicy::PreserveSelection,
        )
        .unwrap();
    assert!(field.snapshot().composition.is_some());
    field
        .external(
            snapshot.text.revision(),
            "replacement",
            ExternalPolicy::PreserveSelection,
        )
        .unwrap();
    assert!(field.snapshot().composition.is_none());
    field.preedit("x", Some((1, 1))).unwrap();
    field.unmount(100);
    assert!(field.snapshot().composition.is_none());
    assert!(!field.focused());
}
#[test]
fn runtime_owns_control_lifecycle_and_revokes_queued_effects() {
    let mut runtime: Runtime<usize, ()> = Runtime::new(16).unwrap();
    let (_, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let id = runtime
        .mount(
            root,
            Description::new(
                "save",
                Button::new("Save"),
                |button: &mut Button, count: &mut usize, _| {
                    if button.activate().is_ok() {
                        *count += 1;
                    }
                    Dirty::PAINT
                },
            ),
        )
        .unwrap();
    let mut count = 0;
    runtime.enqueue(id, ()).unwrap();
    runtime.flush(&mut count).unwrap();
    assert_eq!(count, 1);
    runtime.enqueue(id, ()).unwrap();
    runtime.unmount(id).unwrap();
    runtime.flush(&mut count).unwrap();
    assert_eq!(count, 1);
    assert!(runtime.state::<Button>(id).is_err());
}

#[test]
fn runtime_capture_continues_drag_and_cancels_when_hidden() {
    let mut runtime: Runtime<(), ()> = Runtime::new(16).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .layout(rect(0.0, 0.0, 200.0, 200.0)),
        )
        .unwrap();
    let id = runtime
        .mount(
            root,
            Description::new("slider", (), |_, _, _| Dirty::NONE)
                .layout(rect(0.0, 0.0, 30.0, 30.0)),
        )
        .unwrap();
    runtime
        .route_pointer(window, pointer(PointerPhase::Down, 10.0, 10.0), &mut ())
        .unwrap();
    runtime.capture_pointer(window, id).unwrap();
    let actions = runtime
        .route_pointer(window, pointer(PointerPhase::Move, 190.0, 190.0), &mut ())
        .unwrap();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a,RoutedPointerAction::Move{target,..} if *target==id))
    );
    runtime.set_visibility(id, Visibility::Hidden).unwrap();
    let actions = runtime
        .route_pointer(window, pointer(PointerPhase::Move, 190.0, 190.0), &mut ())
        .unwrap();
    assert!(actions.contains(&RoutedPointerAction::Cancel(id)));
    assert!(runtime.capture_pointer(window, id).is_err());
    runtime.unmount(id).unwrap();
    assert!(runtime.capture_pointer(window, id).is_err());
}

#[test]
fn tabs_wrap_skip_disabled_and_leave_host_keys_alone() {
    let mut tabs = TabList::new(
        vec![
            TabOption {
                key: "one".into(),
                label: "One".into(),
                disabled: false,
            },
            TabOption {
                key: "disabled".into(),
                label: "Disabled".into(),
                disabled: true,
            },
            TabOption {
                key: "three".into(),
                label: "Three".into(),
                disabled: false,
            },
        ],
        None,
    )
    .unwrap();
    assert!(
        !tabs
            .key(&key(NamedKey::ArrowRight, KeyPhase::Down))
            .unwrap()
    );
    tabs.set_focused(true);
    assert!(
        tabs.key(&key(NamedKey::ArrowRight, KeyPhase::Down))
            .unwrap()
    );
    assert_eq!(tabs.selected(), "three");
    tabs.key(&key(NamedKey::ArrowRight, KeyPhase::Repeat))
        .unwrap();
    assert_eq!(tabs.selected(), "one");
    tabs.key(&key(NamedKey::End, KeyPhase::Down)).unwrap();
    assert_eq!(tabs.selected(), "three");
    assert_eq!(tabs.select("disabled"), Err(ControlError::Disabled));
    assert_eq!(tabs.selected(), "three");
    assert_eq!(tabs.select("missing"), Err(ControlError::Stale));
    for named in [NamedKey::Tab, NamedKey::ArrowDown, NamedKey::ArrowUp] {
        assert!(!tabs.key(&key(named, KeyPhase::Down)).unwrap());
    }
    let mut modified = key(NamedKey::Home, KeyPhase::Down);
    modified.modifiers.ctrl = true;
    assert!(!tabs.key(&modified).unwrap());
    assert!(!tabs.key(&key(NamedKey::Home, KeyPhase::Up)).unwrap());
    tabs.key(&key(NamedKey::Home, KeyPhase::Down)).unwrap();
    assert_eq!(tabs.selected(), "one");
}

#[test]
fn tabs_reject_invalid_sets_and_expose_selection_without_checkbox_state() {
    assert!(TabList::new(vec![], None).is_err());
    let option = TabOption {
        key: "same".into(),
        label: "Same".into(),
        disabled: false,
    };
    assert!(TabList::new(vec![option.clone(), option.clone()], None).is_err());
    let mut disabled = option.clone();
    disabled.disabled = true;
    assert!(TabList::new(vec![disabled], None).is_err());
    let mut runtime: Runtime<(), ()> = Runtime::new(8).unwrap();
    let (_, id) = runtime
        .open_window(Description::new("tabs", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let mut tabs = TabList::new(vec![option], None).unwrap();
    tabs.set_focused(true);
    let state = tabs.semantics(0, id, rect(0., 0., 100., 30.)).unwrap();
    assert_eq!(state.node.role, Role::Custom("tab".into()));
    assert_eq!(state.node.checked, None);
    assert_eq!(state.selected, Some(true));
    assert!(state.node.focused);
    assert!(tabs.semantics(1, id, rect(0., 0., 100., 30.)).is_err());
}
