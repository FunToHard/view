//! Explicit native windows. --run-hidden never steals focus or injects desktop input.
use crate::{
    Action, Proofs, Result,
    presentation::{Presentation, wait},
};
use view::{render::*, *};
use view_platform::{
    AppConfig, PlatformApp, PlatformError, PlatformHandler, PlatformShell, PlatformWindow,
    PlatformWindowEvent,
};
use winit::event_loop::ActiveEventLoop;

struct Window {
    id: WindowId,
    context: GpuContext,
    surface: WindowSurface<'static>,
    paint: Presentation,
}
struct App {
    proofs: Proofs,
    windows: Vec<Window>,
    hidden: bool,
    service: crate::service::FakeService,
    source_service: crate::service::SourceService,
    drag: Option<viewport::DragSession<f32>>,
    clipboard: MemoryClipboard,
    #[cfg(all(feature = "desktop-test", windows))]
    desktop: Option<DesktopTest>,
}
#[derive(Default)]
struct MemoryClipboard(String);
impl text::Clipboard for MemoryClipboard {
    fn read(&mut self) -> std::result::Result<String, text::TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, s: &str) -> std::result::Result<(), text::TextError> {
        self.0 = s.into();
        Ok(())
    }
}
fn platform(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::EventLoop(e.to_string())
}
impl App {
    fn draw(&mut self, shell: &PlatformShell, id: WindowId) -> Result<()> {
        let Some(window) = shell.get_window(id) else {
            return Ok(());
        };
        let size = window.shared_window().inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let gpu = self
            .windows
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("missing renderer")?;
        let snapshot = gpu.paint.paint(
            &mut self.proofs,
            &gpu.context,
            id,
            [size.width, size.height],
            window.dpi().scale_factor,
        )?;
        let raw = window.shared_window();
        view_platform::ImeService::set_ime_allowed(&raw, gpu.paint.candidate.is_some())?;
        if let Some(candidate) = gpu.paint.candidate {
            view_platform::ImeService::set_ime_cursor_area(
                &raw,
                candidate,
                window.dpi().scale_factor,
            )?;
        }
        gpu.surface.resize(snapshot.size);
        if let Acquire::Ready(frame) = gpu.surface.acquire(&gpu.context)? {
            frame.present(
                &gpu.context,
                gpu.paint.renderer.prepare(&snapshot)?,
                Color([0.05, 0.06, 0.08, 1.]),
            )?;
        }
        Ok(())
    }
    fn event(
        &mut self,
        shell: &mut PlatformShell,
        events: &ActiveEventLoop,
        id: WindowId,
        event: PlatformWindowEvent,
    ) -> Result<()> {
        let editing = self.proofs.editing_node(id);
        match event {
            PlatformWindowEvent::CloseRequested => {
                self.windows.retain(|w| w.id != id);
                shell.close_window(id)?;
                self.proofs.ui.close_window(id)?;
                if self.windows.is_empty() {
                    events.exit();
                }
                return Ok(());
            }
            PlatformWindowEvent::RedrawRequested => return self.draw(shell, id),
            PlatformWindowEvent::Resized { logical_size, .. } => {
                self.proofs.ui.set_window_size(id, logical_size)?
            }
            PlatformWindowEvent::Pointer(e) => {
                #[cfg(all(feature = "desktop-test", windows))]
                if let Some(test) = &mut self.desktop {
                    test.last_pointer = Some((id, e.position));
                }
                let routed = self
                    .proofs
                    .ui
                    .route_pointer(id, e, &mut self.proofs.document)?;
                for route in &routed {
                    if let RoutedPointerAction::Down { target, .. } = route
                        && self
                            .proofs
                            .ui
                            .snapshot()
                            .nodes
                            .iter()
                            .any(|n| n.id == *target && n.focusable)
                    {
                        self.proofs.ui.set_focus(id, Some(*target))?;
                    }
                    if let RoutedPointerAction::Scroll {
                        target,
                        delta_x,
                        delta_y,
                    } = route
                    {
                        let consumed = self
                            .proofs
                            .ui
                            .scroll_by(*target, LogicalPoint::new(-delta_x, -delta_y))?;
                        #[cfg(all(feature = "desktop-test", windows))]
                        if let Some(test) = &mut self.desktop {
                            test.log.push(format!("wheel received at {:?}, target {:?}, delta=({delta_x},{delta_y}), consumed={consumed:?}", e.position, target));
                        }
                        let _ = consumed;
                    }
                    if let RoutedPointerAction::Click { target, .. } = route {
                        self.proofs.activate(*target)?;
                    }
                }
                if let Some(field) = editing
                    && self.proofs.ui.focused(id)? == Some(field)
                {
                    let bounds = self.proofs.ui.global_rect(field)?;
                    self.proofs.ui.update(
                        field,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| {
                            field.set_focused(true, e.timestamp);
                            field.pointer(e, bounds)
                        },
                    )??;
                }
                if id == self.proofs.editor
                    && e.phase == PointerPhase::Down
                    && e.button == Some(PointerButton::Primary)
                    && self
                        .proofs
                        .ui
                        .global_rect(self.proofs.gizmo)?
                        .contains(e.position)
                {
                    self.proofs.ui.capture_pointer(id, self.proofs.gizmo)?;
                    self.drag = Some(viewport::DragSession::new(
                        self.proofs.gizmo,
                        7,
                        self.proofs.document.revision,
                        self.proofs.document.x,
                    ));
                }
                let had_drag = self.drag.is_some();
                if let Some(drag) = &mut self.drag {
                    for route in &routed {
                        if let Some(commit) =
                            drag.route(route, self.proofs.document.revision, |p| {
                                self.proofs.document.x + (p.x - 6.) / 80.
                            })
                        {
                            self.proofs.commit_drag(commit)?;
                        }
                    }
                    if drag.finished() {
                        self.drag = None;
                        self.proofs.set_preview(None)?;
                    } else {
                        self.proofs.set_preview(Some(*drag.preview()))?;
                    }
                }
                if id == self.proofs.editor && e.phase == PointerPhase::Up && !had_drag {
                    let bounds = self.proofs.ui.global_rect(self.proofs.viewport)?;
                    if bounds.contains(e.position) {
                        self.proofs.pick(LogicalPoint::new(
                            e.position.x - bounds.origin.x,
                            e.position.y - bounds.origin.y,
                        ))?;
                    }
                }
                if id == self.proofs.store && matches!(e.phase, PointerPhase::Scroll { .. }) {
                    self.proofs.refresh_catalog()?;
                }
            }
            PlatformWindowEvent::Keyboard(e) => {
                self.proofs
                    .ui
                    .route_keyboard(id, e.clone(), &mut self.proofs.document)?;
                if let Some(field) = editing
                    && self.proofs.ui.focused(id)? == Some(field)
                {
                    self.proofs.ui.update(
                        field,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| {
                            field.set_focused(true, e.timestamp);
                            field.key(&e, &mut self.clipboard)
                        },
                    )??;
                    self.proofs.edited(id)?;
                }
                if e.phase == KeyPhase::Down {
                    if matches!(e.key, Key::Named(NamedKey::Enter | NamedKey::Space))
                        && let Some(target) = self.proofs.ui.focused(id)?
                    {
                        self.proofs.activate(target)?;
                    }
                    if e.key == Key::Named(NamedKey::Escape) {
                        if self.proofs.ui.modal_scope(id)?.is_some() {
                            self.proofs.ui.set_modal_scope(id, None)?;
                        }
                        self.proofs.ui.cancel_pointer(id)?;
                        self.drag = None;
                        self.proofs.set_preview(None)?;
                    }
                    if id == self.proofs.editor
                        && e.modifiers.ctrl
                        && e.key == Key::Character("z".into())
                    {
                        self.proofs.dispatch(Action::Undo)?;
                    }
                    if id == self.proofs.editor && e.key == Key::Named(NamedKey::ArrowRight) {
                        self.proofs.dispatch(Action::Move {
                            base: self.proofs.document.revision,
                            x: self.proofs.document.x + 0.1,
                        })?;
                    }
                    if id == self.proofs.store
                        && e.key == Key::Named(NamedKey::Enter)
                        && self.proofs.ui.focused(id)? == Some(self.proofs.search)
                    {
                        let query = self.proofs.query.clone();
                        let request = self.proofs.search(&query)?;
                        self.service
                            .start(request, std::time::Duration::from_millis(80))
                            .map_err(|_| "fake service capacity; retry retrieval")?;
                    }
                    if id == self.proofs.detached
                        && e.modifiers.ctrl
                        && e.key == Key::Named(NamedKey::Enter)
                    {
                        let request = self.proofs.preview.begin("source")?;
                        self.source_service
                            .start(request, self.proofs.document.source.clone())
                            .map_err(|_| "preview capacity; retry")?;
                    }
                }
            }
            PlatformWindowEvent::TextInput(text) => {
                if let Some(field) = editing
                    && self.proofs.ui.focused(id)? == Some(field)
                {
                    self.proofs.ui.update(
                        field,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| field.insert_text(&text, 0),
                    )??;
                    self.proofs.edited(id)?;
                }
            }
            PlatformWindowEvent::Focused(false) => {
                self.proofs.ui.cancel_pointer(id)?;
                self.drag = None;
                self.proofs.set_preview(None)?;
            }
            PlatformWindowEvent::Ime(event) => {
                #[cfg(all(feature = "desktop-test", windows))]
                if let Some(test) = &mut self.desktop {
                    match &event {
                        view_platform::ImeEvent::Preedit(text, _) if !text.is_empty() => {
                            test.preedits += 1
                        }
                        view_platform::ImeEvent::Commit(text) if !text.is_empty() => {
                            test.commits += 1
                        }
                        _ => {}
                    }
                }
                if let Some(field) = editing
                    && self.proofs.ui.focused(id)? == Some(field)
                {
                    self.proofs.ui.update(
                        field,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| match event {
                            view_platform::ImeEvent::Preedit(text, cursor) => {
                                field.preedit(&text, cursor)
                            }
                            view_platform::ImeEvent::Commit(text) => field.commit_ime(&text, 0),
                            view_platform::ImeEvent::Disabled => field.preedit("", None),
                            view_platform::ImeEvent::Enabled => Ok(()),
                        },
                    )??;
                    self.proofs.edited(id)?;
                }
            }
            _ => {}
        }
        self.proofs.ui.flush(&mut self.proofs.document)?;
        for window in &self.windows {
            if let Some(native) = shell.get_window(window.id) {
                native.shared_window().request_redraw();
            }
        }
        Ok(())
    }
}
impl PlatformHandler for App {
    fn on_ready(
        &mut self,
        shell: &mut PlatformShell,
        events: &ActiveEventLoop,
    ) -> std::result::Result<(), PlatformError> {
        (||->Result<()> {
            self.proofs.verify()?;
            let instance=wgpu::Instance::new(wgpu::InstanceDescriptor {backends:wgpu::Backends::DX12,..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(events.owned_display_handle()))});
            for (id,title) in [(self.proofs.editor,"view M4 — engine editor"),(self.proofs.detached,"view M4 — detached inspector / IDE"),(self.proofs.store,"view M4 — native storefront")] {
                shell.create_window(events,id,PlatformWindow::default_attributes(title,LogicalSize::new(800.,600.)).with_visible(false))?;
                let raw=shell.get_window(id).ok_or("missing window")?.shared_window();
                let surface=instance.create_surface(raw.clone())?;
                let adapter=wait(instance.request_adapter(&wgpu::RequestAdapterOptions {compatible_surface:Some(&surface),..Default::default()}))??;
                let context=wait(GpuContext::request(&adapter,wgpu::Features::empty(),wgpu::Limits::default()))??;
                let surface=WindowSurface::new(surface,&adapter,&context,[800,600])?;
                let paint=Presentation::new(context.clone(),surface.format())?;
                self.windows.push(Window {id,context,surface,paint});self.draw(shell,id)?;
                if !self.hidden {raw.set_visible(true);}
            }
            if self.hidden {
                for window in self.windows.drain(..){assert!(window.context.take_errors().is_empty());shell.close_window(window.id)?;self.proofs.ui.close_window(window.id)?;}
                println!("PASS three hidden native surfaces: editor, detached IDE, storefront; no physical-input certification");events.exit();
            }
            Ok(())
        })().map_err(platform)
    }
    fn on_window_event(
        &mut self,
        shell: &mut PlatformShell,
        events: &ActiveEventLoop,
        id: WindowId,
        event: PlatformWindowEvent,
    ) -> std::result::Result<(), PlatformError> {
        self.event(shell, events, id, event).map_err(platform)
    }
    fn on_idle(
        &mut self,
        shell: &mut PlatformShell,
        events: &ActiveEventLoop,
    ) -> std::result::Result<(), PlatformError> {
        #[cfg(all(feature = "desktop-test", windows))]
        if self.desktop.is_some() {
            self.desktop_tick(shell, events).map_err(platform)?;
        }
        let mut changed = false;
        while let Some((request, source)) = self.source_service.receive() {
            self.proofs
                .source_reply(&request, source)
                .map_err(platform)?;
            if let Some(window) = shell.get_window(self.proofs.detached) {
                window.shared_window().request_redraw();
            }
        }
        while let Some((request, items)) = self.service.receive() {
            self.proofs.reply(&request, items).map_err(platform)?;
            changed = true;
        }
        if let Some((request, progress)) = self.service.progress() {
            self.proofs
                .results
                .set_progress(&request, progress)
                .map_err(platform)?;
            changed = true;
        }
        if changed && let Some(window) = shell.get_window(self.proofs.store) {
            window.shared_window().request_redraw();
        }
        let testing = {
            #[cfg(all(feature = "desktop-test", windows))]
            {
                self.desktop.is_some()
            }
            #[cfg(not(all(feature = "desktop-test", windows)))]
            {
                false
            }
        };
        events.set_control_flow(
            if testing || self.service.pending() || self.source_service.pending() {
                winit::event_loop::ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(16),
                )
            } else {
                winit::event_loop::ControlFlow::Wait
            },
        );
        Ok(())
    }
}
pub fn run(hidden: bool) -> Result<()> {
    run_mode(hidden, false)
}
pub fn run_mode(hidden: bool, desktop_test: bool) -> Result<()> {
    run_config(hidden, desktop_test, false)
}
#[cfg(all(feature = "desktop-test", windows))]
pub fn run_japanese() -> Result<()> {
    let layouts = view_platform::desktop_input::WindowsDriver::installed_layouts();
    if !layouts.iter().any(|layout| layout & 0xffff == 0x411) {
        std::fs::create_dir_all("target/verification")?;
        std::fs::write(
            "target/verification/m5-japanese-ime.txt",
            format!(
                "UNCOVERED: Japanese IME/layout not installed; layouts={layouts:x?}. No language was installed or activated."
            ),
        )?;
        return Err("Japanese IME prerequisite unavailable; uncovered, not passed".into());
    }
    run_config(false, true, true)
}
fn run_config(hidden: bool, desktop_test: bool, japanese: bool) -> Result<()> {
    #[cfg(not(all(feature = "desktop-test", windows)))]
    if desktop_test {
        return Err("Windows desktop-test feature required".into());
    }
    #[cfg(not(all(feature = "desktop-test", windows)))]
    let _ = japanese;
    PlatformApp::new(
        App {
            proofs: Proofs::new()?,
            windows: vec![],
            hidden,
            service: Default::default(),
            source_service: Default::default(),
            drag: None,
            clipboard: Default::default(),
            #[cfg(all(feature = "desktop-test", windows))]
            desktop: desktop_test.then(|| DesktopTest::new(japanese)),
        },
        AppConfig {
            run_hidden: hidden,
            max_iterations: None,
        },
    )?
    .run()?;
    Ok(())
}

#[cfg(all(feature = "desktop-test", windows))]
struct DesktopTest {
    stage: u8,
    next: std::time::Instant,
    started: std::time::Instant,
    focus: Option<NodeId>,
    source: String,
    log: Vec<String>,
    passed: bool,
    last_pointer: Option<(WindowId, LogicalPoint)>,
    japanese: bool,
    preedits: usize,
    commits: usize,
    modal: Option<NodeId>,
}
#[cfg(all(feature = "desktop-test", windows))]
impl DesktopTest {
    fn new(japanese: bool) -> Self {
        Self {stage:0,next:std::time::Instant::now(),started:std::time::Instant::now(),focus:None,source:String::new(),log:vec!["M5 Windows native input; explicit selected desktop; F12 stops; actual event-driven application assertions".into()],passed:false,last_pointer:None,japanese,preedits:0,commits:0,modal:None}
    }
}
#[cfg(all(feature = "desktop-test", windows))]
impl Drop for DesktopTest {
    fn drop(&mut self) {
        let _ = std::fs::create_dir_all("target/verification");
        self.log.push(format!(
            "result={} stage={} elapsed={:?}",
            if self.passed { "passed" } else { "failed" },
            self.stage,
            self.started.elapsed()
        ));
        self.log.push("Coverage: physical en-US keys/pointer/wheel and Unicode insertion are distinct. Japanese IME/native AT/mixed monitor hardware remain uncovered unless separately recorded.".into());
        let path = if self.japanese {
            "target/verification/m5-japanese-ime.txt"
        } else {
            "target/verification/m5-native.txt"
        };
        let _ = std::fs::write(path, self.log.join("\n"));
    }
}
#[cfg(all(feature = "desktop-test", windows))]
impl App {
    fn native_group(
        &self,
        shell: &PlatformShell,
        id: WindowId,
        point: Option<LogicalPoint>,
        input: Vec<view_automation::desktop::Event>,
    ) -> Result<u64> {
        use view_automation::desktop::{DesktopSession, Event};
        let raw = shell
            .get_window(id)
            .ok_or("native target closed")?
            .shared_window();
        let driver = view_platform::desktop_input::WindowsDriver::select(raw, true)?;
        let revision = self.proofs.ui.snapshot().revision;
        let map = driver.screen_map(revision.get())?;
        let keyboard = driver.keyboard_layout();
        let session = DesktopSession::start(driver, true, std::time::Duration::from_secs(3))?;
        if let Some(point) = point {
            let (x, y) = map.convert(point, revision.get())?;
            session.send(Event::Move { x, y })?;
        }
        for event in input {
            session.send(event)?;
        }
        session.finish()?;
        Ok(keyboard)
    }
    fn desktop_tick(&mut self, shell: &mut PlatformShell, events: &ActiveEventLoop) -> Result<()> {
        use view_automation::desktop::{Event, Held};
        let mut test = self.desktop.take().ok_or("test missing")?;
        if std::time::Instant::now() < test.next {
            self.desktop = Some(test);
            return Ok(());
        }
        let tap = |key| vec![Event::Down(Held::Key(key)), Event::Up(Held::Key(key))];
        let chord = |key| {
            vec![
                Event::Down(Held::Key(0x11)),
                Event::Down(Held::Key(key)),
                Event::Up(Held::Key(key)),
                Event::Up(Held::Key(0x11)),
            ]
        };
        let click = || vec![Event::Down(Held::Button(0)), Event::Up(Held::Button(0))];
        let result = (|| -> Result<()> {
            if test.started.elapsed() > std::time::Duration::from_secs(90) {
                return Err("native test deadline".into());
            }
            if test.japanese {
                let id = self.proofs.detached;
                let source = self.proofs.editing_node(id).ok_or("source")?;
                match test.stage {
                    0 => shell
                        .get_window(id)
                        .ok_or("IDE")?
                        .shared_window()
                        .focus_window(),
                    1 => {
                        let layout = self.native_group(
                            shell,
                            id,
                            Some(self.proofs.ui.global_rect(source)?.center()),
                            click(),
                        )?;
                        test.log.push(format!("target keyboard layout={layout:#x}"));
                        if layout & 0xffff != 0x411 {
                            return Err("select the Japanese IME in the test window before running this named scenario".into());
                        }
                    }
                    2 => {
                        let mut input = chord(0x41);
                        input.extend(tap(0x08));
                        self.native_group(shell, id, None, input)?;
                    }
                    3 => {
                        let mut input = vec![];
                        for key in [0x4e, 0x49, 0x48, 0x4f, 0x4e] {
                            input.extend(tap(key));
                        }
                        self.native_group(shell, id, None, input)?;
                    }
                    4 => {
                        self.native_group(shell, id, None, tap(0x20))?;
                    }
                    5 => {
                        self.native_group(shell, id, None, tap(0x0d))?;
                    }
                    6 => {
                        if test.preedits == 0
                            || test.commits == 0
                            || !self.proofs.document.source.contains("日本")
                        {
                            return Err(format!("IME expected native preedit/commit and 日本; preedits={} commits={} source={:?}",test.preedits,test.commits,self.proofs.document.source).into());
                        }
                        test.log.push("PASS Japanese IME native preedit/conversion/commit; candidate service used; no Unicode insertion substitute".into());
                        test.passed = true;
                        events.exit();
                    }
                    _ => {}
                }
                return Ok(());
            }
            match test.stage {
                0 => {
                    self.proofs.document.selected = None;
                    self.proofs.document.x = 0.;
                    self.proofs.document.undo.clear();
                    self.proofs.set_preview(None)?;
                    let raw = shell
                        .get_window(self.proofs.editor)
                        .ok_or("editor")?
                        .shared_window();
                    raw.focus_window();
                    test.log
                        .push("setup: reset app document and explicitly focus editor".into());
                }
                1 => {
                    let target = self
                        .proofs
                        .ui
                        .snapshot()
                        .nodes
                        .iter()
                        .find(|n| n.key == "object-7")
                        .ok_or("hierarchy target")?;
                    let point = target.global_bounds.center();
                    if self
                        .proofs
                        .ui
                        .hit_test(self.proofs.editor, point)?
                        .is_none_or(|h| h.target != target.id)
                    {
                        return Err("hierarchy occluded".into());
                    }
                    let layout =
                        self.native_group(shell, self.proofs.editor, Some(point), click())?;
                    test.log.push(format!(
                        "keyboard layout={layout:#x}; pointer hierarchy click inserted"
                    ));
                    if layout & 0xffff != 0x409 {
                        return Err("named en-US fixture requires target en-US keyboard layout; no layout was changed".into());
                    }
                }
                2 => {
                    if self.proofs.document.selected != Some(7) {
                        return Err("physical selection not observed".into());
                    }
                    let p = self.proofs.ui.global_rect(self.proofs.gizmo)?.center();
                    if self
                        .proofs
                        .ui
                        .hit_test(self.proofs.editor, p)?
                        .is_none_or(|h| h.target != self.proofs.gizmo)
                    {
                        return Err("gizmo occluded".into());
                    }
                    let raw = shell
                        .get_window(self.proofs.editor)
                        .ok_or("editor")?
                        .shared_window();
                    let driver = view_platform::desktop_input::WindowsDriver::select(raw, true)?;
                    let map = driver.screen_map(self.proofs.ui.snapshot().revision.get())?;
                    let (x, y) = map.convert(LogicalPoint::new(p.x + 40., p.y), map.revision)?;
                    drop(driver);
                    self.native_group(
                        shell,
                        self.proofs.editor,
                        Some(p),
                        vec![
                            Event::Down(Held::Button(0)),
                            Event::Move { x, y },
                            Event::Up(Held::Button(0)),
                        ],
                    )?;
                }
                3 => {
                    if (self.proofs.document.x - 0.5).abs() > 0.03 {
                        return Err(format!(
                            "physical drag expected X=.5, actual {}",
                            self.proofs.document.x
                        )
                        .into());
                    }
                    test.log.push(format!(
                        "physical gizmo target/capture observed; X={}",
                        self.proofs.document.x
                    ));
                    self.native_group(shell, self.proofs.editor, None, chord(0x5a))?;
                }
                4 => {
                    if self.proofs.document.x.abs() > 0.001 {
                        return Err("physical Ctrl+Z did not restore X".into());
                    }
                    self.native_group(shell, self.proofs.editor, None, tap(0x27))?;
                }
                5 => {
                    if (self.proofs.document.x - 0.1).abs() > 0.001 {
                        return Err("physical Right Arrow edit missing".into());
                    }
                    test.focus = self.proofs.ui.focused(self.proofs.editor)?;
                    self.native_group(shell, self.proofs.editor, None, tap(0x09))?;
                }
                6 => {
                    if self.proofs.ui.focused(self.proofs.editor)? == test.focus {
                        return Err("Tab did not move focus".into());
                    }
                    self.native_group(
                        shell,
                        self.proofs.editor,
                        None,
                        vec![
                            Event::Down(Held::Key(0x10)),
                            Event::Down(Held::Key(0x09)),
                            Event::Up(Held::Key(0x09)),
                            Event::Up(Held::Key(0x10)),
                        ],
                    )?;
                }
                7 => {
                    if self.proofs.ui.focused(self.proofs.editor)? != test.focus {
                        return Err("reverse Tab did not restore focus".into());
                    }
                    self.native_group(shell, self.proofs.editor, None, tap(0x1b))?;
                    shell
                        .get_window(self.proofs.store)
                        .ok_or("store")?
                        .shared_window()
                        .focus_window();
                }
                8 => {
                    let p = self.proofs.ui.global_rect(self.proofs.search)?.center();
                    self.native_group(shell, self.proofs.store, Some(p), click())?;
                }
                9 => {
                    if self.proofs.ui.focused(self.proofs.store)? != Some(self.proofs.search) {
                        return Err("search focus missing".into());
                    }
                    let mut input = chord(0x41);
                    input.extend(tap(0x41));
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                10 => {
                    if self.proofs.query != "a" {
                        return Err(format!(
                            "physical en-US key expected a, actual {:?}",
                            self.proofs.query
                        )
                        .into());
                    }
                    let mut input = chord(0x41);
                    input.extend(tap(0x08));
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                11 => {
                    if !self.proofs.query.is_empty() {
                        return Err("selection/backspace did not clear text".into());
                    }
                    let mut input = vec![];
                    for unit in "native".encode_utf16() {
                        input.push(Event::Down(Held::Unicode(unit)));
                        input.push(Event::Up(Held::Unicode(unit)));
                    }
                    input.extend(tap(0x0d));
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                12 => {
                    if self.proofs.query != "native"
                        || self.proofs.results.value().is_none_or(|v| v.len() != 100)
                    {
                        return Err("Unicode insertion/async catalog not observed".into());
                    }
                    let p = self.proofs.ui.global_rect(self.proofs.catalog)?.center();
                    self.native_group(shell, self.proofs.store, Some(p), vec![])?;
                }
                13 => {
                    let expected = self.proofs.ui.global_rect(self.proofs.catalog)?.center();
                    if test.last_pointer.is_none_or(|(id, p)| {
                        id != self.proofs.store
                            || (p.x - expected.x).abs() > 2.
                            || (p.y - expected.y).abs() > 2.
                    }) {
                        return Err("pointer positioning not observed before wheel group".into());
                    }
                    self.native_group(
                        shell,
                        self.proofs.store,
                        None,
                        vec![Event::Wheel {
                            horizontal: false,
                            delta: -120,
                        }],
                    )?;
                }
                14 => {
                    let offset = self
                        .proofs
                        .ui
                        .scroll_state(self.proofs.catalog)?
                        .ok_or("scroll state")?
                        .offset
                        .y;
                    if offset <= 0. {
                        return Err("physical wheel did not move catalog".into());
                    }
                    test.log.push(format!("physical wheel offset={offset}; virtual catalog refreshed; no offset assignment in test"));
                    test.source = self.proofs.document.source.clone();
                    shell
                        .get_window(self.proofs.detached)
                        .ok_or("IDE")?
                        .shared_window()
                        .focus_window();
                }
                15 => {
                    let source = self
                        .proofs
                        .editing_node(self.proofs.detached)
                        .ok_or("source")?;
                    self.native_group(
                        shell,
                        self.proofs.detached,
                        Some(self.proofs.ui.global_rect(source)?.center()),
                        click(),
                    )?;
                }
                16 => {
                    let mut input = chord(0x41);
                    input.extend(tap(0x58));
                    self.native_group(shell, self.proofs.detached, None, input)?;
                }
                17 => {
                    if self.proofs.document.source != "x" {
                        return Err("IDE physical selection/edit missing".into());
                    }
                    self.native_group(shell, self.proofs.detached, None, chord(0x5a))?;
                }
                18 => {
                    if self.proofs.document.source != test.source {
                        return Err("IDE physical undo did not restore source".into());
                    }
                    let source = self
                        .proofs
                        .editing_node(self.proofs.detached)
                        .ok_or("source")?;
                    test.focus = self.proofs.ui.focused(self.proofs.detached)?;
                    let modal = self.proofs.ui.mount(
                        source,
                        Description::new("test-modal", (), |_, _, _: Action| Dirty::NONE)
                            .role(Role::Dialog)
                            .name("Automation modal"),
                    )?;
                    self.proofs.ui.mount(
                        modal,
                        Description::new("modal-button", (), |_, _, _: Action| Dirty::NONE)
                            .focusable(true)
                            .role(Role::Button)
                            .name("Close test dialog"),
                    )?;
                    self.proofs
                        .ui
                        .set_modal_scope(self.proofs.detached, Some(modal))?;
                    self.proofs.ui.flush(&mut self.proofs.document)?;
                    test.modal = Some(modal);
                    test.log.push("setup: mounted test modal through public API; subsequent Tab and Escape are physical".into());
                }
                19 => {
                    self.native_group(shell, self.proofs.detached, None, tap(0x09))?;
                }
                20 => {
                    let focus = self
                        .proofs
                        .ui
                        .focused(self.proofs.detached)?
                        .ok_or("modal focus missing")?;
                    if !self
                        .proofs
                        .ui
                        .is_descendant(test.modal.ok_or("modal missing")?, focus)
                    {
                        return Err("Tab escaped modal".into());
                    }
                    self.native_group(shell, self.proofs.detached, None, tap(0x1b))?;
                }
                21 => {
                    if self.proofs.ui.focused(self.proofs.detached)? != test.focus {
                        return Err("Escape did not restore pre-modal focus".into());
                    }
                    self.proofs
                        .ui
                        .unmount(test.modal.take().ok_or("modal missing")?)?;
                    test.passed = true;
                    test.log.push("PASS pointer selection/gizmo/undo, en-US arrows/Tab/reverse Tab/Escape/selection/deletion, Unicode insertion, worker catalog, physical wheel virtualization, IDE edit/undo, modal Tab containment/Escape restoration".into());
                    println!("{}", test.log.last().expect("log"));
                    events.exit();
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(ref e) = result {
            test.log.push(format!("FAIL stage {}: {e}", test.stage));
        }
        test.stage += 1;
        test.next = std::time::Instant::now() + std::time::Duration::from_millis(500);
        self.desktop = Some(test);
        result
    }
}
