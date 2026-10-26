#![forbid(unsafe_code)]

use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view::controls::accessibility::{ControlAction, Entry, SemanticBridge};
use view::{
    controls::{Button, ControlSemantics, Label, Rgba, Style, TextField, frame},
    render::{
        Acquire, Budgets, Color, Draw, GpuContext, Paint, PaintSnapshot, Primitive, Renderer,
        WindowSurface, wgpu,
    },
    text::{Clipboard, EditorConfig, ExternalPolicy, LineMode, TabPolicy},
    *,
};
use view_platform::{
    AppConfig, CursorIcon, CursorService, ImeService, PlatformApp, PlatformError, PlatformHandler,
    PlatformShell, PlatformWindow, PlatformWindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const BUTTONS: [&str; 4] = ["New", "Open", "Save", "Save As"];

fn rect(x: f32, y: f32, width: f32, height: f32) -> LogicalRect {
    LogicalRect::new(
        LogicalPoint::new(x, y),
        LogicalSize::new(width.max(1.0), height.max(1.0)),
    )
}

fn fill(bounds: LogicalRect, color: Rgba) -> Paint {
    Paint::Draw(Draw::new(Primitive::Rect(bounds), Color(color.0)))
}

struct WakeThread(std::thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn wait<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park_timeout(Duration::from_millis(10)),
        }
    }
}

#[cfg(not(windows))]
#[derive(Default)]
struct MemoryClipboard(String);
#[cfg(not(windows))]
impl Clipboard for MemoryClipboard {
    fn read(&mut self) -> std::result::Result<String, view::text::TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, value: &str) -> std::result::Result<(), view::text::TextError> {
        self.0 = value.to_owned();
        Ok(())
    }
}

#[derive(Clone)]
enum PendingAction {
    New,
    Open(PathBuf),
    Close,
}

struct DocumentUi {
    runtime: Runtime<(), ()>,
    window: WindowId,
    root: NodeId,
    editor: NodeId,
    buttons: Vec<NodeId>,
    status: NodeId,
    fonts: view::text::CosmicBackend,
    bridge: SemanticBridge,
    semantic_revision: Revision,
    paint_revision: Revision,
    candidate: Option<LogicalRect>,
    native_active: bool,
    path: Option<PathBuf>,
    saved_text: String,
    pending: Option<PendingAction>,
    message: Option<String>,
    document_style: Style,
    toolbar_style: Style,
    status_style: Style,
}

impl DocumentUi {
    fn new() -> Result<Self> {
        Self::with_fonts(view::text::CosmicBackend::system(256))
    }

    fn with_fonts(fonts: view::text::CosmicBackend) -> Result<Self> {
        let mut runtime = Runtime::new(64)?;
        let (window, root) = runtime.open_window(
            Description::new("view-pad", (), |_, _, _| Dirty::NONE)
                .role(Role::Window)
                .layout(rect(0.0, 0.0, 900.0, 640.0)),
        )?;
        let editor = TextField::new(
            "Document",
            "",
            EditorConfig {
                mode: LineMode::Multiline,
                tab: TabPolicy::Insert,
                placeholder: "Start typing…".into(),
                ..EditorConfig::default()
            },
        )?;
        let editor = runtime.mount(
            root,
            Description::new("document", editor, |_, _, _| Dirty::PAINT)
                .role(Role::TextInput)
                .focusable(true)
                .structure(Structure::Declarative)
                .layout(rect(16.0, 64.0, 868.0, 530.0)),
        )?;
        let mut buttons = Vec::new();
        for (index, label) in BUTTONS.iter().enumerate() {
            buttons.push(
                runtime.mount(
                    root,
                    Description::new(
                        format!("toolbar-{label}"),
                        Button::new(*label),
                        |_, _, _| Dirty::PAINT.union(Dirty::SEMANTICS),
                    )
                    .role(Role::Button)
                    .focusable(true)
                    .structure(Structure::Declarative)
                    .layout(rect(16.0 + index as f32 * 92.0, 12.0, 84.0, 36.0)),
                )?,
            );
        }
        let status = runtime.mount(
            root,
            Description::new("status", Label::new("Ln 1, Col 1"), |_, _, _| Dirty::PAINT)
                .role(Role::StaticText)
                .structure(Structure::Declarative)
                .layout(rect(16.0, 606.0, 868.0, 22.0)),
        )?;
        runtime.set_focus(window, Some(editor))?;
        runtime.update(editor, Dirty::PAINT, |field: &mut TextField| {
            field.set_focused(true, 0)
        })?;

        let common = Style {
            foreground: Rgba([0.12, 0.15, 0.2, 1.0]),
            background: Rgba([0.97, 0.98, 0.99, 1.0]),
            border: Rgba([0.82, 0.85, 0.89, 1.0]),
            accent: Rgba([0.1, 0.38, 0.78, 1.0]),
            selection: Rgba([0.22, 0.48, 0.83, 0.42]),
            padding: 10.0,
            border_width: 1.0,
            font_size: 16.0,
            line_height: 24.0,
            font_family: "sans-serif".into(),
        };
        let document_style = Style {
            background: Rgba([1.0, 1.0, 1.0, 1.0]),
            padding: 14.0,
            font_size: 16.0,
            line_height: 24.0,
            ..common.clone()
        };
        let toolbar_style = Style {
            background: Rgba([0.94, 0.96, 0.98, 1.0]),
            padding: 8.0,
            ..common.clone()
        };
        let status_style = Style {
            foreground: Rgba([0.36, 0.4, 0.46, 1.0]),
            background: Rgba([0.94, 0.96, 0.98, 1.0]),
            padding: 2.0,
            font_size: 13.0,
            line_height: 18.0,
            ..common
        };

        Ok(Self {
            runtime,
            window,
            root,
            editor,
            buttons,
            status,
            fonts,
            bridge: SemanticBridge::default(),
            semantic_revision: Revision::INITIAL,
            paint_revision: Revision::INITIAL,
            candidate: None,
            native_active: true,
            path: None,
            saved_text: String::new(),
            pending: None,
            message: None,
            document_style,
            toolbar_style,
            status_style,
        })
    }

    fn text(&self) -> Result<String> {
        Ok(self
            .runtime
            .state::<TextField>(self.editor)?
            .snapshot()
            .text
            .text()
            .to_owned())
    }

    fn is_dirty(&self) -> Result<bool> {
        Ok(self.text()? != self.saved_text)
    }

    fn title(&self) -> Result<String> {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled");
        Ok(format!(
            "{}{} — View Pad",
            if self.is_dirty()? { "• " } else { "" },
            name
        ))
    }

    fn set_focus(&mut self, target: Option<NodeId>, now: u64) -> Result<()> {
        self.runtime.set_focus(self.window, target)?;
        self.sync_control_focus(now)
    }

    fn sync_control_focus(&mut self, now: u64) -> Result<()> {
        let target = if self.native_active && self.pending.is_none() {
            self.runtime.focused(self.window)?
        } else {
            None
        };
        let editor = self.editor;
        self.runtime
            .update(editor, Dirty::PAINT, |field: &mut TextField| {
                field.set_focused(target == Some(editor), now);
            })?;
        for id in self.buttons.clone() {
            self.runtime
                .update(id, Dirty::PAINT, |button: &mut Button| {
                    button.behavior.set_focused(target == Some(id))
                })?;
        }
        Ok(())
    }

    fn set_native_active(&mut self, active: bool, now: u64) -> Result<()> {
        self.native_active = active;
        if !active {
            self.runtime.cancel_pointer(self.window)?;
            self.candidate = None;
        }
        self.sync_control_focus(now)
    }

    fn editor_accepts_input(&self) -> Result<bool> {
        Ok(self.native_active
            && self.pending.is_none()
            && self.runtime.focused(self.window)? == Some(self.editor)
            && self.runtime.state::<TextField>(self.editor)?.focused())
    }

    fn editor_key(&mut self, event: &KeyboardEvent, clipboard: &mut impl Clipboard) -> Result<()> {
        if self.editor_accepts_input()? {
            self.runtime.update(
                self.editor,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |field: &mut TextField| field.key(event, clipboard),
            )??;
        }
        Ok(())
    }

    fn native_text(&mut self, text: &str, now: u64) -> Result<()> {
        if self.editor_accepts_input()? {
            self.runtime.update(
                self.editor,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |field: &mut TextField| field.insert_text(text, now),
            )??;
        }
        Ok(())
    }

    fn native_ime(&mut self, event: &view_platform::ImeEvent, now: u64) -> Result<()> {
        if self.editor_accepts_input()? {
            self.runtime.update(
                self.editor,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |field: &mut TextField| {
                    view_platform::text::dispatch_ime(field.editor_mut(), event, now)
                },
            )??;
        }
        Ok(())
    }

    fn next_caret_blink(&self, now: u64) -> Result<Option<u64>> {
        if !self.editor_accepts_input()? {
            return Ok(None);
        }
        Ok(self
            .runtime
            .state::<TextField>(self.editor)?
            .editor()
            .next_caret_blink(now))
    }

    fn layout(&mut self, size: LogicalSize) -> Result<()> {
        self.runtime.set_window_size(self.window, size)?;
        self.runtime
            .set_layout(self.root, rect(0.0, 0.0, size.width, size.height))?;
        for (index, id) in self.buttons.iter().copied().enumerate() {
            let x = 16.0 + index as f32 * 92.0;
            self.runtime.set_layout(id, rect(x, 12.0, 84.0, 36.0))?;
        }
        self.runtime.set_layout(
            self.editor,
            rect(
                16.0,
                60.0,
                (size.width - 32.0).max(40.0),
                (size.height - 104.0).max(70.0),
            ),
        )?;
        self.runtime.set_layout(
            self.status,
            rect(
                16.0,
                (size.height - 32.0).max(0.0),
                (size.width - 32.0).max(40.0),
                22.0,
            ),
        )?;
        self.runtime.flush(&mut ())?;
        Ok(())
    }

    fn status_text(&self) -> Result<String> {
        let snapshot = self.runtime.state::<TextField>(self.editor)?.snapshot();
        let value = snapshot.text.text();
        let caret = snapshot.selection.focus.0.min(value.len());
        let before = &value[..caret];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count()
            + 1;
        let mut status = format!(
            "Ln {line}, Col {column}     {} characters     UTF-8     LF",
            value.chars().count()
        );
        if self.is_dirty()? {
            status.push_str("     Unsaved changes");
        }
        if let Some(message) = &self.message {
            status.push_str("     •     ");
            status.push_str(message);
        }
        Ok(status)
    }

    /// Refresh CPU geometry for input that can arrive before the next paint.
    fn prepare_editor_input(&mut self, scale: ScaleFactor, now: u64) -> Result<LogicalRect> {
        let bounds = self.runtime.global_rect(self.editor)?;
        let fonts = &mut self.fonts;
        Ok(self
            .runtime
            .update(self.editor, Dirty::NONE, |field: &mut TextField| {
                field.prepare(fonts, &self.document_style, bounds, scale, now)
            })??
            .clip)
    }

    fn prepare(
        &mut self,
        physical_size: [u32; 2],
        scale: ScaleFactor,
        now: u64,
    ) -> Result<(PaintSnapshot, accesskit::TreeUpdate)> {
        let logical = LogicalSize::new(
            physical_size[0] as f32 / scale.get(),
            physical_size[1] as f32 / scale.get(),
        );
        self.layout(logical)?;
        self.sync_control_focus(now)?;
        self.paint_revision = self.paint_revision.checked_next()?;
        self.semantic_revision = self.semantic_revision.checked_next()?;
        self.candidate = None;

        let mut items = vec![fill(
            rect(0.0, 0.0, logical.width, logical.height),
            Rgba([0.97, 0.98, 0.99, 1.0]),
        )];
        items.push(fill(
            rect(0.0, 0.0, logical.width, 56.0),
            self.toolbar_style.background,
        ));

        let mut root_node = SemanticNode::new(self.root, Role::Window);
        root_node.name = Some("View Pad document editor".into());
        root_node.bounds = rect(0.0, 0.0, logical.width, logical.height);
        let mut entries = vec![Entry {
            parent: None,
            state: ControlSemantics::new(root_node),
        }];
        let focused = if self.native_active && self.pending.is_none() {
            self.runtime.focused(self.window)?
        } else {
            None
        };

        for (index, id) in self.buttons.clone().into_iter().enumerate() {
            let bounds = self.runtime.global_rect(id)?;
            let button = self.runtime.state::<Button>(id)?;
            let state = button.behavior.state();
            let mut semantic = button.semantics(id, bounds);
            semantic.node.focused = focused == Some(id);
            entries.push(Entry {
                parent: Some(self.root),
                state: semantic,
            });
            let style = self.toolbar_style.interaction(state);
            items.extend(frame(bounds, &style));
            items.push(Label::new(BUTTONS[index]).paint(
                &mut self.fonts,
                &self.toolbar_style,
                rect(
                    bounds.left() + 10.0,
                    bounds.top() + 6.0,
                    bounds.size.width - 16.0,
                    24.0,
                ),
                scale,
            )?);
        }

        let editor_bounds = self.runtime.global_rect(self.editor)?;
        let fonts = &mut self.fonts;
        let style = &self.document_style;
        let editor_id = self.editor;
        let (presentation, value) = self.runtime.update(
            editor_id,
            Dirty::NONE,
            |field: &mut TextField| -> Result<_> {
                let presentation = field.prepare(fonts, style, editor_bounds, scale, now)?;
                Ok((presentation, field.snapshot().text.text().to_owned()))
            },
        )??;
        let field = self.runtime.state::<TextField>(self.editor)?;
        let mut semantic = field.semantics(self.editor, editor_bounds);
        semantic.node.focused = focused == Some(self.editor);
        semantic.node.value = Some(value);
        entries.push(Entry {
            parent: Some(self.root),
            state: semantic,
        });
        items.extend(frame(editor_bounds, &self.document_style));
        items.extend(presentation.paint(&self.document_style)?);
        self.candidate = (focused == Some(self.editor)).then_some(presentation.candidate);

        let status_bounds = self.runtime.global_rect(self.status)?;
        let status_text = self.status_text()?;
        if self.runtime.state::<Label>(self.status)?.text != status_text {
            self.runtime
                .update(self.status, Dirty::PAINT, |label: &mut Label| {
                    label.text = status_text
                })?;
        }
        let label = self.runtime.state::<Label>(self.status)?;
        entries.push(Entry {
            parent: Some(self.root),
            state: label.semantics(self.status, status_bounds),
        });
        items.push(fill(
            rect(0.0, (logical.height - 38.0).max(0.0), logical.width, 38.0),
            self.status_style.background,
        ));
        items.push(label.paint(&mut self.fonts, &self.status_style, status_bounds, scale)?);

        if self.pending.is_some() {
            let (panel, buttons) = prompt_layout(logical);
            let panel_style = Style {
                foreground: Rgba([0.12, 0.15, 0.2, 1.0]),
                background: Rgba([1.0, 1.0, 1.0, 1.0]),
                border: Rgba([0.76, 0.79, 0.84, 1.0]),
                accent: Rgba([0.1, 0.38, 0.78, 1.0]),
                selection: self.document_style.selection,
                padding: 16.0,
                border_width: 1.0,
                font_size: 15.0,
                line_height: 22.0,
                font_family: "sans-serif".into(),
            };
            items.extend(frame(panel, &panel_style));
            items.push(
                Label::new(format!(
                    "Save changes to {} before continuing?",
                    self.path
                        .as_deref()
                        .and_then(Path::file_name)
                        .and_then(|name| name.to_str())
                        .unwrap_or("Untitled")
                ))
                .paint(
                    &mut self.fonts,
                    &panel_style,
                    rect(
                        panel.left() + 18.0,
                        panel.top() + 18.0,
                        panel.size.width - 36.0,
                        32.0,
                    ),
                    scale,
                )?,
            );
            for (index, (bounds, label)) in buttons
                .into_iter()
                .zip(["Save", "Discard", "Cancel"])
                .enumerate()
            {
                let button_style = if index == 0 {
                    Style {
                        accent: Rgba([0.1, 0.38, 0.78, 1.0]),
                        ..panel_style.clone()
                    }
                } else {
                    panel_style.clone()
                };
                items.extend(frame(bounds, &button_style));
                items.push(Label::new(label).paint(
                    &mut self.fonts,
                    &panel_style,
                    rect(
                        bounds.left() + 12.0,
                        bounds.top() + 7.0,
                        bounds.size.width - 18.0,
                        24.0,
                    ),
                    scale,
                )?);
            }
        }

        let tree = self
            .bridge
            .commit(self.semantic_revision, &entries, scale)?;
        Ok((
            PaintSnapshot {
                revision: self.paint_revision,
                size: physical_size,
                scale,
                items,
            },
            tree,
        ))
    }

    fn replace_document(&mut self, text: &str, path: Option<PathBuf>, now: u64) -> Result<()> {
        let revision = self
            .runtime
            .state::<TextField>(self.editor)?
            .snapshot()
            .text
            .revision();
        self.runtime.update(
            self.editor,
            Dirty::PAINT.union(Dirty::SEMANTICS),
            |field: &mut TextField| field.external(revision, text, ExternalPolicy::ResetSelection),
        )??;
        self.saved_text = self
            .runtime
            .state::<TextField>(self.editor)?
            .snapshot()
            .text
            .text()
            .to_owned();
        self.path = path;
        self.pending = None;
        self.message = Some("Ready".into());
        self.set_focus(Some(self.editor), now)?;
        Ok(())
    }

    fn load_path(&mut self, path: PathBuf, now: u64) -> Result<()> {
        let mut text = std::fs::read_to_string(&path)?;
        if let Some(without_bom) = text.strip_prefix('\u{feff}') {
            text = without_bom.to_owned();
        }
        self.replace_document(&text, Some(path), now)
    }
}

fn prompt_layout(size: LogicalSize) -> (LogicalRect, [LogicalRect; 3]) {
    let width = 420.0_f32.min((size.width - 32.0).max(260.0));
    let height = 132.0;
    let left = ((size.width - width) * 0.5).max(8.0);
    let top = ((size.height - height) * 0.5).max(8.0);
    let panel = rect(left, top, width, height);
    let button_width = 104.0;
    let gap = 8.0;
    let row_width = button_width * 3.0 + gap * 2.0;
    let start = panel.right() - row_width - 16.0;
    let y = panel.bottom() - 48.0;
    (
        panel,
        [
            rect(start, y, button_width, 32.0),
            rect(start + button_width + gap, y, button_width, 32.0),
            rect(start + (button_width + gap) * 2.0, y, button_width, 32.0),
        ],
    )
}

struct GpuState {
    context: GpuContext,
    surface: WindowSurface<'static>,
    renderer: Renderer,
    _instance: wgpu::Instance,
}

fn blink_control_flow(
    now: Instant,
    next: &mut Instant,
    deadline: Option<Instant>,
) -> (ControlFlow, bool) {
    let Some(deadline) = deadline else {
        return (ControlFlow::Wait, false);
    };
    let redraw = now >= *next;
    *next = deadline;
    (ControlFlow::WaitUntil(*next), redraw)
}

struct App {
    ui: DocumentUi,
    gpu: Option<GpuState>,
    started: Instant,
    next_blink: Instant,
    #[cfg(not(windows))]
    clipboard: MemoryClipboard,
}

impl App {
    fn new() -> Result<Self> {
        Ok(Self {
            ui: DocumentUi::new()?,
            gpu: None,
            started: Instant::now(),
            next_blink: Instant::now() + Duration::from_millis(500),
            #[cfg(not(windows))]
            clipboard: MemoryClipboard::default(),
        })
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn draw(&mut self, shell: &mut PlatformShell) -> Result<()> {
        let Some(window) = shell.get_window(self.ui.window) else {
            return Ok(());
        };
        let raw = window.shared_window();
        let physical = raw.inner_size();
        if physical.width == 0 || physical.height == 0 {
            return Ok(());
        }
        let scale = window.dpi().scale_factor;
        let now = self.now_ms();
        let (snapshot, tree) = self
            .ui
            .prepare([physical.width, physical.height], scale, now)?;
        if let Some(candidate) = self.ui.candidate {
            ImeService::set_ime_allowed(&raw, true)?;
            ImeService::set_ime_cursor_area(&raw, candidate, scale)?;
        } else {
            ImeService::set_ime_allowed(&raw, false)?;
        }
        shell
            .accessibility_mut(self.ui.window)
            .ok_or("missing View Pad accessibility adapter")?
            .publish(self.ui.semantic_revision, tree)?;

        let gpu = self.gpu.as_mut().ok_or("missing renderer")?;
        gpu.context.device().poll(wgpu::PollType::Poll)?;
        let prepared = gpu.renderer.prepare(&snapshot)?;
        gpu.surface.resize(snapshot.size);
        match gpu.surface.acquire(&gpu.context)? {
            Acquire::Ready(frame) => {
                raw.pre_present_notify();
                frame.present(&gpu.context, prepared, Color([0.97, 0.98, 0.99, 1.0]))?;
            }
            Acquire::Retry | Acquire::Reconfigure => raw.request_redraw(),
            Acquire::Suspended => {}
        }
        raw.set_title(&self.ui.title()?);
        Ok(())
    }

    fn request_new(&mut self, now: u64) -> Result<()> {
        if self.ui.is_dirty()? {
            self.ui.pending = Some(PendingAction::New);
        } else {
            self.ui.replace_document("", None, now)?;
        }
        Ok(())
    }

    fn request_open(&mut self, shell: &PlatformShell, now: u64) -> Result<()> {
        let window = shell
            .get_window(self.ui.window)
            .ok_or("missing owner window")?
            .shared_window();
        match view_platform::desktop::file_dialog(
            &window,
            view_platform::desktop::DialogKind::Open,
            "Open text file",
        ) {
            Ok(Some(path)) if self.ui.is_dirty()? => {
                self.ui.pending = Some(PendingAction::Open(path));
                self.ui.message = Some("Choose Save, Discard, or Cancel".into());
            }
            Ok(Some(path)) => {
                let result = self.ui.load_path(path, now);
                self.show_io_result(result, "Opened");
            }
            Ok(None) => {}
            Err(error) => self.ui.message = Some(error.to_string()),
        }
        Ok(())
    }

    fn save_as(&mut self, shell: &PlatformShell) -> Result<bool> {
        let window = shell
            .get_window(self.ui.window)
            .ok_or("missing owner window")?
            .shared_window();
        let chosen = view_platform::desktop::file_dialog(
            &window,
            view_platform::desktop::DialogKind::Save,
            "Save text file",
        )?;
        let Some(path) = chosen else {
            return Ok(false);
        };
        self.write_path(path)?;
        Ok(true)
    }

    fn save(&mut self, shell: &PlatformShell) -> Result<bool> {
        if let Some(path) = self.ui.path.clone() {
            self.write_path(path)?;
            Ok(true)
        } else {
            self.save_as(shell)
        }
    }

    fn write_path(&mut self, path: PathBuf) -> Result<()> {
        let text = self.ui.text()?;
        std::fs::write(&path, text.as_bytes())?;
        self.ui.saved_text = text;
        self.ui.path = Some(path);
        self.ui.message = Some("Saved".into());
        Ok(())
    }

    fn show_io_result(&mut self, result: Result<()>, success: &str) {
        self.ui.message = Some(match result {
            Ok(()) => success.to_owned(),
            Err(error) => format!("Could not complete file operation: {error}"),
        });
    }

    fn save_command(&mut self, shell: &PlatformShell, save_as: bool) {
        let result = if save_as {
            self.save_as(shell)
        } else {
            self.save(shell)
        };
        self.ui.message = match result {
            Ok(true) => Some("Saved".into()),
            Ok(false) => None,
            Err(error) => Some(format!("Could not save file: {error}")),
        };
    }

    fn apply_pending(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        discard: bool,
    ) -> Result<()> {
        let Some(action) = self.ui.pending.take() else {
            return Ok(());
        };
        let now = self.now_ms();
        match action {
            PendingAction::New => self.ui.replace_document("", None, now)?,
            PendingAction::Open(path) => {
                let result = self.ui.load_path(path, now);
                self.show_io_result(result, "Opened");
            }
            PendingAction::Close if discard => {
                shell.close_window(self.ui.window)?;
                self.ui.runtime.close_window(self.ui.window)?;
                self.gpu = None;
                event_loop.exit();
            }
            PendingAction::Close => {}
        }
        Ok(())
    }

    fn confirm_save(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        match self.save(shell) {
            Ok(true) => self.apply_pending(shell, event_loop, true)?,
            Ok(false) => {}
            Err(error) => {
                self.ui.message = Some(format!("Could not save document: {error}"));
            }
        }
        Ok(())
    }

    fn close_request(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        if self.ui.is_dirty()? {
            self.ui.pending = Some(PendingAction::Close);
            self.ui.message = Some("Choose Save, Discard, or Cancel".into());
            let now = self.now_ms();
            self.ui.set_focus(None, now)?;
        } else {
            shell.close_window(self.ui.window)?;
            self.ui.runtime.close_window(self.ui.window)?;
            self.gpu = None;
            event_loop.exit();
        }
        Ok(())
    }

    fn activate_button(
        &mut self,
        index: usize,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        let now = self.now_ms();
        match index {
            0 => self.request_new(now),
            1 => self.request_open(shell, now),
            2 => {
                self.save_command(shell, false);
                Ok(())
            }
            3 => {
                self.save_command(shell, true);
                Ok(())
            }
            4 => self.confirm_save(shell, event_loop),
            5 => self.apply_pending(shell, event_loop, true),
            6 => {
                self.ui.pending = None;
                self.ui.message = None;
                self.ui.set_focus(Some(self.ui.editor), now)
            }
            _ => Ok(()),
        }
    }

    fn handle_pointer(
        &mut self,
        event: PointerEvent,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        let size = shell
            .get_window(self.ui.window)
            .ok_or("missing window")?
            .logical_size();
        self.ui.layout(size)?;
        let window = shell
            .get_window(self.ui.window)
            .ok_or("missing window")?
            .shared_window();
        let on_button = self.ui.buttons.iter().try_fold(false, |hit, id| {
            Ok::<_, Box<dyn std::error::Error>>(
                hit || self.ui.runtime.global_rect(*id)?.contains(event.position),
            )
        })?;
        let cursor = if self.ui.pending.is_some() {
            CursorIcon::Default
        } else if self
            .ui
            .runtime
            .global_rect(self.ui.editor)?
            .contains(event.position)
        {
            CursorIcon::Text
        } else if on_button {
            CursorIcon::Pointer
        } else {
            CursorIcon::Default
        };
        CursorService::set_cursor(&window, cursor)?;
        // Native activation must precede presses. Hover, cancellation and
        // inactive-window wheel scrolling still follow their normal routes.
        if !self.ui.native_active && matches!(event.phase, PointerPhase::Down | PointerPhase::Up) {
            return Ok(());
        }
        if self.ui.pending.is_some() {
            if event.phase == PointerPhase::Up {
                let (_, buttons) = prompt_layout(size);
                for (index, bounds) in buttons.into_iter().enumerate() {
                    if bounds.contains(event.position) {
                        self.activate_button(index + 4, shell, event_loop)?;
                        break;
                    }
                }
            }
            return Ok(());
        }

        let routed = self
            .ui
            .runtime
            .route_pointer(self.ui.window, event, &mut ())?;
        if event.phase == PointerPhase::Down && event.button == Some(PointerButton::Primary) {
            let target = routed.iter().find_map(|action| match action {
                RoutedPointerAction::Down { target, .. } => Some(*target),
                _ => None,
            });
            if let Some(target) = target
                && (self.ui.buttons.contains(&target) || target == self.ui.editor)
            {
                self.ui.set_focus(Some(target), event.timestamp)?;
            }
        }

        let mut activated = None;
        for (index, id) in self.ui.buttons.clone().into_iter().enumerate() {
            let bounds = self.ui.runtime.global_rect(id)?;
            let is_activated =
                self.ui
                    .runtime
                    .update(id, Dirty::PAINT, |button: &mut Button| {
                        button.pointer(event, bounds.contains(event.position))
                    })?;
            if is_activated {
                activated = Some(index);
            }
        }

        let content = self.prepare_editor_for_navigation(shell, event.timestamp)?;
        if let PointerPhase::Scroll { delta_x, delta_y } = event.phase {
            let bounds = self.ui.runtime.global_rect(self.ui.editor)?;
            if bounds.contains(event.position) {
                let viewport = content.size;
                self.ui.runtime.update(
                    self.ui.editor,
                    Dirty::PAINT,
                    |field: &mut TextField| {
                        field.scroll_by(LogicalPoint::new(-delta_x, -delta_y), viewport)
                    },
                )??;
            }
        } else {
            self.ui
                .runtime
                .update(self.ui.editor, Dirty::PAINT, |field: &mut TextField| {
                    field.pointer(event, content)
                })??;
        }
        if let Some(index) = activated {
            self.activate_button(index, shell, event_loop)?;
        }
        Ok(())
    }

    fn handle_key(
        &mut self,
        event: KeyboardEvent,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        clipboard: &mut impl Clipboard,
    ) -> Result<()> {
        if !self.ui.native_active {
            return Ok(());
        }
        if self.ui.pending.is_some() {
            if event.phase == KeyPhase::Down && event.key == Key::Named(NamedKey::Escape) {
                self.ui.pending = None;
                self.ui.message = None;
                self.ui.set_focus(Some(self.ui.editor), event.timestamp)?;
            }
            return Ok(());
        }
        if event.phase == KeyPhase::Down
            && (event.modifiers.ctrl || event.modifiers.meta)
            && let Key::Character(character) = &event.key
        {
            match character.to_lowercase().as_str() {
                "n" => return self.request_new(event.timestamp),
                "o" => return self.request_open(shell, event.timestamp),
                "s" if event.modifiers.shift => {
                    self.save_command(shell, true);
                    return Ok(());
                }
                "s" => {
                    self.save_command(shell, false);
                    return Ok(());
                }
                _ => {}
            }
        }
        let focused = self.ui.runtime.focused(self.ui.window)?;
        if event.phase == KeyPhase::Down
            && event.key == Key::Named(NamedKey::Tab)
            && focused != Some(self.ui.editor)
        {
            let target = self
                .ui
                .runtime
                .route_keyboard(self.ui.window, event.clone(), &mut ())?;
            self.ui.set_focus(target, event.timestamp)?;
            return Ok(());
        }
        if let Some(target) = focused {
            if target == self.ui.editor {
                self.ui.editor_key(&event, clipboard)?;
            } else if let Some(index) = self.ui.buttons.iter().position(|id| *id == target) {
                let activated =
                    self.ui
                        .runtime
                        .update(target, Dirty::PAINT, |button: &mut Button| {
                            button.key(&event)
                        })?;
                if activated {
                    self.activate_button(index, shell, event_loop)?;
                }
            }
        }
        Ok(())
    }

    fn load_startup_file(&mut self, path: PathBuf) -> Result<()> {
        self.ui.load_path(path, self.now_ms())
    }

    fn prepare_editor_for_navigation(
        &mut self,
        shell: &PlatformShell,
        now: u64,
    ) -> Result<LogicalRect> {
        let window = shell.get_window(self.ui.window).ok_or("missing window")?;
        self.ui.layout(window.logical_size())?;
        let scale = window.dpi().scale_factor;
        self.ui.prepare_editor_input(scale, now)
    }

    fn drain_accessibility(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        let queued = shell
            .accessibility_mut(self.ui.window)
            .ok_or("missing accessibility adapter")?
            .drain()?;
        let changed = !queued.is_empty();
        let now = self.now_ms();
        for request in queued {
            let routed = match self.ui.bridge.route(request.revision, &request.request) {
                Ok(action) => action,
                Err(error) => {
                    self.ui.message = Some(format!("Ignored stale accessibility action: {error}"));
                    continue;
                }
            };
            match routed.action {
                ControlAction::Focus => self.ui.set_focus(Some(routed.owner), now)?,
                ControlAction::Click => {
                    if let Some(index) = self.ui.buttons.iter().position(|id| *id == routed.owner) {
                        self.activate_button(index, shell, event_loop)?;
                    } else if routed.owner == self.ui.editor {
                        self.ui.set_focus(Some(self.ui.editor), now)?;
                    }
                }
                ControlAction::SetValue(value) if routed.owner == self.ui.editor => {
                    let revision = self
                        .ui
                        .runtime
                        .state::<TextField>(self.ui.editor)?
                        .snapshot()
                        .text
                        .revision();
                    self.ui.runtime.update(
                        self.ui.editor,
                        Dirty::PAINT.union(Dirty::SEMANTICS),
                        |field: &mut TextField| {
                            field.external(revision, &value, ExternalPolicy::ResetSelection)
                        },
                    )??;
                }
                ControlAction::SetValue(_) => {}
                ControlAction::SetNumericValue(_)
                | ControlAction::SetSelection(_)
                | ControlAction::Expand
                | ControlAction::Collapse => {
                    self.ui.message =
                        Some("That accessibility action is not supported here".into());
                }
            }
        }
        if changed && let Some(window) = shell.get_window(self.ui.window) {
            window.request_redraw();
        }
        Ok(())
    }
}

impl PlatformHandler for App {
    fn on_ready(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> std::result::Result<(), PlatformError> {
        let mut initialize = || -> Result<()> {
            let raw = shell
                .create_window(
                    event_loop,
                    self.ui.window,
                    PlatformWindow::default_attributes("View Pad", LogicalSize::new(900.0, 640.0))
                        .with_visible(false),
                )?
                .shared_window();
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                    event_loop.owned_display_handle(),
                ))
            });
            let raw_surface = instance.create_surface(raw.clone())?;
            let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&raw_surface),
                ..Default::default()
            }))?;
            println!("View Pad adapter: {:?}", adapter.get_info());
            if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
                return Err("View Pad requires a hardware graphics adapter".into());
            }
            let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                // View Pad is a small text application. Keep hardware rendering,
                // while favoring smaller allocator blocks based on local DX12 data.
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            }))?;
            let context =
                GpuContext::from_device(device, queue, view::render::DeviceErrorPolicy::Capture)?;
            let size = raw.inner_size();
            let surface =
                WindowSurface::new(raw_surface, &adapter, &context, [size.width, size.height])?;
            let renderer = Renderer::new(context.clone(), surface.format(), Budgets::default())?;
            self.gpu = Some(GpuState {
                _instance: instance,
                context,
                surface,
                renderer,
            });

            let scale = shell
                .get_window(self.ui.window)
                .ok_or("missing window")?
                .dpi()
                .scale_factor;
            let now = self.now_ms();
            self.ui.set_native_active(raw.has_focus(), now)?;
            let (_, tree) = self
                .ui
                .prepare([size.width.max(1), size.height.max(1)], scale, now)?;
            shell.enable_accessibility(
                event_loop,
                self.ui.window,
                self.ui.semantic_revision,
                tree,
            )?;
            self.draw(shell)?;
            raw.set_visible(true);
            raw.request_redraw();
            Ok(())
        };
        initialize().map_err(|error| PlatformError::EventLoop(error.to_string()))
    }

    fn on_window_event(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: PlatformWindowEvent,
    ) -> std::result::Result<(), PlatformError> {
        if id != self.ui.window {
            return Ok(());
        }
        let result: Result<()> = (|| {
            let now = self.now_ms();
            if !self.ui.native_active
                && matches!(
                    &event,
                    PlatformWindowEvent::Keyboard(_)
                        | PlatformWindowEvent::TextInput(_)
                        | PlatformWindowEvent::Ime(_)
                )
            {
                return Ok(());
            }
            match event {
                PlatformWindowEvent::CloseRequested => {
                    self.close_request(shell, event_loop)?;
                    if let Some(window) = shell.get_window(self.ui.window) {
                        window.request_redraw();
                    }
                }
                PlatformWindowEvent::RedrawRequested => self.draw(shell)?,
                PlatformWindowEvent::Resized { logical_size, .. } => {
                    self.ui
                        .runtime
                        .set_window_size(self.ui.window, logical_size)?;
                    self.draw(shell)?;
                }
                PlatformWindowEvent::ScaleFactorChanged(_) => self.draw(shell)?,
                PlatformWindowEvent::Pointer(event) => {
                    self.handle_pointer(event, shell, event_loop)?;
                    if let Some(window) = shell.get_window(self.ui.window) {
                        window.request_redraw();
                    }
                }
                PlatformWindowEvent::Keyboard(event) => {
                    let window = shell
                        .get_window(self.ui.window)
                        .ok_or("missing window")?
                        .shared_window();
                    if event.phase != KeyPhase::Up
                        && matches!(
                            &event.key,
                            Key::Named(
                                NamedKey::ArrowLeft
                                    | NamedKey::ArrowRight
                                    | NamedKey::ArrowUp
                                    | NamedKey::ArrowDown
                                    | NamedKey::PageUp
                                    | NamedKey::PageDown
                            )
                        )
                    {
                        self.prepare_editor_for_navigation(shell, now)?;
                    }
                    #[cfg(windows)]
                    {
                        let mut clipboard = view_platform::text::NativeClipboard::new(&window)?;
                        self.handle_key(event, shell, event_loop, &mut clipboard)?;
                    }
                    #[cfg(not(windows))]
                    {
                        // `handle_key` mutably borrows the app as well as the
                        // clipboard field. Move the in-memory clipboard out
                        // for the call, then restore it even if handling fails.
                        let mut clipboard = std::mem::take(&mut self.clipboard);
                        let result = self.handle_key(event, shell, event_loop, &mut clipboard);
                        self.clipboard = clipboard;
                        result?;
                    }
                    window.request_redraw();
                }
                PlatformWindowEvent::TextInput(text) => {
                    self.ui.native_text(&text, now)?;
                    shell
                        .get_window(self.ui.window)
                        .ok_or("missing window")?
                        .request_redraw();
                }
                PlatformWindowEvent::Ime(event) => {
                    self.ui.native_ime(&event, now)?;
                    shell
                        .get_window(self.ui.window)
                        .ok_or("missing window")?
                        .request_redraw();
                }
                PlatformWindowEvent::Focused(active) => {
                    self.ui.set_native_active(active, now)?;
                    self.next_blink = Instant::now() + Duration::from_millis(500);
                    shell
                        .get_window(self.ui.window)
                        .ok_or("missing window")?
                        .request_redraw();
                }
                PlatformWindowEvent::FileDropped(path) => {
                    if self.ui.is_dirty()? {
                        self.ui.pending = Some(PendingAction::Open(path));
                        self.ui.message = Some("Choose Save, Discard, or Cancel".into());
                    } else {
                        let result = self.ui.load_path(path, now);
                        self.show_io_result(result, "Opened dropped file");
                    }
                    shell
                        .get_window(self.ui.window)
                        .ok_or("missing window")?
                        .request_redraw();
                }
                _ => {}
            }
            if let Some(window) = shell.get_window(self.ui.window) {
                window.shared_window().set_title(&self.ui.title()?);
            }
            Ok(())
        })();
        result.map_err(|error| PlatformError::EventLoop(error.to_string()))
    }

    fn on_idle(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> std::result::Result<(), PlatformError> {
        self.drain_accessibility(shell, event_loop)
            .map_err(|error| PlatformError::EventLoop(error.to_string()))?;
        let now = Instant::now();
        let deadline = self
            .ui
            .next_caret_blink(self.now_ms())
            .map_err(|error| PlatformError::EventLoop(error.to_string()))?
            .and_then(|millis| self.started.checked_add(Duration::from_millis(millis)));
        let (control_flow, redraw) = blink_control_flow(now, &mut self.next_blink, deadline);
        if redraw && let Some(window) = shell.get_window(self.ui.window) {
            window.request_redraw();
        }
        event_loop.set_control_flow(control_flow);
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut app = App::new()?;
    if let Some(path) = std::env::args_os().nth(1) {
        app.load_startup_file(PathBuf::from(path))?;
    }
    PlatformApp::new(app, AppConfig::default())?.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use view::text::{CosmicBackend, EditorSession, MemoryClipboard, Selection, TextIndex};
    use view_platform::ImeEvent;

    fn document() -> DocumentUi {
        DocumentUi::with_fonts(
            CosmicBackend::from_fonts(
                [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
                32,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn key(named: NamedKey, phase: KeyPhase) -> KeyboardEvent {
        KeyboardEvent {
            key: Key::Named(named),
            phase,
            modifiers: Modifiers::default(),
            timestamp: 100,
        }
    }

    fn paint(document: &mut DocumentUi, now: u64) -> (PaintSnapshot, accesskit::TreeUpdate) {
        document.prepare([900, 640], ScaleFactor::ONE, now).unwrap()
    }

    fn caret_draws(snapshot: &PaintSnapshot) -> usize {
        snapshot.items.iter().filter(|paint| matches!(paint,
            Paint::Draw(draw) if draw.clip.is_some()
                && matches!(&draw.primitive, Primitive::Rect(bounds) if bounds.size.width == 1.0)
        )).count()
    }

    #[test]
    fn queued_edit_pointer_refresh_uses_current_revision_and_content_origin() {
        let mut document = document();
        paint(&mut document, 0);
        document.native_text("new text before paint", 1).unwrap();
        let outer = document.runtime.global_rect(document.editor).unwrap();
        let content = document.prepare_editor_input(ScaleFactor::ONE, 2).unwrap();
        assert_eq!(
            content.origin,
            outer.origin
                + LogicalPoint::new(
                    document.document_style.padding,
                    document.document_style.padding
                )
        );
        document
            .runtime
            .update(document.editor, Dirty::PAINT, |field: &mut TextField| {
                field.pointer(
                    PointerEvent {
                        phase: PointerPhase::Down,
                        position: content.origin + LogicalPoint::new(0.0, 1.0),
                        button: Some(PointerButton::Primary),
                        modifiers: Modifiers::default(),
                        timestamp: 2,
                    },
                    content,
                )
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            document
                .runtime
                .state::<TextField>(document.editor)
                .unwrap()
                .snapshot()
                .selection,
            Selection::caret(TextIndex(0))
        );
    }

    #[test]
    fn blur_redraw_retains_logical_focus_but_hides_native_focus_and_rejects_late_edits() {
        let mut document = document();
        document.native_text("hello", 0).unwrap();
        document
            .native_ime(&ImeEvent::Preedit("transient".into(), Some((9, 9))), 1)
            .unwrap();
        assert!(
            document
                .runtime
                .state::<TextField>(document.editor)
                .unwrap()
                .snapshot()
                .composition
                .is_some()
        );
        let (_, active_tree) = paint(&mut document, 2);
        assert_eq!(
            active_tree.focus,
            document.bridge.id(document.editor).unwrap()
        );
        document.set_native_active(false, 3).unwrap();
        assert_eq!(
            document.runtime.focused(document.window).unwrap(),
            Some(document.editor)
        );
        let before = document
            .runtime
            .state::<TextField>(document.editor)
            .unwrap()
            .snapshot();
        assert!(before.composition.is_none());
        for now in [4, 510, 1000] {
            let (snapshot, tree) = paint(&mut document, now);
            assert_eq!(tree.focus, document.bridge.id(document.root).unwrap());
            assert_eq!(caret_draws(&snapshot), 0);
            assert!(document.candidate.is_none());
            assert!(
                !document
                    .runtime
                    .state::<TextField>(document.editor)
                    .unwrap()
                    .focused()
            );
        }
        document.native_text("late text", 1010).unwrap();
        document
            .native_ime(
                &ImeEvent::Preedit("late preedit".into(), Some((0, 0))),
                1011,
            )
            .unwrap();
        document
            .native_ime(&ImeEvent::Commit("late commit".into()), 1012)
            .unwrap();
        document
            .editor_key(
                &key(NamedKey::Backspace, KeyPhase::Down),
                &mut MemoryClipboard::default(),
            )
            .unwrap();
        assert_eq!(
            document
                .runtime
                .state::<TextField>(document.editor)
                .unwrap()
                .snapshot(),
            before
        );
        document.set_native_active(true, 1020).unwrap();
        let (snapshot, tree) = paint(&mut document, 1021);
        assert_eq!(tree.focus, document.bridge.id(document.editor).unwrap());
        assert_eq!(caret_draws(&snapshot), 1);
        assert!(document.candidate.is_some());
        document.native_text("!", 1022).unwrap();
        assert_eq!(document.text().unwrap(), "hello!");
    }

    #[test]
    fn paint_preserves_blink_epoch_and_typing_history_group() {
        let mut document = document();
        document.native_text("a", 0).unwrap();
        let (shown, _) = paint(&mut document, 100);
        assert_eq!(caret_draws(&shown), 1);
        let (hidden, _) = paint(&mut document, 500);
        assert_eq!(caret_draws(&hidden), 0);
        document.native_text("b", 900).unwrap();
        let (shown_again, _) = paint(&mut document, 1000);
        assert_eq!(caret_draws(&shown_again), 1);
        document
            .runtime
            .update(document.editor, Dirty::PAINT, |field: &mut TextField| {
                field.editor_mut().undo()
            })
            .unwrap()
            .unwrap();
        assert_eq!(document.text().unwrap(), "");
    }

    #[test]
    fn activation_cancels_held_button_and_restores_the_retained_target() {
        let mut document = document();
        let button = document.buttons[0];
        document.set_focus(Some(button), 0).unwrap();
        document
            .runtime
            .capture_pointer(document.window, button)
            .unwrap();
        document
            .runtime
            .update(button, Dirty::PAINT, |button: &mut Button| {
                button.key(&key(NamedKey::Space, KeyPhase::Down))
            })
            .unwrap();
        assert!(
            document
                .runtime
                .state::<Button>(button)
                .unwrap()
                .behavior
                .state()
                .pressed
        );
        document.set_native_active(false, 10).unwrap();
        assert_eq!(document.runtime.captured(document.window).unwrap(), None);
        let (_, tree) = paint(&mut document, 11);
        assert_eq!(tree.focus, document.bridge.id(document.root).unwrap());
        let state = document
            .runtime
            .state::<Button>(button)
            .unwrap()
            .behavior
            .state();
        assert!(!state.focused && !state.pressed);
        assert_eq!(
            document.runtime.focused(document.window).unwrap(),
            Some(button)
        );
        document.set_native_active(true, 20).unwrap();
        let (_, tree) = paint(&mut document, 21);
        assert_eq!(tree.focus, document.bridge.id(button).unwrap());
        assert!(
            !document
                .runtime
                .update(button, Dirty::PAINT, |button: &mut Button| button
                    .key(&key(NamedKey::Space, KeyPhase::Up)))
                .unwrap()
        );
        assert!(document.next_caret_blink(0).unwrap().is_none());
    }

    #[test]
    fn idle_waits_without_caret_work_and_rearms_expired_blink_deadlines() {
        let mut document = document();
        let now = Instant::now();
        let mut deadline = now - Duration::from_secs(1);
        assert!(document.next_caret_blink(0).unwrap().is_some());
        document.set_native_active(false, 0).unwrap();
        assert_eq!(
            blink_control_flow(
                now,
                &mut deadline,
                document
                    .next_caret_blink(0)
                    .unwrap()
                    .map(|millis| now + Duration::from_millis(millis))
            ),
            (ControlFlow::Wait, false)
        );
        document.set_native_active(true, 0).unwrap();
        assert_eq!(
            blink_control_flow(
                now,
                &mut deadline,
                document
                    .next_caret_blink(0)
                    .unwrap()
                    .map(|millis| now + Duration::from_millis(millis))
            ),
            (
                ControlFlow::WaitUntil(now + Duration::from_millis(500)),
                true
            )
        );
        assert_eq!(
            blink_control_flow(
                now + Duration::from_millis(1),
                &mut deadline,
                Some(now + Duration::from_millis(500))
            ),
            (
                ControlFlow::WaitUntil(now + Duration::from_millis(500)),
                false
            )
        );
        document.pending = Some(PendingAction::New);
        let (snapshot, _) = paint(&mut document, 2);
        assert!(document.next_caret_blink(0).unwrap().is_none());
        assert_eq!(caret_draws(&snapshot), 0);
        assert_eq!(
            blink_control_flow(
                now + Duration::from_secs(2),
                &mut deadline,
                document
                    .next_caret_blink(0)
                    .unwrap()
                    .map(|millis| now + Duration::from_millis(millis))
            ),
            (ControlFlow::Wait, false)
        );
        document.pending = None;
        document.set_focus(Some(document.editor), 3).unwrap();
        document.native_text("abc", 3).unwrap();
        let revision = document
            .runtime
            .state::<TextField>(document.editor)
            .unwrap()
            .snapshot()
            .text
            .revision();
        document
            .runtime
            .update(document.editor, Dirty::PAINT, |field: &mut TextField| {
                field.editor_mut().select(
                    revision,
                    Selection {
                        anchor: TextIndex(0),
                        focus: TextIndex(2),
                    },
                )
            })
            .unwrap()
            .unwrap();
        assert!(document.next_caret_blink(0).unwrap().is_none());
        document
            .native_ime(&ImeEvent::Preedit("hidden".into(), None), 4)
            .unwrap();
        assert!(document.next_caret_blink(0).unwrap().is_none());
    }
}
