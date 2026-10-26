//! Explicit native windows. --run-hidden never steals focus or injects desktop input.
use crate::{
    Action, Proofs, Result,
    presentation::{Presentation, wait},
};
#[cfg(all(feature = "desktop-test", windows))]
use view::text::EditorSession;
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
    native_focus: std::collections::HashSet<WindowId>,
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
fn set_native_focus(
    proofs: &mut Proofs,
    native_focus: &mut std::collections::HashSet<WindowId>,
    id: WindowId,
    focused: bool,
) -> Result<()> {
    if focused {
        native_focus.insert(id);
    } else {
        native_focus.remove(&id);
    }
    if let Some(field) = proofs.editing_node(id) {
        let field_focused = focused && proofs.ui.focused(id)? == Some(field);
        proofs
            .ui
            .update(field, Dirty::PAINT, |field: &mut controls::TextField| {
                field.set_focused(field_focused, 0)
            })?;
    }
    Ok(())
}
fn active_editing_field(
    proofs: &Proofs,
    native_focus: &std::collections::HashSet<WindowId>,
    id: WindowId,
) -> Option<NodeId> {
    native_focus
        .contains(&id)
        .then(|| proofs.editing_node(id))
        .flatten()
}
fn edit_active_field<R>(
    proofs: &mut Proofs,
    native_focus: &std::collections::HashSet<WindowId>,
    id: WindowId,
    edit: impl FnOnce(&mut controls::TextField) -> std::result::Result<R, text::TextError>,
) -> Result<Option<R>> {
    let Some(field) = active_editing_field(proofs, native_focus, id) else {
        return Ok(None);
    };
    if proofs.ui.focused(id)? != Some(field) {
        return Ok(None);
    }
    Ok(Some(proofs.ui.update(field, Dirty::PAINT, edit)??))
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
        gpu.paint.native_focus = Some(self.native_focus.contains(&id));
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
        let editing = active_editing_field(&self.proofs, &self.native_focus, id);
        match event {
            PlatformWindowEvent::CloseRequested => {
                self.native_focus.remove(&id);
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
                        if let Some(test) = &mut self.desktop
                            && test.log.len() < 4096
                        {
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
                    let scale = shell
                        .get_window(id)
                        .ok_or("native target closed")?
                        .dpi()
                        .scale_factor;
                    let paint = &mut self
                        .windows
                        .iter_mut()
                        .find(|window| window.id == id)
                        .ok_or("missing renderer")?
                        .paint;
                    self.proofs.ui.update(
                        field,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| -> std::result::Result<_, controls::ControlError> {
                            field.set_focused(true, e.timestamp);
                            let content =
                                paint.prepare_pointer(field, bounds, scale, e.timestamp)?;
                            Ok(field.pointer(e, content)?)
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
                #[cfg(all(feature = "desktop-test", windows))]
                if let Some(test) = &mut self.desktop
                    && test.log.len() < 4096
                {
                    test.log.push(format!(
                        "key window={id:?} stage={} event={e:?}",
                        test.stage
                    ));
                }
                if !self.native_focus.contains(&id) {
                    return Ok(());
                }
                self.proofs
                    .ui
                    .route_keyboard(id, e.clone(), &mut self.proofs.document)?;
                if e.phase != KeyPhase::Up
                    && matches!(
                        e.key,
                        Key::Named(
                            NamedKey::ArrowLeft
                                | NamedKey::ArrowRight
                                | NamedKey::ArrowUp
                                | NamedKey::ArrowDown
                                | NamedKey::PageUp
                                | NamedKey::PageDown
                        )
                    )
                    && let Some(field) = active_editing_field(&self.proofs, &self.native_focus, id)
                    && self.proofs.ui.focused(id)? == Some(field)
                {
                    let bounds = self.proofs.ui.global_rect(field)?;
                    let scale = shell
                        .get_window(id)
                        .ok_or("native target closed")?
                        .dpi()
                        .scale_factor;
                    let paint = &mut self
                        .windows
                        .iter_mut()
                        .find(|window| window.id == id)
                        .ok_or("missing renderer")?
                        .paint;
                    self.proofs.ui.update(
                        field,
                        Dirty::NONE,
                        |field: &mut controls::TextField| {
                            field.set_focused(true, e.timestamp);
                            paint.prepare_navigation(field, bounds, scale, &e)
                        },
                    )??;
                }
                if edit_active_field(
                    &mut self.proofs,
                    &self.native_focus,
                    id,
                    |field: &mut controls::TextField| {
                        field.set_focused(true, e.timestamp);
                        field.key(&e, &mut self.clipboard)
                    },
                )?
                .is_some()
                {
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
                if edit_active_field(
                    &mut self.proofs,
                    &self.native_focus,
                    id,
                    |field: &mut controls::TextField| field.insert_text(&text, 0),
                )?
                .is_some()
                {
                    self.proofs.edited(id)?;
                }
            }
            PlatformWindowEvent::Focused(focused) => {
                set_native_focus(&mut self.proofs, &mut self.native_focus, id, focused)?;
                if !focused {
                    self.proofs.ui.cancel_pointer(id)?;
                    self.drag = None;
                    self.proofs.set_preview(None)?;
                }
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
                if edit_active_field(
                    &mut self.proofs,
                    &self.native_focus,
                    id,
                    |field: &mut controls::TextField| match event {
                        view_platform::ImeEvent::Preedit(text, cursor) => {
                            field.preedit(&text, cursor)
                        }
                        view_platform::ImeEvent::Commit(text) => field.commit_ime(&text, 0),
                        view_platform::ImeEvent::Disabled => field.preedit("", None),
                        view_platform::ImeEvent::Enabled => Ok(()),
                    },
                )?
                .is_some()
                {
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
            native_focus: Default::default(),
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
    held_session: Option<view_automation::desktop::DesktopSession>,
}
#[cfg(all(feature = "desktop-test", windows))]
impl DesktopTest {
    fn new(japanese: bool) -> Self {
        Self {stage:0,next:std::time::Instant::now(),started:std::time::Instant::now(),focus:None,source:String::new(),log:vec!["M5 Windows native input; explicit selected desktop; F12 stops; actual event-driven application assertions".into()],passed:false,last_pointer:None,japanese,preedits:0,commits:0,modal:None,held_session:None}
    }
}
#[cfg(all(feature = "desktop-test", windows))]
impl Drop for DesktopTest {
    fn drop(&mut self) {
        // Join cleanup before recording a failure, including an interrupted fixture.
        drop(self.held_session.take());
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
    fn assert_native_page(&self, anchor: usize, focus: usize, scroll: f32) -> Result<()> {
        let source = self
            .proofs
            .editing_node(self.proofs.detached)
            .ok_or("source")?;
        let field = self.proofs.ui.state::<controls::TextField>(source)?;
        let selection = field.snapshot().selection;
        let expected_text = ["abcdefghij"; 40].join("\n");
        if selection.anchor != text::TextIndex(anchor)
            || selection.focus != text::TextIndex(focus)
            || (field.editor().scroll_offset().y - scroll).abs() > 0.01
            || self.proofs.document.source != expected_text
        {
            return Err(format!("native Page expected anchor={anchor},focus={focus},scroll={scroll}; actual selection={selection:?},scroll={:?}", field.editor().scroll_offset()).into());
        }
        Ok(())
    }
    fn hold_test_shift(
        &self,
        shell: &PlatformShell,
        id: WindowId,
    ) -> Result<view_automation::desktop::DesktopSession> {
        use view_automation::desktop::{DesktopSession, Event, Held};
        let raw = shell
            .get_window(id)
            .ok_or("native target closed")?
            .shared_window();
        // Selection rejects pre-existing human modifiers/buttons; send checks
        // Shift again before taking ownership. The watchdog/F12 remain active.
        let driver = view_platform::desktop_input::WindowsDriver::select(raw, true)?;
        let session = DesktopSession::start(driver, true, std::time::Duration::from_secs(3))?;
        session.send(Event::Down(Held::Key(0x10)))?;
        Ok(session)
    }
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
                    let mut input = tap(0x24); // Home
                    input.extend([
                        Event::Down(Held::Key(0x10)), // Shift
                        Event::Down(Held::Key(0x23)), // End
                        Event::Up(Held::Key(0x23)),
                        Event::Up(Held::Key(0x10)),
                    ]);
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                11 => {
                    let selection = self
                        .proofs
                        .ui
                        .state::<controls::TextField>(self.proofs.search)?
                        .snapshot()
                        .selection;
                    if self.proofs.query != "a" || selection.range() != (0..1) {
                        return Err(format!(
                            "physical Home/Shift+End expected selected range 0..1; query={:?}, range={:?}",
                            self.proofs.query,
                            selection.range()
                        )
                        .into());
                    }
                    self.native_group(shell, self.proofs.store, None, tap(0x2e))?; // Delete
                }
                12 => {
                    if !self.proofs.query.is_empty() {
                        return Err("physical Delete did not clear the selected text".into());
                    }
                    let mut input = vec![];
                    for unit in "native".encode_utf16() {
                        input.push(Event::Down(Held::Unicode(unit)));
                        input.push(Event::Up(Held::Unicode(unit)));
                    }
                    input.extend(tap(0x0d));
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                13 => {
                    if self.proofs.query != "native"
                        || self.proofs.results.value().is_none_or(|v| v.len() != 100)
                    {
                        return Err("Unicode insertion/async catalog not observed".into());
                    }
                    let mut input = chord(0x24); // Ctrl+Home
                    input.extend([
                        Event::Down(Held::Key(0x11)), // Ctrl
                        Event::Down(Held::Key(0x10)), // Shift
                        Event::Down(Held::Key(0x23)), // End
                        Event::Up(Held::Key(0x23)),
                        Event::Up(Held::Key(0x10)),
                        Event::Up(Held::Key(0x11)),
                    ]);
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                14 => {
                    let selection = self
                        .proofs
                        .ui
                        .state::<controls::TextField>(self.proofs.search)?
                        .snapshot()
                        .selection;
                    if self.proofs.query != "native" || selection.range() != (0..6) {
                        return Err(format!(
                            "physical Ctrl+Home/Ctrl+Shift+End expected range 0..6; query={:?}, range={:?}",
                            self.proofs.query,
                            selection.range()
                        )
                        .into());
                    }
                    self.native_group(shell, self.proofs.store, None, chord(0x08))?; // Ctrl+Backspace
                }
                15 => {
                    if !self.proofs.query.is_empty() {
                        return Err("Ctrl+Backspace did not delete the selected word".into());
                    }
                    let mut input = vec![];
                    for unit in "native".encode_utf16() {
                        input.push(Event::Down(Held::Unicode(unit)));
                        input.push(Event::Up(Held::Unicode(unit)));
                    }
                    input.extend(tap(0x0d));
                    self.native_group(shell, self.proofs.store, None, input)?;
                }
                16 => {
                    if self.proofs.query != "native"
                        || self.proofs.results.value().is_none_or(|v| v.len() != 100)
                    {
                        return Err("re-entered query/async catalog not observed".into());
                    }
                    let p = self.proofs.ui.global_rect(self.proofs.catalog)?.center();
                    self.native_group(shell, self.proofs.store, Some(p), vec![])?;
                }
                17 => {
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
                18 => {
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
                19 => {
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
                20 => {
                    let mut input = chord(0x41);
                    input.extend(tap(0x58));
                    self.native_group(shell, self.proofs.detached, None, input)?;
                }
                21 => {
                    if self.proofs.document.source != "x" {
                        return Err("IDE physical selection/edit missing".into());
                    }
                    self.native_group(shell, self.proofs.detached, None, chord(0x5a))?;
                }
                22 => {
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
                23 => {
                    self.native_group(shell, self.proofs.detached, None, tap(0x09))?;
                }
                24 => {
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
                25 => {
                    if self.proofs.ui.focused(self.proofs.detached)? != test.focus {
                        return Err("Escape did not restore pre-modal focus".into());
                    }
                    self.proofs
                        .ui
                        .unmount(test.modal.take().ok_or("modal missing")?)?;
                    let source = self
                        .proofs
                        .editing_node(self.proofs.detached)
                        .ok_or("source")?;
                    let lines = ["abcdefghij"; 40].join("\n");
                    self.proofs.ui.update(source, Dirty::PAINT, |field: &mut controls::TextField| -> std::result::Result<(), text::TextError> {
                        field.external(field.snapshot().text.revision(), &lines, text::ExternalPolicy::ResetSelection)?;
                        let revision = field.snapshot().text.revision();
                        field.editor_mut().select(revision, text::Selection::caret(text::TextIndex(6)))?;
                        Ok(())
                    })??;
                    // Fixed public fixture geometry: default 22px line height,
                    // 6px padding, hence exactly three visible hard lines.
                    self.proofs
                        .ui
                        .set_layout(source, LogicalRect::from_xywh(20., 70., 350., 78.))?;
                    self.proofs.edited(self.proofs.detached)?;
                    self.proofs.ui.flush(&mut self.proofs.document)?;
                    self.draw(shell, self.proofs.detached)?;
                    test.log.push("setup: forty 10-byte hard lines, caret byte 6, three-line native text viewport; following Page keys are physical".into());
                    self.native_group(shell, self.proofs.detached, None, tap(0x22))?;
                }
                26 => {
                    self.assert_native_page(39, 39, 22.)?;
                    self.native_group(
                        shell,
                        self.proofs.detached,
                        None,
                        vec![
                            Event::Down(Held::Key(0x10)),
                            Event::Down(Held::Key(0x22)),
                            Event::Up(Held::Key(0x22)),
                            Event::Up(Held::Key(0x10)),
                        ],
                    )?;
                }
                27 => {
                    self.assert_native_page(39, 72, 88.)?;
                    self.native_group(shell, self.proofs.detached, None, tap(0x21))?;
                }
                28 => {
                    self.assert_native_page(39, 39, 66.)?;
                    self.native_group(
                        shell,
                        self.proofs.detached,
                        None,
                        vec![
                            Event::Down(Held::Key(0x10)),
                            Event::Down(Held::Key(0x21)),
                            Event::Up(Held::Key(0x21)),
                            Event::Up(Held::Key(0x10)),
                        ],
                    )?;
                }
                29 => {
                    self.assert_native_page(39, 6, 0.)?;
                    test.log.push("PASS physical multiline PageDown/Shift+PageDown/PageUp/Shift+PageUp; exact caret/anchor bytes 39,72,39,6 and scroll offsets 22,88,66,0 observed".into());
                    self.windows
                        .retain(|window| window.id != self.proofs.detached);
                    shell.close_window(self.proofs.detached)?;
                    self.proofs.ui.close_window(self.proofs.detached)?;
                    test.log
                        .push("setup: closed the owned IDE target after modal cleanup".into());
                }
                30 => {
                    if shell.get_window(self.proofs.detached).is_some() {
                        return Err("closed native target remains registered".into());
                    }
                    match self.native_group(shell, self.proofs.detached, None, vec![]) {
                        Err(error) if error.to_string() == "native target closed" => {}
                        Err(error) => {
                            return Err(format!(
                                "closed native target failed for an unexpected reason: {error}"
                            )
                            .into());
                        }
                        Ok(_) => {
                            return Err("input helper accepted a closed native target".into());
                        }
                    }
                    test.log.push("PASS pointer selection/gizmo/undo, en-US arrows/Tab/reverse Tab/Escape/Home/End/Shift selection/Delete/Ctrl+Backspace, Unicode insertion, worker catalog, physical wheel virtualization, IDE edit/undo, modal Tab containment/Escape restoration, closed-target rejection".into());
                    shell
                        .get_window(self.proofs.store)
                        .ok_or("store")?
                        .shared_window()
                        .focus_window();
                }
                31 => {
                    self.native_group(
                        shell,
                        self.proofs.store,
                        Some(self.proofs.ui.global_rect(self.proofs.search)?.center()),
                        click(),
                    )?;
                }
                32 => {
                    let field = self
                        .proofs
                        .ui
                        .state::<controls::TextField>(self.proofs.search)?;
                    if !field.focused()
                        || self.proofs.ui.focused(self.proofs.store)? != Some(self.proofs.search)
                    {
                        return Err("store field was not focused before blur fixture".into());
                    }
                    // Controlled preedit setup exercises native blur delivery; it
                    // does not certify an installed IME's candidate/commit path.
                    self.proofs.ui.update(
                        self.proofs.search,
                        Dirty::PAINT,
                        |field: &mut controls::TextField| field.preedit("pending", Some((7, 7))),
                    )??;
                    test.held_session = Some(self.hold_test_shift(shell, self.proofs.store)?);
                    test.log.push("setup: controlled preedit plus runner-owned Shift; 3s watchdog and F12 active".into());
                }
                33 => {
                    if !view_platform::desktop_input::WindowsDriver::key_is_down(0x10)? {
                        return Err(
                            "runner-owned Shift down was not observed before focus loss".into()
                        );
                    }
                    if !shell
                        .get_window(self.proofs.store)
                        .ok_or("store")?
                        .shared_window()
                        .has_focus()
                    {
                        return Err(
                            "unexpected foreground loss before owned-window transition".into()
                        );
                    }
                    shell
                        .get_window(self.proofs.editor)
                        .ok_or("editor")?
                        .shared_window()
                        .focus_window();
                }
                34 => {
                    let outcome = test
                        .held_session
                        .take()
                        .ok_or("held-input session missing")?
                        .finish();
                    match outcome {
                        Err(error) if error.code == view_automation::Code::Cancelled => {}
                        Err(error) => {
                            return Err(
                                format!("unexpected foreground cancellation: {error}").into()
                            );
                        }
                        Ok(()) => {
                            return Err("foreground loss did not cancel the desktop worker".into());
                        }
                    }
                    if view_platform::desktop_input::WindowsDriver::key_is_down(0x10)? {
                        return Err(
                            "runner-owned Shift remained down after focus-loss cleanup".into()
                        );
                    }
                    let field = self
                        .proofs
                        .ui
                        .state::<controls::TextField>(self.proofs.search)?;
                    if field.focused()
                        || field.snapshot().composition.is_some()
                        || self.proofs.query != "native"
                    {
                        return Err("native blur failed to clear field focus/composition without committing preedit".into());
                    }
                    test.log.push("PASS app-owned foreground loss cancelled worker; OS Shift down/up observed; native blur cleared controlled preedit without committing it".into());
                    test.held_session = Some(self.hold_test_shift(shell, self.proofs.editor)?);
                }
                35 => {
                    if !view_platform::desktop_input::WindowsDriver::key_is_down(0x10)? {
                        return Err(
                            "runner-owned Shift down was not observed before owner drop".into()
                        );
                    }
                    // Interrupt before matching Up; Drop joins cleanup and only
                    // releases the held set owned by this session.
                    drop(
                        test.held_session
                            .take()
                            .ok_or("held-input session missing")?,
                    );
                }
                36 => {
                    if view_platform::desktop_input::WindowsDriver::key_is_down(0x10)? {
                        return Err(
                            "runner-owned Shift remained down after interrupted owner drop".into(),
                        );
                    }
                    test.log.push("PASS interrupted session owner drop; OS runner-owned Shift release observed".into());
                    shell
                        .get_window(self.proofs.editor)
                        .ok_or("editor")?
                        .shared_window()
                        .set_visible(false);
                }
                37 => {
                    let raw = shell
                        .get_window(self.proofs.editor)
                        .ok_or("editor")?
                        .shared_window();
                    if raw.is_visible() != Some(false) {
                        return Err("hidden-target setup was not observed".into());
                    }
                    match view_platform::desktop_input::WindowsDriver::select(raw, true) {
                        Err(error) if error.code == view_automation::Code::Cancelled => {}
                        Err(error) => {
                            return Err(
                                format!("unexpected hidden-target rejection: {error}").into()
                            );
                        }
                        Ok(_) => {
                            return Err("hidden native target accepted desktop ownership".into());
                        }
                    }
                    test.log.push("PASS inaccessible hidden app-owned target rejected before any input insertion; elevated/secure-desktop coverage remains unavailable".into());
                    test.passed = true;
                    for line in test.log.iter().filter(|line| line.starts_with("PASS")) {
                        println!("{line}");
                    }
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

#[cfg(test)]
mod focus_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn queued_edit_pointer_refresh_uses_content_origin_without_paint() -> Result<()> {
        use crate::presentation::prepare_text_pointer;
        let mut fonts = text::CosmicBackend::from_fonts(
            [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
            8,
        )?;
        let mut area = controls::TextArea::new("pointer", "old")?;
        area.0.set_focused(true, 0);
        let bounds = LogicalRect::from_xywh(20., 70., 350., 78.);
        prepare_text_pointer(&mut fonts, &mut area.0, bounds, ScaleFactor::ONE, 0)?;
        area.0.editor_mut().select_all()?;
        area.0.insert_text("replacement before paint", 1)?;
        let content = prepare_text_pointer(&mut fonts, &mut area.0, bounds, ScaleFactor::ONE, 2)?;
        let padding = controls::Style::default().padding;
        assert_eq!(
            content.origin,
            bounds.origin + LogicalPoint::new(padding, padding)
        );
        area.0.pointer(
            PointerEvent {
                phase: PointerPhase::Down,
                position: content.origin + LogicalPoint::new(0., 1.),
                button: Some(PointerButton::Primary),
                modifiers: Modifiers::default(),
                timestamp: 2,
            },
            content,
        )?;
        assert_eq!(
            area.0.snapshot().selection,
            text::Selection::caret(text::TextIndex(0))
        );
        Ok(())
    }

    #[test]
    fn queued_insert_then_geometric_navigation_refreshes_layout_without_paint() -> Result<()> {
        use crate::presentation::prepare_text_navigation;
        use text::EditorSession;
        let mut fonts = text::CosmicBackend::from_fonts(
            [
                include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec(),
                include_bytes!("../fixtures/fonts/NotoSansHebrew.ttf").to_vec(),
            ],
            8,
        )?;
        let bounds = LogicalRect::from_xywh(20., 70., 350., 78.);
        for (key, inserted, initial, expected) in [
            (NamedKey::ArrowDown, ["abcdefghij"; 40].join("\n"), 6, 17),
            (NamedKey::ArrowUp, ["abcdefghij"; 40].join("\n"), 17, 6),
            (NamedKey::PageDown, ["abcdefghij"; 40].join("\n"), 6, 39),
            (NamedKey::PageUp, ["abcdefghij"; 40].join("\n"), 39, 6),
            (NamedKey::ArrowLeft, "אבג".into(), 2, 4),
            (NamedKey::ArrowRight, "אבג".into(), 2, 0),
        ] {
            let mut area = controls::TextArea::new("queued navigation", "old")?;
            area.0.set_focused(true, 0);
            let event = KeyboardEvent {
                key: Key::Named(key),
                phase: KeyPhase::Down,
                modifiers: Modifiers::default(),
                timestamp: 1,
            };
            assert!(prepare_text_navigation(
                &mut fonts,
                &mut area.0,
                bounds,
                ScaleFactor::ONE,
                &event
            )?);
            area.0.editor_mut().select_all()?;
            area.0.insert_text(&inserted, 2)?;
            let revision = area.0.snapshot().text.revision();
            area.0
                .editor_mut()
                .select(revision, text::Selection::caret(text::TextIndex(initial)))?;
            // The old shaped revision is still installed; no paint/prepare has
            // happened since insertion. Exercise the same CPU helper as native input.
            assert!(prepare_text_navigation(
                &mut fonts,
                &mut area.0,
                bounds,
                ScaleFactor::ONE,
                &event
            )?);
            assert_eq!(
                area.0.key(&event, &mut MemoryClipboard::default())?,
                controls::TextResponse::Handled
            );
            assert_eq!(
                area.0.snapshot().selection,
                text::Selection::caret(text::TextIndex(expected)),
                "{key:?}"
            );
            assert_eq!(area.0.snapshot().text.text(), inserted);
        }
        Ok(())
    }

    #[test]
    fn navigation_preparation_preserves_shortcut_and_composition_policy() -> Result<()> {
        use crate::presentation::prepare_text_navigation;
        let mut fonts = text::CosmicBackend::from_fonts(
            [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
            8,
        )?;
        let bounds = LogicalRect::from_xywh(20., 70., 350., 78.);
        let mut area = controls::TextArea::new("navigation policies", "a\nb\nc")?;
        area.0.set_focused(true, 0);
        let original = area.0.snapshot().selection;
        for modifiers in [
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
            Modifiers {
                alt: true,
                ..Default::default()
            },
            Modifiers {
                meta: true,
                ..Default::default()
            },
        ] {
            let event = KeyboardEvent {
                key: Key::Named(NamedKey::PageDown),
                phase: KeyPhase::Down,
                modifiers,
                timestamp: 1,
            };
            assert!(!prepare_text_navigation(
                &mut fonts,
                &mut area.0,
                bounds,
                ScaleFactor::ONE,
                &event
            )?);
            assert_eq!(
                area.0.key(&event, &mut MemoryClipboard::default())?,
                controls::TextResponse::Ignored
            );
            assert_eq!(area.0.snapshot().selection, original);
        }
        area.0.preedit("pending", Some((7, 7)))?;
        let event = KeyboardEvent {
            key: Key::Named(NamedKey::ArrowDown),
            phase: KeyPhase::Down,
            modifiers: Modifiers::default(),
            timestamp: 2,
        };
        assert!(!prepare_text_navigation(
            &mut fonts,
            &mut area.0,
            bounds,
            ScaleFactor::ONE,
            &event
        )?);
        assert_eq!(
            area.0.key(&event, &mut MemoryClipboard::default())?,
            controls::TextResponse::Ignored
        );
        assert!(area.0.snapshot().composition.is_some());
        assert_eq!(area.0.snapshot().selection, original);
        Ok(())
    }

    #[test]
    fn native_page_fixture_has_independent_byte_and_scroll_expectations() -> Result<()> {
        use text::EditorSession;
        let mut area =
            controls::TextArea::new("native page fixture", &["abcdefghij"; 40].join("\n"))?;
        area.0.set_focused(true, 0);
        let revision = area.0.snapshot().text.revision();
        area.0
            .editor_mut()
            .select(revision, text::Selection::caret(text::TextIndex(6)))?;
        let mut fonts = text::CosmicBackend::from_fonts(
            [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
            8,
        )?;
        let style = controls::Style {
            font_family: "Noto Sans".into(),
            ..Default::default()
        };
        let bounds = LogicalRect::from_xywh(20., 70., 350., 78.);
        let mut clipboard = MemoryClipboard::default();
        for (key, shift, anchor, focus, scroll) in [
            (NamedKey::PageDown, false, 39, 39, 22.),
            (NamedKey::PageDown, true, 39, 72, 88.),
            (NamedKey::PageUp, false, 39, 39, 66.),
            (NamedKey::PageUp, true, 39, 6, 0.),
        ] {
            area.0
                .prepare(&mut fonts, &style, bounds, ScaleFactor::ONE, 0)?;
            area.0.key(
                &KeyboardEvent {
                    key: Key::Named(key),
                    phase: KeyPhase::Down,
                    modifiers: Modifiers {
                        shift,
                        ..Default::default()
                    },
                    timestamp: 1,
                },
                &mut clipboard,
            )?;
            assert_eq!(area.0.snapshot().selection.anchor, text::TextIndex(anchor));
            assert_eq!(area.0.snapshot().selection.focus, text::TextIndex(focus));
            assert_eq!(area.0.editor().scroll_offset().y, scroll);
        }
        Ok(())
    }

    #[test]
    fn native_blur_cancels_composition_and_rejects_late_edit_events_then_restores_focus()
    -> Result<()> {
        let mut proofs = Proofs::new()?;
        let id = proofs.detached;
        let source = proofs.editing_node(id).ok_or("source")?;
        proofs.ui.set_focus(id, Some(source))?;
        let mut focused = HashSet::new();
        set_native_focus(&mut proofs, &mut focused, id, true)?;
        let original = proofs.ui.state::<controls::TextField>(source)?.snapshot();
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| {
                field.preedit("pending", Some((7, 7)))
            })?
            .is_some()
        );
        assert!(
            proofs
                .ui
                .state::<controls::TextField>(source)?
                .snapshot()
                .composition
                .is_some()
        );
        set_native_focus(&mut proofs, &mut focused, id, false)?;
        let mut clipboard = MemoryClipboard::default();
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| {
                field.set_focused(true, 1);
                field.key(
                    &KeyboardEvent {
                        key: Key::Named(NamedKey::ArrowLeft),
                        phase: KeyPhase::Up,
                        modifiers: Modifiers::default(),
                        timestamp: 1,
                    },
                    &mut clipboard,
                )
            })?
            .is_none()
        );
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| field
                .preedit("late", Some((4, 4))))?
            .is_none()
        );
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| field
                .commit_ime("late commit", 2))?
            .is_none()
        );
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| field
                .insert_text("late text", 3))?
            .is_none()
        );
        let field = proofs.ui.state::<controls::TextField>(source)?;
        assert!(!field.focused());
        assert!(field.snapshot().composition.is_none());
        assert_eq!(field.snapshot().text, original.text);
        assert_eq!(field.snapshot().selection, original.selection);
        // Logical focus remains available for a later native activation.
        assert_eq!(proofs.ui.focused(id)?, Some(source));
        set_native_focus(&mut proofs, &mut focused, id, true)?;
        assert!(proofs.ui.state::<controls::TextField>(source)?.focused());
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| field
                .insert_text("accepted", 4))?
            .is_some()
        );
        assert_ne!(
            proofs
                .ui
                .state::<controls::TextField>(source)?
                .snapshot()
                .text,
            original.text
        );
        Ok(())
    }

    #[test]
    fn native_activation_does_not_focus_an_editor_when_a_different_control_has_logical_focus()
    -> Result<()> {
        let mut proofs = Proofs::new()?;
        let id = proofs.store;
        proofs.ui.set_focus(id, None)?;
        let mut focused = HashSet::new();
        set_native_focus(&mut proofs, &mut focused, id, true)?;
        assert!(
            !proofs
                .ui
                .state::<controls::TextField>(proofs.search)?
                .focused()
        );
        assert!(
            edit_active_field(&mut proofs, &focused, id, |field| field
                .commit_ime("late", 0))?
            .is_none()
        );
        Ok(())
    }
}
