use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use view_core::*;
use view_webview::*;
#[derive(Default)]
struct Mock {
    starts: BTreeMap<PanelId, Start>,
    events: Vec<Event>,
    visible: BTreeMap<PanelId, bool>,
    closes: usize,
}
impl Provider for Mock {
    fn start(&mut self, s: Start) -> Result<()> {
        self.visible.insert(s.panel, true);
        self.events.push(Event::Ready(s.panel, 42));
        self.starts.insert(s.panel, s);
        Ok(())
    }
    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
    fn place(&mut self, _: PanelId, _: u64, p: &Placement) -> Result<()> {
        p.pixels().map(|_| ())
    }
    fn visible(&mut self, id: PanelId, v: bool) -> Result<()> {
        self.visible.insert(id, v);
        Ok(())
    }
    fn focus(&mut self, _: PanelId) -> Result<()> {
        Ok(())
    }
    fn navigate(&mut self, _: PanelId, _: &str) -> Result<()> {
        Ok(())
    }
    fn close(&mut self, id: PanelId) -> Result<()> {
        self.closes += 1;
        self.visible.remove(&id);
        Ok(())
    }
    fn release_session(&mut self, s: SessionId) {
        self.starts.retain(|_, p| p.session != s);
    }
}
static N: AtomicU64 = AtomicU64::new(1);
fn root() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("../../target/webview-tests")
        .join(format!(
            "{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
}
fn owner() -> (Runtime<(), ()>, NodeId, Cancellation) {
    let mut rt = Runtime::new(8).unwrap();
    let (_, root) = rt
        .open_window(Description::new("root", (), |_, _, _| Dirty::NONE))
        .unwrap();
    let child = rt
        .mount(
            root,
            Description::new("panel-owner", (), |_, _, _| Dirty::NONE),
        )
        .unwrap();
    let c = rt
        .own(child, ResourceKind::Subscription, || {})
        .unwrap()
        .cancellation();
    (rt, child, c)
}
fn placement() -> Placement {
    Placement {
        bounds: LogicalRect::from_xywh(10., 20., 100., 50.),
        scale: 1.5,
        transformed: false,
        masked: false,
    }
}
fn policy() -> PagePolicy {
    PagePolicy {
        documents: ["https://fixture.test/page".into()].into(),
        ..Default::default()
    }
}
fn open(m: &mut Manager<Mock>, profile: Profile, l: Lifetime, c: Cancellation) -> PanelId {
    let id = m.open(1, placement(), policy(), profile, l, c).unwrap();
    m.tick(Duration::ZERO).unwrap();
    id
}
fn settle(m: &mut Manager<Mock>, id: PanelId) {
    for _ in 0..200 {
        m.tick(Duration::from_secs(40)).unwrap();
        if !m.receipt(id).unwrap().pending {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("cleanup did not finish");
}
#[test]
fn repeated_disposal_requires_matching_process_exit_and_releases_owned_profiles() {
    let (_rt, _, c) = owner();
    let profiles = Profiles::new(root()).unwrap();
    let mut m = Manager::new(Mock::default(), profiles, Budgets::default());
    for _ in 0..12 {
        let id = m
            .open(
                1,
                placement(),
                policy(),
                Profile::default(),
                Lifetime::default(),
                c.clone(),
            )
            .unwrap();
        m.tick(Duration::from_secs(40)).unwrap();
        let path = m.profile(id).unwrap().path().to_owned();
        let sid = m.profile(id).unwrap().session();
        m.dispose(id).unwrap();
        m.provider_mut()
            .events
            .push(Event::Exited(SessionId(sid.0 + 1), 42));
        m.tick(Duration::from_secs(40)).unwrap();
        assert!(path.exists());
        assert!(!m.receipt(id).unwrap().session_exited);
        m.provider_mut().events.push(Event::Exited(sid, 42));
        settle(&mut m, id);
        let receipt = m.forget(id).unwrap();
        assert!(receipt.panel_released && receipt.session_exited && receipt.profile_removed);
        assert!(!path.exists());
        assert!(m.provider_mut().starts.is_empty());
    }
}
#[test]
fn shared_and_persistent_scopes_are_never_deleted_and_siblings_survive() {
    let (_rt, _, c) = owner();
    let base = root();
    let profiles = Profiles::new(&base).unwrap();
    let shared = base.join("shared");
    let persistent = base.join("persistent");
    let mut m = Manager::new(Mock::default(), profiles, Budgets::default());
    let a = open(
        &mut m,
        Profile::Shared(shared.clone()),
        Lifetime::default(),
        c.clone(),
    );
    let b = open(
        &mut m,
        Profile::Shared(shared.clone()),
        Lifetime::default(),
        c.clone(),
    );
    m.dispose(a).unwrap();
    assert!(m.receipt(a).unwrap().shared_retained);
    assert_eq!(m.state(b).unwrap(), State::Visible);
    assert!(shared.exists());
    let p = open(
        &mut m,
        Profile::Persistent(persistent.clone()),
        Lifetime::default(),
        c,
    );
    let session = m.profile(p).unwrap().session();
    m.dispose(p).unwrap();
    m.provider_mut().events.push(Event::Exited(session, 42));
    m.tick(Duration::ZERO).unwrap();
    assert!(m.receipt(p).unwrap().persistent_retained);
    assert!(persistent.exists());
}
#[test]
fn hot_state_budget_unmount_and_crash_do_not_silently_recreate() {
    let (mut rt, node, c) = owner();
    let mut m = Manager::new(
        Mock::default(),
        Profiles::new(root()).unwrap(),
        Budgets {
            hot_count: 1,
            hot_bytes: 100,
            hot_ttl: Duration::from_secs(1),
            ..Default::default()
        },
    );
    let a = open(&mut m, Profile::default(), Lifetime::KeepHot, c.clone());
    let b = open(&mut m, Profile::default(), Lifetime::KeepHot, c.clone());
    m.finish(a, 75).unwrap();
    assert_eq!(m.focus(a), Err(Error::Denied));
    assert_eq!(m.finish(b, 1), Err(Error::Budget));
    assert_eq!(m.state(a).unwrap(), State::Hot);
    m.tick(Duration::from_secs(2)).unwrap();
    assert_eq!(m.eviction_candidates(), vec![a]);
    m.reattach(a, c.clone()).unwrap();
    assert_eq!(m.provider_mut().starts.len(), 2);
    rt.unmount(node).unwrap();
    m.tick(Duration::from_secs(2)).unwrap();
    assert!(matches!(m.state(a).unwrap(), State::Hot | State::Closing));
    m.provider_mut()
        .events
        .push(Event::Failed(a, "crash".into()));
    m.tick(Duration::from_secs(2)).unwrap();
    assert_eq!(m.state(a).unwrap(), State::Failed);
    assert_eq!(m.reattach(a, c), Err(Error::Stale));
}
#[test]
fn locked_or_changed_profile_and_late_callbacks_report_pending_without_deleting_replacement() {
    let (_rt, _, c) = owner();
    let base = root();
    let mut m = Manager::new(
        Mock::default(),
        Profiles::new(&base).unwrap(),
        Budgets::default(),
    );
    let id = open(&mut m, Profile::default(), Lifetime::default(), c);
    let profile = m.profile(id).unwrap().clone();
    m.dispose(id).unwrap();
    std::fs::write(profile.path().join(".view-owner"), "replacement").unwrap();
    m.provider_mut()
        .events
        .push(Event::Exited(profile.session(), 42));
    for _ in 0..200 {
        m.tick(Duration::from_secs(40)).unwrap();
        if m.receipt(id)
            .unwrap()
            .failure
            .as_ref()
            .is_some_and(|s| s.contains("Denied"))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(profile.path().exists());
    assert!(m.receipt(id).unwrap().pending);
    std::fs::write(
        profile.path().join(".view-owner"),
        profile.session().0.to_string(),
    )
    .unwrap();
    m.retry_cleanup(id).unwrap();
    settle(&mut m, id);
    assert!(m.receipt(id).unwrap().profile_removed);
    assert!(
        Profiles::new(base)
            .unwrap()
            .pending_from_previous_run()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn bridge_requires_provider_source_frame_session_epoch_and_typed_schema() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Edit {
        value: i32,
    }
    let mut bridge = Bridge::new(SessionId(7));
    let policy = policy();
    let epoch = bridge
        .grant(&policy, "https://fixture.test/page", &["document.edit"])
        .unwrap();
    let message=serde_json::json!({"version":1,"session":"7","epoch":epoch.to_string(),"command":"document.edit","arguments":{"value":4}}).to_string();
    assert_eq!(
        bridge
            .decode::<Edit>(
                "https://fixture.test/page",
                true,
                "document.edit",
                message.as_bytes()
            )
            .unwrap(),
        Edit { value: 4 }
    );
    assert_eq!(
        bridge.decode::<Edit>(
            "https://evil.test",
            true,
            "document.edit",
            message.as_bytes()
        ),
        Err(Error::Denied)
    );
    assert_eq!(
        bridge.decode::<Edit>(
            "https://fixture.test/page",
            false,
            "document.edit",
            message.as_bytes()
        ),
        Err(Error::Denied)
    );
    bridge.revoke();
    assert!(
        bridge
            .decode::<Edit>(
                "https://fixture.test/page",
                true,
                "document.edit",
                message.as_bytes()
            )
            .is_err()
    );
    bridge.close();
    assert!(
        bridge
            .grant(&policy, "https://fixture.test/page", &["document.edit"])
            .is_err()
    );
    assert_eq!(placement().pixels().unwrap(), [15, 30, 165, 105]);
    let mut bad = placement();
    bad.masked = true;
    assert!(matches!(bad.pixels(), Err(Error::Unsupported(_))));
}

#[test]
fn crashed_group_exit_before_dispose_still_allows_confirmed_cleanup() {
    let (_rt, _, c) = owner();
    let mut m = Manager::new(
        Mock::default(),
        Profiles::new(root()).unwrap(),
        Budgets::default(),
    );
    let id = open(&mut m, Profile::default(), Lifetime::KeepHot, c);
    let lease = m.profile(id).unwrap().clone();
    m.provider_mut()
        .events
        .push(Event::Failed(id, "crash".into()));
    m.provider_mut()
        .events
        .push(Event::Exited(lease.session(), 42));
    m.tick(Duration::ZERO).unwrap();
    assert_eq!(m.state(id).unwrap(), State::Failed);
    assert!(lease.path().exists());
    m.dispose(id).unwrap();
    settle(&mut m, id);
    assert!(m.receipt(id).unwrap().profile_removed);
    assert!(m.provider_mut().starts.is_empty());
}

#[test]
fn dispose_during_creation_and_recreate_before_exit_never_resurrect_or_remove_new_profile() {
    let (_rt, _, c) = owner();
    let mut m = Manager::new(
        Mock::default(),
        Profiles::new(root()).unwrap(),
        Budgets::default(),
    );
    let old = m
        .open(
            1,
            placement(),
            policy(),
            Profile::default(),
            Lifetime::default(),
            c.clone(),
        )
        .unwrap();
    let old_session = m.profile(old).unwrap().session();
    m.dispose(old).unwrap();
    let fresh = m
        .open(
            1,
            placement(),
            policy(),
            Profile::default(),
            Lifetime::KeepHot,
            c,
        )
        .unwrap();
    let fresh_path = m.profile(fresh).unwrap().path().to_owned();
    m.tick(Duration::ZERO).unwrap();
    assert_eq!(m.state(old).unwrap(), State::Closing);
    assert_eq!(m.state(fresh).unwrap(), State::Visible);
    m.provider_mut()
        .events
        .push(Event::Exited(old_session, 999));
    m.tick(Duration::from_secs(40)).unwrap();
    assert!(!m.receipt(old).unwrap().session_exited);
    assert!(m.receipt(old).unwrap().failure.is_some());
    m.provider_mut()
        .events
        .push(Event::Failed(old, "late process failure".into()));
    m.provider_mut().events.push(Event::Exited(old_session, 42));
    settle(&mut m, old);
    m.provider_mut().events.push(Event::Exited(old_session, 42));
    m.provider_mut()
        .events
        .push(Event::Failed(old, "late callback after release".into()));
    m.tick(Duration::from_secs(40)).unwrap();
    assert_eq!(m.state(old).unwrap(), State::Released);
    assert!(fresh_path.exists());
    assert_eq!(m.state(fresh).unwrap(), State::Visible);
    m.shutdown().unwrap();
    assert_eq!(m.state(fresh).unwrap(), State::Closing);
    assert!(!m.bridge(fresh).unwrap().borrow().is_active());
}

#[cfg(windows)]
#[test]
fn real_profile_file_lock_is_pending_and_can_be_retried_after_unlock() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_rt, _, c) = owner();
    let mut m = Manager::new(
        Mock::default(),
        Profiles::new(root()).unwrap(),
        Budgets::default(),
    );
    let id = open(&mut m, Profile::default(), Lifetime::default(), c);
    let lease = m.profile(id).unwrap().clone();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(lease.path().join(".view-owner"))
        .unwrap();
    m.dispose(id).unwrap();
    m.provider_mut()
        .events
        .push(Event::Exited(lease.session(), 42));
    for _ in 0..200 {
        m.tick(Duration::from_secs(1)).unwrap();
        if m.receipt(id).unwrap().failure.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(m.receipt(id).unwrap().failure.is_some());
    assert!(lease.path().exists());
    assert!(m.receipt(id).unwrap().pending);
    drop(lock);
    m.retry_cleanup(id).unwrap();
    settle(&mut m, id);
    assert!(!lease.path().exists());
}
