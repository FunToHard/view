use view_core::{
    Description, Dirty, LogicalRect, Role, Runtime, SemanticAction, SemanticContract, SemanticNode,
};

#[test]
fn test_semantic_node_declaration_and_snapshot() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .role(Role::Window)
                .name("Main Window")
                .layout(LogicalRect::from_xywh(0.0, 0.0, 400.0, 300.0)),
        )
        .unwrap();

    let submit_btn = runtime
        .mount(
            root,
            Description::new("submit", (), |_, _, _| Dirty::NONE)
                .role(Role::Button)
                .name("Submit")
                .semantic_actions(vec![SemanticAction::Click])
                .layout(LogicalRect::from_xywh(10.0, 10.0, 80.0, 30.0)),
        )
        .unwrap();

    let text_field = runtime
        .mount(
            root,
            Description::new("input", (), |_, _, _| Dirty::NONE)
                .role(Role::TextInput)
                .value("Initial text")
                .semantic_actions(vec![SemanticAction::SetValue, SemanticAction::Focus])
                .focusable(true)
                .layout(LogicalRect::from_xywh(10.0, 50.0, 200.0, 30.0)),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();

    let snapshot = runtime.semantic_snapshot(window).unwrap();
    assert_eq!(snapshot.nodes.len(), 3);

    let btn_node = snapshot.nodes.iter().find(|n| n.id == submit_btn).unwrap();
    assert_eq!(btn_node.role, Role::Button);
    assert_eq!(btn_node.name.as_deref(), Some("Submit"));
    assert_eq!(btn_node.actions, vec![SemanticAction::Click]);
    assert_eq!(
        btn_node.bounds,
        LogicalRect::from_xywh(10.0, 10.0, 80.0, 30.0)
    );

    let text_node = snapshot.nodes.iter().find(|n| n.id == text_field).unwrap();
    assert_eq!(text_node.role, Role::TextInput);
    assert_eq!(text_node.value.as_deref(), Some("Initial text"));
}

#[test]
fn test_incremental_semantic_snapshot_diff() {
    let mut runtime = Runtime::<(), ()>::new(8).unwrap();
    let (window, root) = runtime
        .open_window(
            Description::new("root", (), |_, _, _| Dirty::NONE)
                .role(Role::Window)
                .layout(LogicalRect::from_xywh(0.0, 0.0, 100.0, 100.0)),
        )
        .unwrap();

    let btn1 = runtime
        .mount(
            root,
            Description::new("btn1", (), |_, _, _| Dirty::NONE).role(Role::Button),
        )
        .unwrap();

    let mut model = ();
    runtime.flush(&mut model).unwrap();
    let snap1 = runtime.semantic_snapshot(window).unwrap();

    // Now unmount btn1 and add btn2
    runtime.unmount(btn1).unwrap();
    let btn2 = runtime
        .mount(
            root,
            Description::new("btn2", (), |_, _, _| Dirty::NONE).role(Role::Button),
        )
        .unwrap();
    runtime.flush(&mut model).unwrap();
    let snap2 = runtime.semantic_snapshot(window).unwrap();

    let diff = snap2.diff(&snap1);
    assert_eq!(diff.removed, vec![btn1]);
    assert!(diff.updated.iter().any(|n| n.id == btn2));
}

struct CustomWidget;
impl SemanticContract for CustomWidget {
    fn declare_semantics(&self, node_id: view_core::NodeId, bounds: LogicalRect) -> SemanticNode {
        let mut node = SemanticNode::new(node_id, Role::Custom("ColorPicker".to_string()));
        node.name = Some("Custom Color Palette".to_string());
        node.bounds = bounds;
        node
    }
}

#[test]
fn test_custom_semantic_contract() {
    let dummy_id = view_core::NodeId::from_handle(view_core::ArenaHandle::from_parts(
        view_core::ArenaId::new(std::num::NonZeroU64::new(1).unwrap()),
        0,
        view_core::Generation::INITIAL,
    ));
    let custom = CustomWidget;
    let node = custom.declare_semantics(dummy_id, LogicalRect::from_xywh(0.0, 0.0, 50.0, 50.0));
    assert_eq!(node.role, Role::Custom("ColorPicker".to_string()));
    assert_eq!(node.name.as_deref(), Some("Custom Color Palette"));
}
