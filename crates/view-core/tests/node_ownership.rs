use view_core::{Description, Dirty, Runtime};

#[test]
fn live_node_window_rejects_removed_recycled_and_foreign_generations() {
    let mut runtime: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (first, root) = runtime
        .open_window(Description::new("first", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let node = runtime
        .mount(root, Description::new("child", (), |_, _, _| Dirty::NONE))
        .unwrap();
    assert_eq!(runtime.node_window(node).unwrap(), first);
    runtime.unmount(node).unwrap();
    let replacement = runtime
        .mount(root, Description::new("child", (), |_, _, _| Dirty::NONE))
        .unwrap();
    assert_ne!(replacement, node);
    assert!(runtime.node_window(node).is_err());
    assert_eq!(runtime.node_window(replacement).unwrap(), first);
    let mut foreign: Runtime<(), ()> = Runtime::new(4).unwrap();
    let (_, foreign_root) = foreign
        .open_window(Description::new("foreign", (), |_, _, _| Dirty::NONE))
        .unwrap();
    assert!(runtime.node_window(foreign_root).is_err());
    runtime.close_window(first).unwrap();
    assert!(runtime.node_window(root).is_err());
    assert!(runtime.node_window(replacement).is_err());
}
