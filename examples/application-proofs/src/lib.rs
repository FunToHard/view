//! Small executable application proofs. Documents and service reducers stay outside the UI.
#![forbid(unsafe_code)]
#[cfg(feature = "native")]
pub mod native;
pub mod presentation;
pub mod service;
use std::{sync::Arc, time::Duration};
use view::{
    authoring::*,
    resources::{self, Policy, Request, Resource, Status},
    scene::*,
    viewport::*,
    *,
};
use view_markdown::{Limits as MarkdownLimits, MarkdownPreview};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug)]
pub enum Action {
    Navigate(Page),
    ProductDetail(u64),
    Select(u64),
    Move { base: Revision, x: f32 },
    Undo,
    Query(String),
    OpenLink(String),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Store,
    Library,
    Downloads,
}
#[derive(Default)]
pub struct Document {
    pub page: Page,
    pub product: Option<u64>,
    pub selected: Option<u64>,
    pub x: f32,
    pub revision: Revision,
    undo: Vec<f32>,
    pub source: String,
    pub opened_link: Option<String>,
}
impl Document {
    fn apply(&mut self, action: Action) -> Dirty {
        match action {
            Action::Navigate(page) => self.page = page,
            Action::ProductDetail(id) => self.product = Some(id),
            Action::Select(id) => self.selected = Some(id),
            Action::Move { base, x } if base == self.revision && x.is_finite() => {
                if self.undo.len() == 128 {
                    self.undo.remove(0);
                }
                self.undo.push(self.x);
                self.x = x;
                self.revision = self.revision.checked_next().expect("document revision");
            }
            Action::Undo => {
                if let Some(x) = self.undo.pop() {
                    self.x = x;
                    self.revision = self.revision.checked_next().expect("document revision");
                }
            }
            Action::OpenLink(link) => self.opened_link = Some(link),
            _ => return Dirty::NONE,
        }
        Dirty::BUILD.union(Dirty::VIEWPORT)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub image: String,
}
struct CatalogState;
struct CardState;
struct InspectorState;

pub struct Proofs {
    pub preview_x: Option<f32>,
    pub scene_revision: Revision,
    pub ui: Runtime<Document, Action>,
    pub document: Document,
    pub editor: WindowId,
    pub detached: WindowId,
    pub store: WindowId,
    pub editor_root: NodeId,
    pub detached_root: NodeId,
    pub store_root: NodeId,
    pub viewport: NodeId,
    pub inspector: NodeId,
    pub search: NodeId,
    pub catalog: NodeId,
    pub gizmo: NodeId,
    pub results: Resource<Vec<Product>>,
    pub preview: Resource<String>,
    pub images: resources::assets::Cache<resources::assets::Image>,
    pub markdown: MarkdownPreview,
    pub query: String,
    pub download: f32,
    pub frames: ViewportScheduler,
    pub sections: Vec<resources::sections::LocalSection<Action>>,
}
impl Proofs {
    pub fn new() -> Result<Self> {
        let mut ui = Runtime::new(64)?;
        let window = |key| {
            Description::new(key, (), |_, m: &mut Document, a| m.apply(a))
                .structure(Structure::Declarative)
                .layout(LogicalRect::from_xywh(0., 0., 800., 600.))
                .role(Role::Window)
        };
        let (editor, editor_root) = ui.open_window(window("editor"))?;
        let (detached, detached_root) = ui.open_window(window("detached-inspector-ide"))?;
        let (store, store_root) = ui.open_window(window("storefront"))?;
        let scene = Description::new("viewport", (), |_, m: &mut Document, a| m.apply(a))
            .role(Role::Custom("scene".into()))
            .name("Scene: object 7")
            .focusable(true)
            .layout(LogicalRect::from_xywh(240., 20., 320., 320.));
        let inspector = Description::new("inspector", InspectorState, |_, m: &mut Document, a| {
            m.apply(a)
        })
        .structure(Structure::Immediate)
        .layout(LogicalRect::from_xywh(20., 360., 700., 100.));
        let ids = reconcile(
            &mut ui,
            editor_root,
            (
                Column::new(
                    "hierarchy",
                    (
                        Text::new("title", "Hierarchy"),
                        Description::new("object-7", (), |_, m: &mut Document, a| m.apply(a))
                            .role(Role::Button)
                            .name("Object 7")
                            .focusable(true),
                    ),
                ),
                scene,
                inspector,
            ),
        )?;
        let viewport = ids[1];
        let inspector = ids[2];
        let gizmo = ui.mount(
            viewport,
            Description::new("gizmo", (), |_, m: &mut Document, a| m.apply(a))
                .name("Move selected object")
                .role(Role::Button)
                .focusable(true)
                .layout(LogicalRect::from_xywh(154., 154., 12., 12.)),
        )?;
        ui.set_region_builder(inspector, |m, _| {
            vec![
                Description::new("transform", (), |_, m: &mut Document, a| m.apply(a))
                    .name(format!("Selected {:?} — X {}", m.selected, m.x))
                    .role(Role::StaticText),
            ]
        })?;
        let text = controls::TextInput::new("Search catalog", "")?.0;
        // Search state is a retained island; catalog replacement cannot reset editing.
        let search = Description::new("search", text, |_, _, _: Action| Dirty::NONE)
            .focusable(true)
            .role(Role::TextInput)
            .name("Search catalog")
            .layout(LogicalRect::from_xywh(20., 40., 700., 32.));
        let catalog = Description::new("catalog", CatalogState, |_, _, _| Dirty::NONE)
            .structure(Structure::Declarative)
            .layout(LogicalRect::from_xywh(20., 90., 700., 480.))
            .role(Role::ScrollArea);
        let ids = reconcile(
            &mut ui,
            store_root,
            (
                Description::new("navigation", (), |_, _, _| Dirty::NONE)
                    .into_element()
                    .children(
                        ["store", "library", "downloads"]
                            .into_iter()
                            .enumerate()
                            .map(|(i, name)| {
                                Description::new(
                                    format!("nav-{name}"),
                                    (),
                                    |_, m: &mut Document, a| m.apply(a),
                                )
                                .role(Role::Button)
                                .name(name)
                                .focusable(true)
                                .layout(LogicalRect::from_xywh(
                                    20. + i as f32 * 150.,
                                    0.,
                                    140.,
                                    30.,
                                ))
                                .into_element()
                            })
                            .collect::<Vec<_>>(),
                    ),
                search,
                catalog,
            ),
        )?;
        let search = ids[1];
        let catalog = ids[2];
        ui.set_scroll_state(
            catalog,
            ScrollState::new(LogicalSize::new(700., 480.), LogicalSize::new(700., 480.)),
        )?;
        let lifetime = ui
            .own(store_root, ResourceKind::Subscription, || {})?
            .cancellation();
        let mut results = Resource::new(
            store_root,
            lifetime,
            Policy {
                reconnect: true,
                ..Policy::default()
            },
            4,
        )?;
        results.subscribe(catalog)?;
        let lifetime = ui
            .own(detached_root, ResourceKind::Subscription, || {})?
            .cancellation();
        let preview = Resource::new(detached_root, lifetime, Policy::default(), 2)?;
        let markdown = MarkdownPreview::new(
            "# Shared document\n[Project](https://example.test/view)",
            MarkdownLimits::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut document = Document {
            source: "# Shared document".into(),
            ..Default::default()
        };
        reconcile(
            &mut ui,
            detached_root,
            (
                Text::new("inspector-title", "Detached inspector / IDE preview"),
                source_control(&document.source)?,
                markdown
                    .element("preview", |m: &mut Document, a| m.apply(a))
                    .layout(LogicalRect::from_xywh(400., 70., 380., 450.)),
            ),
        )?;
        ui.flush(&mut document)?;
        let mut frames = ViewportScheduler::new(1);
        frames.register(viewport, FramePolicy::OnDemand)?;
        Ok(Self {
            preview_x: None,
            scene_revision: Revision::INITIAL,
            ui,
            document,
            editor,
            detached,
            store,
            editor_root,
            detached_root,
            store_root,
            viewport,
            inspector,
            search,
            catalog,
            gizmo,
            results,
            preview,
            images: resources::assets::Cache::new(16, 1024 * 1024),
            markdown,
            query: String::new(),
            download: 0.,
            frames,
            sections: vec![],
        })
    }
    pub fn dispatch(&mut self, action: Action) -> Result<()> {
        let owner = [self.editor_root, self.detached_root, self.store_root]
            .into_iter()
            .find(|id| self.ui.dirty(*id).is_ok())
            .ok_or("all windows closed")?;
        self.ui.enqueue(owner, action).map_err(|r| r.error)?;
        self.ui.flush(&mut self.document)?;
        self.scene_revision = self.scene_revision.checked_next()?;
        if self.ui.dirty(self.inspector).is_ok() {
            self.ui.invalidate(self.inspector, Dirty::BUILD)?;
        }
        if self.ui.dirty(self.gizmo).is_ok() {
            self.ui.set_layout(
                self.gizmo,
                LogicalRect::from_xywh(154. + self.document.x * 80., 154., 12., 12.),
            )?;
        }
        self.frames.request(self.viewport)?;
        self.refresh_detached()?;
        if self.ui.dirty(self.catalog).is_ok() {
            self.refresh_catalog()?;
        }
        self.ui.flush(&mut self.document)?;
        Ok(())
    }
    fn refresh_detached(&mut self) -> Result<()> {
        if self.ui.dirty(self.detached_root).is_err() {
            return Ok(());
        }
        let focused = self.ui.focused(self.detached)?;
        let fallback = self.editing_node(self.detached);
        reconcile(
            &mut self.ui,
            self.detached_root,
            (
                Text::new(
                    "inspector-title",
                    format!(
                        "Selected {:?}; X {}",
                        self.document.selected, self.document.x
                    ),
                ),
                source_control(&self.document.source)?,
                self.markdown
                    .element("preview", |m: &mut Document, a| m.apply(a))
                    .layout(LogicalRect::from_xywh(400., 70., 380., 450.)),
            ),
        )?;
        if focused.is_some_and(|id| self.ui.dirty(id).is_err()) {
            self.ui.set_focus(self.detached, fallback)?;
        }
        Ok(())
    }
    pub fn search(&mut self, query: &str) -> Result<Request> {
        self.query = query.into();
        let request = self.results.begin(query)?;
        self.refresh_catalog()?;
        Ok(request)
    }
    pub fn reply(&mut self, request: &Request, items: Vec<Product>) -> Result<bool> {
        let mut ids = std::collections::HashSet::new();
        if items.iter().any(|p| !ids.insert(p.id)) {
            return Err("duplicate product identity".into());
        }
        if items.len() > 10000
            || items
                .iter()
                .any(|p| p.name.len() > 256 || p.image.len() > 2048)
        {
            return Err("catalog limit".into());
        }
        let accepted = self.results.complete(request, items)?;
        if accepted {
            self.refresh_catalog()?;
        }
        Ok(accepted)
    }
    pub fn refresh_catalog(&mut self) -> Result<()> {
        let mut children = vec![];
        // Keep content keyed through refresh/error/offline, and virtualize visible cards.
        if self.document.page != Page::Downloads
            && let Some(items) = self.results.value()
        {
            let mut scroll = self.ui.scroll_state(self.catalog)?.expect("catalog scroll");
            scroll.content_size = LogicalSize::new(700., (items.len() as f32 * 40.).max(480.));
            scroll.scroll_to(scroll.offset);
            self.ui.set_scroll_state(self.catalog, scroll)?;
            let offset = self
                .ui
                .scroll_state(self.catalog)?
                .map_or(0., |s| s.offset.y);
            let start = (offset / 40.).floor().max(0.) as usize;
            for (index, p) in items.iter().enumerate().skip(start).take(14) {
                children.push(
                    Description::new(format!("product-{}", p.id), CardState, |_, _, _: Action| {
                        Dirty::NONE
                    })
                    .role(Role::Button)
                    .name(&p.name)
                    .focusable(true)
                    .layout(LogicalRect::from_xywh(0., index as f32 * 40., 680., 36.))
                    .into_element(),
                );
            }
            if items.is_empty() {
                children.push(Text::new("empty", "No results").into_element());
            }
        }
        if self.results.status() != Status::Ready {
            children
                .push(Text::new("status", format!("{:?}", self.results.status())).into_element());
        }
        if self.document.page == Page::Downloads {
            children.push(
                Text::new("download", format!("Download {:.0}%", self.download * 100.))
                    .into_element(),
            );
        }
        for section in &self.sections {
            children.push(
                Description::new(
                    format!("section-{}", section.key),
                    (),
                    |_, m: &mut Document, a| m.apply(a),
                )
                .role(if section.action.is_some() {
                    Role::Button
                } else {
                    Role::StaticText
                })
                .name(&section.text)
                .focusable(section.action.is_some())
                .layout(LogicalRect::from_xywh(420., 0., 260., 28.))
                .into_element(),
            );
        }
        let focused = self.ui.focused(self.store)?;
        reconcile(&mut self.ui, self.catalog, children)?;
        if focused.is_some_and(|id| self.ui.dirty(id).is_err()) {
            self.ui.set_focus(self.store, Some(self.search))?;
        }
        self.results.notify(&mut self.ui)?;
        self.ui.flush(&mut self.document)?;
        Ok(())
    }
    pub fn scene(&self) -> Result<SceneSnapshot> {
        Ok(SceneSnapshot::new(
            self.scene_revision,
            PerspectiveCamera::look_at(
                [0., 0., 0.],
                [0., 0., -1.],
                [0., 1., 0.],
                std::f32::consts::FRAC_PI_2,
                1.,
                10.,
            )?,
            vec![SceneObject {
                id: 7,
                mesh: Arc::new(Mesh3D::new(
                    vec![[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]],
                    vec![0, 1, 2],
                )?),
                transform: Transform3D::translation([
                    self.preview_x.unwrap_or(self.document.x),
                    0.,
                    -2.,
                ])?,
                material: BasicMaterial::new([0.2, 0.7, 0.3])?,
            }],
        )?)
    }
    pub fn commit_drag(&mut self, commit: DragCommit<f32>) -> Result<()> {
        self.set_preview(None)?;
        self.dispatch(Action::Move {
            base: commit.revision,
            x: commit.after,
        })
    }
    pub fn set_preview(&mut self, value: Option<f32>) -> Result<()> {
        if value.is_some_and(|v| !v.is_finite()) {
            return Err("invalid preview".into());
        }
        if self.preview_x != value {
            self.scene_revision = self.scene_revision.checked_next()?;
            self.preview_x = value;
        }
        Ok(())
    }
    pub fn activate(&mut self, target: NodeId) -> Result<()> {
        let node = self
            .ui
            .snapshot()
            .nodes
            .iter()
            .find(|n| n.id == target)
            .ok_or("stale activation")?;
        let action = match node.key.as_str() {
            "nav-store" => Some(Action::Navigate(Page::Store)),
            "nav-library" => Some(Action::Navigate(Page::Library)),
            "nav-downloads" => Some(Action::Navigate(Page::Downloads)),
            "object-7" => Some(Action::Select(7)),
            key if key.starts_with("product-") => Some(Action::ProductDetail(key[8..].parse()?)),
            key if key.starts_with("section-") => self
                .sections
                .iter()
                .find(|s| s.key == key[8..])
                .and_then(|s| s.action.clone()),
            key if node.role == Some(Role::Custom("link".into())) => {
                let index = key.rsplit('-').next().ok_or("link key")?.parse()?;
                Some(Action::OpenLink(
                    self.markdown
                        .activate(index, self.markdown.revision())
                        .map_err(|e| format!("{e:?}"))?
                        .target,
                ))
            }
            _ => None,
        };
        if let Some(action) = action {
            self.dispatch(action)?;
        }
        Ok(())
    }
    pub fn pick(&mut self, point: LogicalPoint) -> Result<bool> {
        let scene = self.scene()?;
        let mut picks = PickTracker::new()?;
        let stamp = PickStamp {
            scene: scene.revision(),
            camera: Revision::INITIAL,
            viewport: Revision::INITIAL,
        };
        let request = picks.request(stamp, point, LogicalSize::new(320., 320.))?;
        let result = scene.pick(request)?;
        if let Some(hit) = picks.accept(result, stamp).flatten() {
            self.dispatch(Action::Select(hit.object))?;
            return Ok(true);
        }
        Ok(false)
    }
    /// Worker parses/loads source outside build; only the current result updates the preview.
    pub fn source_reply(&mut self, request: &Request, source: String) -> Result<bool> {
        if !self.preview.is_current(request) {
            return Ok(false);
        }
        if source.len() > 256 * 1024 {
            return Err("source limit".into());
        }
        MarkdownPreview::new(&source, MarkdownLimits::default()).map_err(|e| format!("{e:?}"))?;
        if !self.preview.complete(request, source)? {
            return Ok(false);
        }
        let source = self.preview.value().expect("accepted");
        self.markdown
            .update(source, self.markdown.revision())
            .map_err(|e| format!("{e:?}"))?;
        self.document.source = source.clone();
        if let Some(field) = self.editing_node(self.detached) {
            self.ui
                .update(field, Dirty::PAINT, |field: &mut controls::TextField| {
                    field.external(
                        field.snapshot().text.revision(),
                        source,
                        text::ExternalPolicy::PreserveSelection,
                    )
                })??;
        }
        self.refresh_detached()?;
        self.ui.flush(&mut self.document)?;
        Ok(true)
    }
    pub fn editing_node(&self, window: WindowId) -> Option<NodeId> {
        if window == self.store && self.ui.dirty(self.search).is_ok() {
            return Some(self.search);
        }
        if window == self.detached {
            return self
                .ui
                .snapshot()
                .nodes
                .iter()
                .find(|n| n.window == window && n.key == "source")
                .map(|n| n.id);
        }
        None
    }
    pub fn edited(&mut self, window: WindowId) -> Result<()> {
        if let Some(id) = self.editing_node(window) {
            let value = self
                .ui
                .state::<controls::TextField>(id)?
                .snapshot()
                .text
                .text()
                .to_owned();
            if window == self.store {
                self.query = value;
            } else if self.document.source != value {
                self.document.source = value;
                self.preview.cancel()?;
            }
        }
        Ok(())
    }
    pub fn verify(&mut self) -> Result<()> {
        let a = self.search("old")?;
        let b = self.search("new")?;
        let items = vec![Product {
            id: 1,
            name: "Newest game".into(),
            image: "cover".into(),
        }];
        assert!(self.reply(&b, items.clone())?);
        assert!(!self.reply(&a, vec![])?);
        let card = self
            .ui
            .snapshot()
            .nodes
            .iter()
            .find(|n| n.key == "product-1")
            .unwrap()
            .id;
        self.ui.set_focus(self.store, Some(card))?;
        let refresh = self.search("new")?;
        assert_eq!(self.ui.focused(self.store)?, Some(card));
        self.reply(&refresh, items)?;
        self.results.offline()?;
        self.refresh_catalog()?;
        let reconnect = self.results.reconnect()?.unwrap();
        self.reply(&reconnect, vec![])?;
        assert_eq!(self.ui.focused(self.store)?, Some(self.search));
        assert!(self.pick(LogicalPoint::new(160., 160.))?);
        let base = self.document.revision;
        self.dispatch(Action::Move { base, x: 0.5 })?;
        self.dispatch(Action::Undo)?;
        assert_eq!(self.document.x, 0.);
        let a = self.preview.begin("source")?;
        let b = self.preview.begin("source")?;
        assert!(self.source_reply(&b, "# Updated\n[Open](https://example.test)".into())?);
        assert!(!self.source_reply(&a, "old".into())?);
        self.ui.focus_next(self.detached)?;
        let before = self.ui.counters();
        for _ in 0..20 {
            self.ui.flush(&mut self.document)?;
        }
        assert_eq!(self.ui.counters().builds, before.builds);
        self.frames
            .poll(self.ui.snapshot(), Duration::ZERO, |_| true)?;
        self.load_fixture_assets()?;
        Ok(())
    }
    pub fn load_fixture_assets(&mut self) -> Result<()> {
        use resources::sections::*;
        let registry = Registry::new(
            1,
            ["featured".into()],
            [("select".into(), Action::Select(7))].into(),
            ["cover".into()].into(),
            Limits {
                depth: 3,
                items: 16,
                text_bytes: 4096,
            },
        );
        self.sections = registry
            .compile(
                1,
                &[Section {
                    key: "featured".into(),
                    control: "featured".into(),
                    mandatory: true,
                    text: "Featured games".into(),
                    action: Some("select".into()),
                    asset: Some("cover".into()),
                    children: vec![],
                }],
            )
            .map_err(|e| format!("{e:?}"))?;
        let image = std::thread::spawn(|| {
            resources::assets::Image::rgba(
                2,
                2,
                vec![
                    40, 160, 230, 255, 80, 200, 100, 255, 220, 120, 50, 255, 140, 70, 220, 255,
                ],
                1024,
            )
        })
        .join()
        .map_err(|_| "asset worker")?
        .map_err(|e| format!("{e:?}"))?;
        self.images
            .insert("cover".into(), image, 16, Duration::from_secs(60))
            .map_err(|e| format!("{e:?}"))?;
        let queue = resources::stream::Mailbox::<(), f32>::new(1);
        let worker = queue.clone();
        std::thread::spawn(move || {
            for i in 0..=100 {
                worker.preview(i as f32 / 100.);
            }
        })
        .join()
        .map_err(|_| "progress worker")?;
        self.download = queue.take_preview().expect("progress");
        assert_eq!(self.download, 1.);
        self.refresh_catalog()?;
        Ok(())
    }
}
fn source_control(source: &str) -> Result<Element<Document, Action>> {
    Ok(Description::new(
        "source",
        controls::TextArea::new("Source — Ctrl+Enter updates preview", source)?.0,
        |_, _, _| Dirty::NONE,
    )
    .focusable(true)
    .role(Role::TextInput)
    .name("Source — Ctrl+Enter updates preview")
    .layout(LogicalRect::from_xywh(20., 70., 350., 350.))
    .into_element())
}
