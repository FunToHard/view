//! Explicit passive native fixture; never runs from normal cargo test.
#[cfg(not(windows))]
fn main() {
    eprintln!("Unsupported: native WebView2 fixture requires Windows");
    std::process::exit(2);
}
#[cfg(windows)]
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    app::run()
}
#[cfg(windows)]
mod app {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
        sync::Arc,
        time::{Duration, Instant},
    };
    use view_core::{Cancellation, Description, Dirty, LogicalRect, ResourceKind, Runtime};
    use view_webview::{native::*, *};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };
    const HTML: &str = r#"<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'"><title>Owned M6 fixture</title><input id="edit" value="native panel"><p>Local state fixture</p><div style="height:2000px">Scrollable content</div><script>window.fixture=7;</script>"#;
    type ScriptReplies = Rc<RefCell<Vec<(PanelId, Result<String>)>>>;
    struct App {
        manager: Option<Manager<WindowsProvider>>,
        window: Option<Arc<Window>>,
        _runtime: Runtime<(), ()>,
        owner: Cancellation,
        start: Instant,
        ids: Vec<PanelId>,
        navigated: BTreeSet<PanelId>,
        stage: u8,
        replies: ScriptReplies,
        cycle: usize,
        passed: bool,
        failure: Option<String>,
        show: bool,
        log: Vec<String>,
        bridge_verified: bool,
    }
    impl App {
        fn script(&mut self, id: PanelId, code: &str) -> Result<()> {
            let replies = self.replies.clone();
            self.manager
                .as_mut()
                .ok_or(Error::Stale)?
                .provider_mut()
                .execute_script(id, code, move |r| replies.borrow_mut().push((id, r)))
        }
        fn placement(x: f32) -> Placement {
            Placement {
                bounds: LogicalRect::from_xywh(x, 0., 190., 500.),
                scale: 1.,
                transformed: false,
                masked: false,
            }
        }
        fn policy() -> PagePolicy {
            PagePolicy {
                documents: ["about:blank".into()].into(),
                ..Default::default()
            }
        }
        fn open(&mut self, profile: Profile, lifetime: Lifetime, x: f32) -> Result<PanelId> {
            self.manager.as_mut().ok_or(Error::Stale)?.open(
                1,
                Self::placement(x),
                Self::policy(),
                profile,
                lifetime,
                self.owner.clone(),
            )
        }
        fn tick(&mut self) -> Result<bool> {
            if self.start.elapsed() > Duration::from_secs(120) {
                return Err(Error::Native("native fixture deadline exceeded".into()));
            }
            let m = self.manager.as_mut().ok_or(Error::Stale)?;
            m.tick(self.start.elapsed())?;
            for event in m.notices() {
                println!("M6 fixture event: {event:?}");
                match event {
                    Event::Navigation(id, _, true) => {
                        self.navigated.insert(id);
                    }
                    Event::Failed(_, error) => return Err(Error::Native(error)),
                    Event::Message(id, source, bytes) => {
                        #[derive(serde::Deserialize)]
                        #[serde(deny_unknown_fields)]
                        struct Edit {
                            value: u32,
                        }
                        let edit: Edit =
                            m.bridge(id)?
                                .borrow()
                                .decode(&source, true, "fixture.edit", &bytes)?;
                        if id != self.ids[1] || edit.value != 42 {
                            return Err(Error::Denied);
                        }
                        self.bridge_verified = true;
                    }
                    Event::Navigation(_, url, false) => {
                        return Err(Error::Native(format!("navigation failed: {url}")));
                    }
                    _ => {}
                }
            }
            match self.stage {
                0 if self.ids.iter().all(|id| m.state(*id) == Ok(State::Visible)) => {
                    for id in &self.ids {
                        m.provider_mut().navigate_html(*id, HTML)?;
                    }
                    self.stage = 1;
                }
                1 if self.ids.iter().all(|id| self.navigated.contains(id)) => {
                    let hot = self.ids[1];
                    let epoch = m.bridge(hot)?.borrow_mut().grant(
                        &Self::policy(),
                        "about:blank",
                        &["fixture.edit"],
                    )?;
                    let payload = serde_json::json!({"version":1,"session":m.profile(hot)?.session().0.to_string(),"epoch":epoch.to_string(),"command":"fixture.edit","arguments":{"value":42}});
                    self.script(hot, &format!("window.fixture=42; window.chrome.webview.postMessage({payload}); window.fixture"))?;
                    self.stage = 2;
                }
                2 if !self.replies.borrow().is_empty() && self.bridge_verified => {
                    let response = self.replies.borrow_mut().remove(0).1?;
                    if response != "42" {
                        return Err(Error::Native(format!("script setup: {response}")));
                    }
                    m.finish(self.ids[1], 64 * 1024 * 1024)?;
                    if m.focus(self.ids[1]) != Err(Error::Denied) {
                        return Err(Error::Native("hot focus was permitted".into()));
                    }
                    m.finish(self.ids[0], 0)?;
                    m.dispose(self.ids[2])?;
                    self.log.push(
                        "closed dedicated and one shared sibling; hot controller hidden".into(),
                    );
                    self.stage = 3;
                }
                3 if m.receipt(self.ids[0])?.profile_removed => {
                    if !m.receipt(self.ids[0])?.session_exited
                        || !m.receipt(self.ids[2])?.shared_retained
                    {
                        return Err(Error::Native("incorrect cleanup scopes".into()));
                    }
                    m.reattach(self.ids[1], self.owner.clone())?;
                    self.script(self.ids[1], "window.fixture")?;
                    self.script(self.ids[3], "window.fixture")?;
                    self.stage = 4;
                }
                4 if self.replies.borrow().len() == 2 => {
                    let values = self
                        .replies
                        .borrow_mut()
                        .drain(..)
                        .map(|(id, result)| result.map(|value| (id, value)))
                        .collect::<Result<BTreeMap<_, _>>>()?;
                    if values.get(&self.ids[1]).map(String::as_str) != Some("42")
                        || values.get(&self.ids[3]).map(String::as_str) != Some("7")
                    {
                        return Err(Error::Native(format!(
                            "hot/sibling state changed: {values:?}"
                        )));
                    }
                    self.log.push(
                        "PASS hot state=42 without reload/suspend; shared sibling state=7".into(),
                    );
                    self.stage = 5;
                }
                5 => {
                    let receipt = m.forget(self.ids[0])?;
                    self.log.push(format!(
                        "cycle {} cleanup={receipt:?} owned={:?}",
                        self.cycle,
                        m.provider_mut().owned_counts()
                    ));
                    self.cycle += 1;
                    if self.cycle < 3 {
                        let fresh = self.open(Profile::default(), Lifetime::default(), 0.)?;
                        self.ids[0] = fresh;
                        self.stage = 6;
                    } else {
                        m.shutdown()?;
                        self.stage = 8;
                    }
                }
                6 if m.state(self.ids[0])? == State::Visible => {
                    m.provider_mut().navigate_html(self.ids[0], HTML)?;
                    self.stage = 7;
                }
                7 if self.navigated.contains(&self.ids[0]) => {
                    m.dispose(self.ids[0])?;
                    self.stage = 9;
                }
                9 if m.receipt(self.ids[0])?.profile_removed => {
                    self.stage = 5;
                }
                8 if m.receipt(self.ids[1])?.profile_removed => {
                    let counts = m.provider_mut().owned_counts();
                    if counts != (0, 0, 0) {
                        return Err(Error::Native(format!(
                            "owned references remain: {counts:?}"
                        )));
                    }
                    self.log.push(format!("PASS shutdown dedicated exit/profile cleanup; shared profile retained; owned controllers/environments/pending={counts:?}; elapsed={:?}",self.start.elapsed()));
                    self.log.push("Native keyboard/IME/UIA client/mixed-monitor certification not exercised by this passive fixture.".into());
                    return Ok(true);
                }
                _ => {}
            }
            Ok(false)
        }
    }
    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            println!("M6 fixture: creating native parent and provider");
            let result = (|| -> Result<()> {
                let window = Arc::new(
                    event_loop
                        .create_window(
                            Window::default_attributes()
                                .with_title("view M6 owned WebView2 fixture")
                                .with_inner_size(winit::dpi::LogicalSize::new(800., 600.))
                                .with_visible(self.show),
                        )
                        .map_err(|e| Error::Native(e.to_string()))?,
                );
                let mut provider = WindowsProvider::new()?;
                provider.register_parent(1, window.clone())?;
                self.log
                    .push(format!("WebView2 runtime={}", runtime_version()?));
                let root = std::env::current_dir()?
                    .join("target/webview-native")
                    .join(format!("run-{}", std::process::id()));
                let profiles = Profiles::new(&root)?;
                self.log.push(format!("owned root={}", root.display()));
                self.manager = Some(Manager::new(provider, profiles, Budgets::default()));
                self.window = Some(window);
                let disposable = self.open(Profile::default(), Lifetime::default(), 0.)?;
                println!("M6 fixture: disposable creation scheduled");
                let hot = self.open(Profile::default(), Lifetime::KeepHot, 200.)?;
                println!("M6 fixture: hot creation scheduled");
                let shared_a = self.open(
                    Profile::Shared(root.join("explicit-shared")),
                    Lifetime::default(),
                    400.,
                )?;
                let shared_b = self.open(
                    Profile::Shared(root.join("explicit-shared")),
                    Lifetime::default(),
                    600.,
                )?;
                self.ids = vec![disposable, hot, shared_a, shared_b];
                println!("M6 fixture: all creation scheduled");
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(20),
                ));
                Ok(())
            })();
            if let Err(e) = result {
                self.failure = Some(e.to_string());
                event_loop.exit();
            }
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            if event == WindowEvent::CloseRequested {
                self.failure = Some("fixture interrupted".into());
                event_loop.exit();
            }
            if matches!(
                event,
                WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Moved(_)
            ) && let (Some(window), Some(m)) = (&self.window, &mut self.manager)
            {
                for (index, id) in self.ids.iter().enumerate() {
                    let mut p = Self::placement(index as f32 * 200.);
                    p.scale = window.scale_factor();
                    if matches!(m.state(*id), Ok(State::Visible | State::Hot)) {
                        let _ = m.place(*id, 1, p);
                    }
                }
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if self.passed || self.failure.is_some() {
                event_loop.exit();
                return;
            }
            if self.manager.is_none() {
                return;
            }
            let stage = self.stage;
            let result = self.tick();
            if self.stage != stage {
                println!("M6 fixture: stage {stage} -> {}", self.stage);
            }
            match result {
                Ok(true) => {
                    self.passed = true;
                    event_loop.exit();
                }
                Ok(false) => event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(20),
                )),
                Err(e) => {
                    self.failure = Some(e.to_string());
                    event_loop.exit();
                }
            }
        }
    }
    pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args == ["--runtime"] {
            println!("{}", runtime_version()?);
            return Ok(());
        }
        if args != ["--native-check"] && args != ["--show"] {
            return Err("explicit --native-check (hidden), --show, or --runtime required".into());
        }
        let mut runtime = Runtime::<(), ()>::new(8)?;
        let (_, root) =
            runtime.open_window(Description::new("webview-fixture", (), |_, _, _| {
                Dirty::NONE
            }))?;
        let owner = runtime
            .own(root, ResourceKind::Subscription, || {})?
            .cancellation();
        let mut app = App {
            manager: None,
            window: None,
            _runtime: runtime,
            owner,
            start: Instant::now(),
            ids: vec![],
            navigated: BTreeSet::new(),
            stage: 0,
            replies: Default::default(),
            cycle: 0,
            passed: false,
            failure: None,
            show: args == ["--show"],
            log: vec![],
            bridge_verified: false,
        };
        EventLoop::new()?.run_app(&mut app)?;
        std::fs::create_dir_all("target/verification")?;
        let report = format!(
            "{}\npassed={} failure={:?}\n",
            app.log.join("\n"),
            app.passed,
            app.failure
        );
        std::fs::write("target/verification/m6-native.txt", &report)?;
        println!("{report}");
        if !app.passed {
            return Err(app
                .failure
                .unwrap_or("native fixture did not complete".into())
                .into());
        }
        Ok(())
    }
}
