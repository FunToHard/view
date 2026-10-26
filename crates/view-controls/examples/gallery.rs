//! Explicit modes only: --headless, --gpu <backend>, --run-hidden <backend>,
//! --resize-soak <backend>, --show <backend>. Only --show opens an interactive window.
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_controls::{accessibility::*, *};
use view_core::*;
use view_platform::{
    AppConfig, ImeService, PlatformApp, PlatformError, PlatformHandler, PlatformShell,
    PlatformWindow, PlatformWindowEvent,
};
use view_text::*;
use view_wgpu::{
    Acquire, Budgets, Color, Draw, GpuContext, Paint, PaintSnapshot, Primitive, Renderer,
    WindowSurface, wgpu,
};
use winit::event_loop::ActiveEventLoop;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
}
fn quad(bounds: LogicalRect, color: Rgba) -> Paint {
    Paint::Draw(Draw::new(Primitive::Rect(bounds), Color(color.0)))
}
fn wait<T>(future: impl Future<Output = T>) -> Result<T> {
    struct WakeThread(std::thread::Thread);
    impl Wake for WakeThread {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let end = Instant::now() + Duration::from_secs(30);
    let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return Ok(value);
        }
        std::thread::park_timeout(
            end.checked_duration_since(Instant::now())
                .ok_or("GPU timeout")?,
        );
    }
}
fn backend(name: &str) -> Result<wgpu::Backends> {
    match name {
        "dx12" => Ok(wgpu::Backends::DX12),
        "vulkan" => Ok(wgpu::Backends::VULKAN),
        "metal" => Ok(wgpu::Backends::METAL),
        _ => Err("specify dx12, vulkan or metal".into()),
    }
}
#[derive(Default)]
struct MemoryClipboard(String);
impl Clipboard for MemoryClipboard {
    fn read(&mut self) -> std::result::Result<String, TextError> {
        Ok(self.0.clone())
    }
    fn write(&mut self, s: &str) -> std::result::Result<(), TextError> {
        self.0 = s.into();
        Ok(())
    }
}
enum Widget {
    Label(Label),
    Button(Button),
    Check(Checkbox),
    Slider(Slider),
    Number(NumericField),
    Field(TextField, Style),
    List(VirtualList),
    Radio(RadioGroup),
    Tabs(TabList),
    TabPanel,
    Tree(Tree),
    Select(Select, Pressable),
    Table(Table),
}
impl Widget {
    fn field(&mut self) -> Option<&mut TextField> {
        match self {
            Self::Field(field, _) => Some(field),
            Self::Number(number) => Some(&mut number.field),
            _ => None,
        }
    }
    fn focus(&mut self, focus: bool, now: u64) {
        match self {
            Self::Button(b) => b.behavior.set_focused(focus),
            Self::Check(c) => c.button.behavior.set_focused(focus),
            Self::Radio(group) => group.set_focused(focus),
            Self::Tabs(group) => group.set_focused(focus),
            Self::Select(select, press) => {
                select.set_focused(focus);
                press.set_focused(focus);
            }
            Self::Slider(s) => {
                s.focused = focus;
                if !focus {
                    s.cancel();
                }
            }
            _ => {
                if let Some(field) = self.field() {
                    field.set_focused(focus, now);
                }
            }
        }
    }
    fn semantics(&self, id: NodeId, bounds: LogicalRect, popups: &PopupStack) -> ControlSemantics {
        match self {
            Self::Label(s) => s.semantics(id, bounds),
            Self::Button(s) => s.semantics(id, bounds),
            Self::Check(s) => s.semantics(id, bounds),
            Self::Slider(s) => s.semantics(id, bounds),
            Self::Number(s) => s.field.semantics(id, bounds),
            Self::Field(s, _) => s.semantics(id, bounds),
            Self::Select(select, _) => select.semantics(id, bounds, popups),
            Self::Table(table) => table.semantics(
                id,
                bounds,
                "Asset table: arrows navigate, Shift selects rows",
            ),
            Self::List(s) => {
                let mut node = SemanticNode::new(id, Role::Custom("list".into()));
                node.name = Some("10,000 keyed rows".into());
                node.value = s.selection.focused().map(str::to_owned);
                node.bounds = bounds;
                node.actions = vec![SemanticAction::Focus];
                ControlSemantics::new(node)
            }
            Self::Radio(s) => {
                let mut node = SemanticNode::new(id, Role::Container);
                node.name = Some("Quality: arrows select".into());
                node.value = s.selected().map(str::to_owned);
                node.bounds = bounds;
                node.actions = vec![SemanticAction::Focus];
                ControlSemantics::new(node)
            }
            Self::Tabs(s) => {
                let mut node = SemanticNode::new(id, Role::Custom("tab-list".into()));
                node.name = Some("Control guide: Left/Right, Home/End".into());
                node.value = Some(s.selected().into());
                node.bounds = bounds;
                node.actions = vec![SemanticAction::Focus];
                ControlSemantics::new(node)
            }
            Self::TabPanel => {
                let mut node = SemanticNode::new(id, Role::Custom("tab-panel".into()));
                node.bounds = bounds;
                ControlSemantics::new(node)
            }
            Self::Tree(s) => {
                let mut node = SemanticNode::new(id, Role::Custom("tree".into()));
                node.name = Some("Project tree: arrows expand".into());
                node.value = s.list.selection.focused().map(str::to_owned);
                node.bounds = bounds;
                node.actions = vec![SemanticAction::Focus];
                ControlSemantics::new(node)
            }
        }
    }
}
struct Gallery {
    runtime: Runtime<(), ()>,
    window: WindowId,
    root: NodeId,
    ids: Vec<NodeId>,
    fonts: CosmicBackend,
    bridge: SemanticBridge,
    revision: Revision,
    status: String,
    candidate: Option<LogicalRect>,
    native_active: bool,
    rows: std::collections::HashMap<NodeId, (NodeId, String)>,
    popups: PopupStack,
    select_popup: Option<(NodeId, NodeId)>,
    cells: std::collections::HashMap<NodeId, (NodeId, String, usize)>,
}
impl Gallery {
    fn new() -> Result<Self> {
        let mut runtime = Runtime::new(128)?;
        let (window, root) = runtime.open_window(
            Description::new("gallery", (), |_, _, _| Dirty::NONE)
                .role(Role::Window)
                .layout(rect(0.0, 0.0, 900.0, 1000.0)),
        )?;
        let mut this = Self {
            runtime,
            window,
            root,
            ids: vec![],
            fonts: CosmicBackend::from_fonts(
                [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
                128,
            )?,
            bridge: SemanticBridge::default(),
            revision: Revision::INITIAL,
            status: "Tab moves focus; arrows navigate; right-click text for native commands".into(),
            candidate: None,
            native_active: true,
            rows: Default::default(),
            popups: PopupStack::default(),
            select_popup: None,
            cells: Default::default(),
        };
        this.add(
            "heading",
            Widget::Label(Label::new("view / standard controls")),
            rect(24.0, 18.0, 820.0, 30.0),
            false,
        )?;
        this.add(
            "apply",
            Widget::Button(Button::new("Apply")),
            rect(24.0, 65.0, 130.0, 40.0),
            true,
        )?;
        let mut disabled = Button::new("Disabled");
        disabled.behavior.set_disabled(true);
        this.add(
            "disabled",
            Widget::Button(disabled),
            rect(170.0, 65.0, 130.0, 40.0),
            false,
        )?;
        this.add(
            "check",
            Widget::Check(Checkbox::new("Mixed / On / Off", CheckState::Mixed, true)?),
            rect(330.0, 65.0, 230.0, 40.0),
            true,
        )?;
        this.add(
            "radio",
            Widget::Radio(RadioGroup::new(
                [("low", "Low"), ("medium", "Medium"), ("high", "High")]
                    .into_iter()
                    .map(|(key, label)| RadioOption {
                        key: key.into(),
                        label: label.into(),
                        disabled: false,
                    })
                    .collect(),
                None,
                true,
            )?),
            rect(590.0, 65.0, 280.0, 40.0),
            true,
        )?;
        let spec = RangeSpec {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            page: 10.0,
            decimals: 1,
        };
        this.add(
            "slider",
            Widget::Slider(Slider::new(spec, 35.0)?),
            rect(24.0, 133.0, 430.0, 32.0),
            true,
        )?;
        this.add(
            "number",
            Widget::Number(NumericField::new(
                "Numeric value; Enter commits",
                spec,
                35.0,
            )?),
            rect(480.0, 126.0, 140.0, 44.0),
            true,
        )?;
        this.add(
            "standard-label",
            Widget::Label(Label::new("Standard TextInput / shared editor")),
            rect(24.0, 188.0, 410.0, 26.0),
            false,
        )?;
        this.add(
            "standard",
            Widget::Field(
                TextField::new(
                    "Standard name",
                    "Edit me; select, copy, undo",
                    EditorConfig::default(),
                )?,
                Style::default(),
            ),
            rect(24.0, 219.0, 400.0, 44.0),
            true,
        )?;
        this.add(
            "custom-label",
            Widget::Label(Label::new("Custom presentation / identical session")),
            rect(470.0, 188.0, 410.0, 26.0),
            false,
        )?;
        let custom = Style {
            background: Rgba([0.02, 0.19, 0.16, 1.0]),
            foreground: Rgba([0.72, 1.0, 0.85, 1.0]),
            border: Rgba([0.2, 0.85, 0.55, 1.0]),
            accent: Rgba([0.4, 1.0, 0.7, 1.0]),
            padding: 10.0,
            border_width: 3.0,
            ..Default::default()
        };
        this.add(
            "custom",
            Widget::Field(
                TextField::new(
                    "Custom name",
                    "Same editing, different decoration",
                    EditorConfig::default(),
                )?,
                custom,
            ),
            rect(470.0, 219.0, 400.0, 44.0),
            true,
        )?;
        let mut readonly = TextField::new(
            "Read only, copy allowed",
            "Read-only text: selection and Copy remain enabled",
            EditorConfig::default(),
        )?;
        readonly.set_modes(true, false);
        this.add(
            "readonly",
            Widget::Field(readonly, Style::default()),
            rect(24.0, 284.0, 846.0, 40.0),
            true,
        )?;
        this.add("area",Widget::Field(TextField::new("Notes","Plain TextArea\nMultiple lines, wrapping, selection and IME.\nTab traverses by default.",EditorConfig{mode:LineMode::Multiline,..Default::default()})?,Style::default()),rect(24.0,345.0,400.0,160.0),true)?;
        this.add(
            "list",
            Widget::List(VirtualList::new(
                (0..10000).map(|i| format!("Item {i:05}")).collect(),
                26.0,
                LogicalSize::new(240.0, 156.0),
                8,
            )?),
            rect(450.0, 345.0, 240.0, 156.0),
            true,
        )?;
        let mut tree = Tree::new(
            vec![
                TreeItem {
                    key: "Project".into(),
                    parent: None,
                    label: "Project".into(),
                },
                TreeItem {
                    key: "Sources".into(),
                    parent: Some("Project".into()),
                    label: "Sources".into(),
                },
                TreeItem {
                    key: "main.rs".into(),
                    parent: Some("Sources".into()),
                    label: "main.rs".into(),
                },
                TreeItem {
                    key: "Assets".into(),
                    parent: Some("Project".into()),
                    label: "Assets".into(),
                },
            ],
            LogicalSize::new(150.0, 156.0),
        )?;
        tree.set_expanded("Project", true)?;
        this.add(
            "tree",
            Widget::Tree(tree),
            rect(720.0, 345.0, 150.0, 156.0),
            true,
        )?;
        this.add(
            "tabs",
            Widget::Tabs(TabList::new(
                [
                    ("overview", "Overview", false),
                    ("keyboard", "Keyboard", false),
                    ("later", "Later", true),
                    ("coverage", "Coverage", false),
                ]
                .into_iter()
                .map(|(key, label, disabled)| TabOption {
                    key: key.into(),
                    label: label.into(),
                    disabled,
                })
                .collect(),
                None,
            )?),
            rect(24.0, 518.0, 846.0, 38.0),
            true,
        )?;
        this.add(
            "tab-panel",
            Widget::TabPanel,
            rect(24.0, 564.0, 846.0, 44.0),
            false,
        )?;
        this.add("footer",Widget::Label(Label::new("Preliminary profiles: CPU/GPU checks are separate from physical native certification.")),rect(24.0,680.0,850.0,36.0),false)?;
        let select = this.add(
            "select",
            Widget::Select(
                Select::new(
                    "Render quality",
                    [
                        ("draft", "Draft", false),
                        ("balanced", "Balanced", false),
                        ("unavailable", "Unavailable", true),
                        ("high", "High quality", false),
                    ]
                    .into_iter()
                    .map(|(key, label, disabled)| SelectOption {
                        key: key.into(),
                        label: label.into(),
                        disabled,
                    })
                    .collect(),
                    Some("balanced".into()),
                )?,
                Pressable::default(),
            ),
            rect(24.0, 730.0, 270.0, 38.0),
            true,
        )?;
        let content = this.runtime.reconcile(
            select,
            vec![
                Description::new("options", (), |_, _, _| Dirty::NONE)
                    .structure(Structure::Declarative),
            ],
        )?[0];
        this.select_popup = Some((select, content));
        this.add(
            "table",
            Widget::Table(Table::new(
                [
                    ("name", "Asset", 300.0),
                    ("kind", "Kind", 160.0),
                    ("state", "State", 160.0),
                    ("size", "Bytes", 240.0),
                ]
                .into_iter()
                .map(|(key, label, width)| TableColumn {
                    key: key.into(),
                    label: label.into(),
                    width,
                })
                .collect(),
                (0..1000)
                    .map(|i| TableRow {
                        key: format!("asset-{i}"),
                        cells: vec![
                            format!("Asset {i:04}"),
                            "Mesh".into(),
                            "Ready".into(),
                            (i * 4096).to_string(),
                        ],
                    })
                    .collect(),
                28.0,
                LogicalSize::new(846.0, 174.0),
                TableLimits {
                    max_rows: 1000,
                    max_columns: 8,
                    max_realized_cells: 64,
                },
            )?),
            rect(24.0, 792.0, 846.0, 174.0),
            true,
        )?;
        this.runtime.flush(&mut ())?;
        Ok(this)
    }
    fn add(
        &mut self,
        key: &str,
        widget: Widget,
        bounds: LogicalRect,
        focus: bool,
    ) -> Result<NodeId> {
        let id = self.runtime.mount(
            self.root,
            Description::new(key, widget, |_, _, _| Dirty::PAINT)
                .layout(bounds)
                .structure(Structure::Declarative)
                .focusable(focus),
        )?;
        self.ids.push(id);
        Ok(id)
    }
    fn focus(&mut self, id: Option<NodeId>, now: u64) -> Result<()> {
        if let Some((owner, _)) = self.select_popup
            && id != Some(owner)
        {
            let popups = &mut self.popups;
            self.runtime
                .update(owner, Dirty::SEMANTICS, |widget: &mut Widget| {
                    if let Widget::Select(select, _) = widget {
                        select.close(popups, |_| false);
                    }
                })?;
        }
        self.runtime.set_focus(self.window, id)?;
        self.sync_native_focus(now)
    }

    fn sync_native_focus(&mut self, now: u64) -> Result<()> {
        let id = if self.native_active {
            self.runtime.focused(self.window)?
        } else {
            None
        };
        for owner in &self.ids {
            self.runtime
                .update(*owner, Dirty::PAINT, |widget: &mut Widget| {
                    widget.focus(Some(*owner) == id, now)
                })?;
        }
        Ok(())
    }

    fn set_native_active(&mut self, active: bool, now: u64) -> Result<()> {
        self.native_active = active;
        if !active {
            self.runtime.cancel_pointer(self.window)?;
            self.candidate = None;
            if let Some((owner, _)) = self.select_popup {
                let popups = &mut self.popups;
                self.runtime
                    .update(owner, Dirty::SEMANTICS, |widget: &mut Widget| {
                        if let Widget::Select(select, _) = widget {
                            select.close(popups, |_| false);
                        }
                    })?;
            }
        }
        self.sync_native_focus(now)
    }

    fn native_text(&mut self, text: &str, now: u64) -> Result<()> {
        if self.native_active
            && let Some(id) = self.runtime.focused(self.window)?
        {
            self.runtime
                .update(id, Dirty::PAINT, |widget: &mut Widget| {
                    if let Some(field) = widget.field()
                        && field.focused()
                        && !field.editor().read_only()
                    {
                        field.insert_text(text, now)?;
                    }
                    Ok::<_, TextError>(())
                })??;
        }
        Ok(())
    }

    fn native_ime(&mut self, event: &view_platform::ImeEvent, now: u64) -> Result<()> {
        if self.native_active
            && let Some(id) = self.runtime.focused(self.window)?
        {
            self.runtime
                .update(id, Dirty::PAINT, |widget: &mut Widget| {
                    if let Some(field) = widget.field()
                        && field.focused()
                    {
                        view_platform::text::dispatch_ime(field.editor_mut(), event, now)?;
                    }
                    Ok::<_, TextError>(())
                })??;
        }
        Ok(())
    }

    fn next_caret_blink(&self, now: u64) -> Result<Option<u64>> {
        if !self.native_active {
            return Ok(None);
        }
        let Some(id) = self.runtime.focused(self.window)? else {
            return Ok(None);
        };
        let field = match self.runtime.state::<Widget>(id)? {
            Widget::Field(field, _) => field,
            Widget::Number(number) => &number.field,
            _ => return Ok(None),
        };
        Ok(field.editor().next_caret_blink(now))
    }
    fn label(
        &mut self,
        text: &str,
        bounds: LogicalRect,
        style: &Style,
        scale: ScaleFactor,
    ) -> Result<Paint> {
        let layout = Arc::new(self.fonts.shape(&LayoutRequest {
            text: TextSnapshot::new(text, TextRevision::INITIAL),
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            line_height: style.line_height,
            max_width: Some(bounds.size.width),
            wrap: Wrap::Word,
            scale,
            font_revision: self.fonts.font_revision(),
        })?);
        let mut draw = Draw::new(
            Primitive::Text {
                layout,
                origin: bounds.origin,
            },
            Color(style.foreground.0),
        );
        draw.clip = Some(bounds);
        Ok(Paint::Draw(draw))
    }
    fn prepare(
        &mut self,
        size: [u32; 2],
        scale: ScaleFactor,
        now: u64,
    ) -> Result<(PaintSnapshot, accesskit::TreeUpdate)> {
        let selected = self
            .ids
            .iter()
            .find_map(|id| match self.runtime.state::<Widget>(*id).ok()? {
                Widget::Tabs(tabs) => Some(tabs.selected()),
                _ => None,
            })
            .unwrap_or("overview");
        let panel_text = match selected {
            "keyboard" => {
                "Tab enters/leaves the group. Left/Right wrap; Home/End select endpoints. Disabled tabs are skipped."
            }
            "coverage" => {
                "Preview: CPU and rendering checks are separate from native IME and assistive-technology certification."
            }
            _ => {
                "Explore controls above: enabled/disabled buttons, mixed checkbox, radio choices, ranges and editable/read-only text."
            }
        };
        self.runtime.flush(&mut ())?;
        self.revision = self.revision.checked_next()?;
        self.candidate = None;
        let focused = if self.native_active {
            self.runtime.focused(self.window)?
        } else {
            None
        };
        let mut root = SemanticNode::new(self.root, Role::Window);
        root.name = Some("view control gallery".into());
        root.bounds = rect(
            0.0,
            0.0,
            size[0] as f32 / scale.get(),
            size[1] as f32 / scale.get(),
        );
        let mut entries = vec![Entry {
            parent: None,
            state: ControlSemantics::new(root),
        }];
        let mut items = vec![];
        self.rows.clear();
        for id in self.ids.clone() {
            let bounds = self.runtime.global_rect(id)?;
            let mut state = self
                .runtime
                .state::<Widget>(id)?
                .semantics(id, bounds, &self.popups);
            if matches!(self.runtime.state::<Widget>(id)?, Widget::TabPanel) {
                state.node.name = Some(panel_text.into());
            }
            state.node.focused = focused == Some(id) && !state.node.disabled;
            entries.push(Entry {
                parent: Some(self.root),
                state,
            });
            let style = Style::default();
            let mut labels = vec![];
            let mut local = vec![];
            let fonts = &mut self.fonts;
            let mut candidate = None;
            self.runtime
                .update(id, Dirty::NONE, |widget: &mut Widget| -> Result<()> {
                    match widget {
                        Widget::Label(label) => {
                            labels.push((label.text.clone(), bounds, style.clone()))
                        }
                        Widget::Button(button) => {
                            let style = style.interaction(button.behavior.state());
                            local.extend(frame(bounds, &style));
                            labels.push((
                                button.label.clone(),
                                rect(
                                    bounds.left() + 12.0,
                                    bounds.top() + 8.0,
                                    bounds.size.width - 20.0,
                                    26.0,
                                ),
                                style,
                            ));
                        }
                        Widget::Check(check) => {
                            let style = style.interaction(check.button.behavior.state());
                            local.extend(frame(
                                rect(bounds.left(), bounds.top() + 8.0, 24.0, 24.0),
                                &style,
                            ));
                            if check.state() != CheckState::Off {
                                local.push(quad(
                                    rect(
                                        bounds.left() + 5.0,
                                        bounds.top() + 17.0,
                                        14.0,
                                        if check.state() == CheckState::Mixed {
                                            4.0
                                        } else {
                                            10.0
                                        },
                                    ),
                                    style.accent,
                                ));
                            }
                            labels.push((
                                check.button.label.clone(),
                                rect(
                                    bounds.left() + 34.0,
                                    bounds.top() + 8.0,
                                    bounds.size.width - 34.0,
                                    26.0,
                                ),
                                style,
                            ));
                        }
                        Widget::Slider(slider) => {
                            local.push(quad(
                                rect(bounds.left(), bounds.top() + 14.0, bounds.size.width, 4.0),
                                style.border,
                            ));
                            local.push(quad(
                                rect(
                                    bounds.left()
                                        + slider.range.fraction() as f32
                                            * (bounds.size.width - 12.0),
                                    bounds.top() + 4.0,
                                    12.0,
                                    24.0,
                                ),
                                style.accent,
                            ));
                        }
                        Widget::Number(number) => {
                            let style = style.interaction(Interaction {
                                focused: focused == Some(id),
                                ..Default::default()
                            });
                            local.extend(frame(bounds, &style));
                            let p = number.field.prepare(fonts, &style, bounds, scale, now)?;
                            if focused == Some(id) {
                                candidate = Some(p.candidate);
                            }
                            local.extend(p.paint(&style)?);
                        }
                        Widget::Field(field, custom) => {
                            let mut style = custom.interaction(Interaction {
                                focused: focused == Some(id),
                                disabled: field.editor().disabled(),
                                ..Default::default()
                            });
                            if field.invalid {
                                style.border = Rgba([1.0, 0.2, 0.2, 1.0]);
                            }
                            local.extend(frame(bounds, &style));
                            let p = field.prepare(fonts, &style, bounds, scale, now)?;
                            if focused == Some(id) && !field.editor().read_only() {
                                candidate = Some(p.candidate);
                            }
                            local.extend(p.paint(&style)?);
                        }
                        Widget::List(list) => {
                            local.extend(frame(bounds, &style));
                            for row in list.realized()? {
                                let mut r = list.row_rect(row);
                                r.origin = r.origin + bounds.origin;
                                if list
                                    .selection
                                    .selected()
                                    .contains(&list.selection.keys()[row])
                                {
                                    local.push(quad(r, style.selection));
                                }
                                labels.push((list.selection.keys()[row].clone(), r, style.clone()));
                            }
                        }
                        Widget::Tree(tree) => {
                            local.extend(frame(bounds, &style));
                            for row in tree.presentation(bounds, focused == Some(id))? {
                                local.extend(row.paint(fonts, &style, scale)?);
                            }
                        }
                        Widget::Select(select, press) => {
                            let style = style.interaction(press.state());
                            local.extend(frame(bounds, &style));
                            let value = select
                                .selected()
                                .and_then(|key| {
                                    select.options().iter().find(|option| option.key == key)
                                })
                                .map_or("Choose", |option| option.label.as_str());
                            labels.push((
                                format!("{value}  ▾"),
                                rect(
                                    bounds.left() + 8.0,
                                    bounds.top() + 6.0,
                                    bounds.size.width - 16.0,
                                    28.0,
                                ),
                                style,
                            ));
                        }
                        Widget::Table(table) => {
                            local.extend(frame(bounds, &style));
                            for cell in table.presentation(bounds, focused == Some(id))? {
                                local.extend(cell.paint(fonts, &style, scale)?);
                            }
                        }
                        Widget::Tabs(group) => {
                            let width = bounds.size.width / group.options().len() as f32;
                            for (index, option) in group.options().iter().enumerate() {
                                let r = rect(
                                    bounds.left() + index as f32 * width,
                                    bounds.top(),
                                    width - 4.0,
                                    bounds.size.height,
                                );
                                let active = group.selected() == option.key;
                                let tab_style = style.interaction(Interaction {
                                    disabled: option.disabled,
                                    focused: group.focused() && active,
                                    ..Default::default()
                                });
                                local.extend(frame(r, &tab_style));
                                if active {
                                    local.push(quad(
                                        rect(r.left(), r.bottom() - 3.0, r.size.width, 3.0),
                                        style.accent,
                                    ));
                                }
                                labels.push((
                                    option.label.clone(),
                                    rect(r.left() + 8.0, r.top() + 7.0, r.size.width - 16.0, 26.0),
                                    tab_style,
                                ));
                            }
                        }
                        Widget::TabPanel => {
                            labels.push((panel_text.into(), bounds, style.clone()));
                        }
                        Widget::Radio(group) => {
                            for (index, option) in group.options().iter().enumerate() {
                                let x = bounds.left() + index as f32 * 90.0;
                                local.push(quad(
                                    rect(x, bounds.top() + 12.0, 12.0, 12.0),
                                    if group.selected() == Some(option.key.as_str()) {
                                        style.accent
                                    } else {
                                        style.border
                                    },
                                ));
                                labels.push((
                                    option.label.clone(),
                                    rect(x + 18.0, bounds.top() + 8.0, 72.0, 26.0),
                                    style.clone(),
                                ));
                            }
                        }
                    }
                    Ok(())
                })??;
            if candidate.is_some() {
                self.candidate = candidate;
            }
            items.extend(local);
            for (text, bounds, style) in labels {
                items.push(self.label(&text, bounds, &style, scale)?);
            }
            // Realized semantic rows use the runtime's keyed child identities.
            // Zero layout avoids duplicate hit targets; the list owns hit/virtual
            // focus while each accessible row retains its named action target.
            let mut rows = Vec::new();
            match self.runtime.state::<Widget>(id)? {
                Widget::List(list) => {
                    for index in list.realized()? {
                        let key = &list.selection.keys()[index];
                        let mut bounds = list.row_rect(index);
                        bounds.origin = bounds.origin + self.runtime.global_rect(id)?.origin;
                        rows.push((
                            key.clone(),
                            key.clone(),
                            Role::Custom("list-item".into()),
                            bounds,
                            list.selection.selected().contains(key),
                            list.selection.focused() == Some(key.as_str()),
                            None,
                        ));
                    }
                }
                Widget::Tree(tree) => {
                    for index in tree.list.realized()? {
                        let key = &tree.list.selection.keys()[index];
                        let mut bounds = tree.list.row_rect(index);
                        bounds.origin = bounds.origin + self.runtime.global_rect(id)?.origin;
                        let expanded = tree
                            .items()
                            .iter()
                            .any(|item| item.parent.as_ref() == Some(key))
                            .then(|| tree.expanded(key));
                        rows.push((
                            key.clone(),
                            key.clone(),
                            Role::Custom("tree-item".into()),
                            bounds,
                            tree.list.selection.selected().contains(key),
                            tree.list.selection.focused() == Some(key.as_str()),
                            expanded,
                        ));
                    }
                }
                Widget::Tabs(group) => {
                    let width = bounds.size.width / group.options().len() as f32;
                    for (index, option) in group.options().iter().enumerate() {
                        rows.push((
                            option.key.clone(),
                            option.label.clone(),
                            Role::Custom("tab".into()),
                            rect(
                                bounds.left() + index as f32 * width,
                                bounds.top(),
                                width,
                                bounds.size.height,
                            ),
                            group.selected() == option.key,
                            group.selected() == option.key,
                            None,
                        ));
                    }
                }
                Widget::Radio(group) => {
                    for (index, option) in group.options().iter().enumerate() {
                        rows.push((
                            option.key.clone(),
                            option.label.clone(),
                            Role::Radio,
                            rect(
                                bounds.left() + index as f32 * 90.0,
                                bounds.top(),
                                90.0,
                                bounds.size.height,
                            ),
                            group.selected() == Some(option.key.as_str()),
                            group.selected() == Some(option.key.as_str()),
                            None,
                        ));
                    }
                }
                _ => {}
            }
            if !rows.is_empty() {
                let owners = self.runtime.reconcile(
                    id,
                    rows.iter()
                        .map(|(key, ..)| Description::new(key, (), |_, _, _| Dirty::NONE))
                        .collect(),
                )?;
                let child_focused =
                    rows.iter().any(|(_, _, _, _, _, focus, _)| *focus) && focused == Some(id);
                if child_focused {
                    entries
                        .iter_mut()
                        .find(|entry| entry.state.node.id == id)
                        .unwrap()
                        .state
                        .node
                        .focused = false;
                }
                for (owner, (key, label, role, bounds, selected, focus, expanded)) in
                    owners.into_iter().zip(rows)
                {
                    let radio = role == Role::Radio;
                    let mut node = SemanticNode::new(owner, role);
                    node.name = Some(label);
                    node.bounds = bounds;
                    node.focused = focus && focused == Some(id);
                    node.actions = vec![SemanticAction::Click, SemanticAction::Focus];
                    if radio {
                        node.checked = Some(selected);
                    }
                    if let Some(expanded) = expanded {
                        node.actions.push(if expanded {
                            SemanticAction::Collapse
                        } else {
                            SemanticAction::Expand
                        });
                    }
                    let mut state = ControlSemantics::new(node);
                    state.selected = Some(selected);
                    state.expanded = expanded;
                    if let Widget::Tabs(tabs) = self.runtime.state::<Widget>(id)? {
                        let index = tabs
                            .options()
                            .iter()
                            .position(|option| option.key == key)
                            .ok_or("missing tab")?;
                        state = tabs.semantics(index, owner, bounds)?;
                    }
                    entries.push(Entry {
                        parent: Some(id),
                        state,
                    });
                    self.rows.insert(owner, (id, key));
                }
            }
        }
        self.append_table_semantics(&mut entries, focused)?;
        self.append_select_popup(&mut entries, &mut items, scale)?;
        items.push(self.label(
            &self.status.clone(),
            rect(24.0, 620.0, 850.0, 44.0),
            &Style::default(),
            scale,
        )?);
        let tree = self.bridge.commit(self.revision, &entries, scale)?;
        self.runtime.flush(&mut ())?;
        Ok((
            PaintSnapshot {
                revision: self.revision,
                size,
                scale,
                items,
            },
            tree,
        ))
    }
    fn select_popup_definition(&self, owner: NodeId, content: NodeId) -> Result<Popup> {
        let bounds = PopupStack::placement(
            self.runtime.global_rect(owner)?,
            LogicalSize::new(270.0, 120.0),
            self.runtime.global_rect(self.root)?,
        )?;
        Ok(Popup {
            owner,
            content,
            return_focus: Some(owner),
            bounds,
            modal: false,
        })
    }
    fn append_select_popup(
        &mut self,
        entries: &mut Vec<Entry>,
        items: &mut Vec<Paint>,
        scale: ScaleFactor,
    ) -> Result<()> {
        let Some((owner, content)) = self.select_popup else {
            return Ok(());
        };
        if !self.popups.contains(content) {
            return Ok(());
        }
        let popup = *self.popups.top().ok_or("missing popup")?;
        let Widget::Select(select, _) = self.runtime.state::<Widget>(owner)? else {
            return Err("missing select".into());
        };
        let visible = select.visible_options(popup.bounds.size.height, 30.0)?;
        let keys: Vec<_> = visible
            .clone()
            .map(|index| select.options()[index].key.clone())
            .collect();
        let rows = select.presentation(popup.bounds, 30.0, &self.popups)?;
        let mut node = SemanticNode::new(content, Role::Custom("list".into()));
        node.name = Some("Render quality options".into());
        node.bounds = popup.bounds;
        entries.push(Entry {
            parent: Some(owner),
            state: ControlSemantics::new(node),
        });
        entries
            .iter_mut()
            .find(|entry| entry.state.node.id == owner)
            .unwrap()
            .state
            .node
            .focused = false;
        items.extend(frame(popup.bounds, &Style::default()));
        for row in rows {
            items.extend(row.paint(&mut self.fonts, &Style::default(), scale)?);
        }
        let ids = self.runtime.reconcile(
            content,
            keys.iter()
                .map(|key| Description::new(key, (), |_, _, _| Dirty::NONE))
                .collect(),
        )?;
        for (offset, id) in ids.into_iter().enumerate() {
            let Widget::Select(select, _) = self.runtime.state::<Widget>(owner)? else {
                unreachable!()
            };
            let index = visible.start + offset;
            let bounds = rect(
                popup.bounds.left(),
                popup.bounds.top() + offset as f32 * 30.0,
                popup.bounds.size.width,
                30.0,
            );
            entries.push(Entry {
                parent: Some(content),
                state: select.option_semantics(index, id, bounds, &self.popups)?,
            });
            self.rows.insert(id, (owner, keys[offset].clone()));
        }
        Ok(())
    }
    fn append_table_semantics(
        &mut self,
        entries: &mut Vec<Entry>,
        focused: Option<NodeId>,
    ) -> Result<()> {
        self.cells.clear();
        for owner in self.ids.clone() {
            let Widget::Table(table) = self.runtime.state::<Widget>(owner)? else {
                continue;
            };
            let visible = table.realized_rows()?;
            let columns = table.realized_columns();
            let mut row_keys = vec!["header".to_string()];
            row_keys.extend(
                visible
                    .clone()
                    .map(|index| format!("row:{}", table.rows()[index].key)),
            );
            let ids = self.runtime.reconcile(
                owner,
                row_keys
                    .iter()
                    .map(|key| {
                        Description::new(key, (), |_, _, _| Dirty::NONE)
                            .structure(Structure::Declarative)
                    })
                    .collect(),
            )?;
            let whole = self.runtime.global_rect(owner)?;
            for (offset, id) in ids.into_iter().enumerate() {
                let row = if offset == 0 {
                    None
                } else {
                    Some(visible.start + offset - 1)
                };
                let Widget::Table(table) = self.runtime.state::<Widget>(owner)? else {
                    unreachable!()
                };
                let mut row_bounds = table.cell_rect(row, 0)?;
                row_bounds.origin = whole.origin + LogicalPoint::new(0.0, row_bounds.origin.y);
                row_bounds.size.width = whole.size.width;
                let clip = if row.is_some() {
                    rect(
                        whole.left(),
                        whole.top() + table.row_height(),
                        whole.size.width,
                        whole.size.height - table.row_height(),
                    )
                } else {
                    whole
                };
                let mut node = SemanticNode::new(id, Role::Custom("row".into()));
                node.bounds = row_bounds.intersection(clip).ok_or("invisible table row")?;
                entries.push(Entry {
                    parent: Some(owner),
                    state: ControlSemantics::new(node),
                });
                let column_keys: Vec<_> = columns
                    .clone()
                    .map(|index| table.columns()[index].key.clone())
                    .collect();
                let cell_ids = self.runtime.reconcile(
                    id,
                    column_keys
                        .iter()
                        .map(|key| Description::new(key, (), |_, _, _| Dirty::NONE))
                        .collect(),
                )?;
                for (column, cell_id) in columns.clone().zip(cell_ids) {
                    let Widget::Table(table) = self.runtime.state::<Widget>(owner)? else {
                        unreachable!()
                    };
                    let mut bounds = table.cell_rect(row, column)?;
                    bounds.origin = bounds.origin + whole.origin;
                    let state = table.cell_semantics(
                        row,
                        column,
                        cell_id,
                        bounds.intersection(clip).ok_or("invisible table cell")?,
                        focused == Some(owner),
                    )?;
                    if state.node.focused {
                        entries
                            .iter_mut()
                            .find(|entry| entry.state.node.id == owner)
                            .unwrap()
                            .state
                            .node
                            .focused = false;
                    }
                    entries.push(Entry {
                        parent: Some(id),
                        state,
                    });
                    if let Some(row) = row {
                        self.cells
                            .insert(cell_id, (owner, table.rows()[row].key.clone(), column));
                    }
                }
            }
        }
        Ok(())
    }
    fn key(&mut self, event: KeyboardEvent, clipboard: &mut impl Clipboard) -> Result<()> {
        if !self.native_active {
            return Ok(());
        }
        if let Some((owner, content)) = self.select_popup
            && self.runtime.focused(self.window)? == Some(owner)
        {
            let popup = self.select_popup_definition(owner, content)?;
            let popups = &mut self.popups;
            let response = self.runtime.update(
                owner,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |widget: &mut Widget| -> Result<SelectResponse> {
                    let Widget::Select(select, _) = widget else {
                        return Err("select expected".into());
                    };
                    Ok(select.key(&event, popups, popup, |id| id == owner)?)
                },
            )??;
            if response.handled {
                return Ok(());
            }
        }
        if event.key == Key::Named(NamedKey::Tab) && event.phase == KeyPhase::Down {
            let target = self
                .runtime
                .route_keyboard(self.window, event.clone(), &mut ())?;
            return self.focus(target, event.timestamp);
        }
        let Some(id) = self.runtime.focused(self.window)? else {
            return Ok(());
        };
        let status = self.runtime.update(
            id,
            Dirty::PAINT.union(Dirty::SEMANTICS),
            |widget: &mut Widget| -> Result<Option<String>> {
                match widget {
                    Widget::Button(button) => {
                        if button.key(&event) {
                            return Ok(Some("Apply activated once".into()));
                        }
                    }
                    Widget::Check(check) => {
                        check.key(&event)?;
                    }
                    Widget::Slider(slider) => {
                        slider.key(&event)?;
                    }
                    Widget::Number(number) => {
                        number.key(&event, clipboard)?;
                    }
                    Widget::Field(field, _) => {
                        field.key(&event, clipboard)?;
                    }
                    Widget::List(list) => {
                        list.key(&event)?;
                    }
                    Widget::Tree(tree) => {
                        tree.key(&event)?;
                    }
                    Widget::Table(table) => {
                        table.key(&event)?;
                    }
                    Widget::Radio(group) => {
                        group.key(&event)?;
                    }
                    Widget::Tabs(group) => {
                        group.key(&event)?;
                    }
                    _ => {}
                }
                Ok(None)
            },
        )??;
        if let Some(status) = status {
            self.status = status;
        }
        Ok(())
    }

    fn prepare_input(&mut self, scale: ScaleFactor, now: u64) -> Result<()> {
        // Preparing a widget mutably updates the runtime, so don't keep an
        // immutable borrow of the ID list across that update.
        for id in self.ids.clone() {
            self.prepare_input_for(id, scale, now)?;
        }
        Ok(())
    }
    fn prepare_input_for(&mut self, id: NodeId, scale: ScaleFactor, now: u64) -> Result<()> {
        let bounds = self.runtime.global_rect(id)?;
        let fonts = &mut self.fonts;
        self.runtime
            .update(id, Dirty::NONE, |widget: &mut Widget| -> Result<()> {
                match widget {
                    Widget::Field(field, style) => {
                        field.prepare(fonts, style, bounds, scale, now)?;
                    }
                    Widget::Number(number) => {
                        number
                            .field
                            .prepare(fonts, &Style::default(), bounds, scale, now)?;
                    }
                    _ => {}
                }
                Ok(())
            })??;
        Ok(())
    }
    fn pointer(&mut self, event: PointerEvent) -> Result<Option<NodeId>> {
        if !self.native_active && matches!(event.phase, PointerPhase::Down | PointerPhase::Up) {
            return Ok(None);
        }
        if let Some(popup) = self.popups.top().copied()
            && self
                .select_popup
                .is_some_and(|(_, content)| content == popup.content)
        {
            if popup.bounds.contains(event.position) {
                if event.phase == PointerPhase::Up && event.button == Some(PointerButton::Primary) {
                    let popups = &mut self.popups;
                    self.runtime.update(
                        popup.owner,
                        Dirty::PAINT.union(Dirty::SEMANTICS),
                        |widget: &mut Widget| -> Result<()> {
                            if let Widget::Select(select, _) = widget {
                                let range =
                                    select.visible_options(popup.bounds.size.height, 30.0)?;
                                let index = range.start
                                    + ((event.position.y - popup.bounds.top()) / 30.0) as usize;
                                if let Some(key) = select
                                    .options()
                                    .get(index)
                                    .filter(|option| !option.disabled)
                                    .map(|option| option.key.clone())
                                {
                                    select.activate(&key, popups, |id| id == popup.owner)?;
                                }
                            }
                            Ok(())
                        },
                    )??;
                }
                return Ok(None);
            }
            if event.phase == PointerPhase::Down {
                self.popups.outside(event.position, |id| id == popup.owner);
            }
        }
        let actions = self.runtime.route_pointer(self.window, event, &mut ())?;
        let mut context = None;
        for action in actions {
            let event = if matches!(action, RoutedPointerAction::Cancel(_)) {
                PointerEvent {
                    phase: PointerPhase::Cancel,
                    ..event
                }
            } else {
                event
            };
            let id = match action {
                RoutedPointerAction::Down { target, .. }
                | RoutedPointerAction::Up { target, .. }
                | RoutedPointerAction::Move { target, .. }
                | RoutedPointerAction::Scroll { target, .. } => target,
                RoutedPointerAction::Cancel(id) => id,
                _ => continue,
            };
            if id == self.root {
                continue;
            }
            if event.phase == PointerPhase::Down
                && self
                    .runtime
                    .snapshot()
                    .nodes
                    .iter()
                    .any(|n| n.id == id && n.focusable)
            {
                self.focus(Some(id), event.timestamp)?;
                if event.button == Some(PointerButton::Primary) {
                    self.runtime.capture_pointer(self.window, id)?;
                }
            }
            let bounds = self.runtime.global_rect(id)?;
            let mut activated = false;
            let popup_definition = self
                .select_popup
                .filter(|(owner, _)| *owner == id)
                .map(|(owner, content)| self.select_popup_definition(owner, content))
                .transpose()?;
            let popups = &mut self.popups;
            let response = self.runtime.update(
                id,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |widget: &mut Widget| -> Result<bool> {
                    match widget {
                        Widget::Button(button) => {
                            activated = button.pointer(event, bounds.contains(event.position));
                        }
                        Widget::Check(check) => {
                            check.pointer(event, bounds.contains(event.position))?;
                        }
                        Widget::Slider(slider) => {
                            slider.pointer(event, bounds)?;
                        }
                        Widget::Select(select, press) => {
                            if press.pointer(event, bounds.contains(event.position)) {
                                if select.is_open(popups) {
                                    select
                                        .close(popups, |id| id == popup_definition.unwrap().owner);
                                } else {
                                    select.open(
                                        popups,
                                        popup_definition.ok_or("missing select popup")?,
                                    )?;
                                }
                            }
                        }
                        Widget::Table(table) => {
                            if let PointerPhase::Scroll { delta_x, delta_y } = event.phase {
                                table.scroll_by(LogicalPoint::new(-delta_x, -delta_y));
                            } else if event.phase == PointerPhase::Down
                                && event.button == Some(PointerButton::Primary)
                                && event.position.y >= bounds.top() + table.row_height()
                            {
                                table.pointer(
                                    event.position - bounds.origin,
                                    event.modifiers.shift,
                                    event.modifiers.ctrl,
                                )?;
                            }
                        }
                        Widget::Tabs(group)
                            if event.phase == PointerPhase::Down
                                && event.button == Some(PointerButton::Primary)
                                && bounds.contains(event.position) =>
                        {
                            let index = ((event.position.x - bounds.left())
                                / (bounds.size.width / group.options().len() as f32))
                                as usize;
                            if let Some(option) =
                                group.options().get(index).filter(|option| !option.disabled)
                            {
                                let key = option.key.clone();
                                group.select(&key)?;
                            }
                        }
                        Widget::Radio(group) if event.phase == PointerPhase::Down => {
                            let index = ((event.position.x - bounds.left()) / 90.0) as usize;
                            if let Some(key) = group.options().get(index).map(|o| o.key.clone()) {
                                group.select(&key)?;
                            }
                        }
                        Widget::List(list) => {
                            if let PointerPhase::Scroll { delta_y, .. } = event.phase {
                                list.scroll.scroll_by(LogicalPoint::new(0.0, -delta_y));
                            } else if event.phase == PointerPhase::Down {
                                list.pointer_row(
                                    event.position.y - bounds.top(),
                                    event.modifiers.shift,
                                    event.modifiers.ctrl,
                                )?;
                            }
                        }
                        Widget::Tree(tree) => {
                            if event.phase == PointerPhase::Down {
                                tree.list.pointer_row(
                                    event.position.y - bounds.top(),
                                    event.modifiers.shift,
                                    event.modifiers.ctrl,
                                )?;
                            }
                        }
                        _ => {
                            let padding = match widget {
                                Widget::Field(_, style) => style.padding,
                                _ => Style::default().padding,
                            };
                            if let Some(field) = widget.field() {
                                return Ok(field.pointer(
                                    event,
                                    rect(
                                        bounds.left() + padding,
                                        bounds.top() + padding,
                                        bounds.size.width - padding * 2.0,
                                        bounds.size.height - padding * 2.0,
                                    ),
                                )? == TextResponse::ContextMenu);
                            }
                        }
                    }
                    Ok(false)
                },
            )??;
            if activated {
                self.status = "Apply activated once".into();
            }
            if response {
                context = Some(id);
            }
        }
        Ok(context)
    }
    fn accessibility(&mut self, action: RoutedAction) -> Result<()> {
        if let Some((owner, key, column)) = self.cells.get(&action.owner).cloned() {
            self.focus(Some(owner), 0)?;
            self.runtime.update(
                owner,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |widget: &mut Widget| -> Result<()> {
                    if let Widget::Table(table) = widget {
                        table.focus_cell(&key, column)?;
                    }
                    Ok(())
                },
            )??;
            return Ok(());
        }
        if let Some((owner, key)) = self.rows.get(&action.owner).cloned() {
            self.focus(Some(owner), 0)?;
            let popups = &mut self.popups;
            self.runtime.update(
                owner,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |widget: &mut Widget| -> Result<()> {
                    match widget {
                        Widget::List(list) => {
                            list.selection.select(&key, false, false)?;
                            list.reveal_focus();
                        }
                        Widget::Tree(tree) => match action.action {
                            ControlAction::Expand => tree.set_expanded(&key, true)?,
                            ControlAction::Collapse => tree.set_expanded(&key, false)?,
                            _ => {
                                tree.list.selection.select(&key, false, false)?;
                                tree.list.reveal_focus();
                            }
                        },
                        Widget::Radio(group) => group.select(&key)?,
                        Widget::Tabs(group) => group.select(&key)?,
                        Widget::Select(select, _) => {
                            select.activate(&key, popups, |id| id == owner)?;
                        }
                        _ => {}
                    }
                    Ok(())
                },
            )??;
            return Ok(());
        }
        if action.action == ControlAction::Focus {
            return self.focus(Some(action.owner), 0);
        }
        if let Some((owner, content)) = self.select_popup
            && action.owner == owner
            && matches!(
                action.action,
                ControlAction::Expand | ControlAction::Collapse
            )
        {
            self.focus(Some(owner), 0)?;
            let popup = self.select_popup_definition(owner, content)?;
            let popups = &mut self.popups;
            self.runtime.update(
                owner,
                Dirty::PAINT.union(Dirty::SEMANTICS),
                |widget: &mut Widget| -> Result<()> {
                    if let Widget::Select(select, _) = widget {
                        if action.action == ControlAction::Expand {
                            select.open(popups, popup)?;
                        } else {
                            select.close(popups, |id| id == owner);
                        }
                    }
                    Ok(())
                },
            )??;
            return Ok(());
        }
        let mut activated = false;
        self.runtime.update(
            action.owner,
            Dirty::PAINT.union(Dirty::SEMANTICS),
            |widget: &mut Widget| -> Result<()> {
                match action.action {
                    ControlAction::Click => match widget {
                        Widget::Button(button) => {
                            button.activate()?;
                            activated = true;
                        }
                        Widget::Check(check) => {
                            check.activate()?;
                        }
                        _ => {}
                    },
                    ControlAction::SetNumericValue(value) => {
                        if let Widget::Slider(slider) = widget {
                            slider.range.set(value)?;
                        }
                    }
                    ControlAction::SetValue(value) => {
                        if let Some(field) = widget.field() {
                            field.command(
                                TextCommand::SelectAll,
                                &mut MemoryClipboard::default(),
                                0,
                            )?;
                            field.insert_text(&value, 0)?;
                        }
                    }
                    ControlAction::SetSelection(selection) => {
                        if let Some(field) = widget.field() {
                            let revision = field.snapshot().text.revision();
                            field.editor_mut().select(revision, selection)?;
                        }
                    }
                    _ => {}
                }
                Ok(())
            },
        )??;
        if activated {
            self.status = "Apply activated once".into();
        }
        Ok(())
    }
}

struct Gpu {
    context: GpuContext,
    renderer: Renderer,
    surface: WindowSurface<'static>,
}
struct App {
    gallery: Gallery,
    gpu: Option<Gpu>,
    hidden: bool,
    backend: wgpu::Backends,
    started: Instant,
    next_blink: Instant,
    resize_soak: Option<u32>,
}
fn platform(error: impl std::fmt::Display) -> PlatformError {
    PlatformError::EventLoop(error.to_string())
}
impl App {
    fn draw(&mut self, shell: &mut PlatformShell) -> Result<()> {
        let window = shell
            .get_window(self.gallery.window)
            .ok_or("missing gallery window")?;
        let raw = window.shared_window();
        let size = raw.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let scale = window.dpi().scale_factor;
        let (snapshot, tree) = self.gallery.prepare(
            [size.width, size.height],
            scale,
            self.started.elapsed().as_millis() as u64,
        )?;
        if let Some(candidate) = self.gallery.candidate {
            ImeService::set_ime_allowed(&raw, true)?;
            ImeService::set_ime_cursor_area(&raw, candidate, scale)?;
        } else {
            ImeService::set_ime_allowed(&raw, false)?;
        }
        shell
            .accessibility_mut(self.gallery.window)
            .ok_or("missing accessibility adapter")?
            .publish(self.gallery.revision, tree)?;
        let gpu = self.gpu.as_mut().ok_or("missing GPU")?;
        gpu.context.device().poll(wgpu::PollType::Poll)?;
        // Finish uploads and draw preparation before resize can replace the
        // presented swapchain buffers. Keep the acquire-to-present gap short.
        let prepared = gpu.renderer.prepare(&snapshot)?;
        gpu.surface.resize(snapshot.size);
        match gpu.surface.acquire(&gpu.context)? {
            Acquire::Ready(frame) => {
                raw.pre_present_notify();
                frame.present(&gpu.context, prepared, Color([0.06, 0.075, 0.1, 1.0]))?;
            }
            Acquire::Retry | Acquire::Reconfigure => raw.request_redraw(),
            Acquire::Suspended => {}
        }
        Ok(())
    }
    fn event(&mut self, shell: &mut PlatformShell, event: PlatformWindowEvent) -> Result<()> {
        let window = shell
            .get_window(self.gallery.window)
            .ok_or("missing window")?
            .shared_window();
        let scale = ScaleFactor::new(window.scale_factor() as f32)?;
        let now = self.started.elapsed().as_millis() as u64;
        if !self.gallery.native_active
            && matches!(
                &event,
                PlatformWindowEvent::Keyboard(_)
                    | PlatformWindowEvent::TextInput(_)
                    | PlatformWindowEvent::Ime(_)
            )
        {
            return Ok(());
        }
        match &event {
            PlatformWindowEvent::Pointer(_) => self.gallery.prepare_input(scale, now)?,
            PlatformWindowEvent::Keyboard(key)
                if key.phase != KeyPhase::Up
                    && matches!(
                        key.key,
                        Key::Named(
                            NamedKey::ArrowLeft
                                | NamedKey::ArrowRight
                                | NamedKey::ArrowUp
                                | NamedKey::ArrowDown
                                | NamedKey::PageUp
                                | NamedKey::PageDown
                        )
                    ) =>
            {
                if let Some(id) = self.gallery.runtime.focused(self.gallery.window)? {
                    self.gallery.prepare_input_for(id, scale, now)?;
                }
            }
            _ => {}
        }
        match event {
            PlatformWindowEvent::CloseRequested => {
                shell.close_window(self.gallery.window)?;
                return Ok(());
            }
            // Paint the current client size before returning from native resize
            // delivery, rather than waiting for a separate redraw notification.
            PlatformWindowEvent::RedrawRequested | PlatformWindowEvent::Resized { .. } => {
                return self.draw(shell);
            }
            PlatformWindowEvent::Keyboard(event) => {
                #[cfg(windows)]
                {
                    let mut clipboard = view_platform::text::NativeClipboard::new(&window)?;
                    self.gallery.key(event, &mut clipboard)?;
                }
                #[cfg(not(windows))]
                {
                    self.gallery.key(event, &mut MemoryClipboard::default())?;
                }
            }
            PlatformWindowEvent::Pointer(event) => {
                if let Some(id) = self.gallery.pointer(event)? {
                    let commands =
                        self.gallery
                            .runtime
                            .update(id, Dirty::NONE, |w: &mut Widget| {
                                w.field().map(|f| f.context_commands()).unwrap_or_default()
                            })?;
                    let labels: Vec<_> = commands.iter().map(|c| format!("{c:?}")).collect();
                    let entries: Vec<_> = labels.iter().map(|s| (s.as_str(), true)).collect();
                    let screen = window.inner_position()?;
                    let scale = window.scale_factor();
                    let position = (
                        screen.x + (event.position.x as f64 * scale) as i32,
                        screen.y + (event.position.y as f64 * scale) as i32,
                    );
                    if let Some(index) =
                        view_platform::desktop::context_menu(&window, position, &entries)?
                    {
                        #[cfg(windows)]
                        {
                            let mut clipboard = view_platform::text::NativeClipboard::new(&window)?;
                            self.gallery.runtime.update(
                                id,
                                Dirty::PAINT,
                                |widget: &mut Widget| {
                                    if let Some(field) = widget.field() {
                                        field
                                            .command(
                                                commands[index],
                                                &mut clipboard,
                                                event.timestamp,
                                            )
                                            .map(|_| ())
                                    } else {
                                        Ok(())
                                    }
                                },
                            )??;
                        }
                        #[cfg(not(windows))]
                        let _ = index;
                    }
                }
            }
            PlatformWindowEvent::TextInput(text) => {
                self.gallery.native_text(&text, now)?;
            }
            PlatformWindowEvent::Ime(ime) => {
                self.gallery.native_ime(&ime, now)?;
            }
            PlatformWindowEvent::Focused(active) => {
                self.gallery.set_native_active(active, now)?;
                self.next_blink = Instant::now() + Duration::from_millis(500);
            }
            PlatformWindowEvent::FileDropped(path) => {
                self.gallery.status = format!(
                    "Dropped {} (application decides whether to open)",
                    path.display()
                )
            }
            _ => {}
        }
        window.request_redraw();
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
            let window = shell
                .create_window(
                    event_loop,
                    self.gallery.window,
                    PlatformWindow::default_attributes(
                        "view control gallery",
                        LogicalSize::new(900.0, 1000.0),
                    )
                    .with_visible(false)
                    .with_active(false),
                )?
                .shared_window();
            if self.hidden {
                let id = self
                    .gallery
                    .ids
                    .iter()
                    .copied()
                    .find(|id| {
                        matches!(
                            self.gallery.runtime.state::<Widget>(*id),
                            Ok(Widget::Field(..))
                        )
                    })
                    .ok_or("missing field")?;
                self.gallery.focus(Some(id), 0)?;
                self.gallery
                    .runtime
                    .update(id, Dirty::PAINT, |widget: &mut Widget| {
                        widget.field().unwrap().preedit("preedit", Some((7, 7)))
                    })??;
            } else {
                self.gallery.set_native_active(window.has_focus(), 0)?;
            }
            let (_, tree) = self.gallery.prepare([900, 1000], ScaleFactor::ONE, 0)?;
            shell.enable_accessibility(
                event_loop,
                self.gallery.window,
                self.gallery.revision,
                tree,
            )?;
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: self.backend,
                ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                    event_loop.owned_display_handle(),
                ))
            });
            let surface = instance.create_surface(window.clone())?;
            let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            }))??;
            println!("gallery adapter={:?}", adapter.get_info());
            let context = wait(GpuContext::request(
                &adapter,
                wgpu::Features::empty(),
                wgpu::Limits::default(),
            ))??;
            let surface = WindowSurface::new(surface, &adapter, &context, [900, 1000])?;
            let renderer = Renderer::new(context.clone(), surface.format(), Budgets::default())?;
            self.gpu = Some(Gpu {
                context,
                renderer,
                surface,
            });
            self.draw(shell)?;
            if self.hidden && self.resize_soak.is_none() {
                println!(
                    "PASS hidden gallery surface + AccessKit installation/update + IME candidate geometry service call; no native-client or physical-input certification"
                );
                shell.close_window(self.gallery.window)?;
                event_loop.exit();
            } else if !self.hidden {
                self.gallery.fonts = CosmicBackend::system(128);
                window.set_visible(true);
                window.request_redraw();
            }
            Ok(())
        };
        initialize().map_err(platform)
    }
    fn on_window_event(
        &mut self,
        shell: &mut PlatformShell,
        _: &ActiveEventLoop,
        _: WindowId,
        event: PlatformWindowEvent,
    ) -> std::result::Result<(), PlatformError> {
        if let Err(error) = self.event(shell, event) {
            self.gallery.status = error.to_string();
            eprintln!("gallery input: {error}");
        }
        Ok(())
    }
    fn on_idle(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> std::result::Result<(), PlatformError> {
        if let Some(count) = self.resize_soak {
            if count >= 1000 {
                event_loop.exit();
                return Ok(());
            }
            if self.started.elapsed() > Duration::from_secs(180) {
                return Err(platform("Resize soak timeout"));
            }
            let window = shell
                .get_window(self.gallery.window)
                .ok_or_else(|| platform("missing soak window"))?
                .shared_window();
            let phase = count % 400;
            let step = if phase < 200 { phase } else { 399 - phase };
            let _ =
                window.request_inner_size(winit::dpi::PhysicalSize::new(900 + step, 1000 + step));
            self.draw(shell).map_err(platform)?;
            let completed = count + 1;
            if completed.is_multiple_of(100) {
                println!(
                    "resize {completed}: {:?}",
                    self.gpu.as_ref().unwrap().renderer.cache_stats()
                );
            }
            self.resize_soak = Some(completed);
            if completed == 1000 {
                if !self.gpu.as_ref().unwrap().context.take_errors().is_empty() {
                    return Err(platform("GPU validation error"));
                }
                println!("PASS 1000 native resize/draw cycles");
                self.gpu = None;
                shell.close_window(self.gallery.window)?;
                event_loop.exit();
            } else {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
            }
            return Ok(());
        }
        let mut changed = false;
        if let Some(adapter) = shell.accessibility_mut(self.gallery.window) {
            for queued in adapter.drain()? {
                changed = true;
                match self.gallery.bridge.route(queued.revision, &queued.request) {
                    Ok(action) => self.gallery.accessibility(action).map_err(platform)?,
                    Err(error) => {
                        self.gallery.status = format!("Rejected accessibility action: {error}")
                    }
                }
            }
        }
        if changed && let Some(window) = shell.get_window(self.gallery.window) {
            window.request_redraw();
        }
        let deadline = if self.hidden {
            None
        } else {
            self.gallery
                .next_caret_blink(self.started.elapsed().as_millis() as u64)
                .map_err(platform)?
                .and_then(|millis| self.started.checked_add(Duration::from_millis(millis)))
        };
        if let Some(deadline) = deadline {
            if Instant::now() >= self.next_blink
                && let Some(window) = shell.get_window(self.gallery.window)
            {
                window.raw_window().request_redraw();
            }
            self.next_blink = deadline;
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.next_blink));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
        Ok(())
    }
}
fn profiles() -> Result<()> {
    std::fs::create_dir_all("target/verification")?;
    let mut report = String::new();
    for (name, mode) in [
        ("standard", LineMode::SingleLine),
        ("custom", LineMode::SingleLine),
        ("textarea", LineMode::Multiline),
    ] {
        let mut field = TextField::new(
            name,
            "",
            EditorConfig {
                mode,
                ..Default::default()
            },
        )?;
        let profile = if mode == LineMode::Multiline {
            Profile::PlainMultilineV1
        } else {
            Profile::SingleLineV1
        };
        let result = run_profile(&mut field, profile, name, std::env::consts::OS);
        if result
            .cases
            .iter()
            .any(|(_, r)| matches!(r, CaseResult::Failed(_)))
        {
            return Err("control profile failed".into());
        }
        report.push_str(&format!("{result:#?}\n"));
    }
    std::fs::write("target/verification/control-profiles.txt", report)?;
    Ok(())
}
fn main() -> Result<()> {
    let mode = std::env::args()
        .nth(1)
        .ok_or("choose --headless, --gpu, --run-hidden or --show")?;
    profiles()?;
    let mut gallery = Gallery::new()?;
    if mode == "--headless" {
        let (paint, tree) = gallery.prepare([900, 1000], ScaleFactor::ONE, 0)?;
        assert!(paint.items.len() > 30);
        assert!(tree.nodes.len() > 15);
        println!(
            "PASS gallery CPU preparation and profile reports; native/render requirements uncovered"
        );
        return Ok(());
    }
    let backend = backend(&std::env::args().nth(2).ok_or("backend required")?)?;
    if mode == "--gpu" {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = wait(instance.request_adapter(&Default::default()))??;
        println!("gallery adapter={:?}", adapter.get_info());
        let context = wait(GpuContext::request(
            &adapter,
            wgpu::Features::empty(),
            wgpu::Limits::default(),
        ))??;
        let mut renderer = Renderer::new(
            context.clone(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Budgets::default(),
        )?;
        let id = gallery
            .ids
            .iter()
            .copied()
            .find(|id| matches!(gallery.runtime.state::<Widget>(*id), Ok(Widget::Field(..))))
            .unwrap();
        gallery.focus(Some(id), 0)?;
        gallery
            .runtime
            .update(id, Dirty::PAINT, |widget: &mut Widget| {
                let field = widget.field().unwrap();
                field.editor_mut().select_all()?;
                field.preedit("preedit selection", Some((0, 7)))
            })??;
        for scale in [1.0, 1.5, 2.0] {
            let size = [(900.0 * scale) as u32, (1000.0 * scale) as u32];
            let (snapshot, _) = gallery.prepare(size, ScaleFactor::new(scale)?, 0)?;
            let target = context.target(size)?;
            let readback = renderer
                .prepare(&snapshot)?
                .submit_capture(&context, &target, Color([0.06, 0.075, 0.1, 1.0]))?
                .readback(&context)?;
            assert_eq!(readback.revision, snapshot.revision);
            assert!(
                readback
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[0] > 100)
                    .count()
                    > 1000
            );
            write_bmp(
                &format!("target/verification/gallery-{scale}.bmp"),
                &readback,
            )?;
            gallery
                .runtime
                .update(id, Dirty::PAINT, |widget: &mut Widget| {
                    widget
                        .field()
                        .unwrap()
                        .preedit("preedit selection", Some((7, 7)))
                })??;
            let (cursor, _) = gallery.prepare(size, ScaleFactor::new(scale)?, 0)?;
            let candidate = gallery.candidate.ok_or("missing focused candidate")?;
            let x = ((candidate.left() + 0.5) * scale) as usize;
            let y = ((candidate.top() + candidate.size.height * 0.5) * scale) as usize;
            let offset = (y * size[0] as usize + x) * 4;
            let visible = renderer
                .prepare(&cursor)?
                .submit_capture(&context, &target, Color([0.06, 0.075, 0.1, 1.0]))?
                .readback(&context)?;
            gallery
                .runtime
                .update(id, Dirty::PAINT, |widget: &mut Widget| {
                    widget
                        .field()
                        .unwrap()
                        .editor_mut()
                        .set_composition_cursor_visible(false)
                })?;
            let (hidden, _) = gallery.prepare(size, ScaleFactor::new(scale)?, 0)?;
            let hidden = renderer
                .prepare(&hidden)?
                .submit_capture(&context, &target, Color([0.06, 0.075, 0.1, 1.0]))?
                .readback(&context)?;
            assert!(
                visible.rgba[offset] > hidden.rgba[offset].saturating_add(100),
                "caret must change pixels at the shared candidate geometry"
            );
            gallery
                .runtime
                .update(id, Dirty::PAINT, |widget: &mut Widget| {
                    widget
                        .field()
                        .unwrap()
                        .preedit("preedit selection", Some((0, 7)))
                })??;
        }
        assert!(context.take_errors().is_empty());
        println!(
            "PASS gallery GPU readback at 100%, 150%, 200%; selection/preedit/caret use shared geometry"
        );
        return Ok(());
    }
    if mode != "--show" && mode != "--run-hidden" && mode != "--resize-soak" {
        return Err("unknown gallery mode".into());
    }
    let hidden = mode != "--show";
    PlatformApp::new(
        App {
            gallery,
            gpu: None,
            hidden,
            backend,
            started: Instant::now(),
            next_blink: Instant::now() + Duration::from_millis(500),
            resize_soak: (mode == "--resize-soak").then_some(0),
        },
        AppConfig {
            run_hidden: hidden,
            max_iterations: if mode == "--run-hidden" {
                Some(2)
            } else {
                None
            },
        },
    )?
    .run()?;
    Ok(())
}
fn write_bmp(path: &str, readback: &view_wgpu::Readback) -> Result<()> {
    let [width, height] = readback.size;
    let bytes = width * height * 4;
    let mut out = vec![];
    out.extend(b"BM");
    out.extend((54 + bytes).to_le_bytes());
    out.extend([0; 4]);
    out.extend(54u32.to_le_bytes());
    out.extend(40u32.to_le_bytes());
    out.extend(width.to_le_bytes());
    out.extend((-(height as i32)).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend([0; 4]);
    out.extend(bytes.to_le_bytes());
    out.extend([0; 16]);
    for pixel in readback.rgba.as_chunks::<4>().0 {
        out.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    std::fs::write(path, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pointer(phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            phase,
            position: LogicalPoint::new(40.0, 80.0),
            button: Some(PointerButton::Primary),
            modifiers: Modifiers::default(),
            timestamp: 0,
        }
    }
    #[test]
    fn gallery_pointer_activation_and_revoked_capture_use_routed_intent() {
        let mut gallery = Gallery::new().unwrap();
        gallery.pointer(pointer(PointerPhase::Down)).unwrap();
        gallery.pointer(pointer(PointerPhase::Up)).unwrap();
        assert_eq!(gallery.status, "Apply activated once");
        let id = gallery.runtime.focused(gallery.window).unwrap().unwrap();
        gallery.pointer(pointer(PointerPhase::Down)).unwrap();
        gallery
            .runtime
            .set_visibility(id, Visibility::Hidden)
            .unwrap();
        gallery.pointer(pointer(PointerPhase::Move)).unwrap();
        let Widget::Button(button) = gallery.runtime.state::<Widget>(id).unwrap() else {
            panic!("button expected")
        };
        assert!(!button.behavior.state().pressed);
    }

    #[test]
    fn gallery_native_blur_retains_focus_rejects_late_edits_and_masks_repeated_paints() {
        let mut gallery = Gallery::new().unwrap();
        let editor = *gallery.ids.iter().find(|id| matches!(gallery.runtime.state::<Widget>(**id), Ok(Widget::Field(field, _)) if !field.editor().read_only())).unwrap();
        gallery.focus(Some(editor), 0).unwrap();
        gallery.native_text("before", 0).unwrap();
        gallery
            .native_ime(
                &view_platform::ImeEvent::Preedit("pending".into(), Some((7, 7))),
                1,
            )
            .unwrap();
        gallery.set_native_active(false, 2).unwrap();
        assert_eq!(
            gallery.runtime.focused(gallery.window).unwrap(),
            Some(editor)
        );
        assert!(gallery.next_caret_blink(0).unwrap().is_none());
        let before = match gallery.runtime.state::<Widget>(editor).unwrap() {
            Widget::Field(field, _) => field.snapshot(),
            _ => unreachable!(),
        };
        assert!(before.composition.is_none());
        for now in [3, 510, 1010] {
            let (_, tree) = gallery.prepare([900, 1000], ScaleFactor::ONE, now).unwrap();
            assert_eq!(tree.focus, gallery.bridge.id(gallery.root).unwrap());
            assert!(gallery.candidate.is_none());
        }
        gallery.native_text("late", 1011).unwrap();
        gallery
            .native_ime(&view_platform::ImeEvent::Preedit("late".into(), None), 1012)
            .unwrap();
        gallery
            .native_ime(&view_platform::ImeEvent::Commit("late".into()), 1013)
            .unwrap();
        gallery
            .key(
                KeyboardEvent {
                    key: Key::Named(NamedKey::Backspace),
                    phase: KeyPhase::Down,
                    modifiers: Modifiers::default(),
                    timestamp: 1014,
                },
                &mut view_text::MemoryClipboard::default(),
            )
            .unwrap();
        match gallery.runtime.state::<Widget>(editor).unwrap() {
            Widget::Field(field, _) => {
                assert_eq!(field.snapshot(), before);
                assert!(!field.focused());
            }
            _ => unreachable!(),
        }
        gallery.set_native_active(true, 1020).unwrap();
        let (_, tree) = gallery
            .prepare([900, 1000], ScaleFactor::ONE, 1021)
            .unwrap();
        assert_eq!(tree.focus, gallery.bridge.id(editor).unwrap());
        assert!(gallery.candidate.is_some());
        assert!(gallery.next_caret_blink(0).unwrap().is_some());
        gallery.native_text("after", 1022).unwrap();
        match gallery.runtime.state::<Widget>(editor).unwrap() {
            Widget::Field(field, _) => {
                assert!(field.snapshot().text.text().starts_with("beforeafter"))
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn gallery_inactive_hover_and_wheel_survive_but_held_press_cannot_activate() {
        let mut gallery = Gallery::new().unwrap();
        gallery.pointer(pointer(PointerPhase::Down)).unwrap();
        let button = gallery.runtime.focused(gallery.window).unwrap().unwrap();
        gallery.set_native_active(false, 1).unwrap();
        assert_eq!(gallery.runtime.captured(gallery.window).unwrap(), None);
        gallery.pointer(pointer(PointerPhase::Move)).unwrap();
        let Widget::Button(state) = gallery.runtime.state::<Widget>(button).unwrap() else {
            panic!("button")
        };
        assert!(state.behavior.state().hovered);
        assert!(!state.behavior.state().pressed && !state.behavior.state().focused);
        gallery.pointer(pointer(PointerPhase::Up)).unwrap();
        gallery.set_native_active(true, 2).unwrap();
        gallery.pointer(pointer(PointerPhase::Up)).unwrap();
        assert_ne!(gallery.status, "Apply activated once");
        gallery.set_native_active(false, 3).unwrap();
        let list = *gallery
            .ids
            .iter()
            .find(|id| matches!(gallery.runtime.state::<Widget>(**id), Ok(Widget::List(_))))
            .unwrap();
        let bounds = gallery.runtime.global_rect(list).unwrap();
        gallery
            .pointer(PointerEvent {
                phase: PointerPhase::Scroll {
                    delta_x: 0.0,
                    delta_y: -48.0,
                },
                position: bounds.origin + LogicalPoint::new(20.0, 20.0),
                ..pointer(PointerPhase::Move)
            })
            .unwrap();
        let Widget::List(state) = gallery.runtime.state::<Widget>(list).unwrap() else {
            panic!("list")
        };
        assert_eq!(state.scroll.offset.y, 48.0);
        assert_eq!(
            gallery.runtime.focused(gallery.window).unwrap(),
            Some(button)
        );
        assert!(gallery.next_caret_blink(0).unwrap().is_none());
    }
}

#[cfg(test)]
mod tab_gallery_tests {
    use super::*;
    #[test]
    fn tabs_switch_panel_and_publish_disabled_and_selected_accessibility() {
        let mut gallery = Gallery::new().unwrap();
        let id = *gallery
            .ids
            .iter()
            .find(|id| matches!(gallery.runtime.state::<Widget>(**id), Ok(Widget::Tabs(_))))
            .unwrap();
        gallery.focus(Some(id), 0).unwrap();
        gallery
            .key(
                KeyboardEvent {
                    key: Key::Named(NamedKey::ArrowRight),
                    phase: KeyPhase::Down,
                    modifiers: Modifiers::default(),
                    timestamp: 1,
                },
                &mut MemoryClipboard::default(),
            )
            .unwrap();
        let (paint, tree) = gallery.prepare([900, 1000], ScaleFactor::ONE, 1).unwrap();
        assert!(!paint.items.is_empty());
        let Widget::Tabs(tabs) = gallery.runtime.state::<Widget>(id).unwrap() else {
            panic!()
        };
        assert_eq!(tabs.selected(), "keyboard");
        let nodes: Vec<_> = tree.nodes.iter().map(|(_, node)| node).collect();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.role() == accesskit::Role::Tab)
                .count(),
            4
        );
        assert!(
            nodes
                .iter()
                .any(|node| node.role() == accesskit::Role::Tab && node.is_disabled())
        );
        assert!(
            nodes
                .iter()
                .any(|node| node.role() == accesskit::Role::TabPanel
                    && node.label().is_some_and(|label| label.contains("Home/End")))
        );
        gallery
            .key(
                KeyboardEvent {
                    key: Key::Named(NamedKey::ArrowRight),
                    phase: KeyPhase::Down,
                    modifiers: Modifiers::default(),
                    timestamp: 2,
                },
                &mut MemoryClipboard::default(),
            )
            .unwrap();
        let Widget::Tabs(tabs) = gallery.runtime.state::<Widget>(id).unwrap() else {
            panic!()
        };
        assert_eq!(tabs.selected(), "coverage");
    }
}
