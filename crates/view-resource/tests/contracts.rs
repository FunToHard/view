use std::time::Duration;
use view_core::*;
use view_resource::{assets::*, stream::*, *};

fn owner() -> (Runtime<(), ()>, NodeId, Cancellation) {
    let mut ui = Runtime::new(2).unwrap();
    let (_, root) = ui
        .open_window(Description::new("owner", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let root = ui
        .mount(
            root,
            Description::new("resource-owner", (), |_, _, _| Dirty::NONE),
        )
        .unwrap();
    let lifetime = ui
        .own(root, ResourceKind::Subscription, || {})
        .unwrap()
        .cancellation();
    (ui, root, lifetime)
}
#[test]
fn reordered_cancelled_foreign_and_unmounted_completions_never_replace_current_data() {
    let (mut ui, owner, lifetime) = owner();
    let mut r = Resource::new(owner, lifetime.clone(), Policy::default(), 2).unwrap();
    let a = r.begin("a").unwrap();
    let b = r.begin("b").unwrap();
    assert!(a.cancelled());
    assert!(!r.complete(&a, 1).unwrap());
    assert!(r.complete(&b, 2).unwrap());
    assert!(!r.complete(&b, 3).unwrap());
    let refresh = r.begin("b").unwrap();
    assert_eq!(r.value(), Some(&2));
    let mut foreign = Resource::<i32>::new(owner, lifetime, Policy::default(), 2).unwrap();
    assert!(!foreign.complete(&refresh, 99).unwrap());
    r.subscribe(owner).unwrap();
    assert_eq!(r.notify(&mut ui).unwrap(), 1);
    assert_eq!(r.notify(&mut ui).unwrap(), 0);
    ui.unmount(owner).unwrap();
    assert!(refresh.cancelled());
    assert!(!r.complete(&refresh, 4).unwrap());
    assert!(r.begin("c").is_err());
}
#[test]
fn explicit_retry_refresh_offline_reconnect_and_progress_have_bounded_policy() {
    let (_, _, l) = owner(); // runtime drop cancels leases: keep it alive below.
    assert!(l.is_revoked());
    let (_ui, o, l) = owner();
    let mut r = Resource::new(
        o,
        l,
        Policy {
            max_retries: 1,
            reconnect: true,
            ..Policy::default()
        },
        1,
    )
    .unwrap();
    let a = r.begin("catalog").unwrap();
    r.complete(&a, vec![7]).unwrap();
    let a = r.begin("catalog").unwrap();
    assert!(r.set_progress(&a, 0.5).unwrap());
    assert!(!r.set_progress(&a, 0.3).unwrap());
    assert!(r.set_progress(&a, f32::NAN).is_err());
    r.fail(&a, "offline".into(), Duration::ZERO).unwrap();
    assert_eq!(r.value(), Some(&vec![7]));
    assert!(r.retry(Duration::ZERO).unwrap().is_none());
    let b = r.retry(Duration::from_secs(1)).unwrap().unwrap();
    r.fail(&b, "again".into(), Duration::from_secs(1)).unwrap();
    assert_eq!(r.retry_at(), None);
    r.offline().unwrap();
    let c = r.reconnect().unwrap().unwrap();
    r.complete(&c, vec![]).unwrap();
    assert_eq!(r.status(), Status::Ready);
    assert!(r.value().unwrap().is_empty());
    let _ = o;
}
#[test]
fn mailbox_retains_required_events_and_stream_gaps_require_resync() {
    let q = Mailbox::new(2);
    let producer = q.clone();
    let returned = std::thread::spawn(move || {
        producer.send(1).unwrap();
        producer.send(2).unwrap();
        for i in 0..10000 {
            producer.preview(i);
        }
        producer.send(3).unwrap_err()
    })
    .join()
    .unwrap();
    assert_eq!(returned, 3);
    assert_eq!(q.take_preview(), Some(9999));
    assert_eq!(q.receive(), Some(1));
    q.send(returned).unwrap();
    assert_eq!(q.receive(), Some(2));
    assert_eq!(q.receive(), Some(3));
    let mut s = Sequence::new(1, 0);
    assert_eq!(s.accept(1, 0), SequenceResult::Apply);
    assert_eq!(s.accept(1, 0), SequenceResult::Duplicate);
    assert_eq!(s.accept(1, 2), SequenceResult::Resync);
    assert_eq!(s.accept(1, 1), SequenceResult::Resync);
    s.resynchronize(2, 5);
    assert_eq!(s.accept(2, 5), SequenceResult::Apply);
}
#[test]
fn cache_eviction_and_expiry_are_explicit_and_over_budget_insert_is_atomic() {
    let mut c = Cache::new(2, 8);
    let expiry = Duration::from_secs(10);
    c.insert("a".into(), 1, 4, expiry).unwrap();
    c.insert("b".into(), 2, 4, expiry).unwrap();
    assert_eq!(c.get("a", Duration::ZERO, false), Some(&1));
    c.insert("c".into(), 3, 4, expiry).unwrap();
    assert_eq!(c.get("b", Duration::ZERO, true), None);
    assert_eq!(c.bytes(), 8);
    assert!(c.insert("a".into(), 9, 9, expiry).is_err());
    assert_eq!(c.get("a", expiry, false), None);
    assert_eq!(c.get("a", expiry, true), Some(&1));
    assert!(Image::rgba(u32::MAX, u32::MAX, vec![], 32).is_err());
}
#[test]
fn remote_schema_is_bounded_and_never_enables_unknown_actions() {
    use sections::*;
    let registry = Registry::new(
        1,
        ["card".into()],
        [("buy".into(), 7)].into(),
        ["cover".into()].into(),
        Limits {
            depth: 2,
            items: 4,
            text_bytes: 100,
        },
    );
    let mut s = Section {
        key: "a".into(),
        control: "card".into(),
        mandatory: true,
        text: "Product".into(),
        action: Some("buy".into()),
        asset: Some("cover".into()),
        children: vec![],
    };
    assert_eq!(
        registry.compile(1, &[s.clone()]).unwrap()[0].action,
        Some(7)
    );
    s.action = Some("execute".into());
    assert_eq!(
        registry.compile(1, &[s.clone()]).unwrap_err(),
        SchemaError::ForbiddenAction
    );
    s.action = None;
    s.control = "future".into();
    s.mandatory = false;
    assert!(registry.compile(1, &[s.clone()]).unwrap()[0].fallback);
    assert_eq!(
        registry.compile(2, &[s.clone()]).unwrap_err(),
        SchemaError::Version
    );
    assert_eq!(
        registry.compile(1, &[s.clone(), s.clone()]).unwrap_err(),
        SchemaError::DuplicateKey
    );
    s.children = vec![s.clone()];
    s.children[0].children = vec![s.clone()];
    assert_eq!(registry.compile(1, &[s]).unwrap_err(), SchemaError::Limit);
}
