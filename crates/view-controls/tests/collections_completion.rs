use view_controls::*;
use view_core::*;

fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
}
fn key(key: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(key),
        phase: KeyPhase::Down,
        modifiers: Modifiers::default(),
        timestamp: 1,
    }
}
fn options() -> Vec<SelectOption> {
    [
        ("a", "Apple", false),
        ("b", "Banana", true),
        ("c", "Cherry", false),
        ("d", "Coconut", false),
    ]
    .into_iter()
    .map(|(key, label, disabled)| SelectOption {
        key: key.into(),
        label: label.into(),
        disabled,
    })
    .collect()
}
fn ids() -> (Runtime<(), ()>, NodeId, NodeId) {
    let mut runtime = Runtime::new(32).unwrap();
    let (_, root) = runtime
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let content = runtime
        .mount(root, Description::new("popup", (), |_, _, _| Dirty::NONE))
        .unwrap();
    (runtime, root, content)
}
#[test]
fn select_pending_navigation_escape_commit_and_external_popup_dismissal() {
    let (_runtime, owner, content) = ids();
    let popup = Popup {
        owner,
        content,
        return_focus: Some(owner),
        bounds: rect(0.0, 30.0, 200.0, 120.0),
        modal: false,
    };
    let mut stack = PopupStack::default();
    let mut select = Select::new("Fruit", options(), Some("a".into())).unwrap();
    select.set_focused(true);
    select
        .key(&key(NamedKey::Enter), &mut stack, popup, |_| true)
        .unwrap();
    select
        .key(&key(NamedKey::ArrowDown), &mut stack, popup, |_| true)
        .unwrap();
    assert_eq!(select.active(), Some("c"));
    assert_eq!(select.selected(), Some("a"));
    assert_eq!(
        select
            .key(&key(NamedKey::Escape), &mut stack, popup, |_| true)
            .unwrap()
            .return_focus,
        Some(owner)
    );
    assert_eq!(select.selected(), Some("a"));
    assert!(!select.is_open(&stack));
    select.open(&mut stack, popup).unwrap();
    select
        .key(&key(NamedKey::End), &mut stack, popup, |_| true)
        .unwrap();
    let response = select
        .key(&key(NamedKey::Space), &mut stack, popup, |_| true)
        .unwrap();
    assert!(response.changed);
    assert_eq!(select.selected(), Some("d"));
    select.open(&mut stack, popup).unwrap();
    stack.outside(LogicalPoint::new(500.0, 500.0), |_| true);
    assert!(!select.is_open(&stack));
    assert_eq!(
        select.activate("a", &mut stack, |_| true),
        Err(ControlError::Stale)
    );
    select.open(&mut stack, popup).unwrap();
    let response = select
        .key(&key(NamedKey::Tab), &mut stack, popup, |_| true)
        .unwrap();
    assert!(!response.handled);
    assert!(!select.is_open(&stack));
}
#[test]
fn select_typeahead_disabled_replacement_and_stale_focus_are_explicit() {
    let (_runtime, owner, content) = ids();
    let popup = Popup {
        owner,
        content,
        return_focus: Some(content),
        bounds: rect(0.0, 0.0, 100.0, 100.0),
        modal: false,
    };
    let mut stack = PopupStack::default();
    let mut select = Select::new("Fruit", options(), Some("a".into())).unwrap();
    select.set_focused(true);
    let mut event = key(NamedKey::Enter);
    event.key = Key::Character("c".into());
    select.key(&event, &mut stack, popup, |_| true).unwrap();
    assert_eq!(select.active(), Some("c"));
    event.timestamp = 100;
    select.key(&event, &mut stack, popup, |_| true).unwrap();
    assert_eq!(select.active(), Some("d"));
    assert_eq!(select.select("b"), Err(ControlError::Disabled));
    assert_eq!(
        select.replace(vec![options()[0].clone(), options()[0].clone()]),
        Err(ControlError::Invalid("duplicate select key"))
    );
    assert_eq!(select.options().len(), 4);
    assert_eq!(
        select.set_disabled(true, &mut stack, |id| id == owner),
        Some(owner)
    );
    assert!(!select.is_open(&stack));
    select.set_disabled(false, &mut stack, |_| false);
    select.replace(vec![]).unwrap();
    assert_eq!(select.selected(), None);
    assert_eq!(select.open(&mut stack, popup), Err(ControlError::Disabled));
}
fn table() -> Table {
    Table::new(
        vec![
            TableColumn {
                key: "name".into(),
                label: "Name".into(),
                width: 100.0,
            },
            TableColumn {
                key: "value".into(),
                label: "Value".into(),
                width: 100.0,
            },
            TableColumn {
                key: "status".into(),
                label: "Status".into(),
                width: 100.0,
            },
        ],
        (0..100)
            .map(|i| TableRow {
                key: format!("row-{i}"),
                cells: vec![format!("Name {i}"), i.to_string(), "Ready".into()],
            })
            .collect(),
        20.0,
        LogicalSize::new(150.0, 80.0),
        TableLimits {
            max_rows: 100,
            max_columns: 3,
            max_realized_cells: 12,
        },
    )
    .unwrap()
}
#[test]
fn table_navigation_virtualization_selection_and_keyed_sort_survive_replacement() {
    let mut table = table();
    assert_eq!(table.realized_rows().unwrap(), 0..3);
    assert_eq!(table.realized_columns(), 0..2);
    table.key(&key(NamedKey::End)).unwrap();
    assert_eq!(table.selection().focused(), Some("row-99"));
    assert_eq!(table.realized_rows().unwrap(), 97..100);
    table.key(&key(NamedKey::ArrowRight)).unwrap();
    table.key(&key(NamedKey::ArrowRight)).unwrap();
    assert_eq!(table.active_column(), 2);
    assert_eq!(table.scroll().offset.x, 150.0);
    assert!(table.realized_cells().unwrap().len() <= 12);
    let mut rows = table.rows().to_vec();
    rows.reverse();
    table.replace(rows).unwrap();
    assert_eq!(table.selection().focused(), Some("row-99"));
    assert_eq!(table.realized_rows().unwrap(), 0..3);
    table
        .pointer(LogicalPoint::new(20.0, 40.0), false, false)
        .unwrap();
    assert_eq!(table.selection().focused(), Some("row-98"));
    assert_eq!(table.active_column(), 0); // row reveal resets horizontal origin
}
#[test]
fn table_rejects_malformed_updates_atomically_and_enforces_visible_cell_budget() {
    let mut table = table();
    table.select("row-4", false, false).unwrap();
    let before = table.scroll().offset;
    assert!(
        table
            .replace(vec![TableRow {
                key: "bad".into(),
                cells: vec![]
            }])
            .is_err()
    );
    assert_eq!(table.rows().len(), 100);
    assert_eq!(table.selection().focused(), Some("row-4"));
    assert_eq!(table.scroll().offset, before);
    table.resize(LogicalSize::new(300.0, 200.0)).unwrap();
    assert!(table.realized_cells().is_err());
    assert!(
        table
            .pointer(LogicalPoint::new(f32::NAN, 20.0), false, false)
            .is_err()
    );
    assert!(
        table
            .pointer(LogicalPoint::new(10.0, 5.0), false, false)
            .is_err()
    );
}
#[test]
fn menu_reverse_entry_and_dialog_child_command_policy_use_typed_actions() {
    let mut menu = Menu {
        items: vec![
            MenuItem {
                label: "One".into(),
                action: 1,
                enabled: true,
            },
            MenuItem {
                label: "Disabled".into(),
                action: 2,
                enabled: false,
            },
            MenuItem {
                label: "Three".into(),
                action: 3,
                enabled: true,
            },
        ],
        focused: None,
    };
    assert_eq!(menu.key(&key(NamedKey::ArrowUp)), MenuResponse::Handled);
    assert_eq!(menu.focused, Some(2));
    assert_eq!(menu.key(&key(NamedKey::Enter)), MenuResponse::Activate(3));
    let dialog = Dialog {
        title: "Confirm".into(),
        body: "Proceed?".into(),
        actions: menu.items,
        default: Some(0),
        cancel: Some(2),
    };
    assert_eq!(
        dialog.key(&key(NamedKey::Enter), true),
        MenuResponse::Ignored
    );
    assert_eq!(
        dialog.key(&key(NamedKey::Enter), false),
        MenuResponse::Activate(1)
    );
    assert_eq!(
        dialog.key(&key(NamedKey::Escape), false),
        MenuResponse::Activate(3)
    );
    assert_eq!(dialog.activate(1), Err(ControlError::Disabled));
    let (_, owner, content) = ids();
    let mut stack = PopupStack::default();
    let popup = Popup {
        owner,
        content,
        return_focus: Some(owner),
        bounds: rect(0.0, 0.0, 100.0, 100.0),
        modal: true,
    };
    dialog.open(&mut stack, popup).unwrap();
    assert!(!stack.focus_allowed(owner, |_, _| false));
    assert!(stack.focus_allowed(content, |_, _| false));
    stack.outside(LogicalPoint::new(500.0, 500.0), |_| true);
    assert!(stack.contains(content));
    assert_eq!(stack.dismiss(content, |_| false), None);
    assert!(!stack.contains(content));
}
#[test]
fn modified_release_clears_press_and_shortcuts_do_not_adjust_range() {
    let mut button = Button::new("Run");
    button.behavior.set_focused(true);
    button.key(&key(NamedKey::Space));
    let mut release = key(NamedKey::Space);
    release.phase = KeyPhase::Up;
    release.modifiers.ctrl = true;
    assert!(!button.key(&release));
    assert!(!button.behavior.state().pressed);
    let mut range = RangeBehavior::new(
        RangeSpec {
            min: 0.0,
            max: 10.0,
            step: 1.0,
            page: 2.0,
            decimals: 0,
        },
        5.0,
    )
    .unwrap();
    let mut event = key(NamedKey::End);
    event.modifiers.alt = true;
    assert!(!range.key(&event).unwrap());
    assert_eq!(range.value(), 5.0);
}
#[test]
fn tree_presentation_uses_labels_depth_and_no_leaf_expansion() {
    let mut tree = Tree::new(
        vec![
            TreeItem {
                key: "root".into(),
                parent: None,
                label: "Project".into(),
            },
            TreeItem {
                key: "child".into(),
                parent: Some("root".into()),
                label: "Sources".into(),
            },
        ],
        LogicalSize::new(200.0, 48.0),
    )
    .unwrap();
    tree.set_expanded("root", true).unwrap();
    tree.list.selection.select("child", false, false).unwrap();
    tree.key(&key(NamedKey::ArrowRight)).unwrap();
    assert!(!tree.expanded("child"));
    let rows = tree
        .presentation(rect(10.0, 20.0, 200.0, 48.0), true)
        .unwrap();
    assert!(rows[0].label.contains("Project"));
    assert!(rows[1].label.contains("Sources"));
    assert_eq!(rows[1].bounds.left(), 26.0);
}
