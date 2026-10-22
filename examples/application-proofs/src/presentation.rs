//! Shared offscreen/native paint adapter; pure layout descriptions never load assets.
use crate::{Proofs, Result};
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view::{render::*, scene::*, text::*, *};
pub fn wait<T>(future: impl Future<Output = T>) -> Result<T> {
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
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return Ok(v);
        }
        std::thread::park_timeout(
            end.checked_duration_since(Instant::now())
                .ok_or("GPU timeout")?,
        );
    }
}
pub struct Presentation {
    pub renderer: Renderer,
    pub candidate: Option<LogicalRect>,
    fonts: CosmicBackend,
}
impl Presentation {
    pub fn new(context: GpuContext, format: wgpu::TextureFormat) -> Result<Self> {
        Ok(Self {
            renderer: Renderer::new(context, format, Budgets::default())?,
            candidate: None,
            fonts: CosmicBackend::from_fonts(
                [include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
                128,
            )?,
        })
    }
    pub fn paint(
        &mut self,
        app: &mut Proofs,
        context: &GpuContext,
        window: WindowId,
        size: [u32; 2],
        scale: ScaleFactor,
    ) -> Result<PaintSnapshot> {
        self.renderer.clear_caches();
        let mut items = vec![];
        self.candidate = None;
        if let Some(field) = app.editing_node(window) {
            let focused = app.ui.focused(window)? == Some(field);
            let bounds = app.ui.global_rect(field)?;
            let style = view::controls::Style {
                font_family: "Noto Sans".into(),
                ..Default::default()
            };
            let prepared = app.ui.update(
                field,
                Dirty::NONE,
                |field: &mut view::controls::TextField| {
                    field.set_focused(focused, 0);
                    field.prepare(&mut self.fonts, &style, bounds, scale, 0)
                },
            )??;
            items.extend(view::controls::frame(bounds, &style));
            items.extend(prepared.paint(&style)?);
            self.candidate = focused.then_some(prepared.candidate);
            app.ui.flush(&mut app.document)?;
        }
        let mut text = |label: &str, origin: LogicalPoint| -> Result<Paint> {
            let layout = Arc::new(self.fonts.shape(&LayoutRequest {
                text: TextSnapshot::new(label, TextRevision::INITIAL),
                font_family: "Noto Sans".into(),
                font_size: 16.,
                line_height: 24.,
                max_width: Some(700.),
                wrap: Wrap::None,
                scale,
                font_revision: Revision::INITIAL,
            })?);
            Ok(Paint::Draw(Draw::new(
                Primitive::Text { layout, origin },
                Color::WHITE,
            )))
        };
        for node in app
            .ui
            .snapshot()
            .nodes
            .iter()
            .filter(|n| n.window == window && n.visibility == Visibility::Visible)
        {
            if Some(node.id) != app.editing_node(window)
                && let Some(label) = &node.name
            {
                let origin = app.ui.global_rect(node.id)?.origin;
                items.push(text(label, origin)?);
            }
        }
        if window == app.editor {
            let target = context.target([320, 320])?;
            let scene = app.scene()?;
            let scene_revision = scene.revision();
            let mut provider = ProviderSession::new(
                SceneProvider::new(scene, SceneBudgets::default())?,
                context,
                AttachmentContract {
                    format: target.format(),
                    size: [320, 320],
                    sample_count: 1,
                    isolated: true,
                },
            )?;
            let mut encoder = context.device().create_command_encoder(&Default::default());
            provider.encode_offscreen(context, &mut encoder, &target, scene_revision)?;
            context.queue().submit([encoder.finish()]);
            provider.submitted(context, scene_revision)?;
            let image = wait(self.renderer.import_texture(
                target.texture().clone(),
                context.generation(),
                TextureMetadata::default(),
            ))??;
            items.push(Paint::Draw(Draw::new(
                Primitive::Image {
                    image,
                    rect: LogicalRect::from_xywh(240., 20., 320., 320.),
                },
                Color::WHITE,
            )));
            if app.document.selected.is_some() {
                let anchor = app
                    .scene()?
                    .project_overlay(
                        [app.preview_x.unwrap_or(app.document.x), 0., -2.],
                        LogicalRect::from_xywh(240., 20., 320., 320.),
                    )?
                    .ok_or("gizmo outside viewport")?;
                items.push(Paint::Draw(Draw::new(
                    Primitive::Rect(LogicalRect::from_xywh(
                        anchor.x - 6.,
                        anchor.y - 6.,
                        12.,
                        12.,
                    )),
                    Color([1., 0.8, 0.1, 1.]),
                )));
            }
        }
        if window == app.store {
            if let Some(product) = app.document.product {
                items.push(text(
                    &format!("Product {product} details"),
                    LogicalPoint::new(440., 360.),
                )?);
            }
            if let Some(image) = app.images.get("cover", Duration::ZERO, true) {
                let id = self.renderer.upload_image(
                    [image.width(), image.height()],
                    image.pixels(),
                    TextureMetadata::default(),
                )?;
                items.push(Paint::Draw(Draw::new(
                    Primitive::Image {
                        image: id,
                        rect: LogicalRect::from_xywh(440., 160., 180., 180.),
                    },
                    Color::WHITE,
                )));
            }
            items.push(text(
                &format!(
                    "Download: {:.0}% — {:?}",
                    app.download * 100.,
                    app.results.status()
                ),
                LogicalPoint::new(20., 560.),
            )?);
        }
        Ok(PaintSnapshot {
            revision: app.ui.snapshot().revision,
            size,
            scale,
            items,
        })
    }
}
pub fn gpu_proof(backend: wgpu::Backends) -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = wait(instance.request_adapter(&Default::default()))??;
    println!("M4 adapter={:?}", adapter.get_info());
    let context = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        wgpu::Limits::default(),
    ))??;
    let mut presentation = Presentation::new(context.clone(), wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut app = Proofs::new()?;
    app.verify()?;
    #[cfg(feature = "automation")]
    let mut receipts = vec![];
    for window in [app.editor, app.detached, app.store] {
        let paint = presentation.paint(&mut app, &context, window, [800, 600], ScaleFactor::ONE)?;
        let target = context.target([800, 600])?;
        #[cfg(feature = "automation")]
        let started = Instant::now();
        let capture = presentation.renderer.prepare(&paint)?.submit_capture(
            &context,
            &target,
            Color([0.05, 0.06, 0.08, 1.]),
        )?;
        let pixels = capture.readback(&context)?;
        assert!(
            pixels
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 150 || p[1] > 150)
                .count()
                > 100,
            "expected rendered text/scene/image"
        );
        if window == app.editor {
            let i = (180 * 800 + 400) * 4;
            assert!(
                pixels.rgba[i] > 180 && pixels.rgba[i + 1] > 150,
                "selected gizmo"
            );
        }
        assert!(context.take_errors().is_empty());
        #[cfg(feature = "automation")]
        receipts.push((
            window,
            paint.revision.get(),
            started.elapsed().as_micros() as u64,
            pixels.rgba,
        ));
    }
    #[cfg(feature = "automation")]
    {
        use view_automation::*;
        let mut session = Session::new(
            "gpu-proof".into(),
            view_testing::Harness::new(app.ui, app.document, 16),
            vec![Scope::Inspect],
            32,
        )?;
        for (i, (window, revision, overhead, rgba)) in receipts.into_iter().enumerate() {
            session.publish_presentation(window, revision, "offscreen_readback")?;
            session.publish_capture(Capture {
                window: window_id(window),
                revision,
                width: 800,
                height: 600,
                provenance: "offscreen_readback".into(),
                overhead_micros: overhead,
                rgba,
            })?;
            let response = session.execute(Request {
                version: VERSION,
                session: "gpu-proof".into(),
                id: i as u64,
                operation: Operation::Capture {
                    window: window_id(window),
                    revision,
                },
            });
            let value = response.result?;
            assert_eq!(
                value["rgba"].as_array().ok_or("capture pixels")?.len(),
                800 * 600 * 4
            );
        }
        let inspector = session
            .execute(Request {
                version: VERSION,
                session: "gpu-proof".into(),
                id: 10,
                operation: Operation::Inspector,
            })
            .result?;
        std::fs::create_dir_all("target/verification")?;
        std::fs::write(
            "target/verification/m5-inspector.html",
            inspector["html"].as_str().ok_or("inspector html")?,
        )?;
        let bundle = session
            .execute(Request {
                version: VERSION,
                session: "gpu-proof".into(),
                id: 11,
                operation: Operation::FailureBundle,
            })
            .result?;
        std::fs::write(
            "target/verification/m5-gpu-bundle.json",
            serde_json::to_vec_pretty(&bundle)?,
        )?;
        println!("PASS M5 revision-tagged actual GPU captures and public inspector/failure bundle");
    }
    println!("PASS M4 three-window offscreen scene/text/asset composition");
    Ok(())
}
