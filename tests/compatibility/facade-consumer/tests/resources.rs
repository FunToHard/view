#![cfg(feature = "resources")]
use view::{resources::*, *};
#[test]
fn optional_resource_facade_uses_runtime_owner_lifetime() {
    let mut ui = Runtime::<(), ()>::new(4).unwrap();
    let (window, root) = ui
        .open_window(Description::new("owner", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let lease = ui.own(root, ResourceKind::Subscription, || {}).unwrap();
    let mut resource = Resource::new(root, lease.cancellation(), Policy::default(), 1).unwrap();
    let first = resource.begin("search").unwrap();
    assert!(resource.complete(&first, vec![1]).unwrap());
    let refresh = resource.begin("search").unwrap();
    assert_eq!(resource.value(), Some(&vec![1]));
    ui.close_window(window).unwrap();
    assert!(!resource.complete(&refresh, vec![2]).unwrap());
}
