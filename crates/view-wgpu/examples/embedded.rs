//! Host-owned offscreen loop/device/targets. Explicit GPU mode; no native input.
#[path = "support/brush.rs"]
mod brush;
use brush::*;
use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_core::{canvas::*, host::*, viewport::*, *};
use view_text::{CosmicBackend, LayoutRequest, TextBackend, TextRevision, TextSnapshot, Wrap};
use view_wgpu::{scene::*, *};

struct WakeThread(std::thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<T>(future: impl Future<Output = T>) -> T {
    let until = Instant::now() + Duration::from_secs(30);
    let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::park_timeout(
            until
                .checked_duration_since(Instant::now())
                .expect("GPU timeout"),
        );
    }
}
/// Headless host consumes semantics but explicitly supplies no desktop services.
struct Services {
    publications: usize,
}
impl HostServices for Services {
    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities {
            clipboard: ServiceSupport::Unavailable,
            ime: ServiceSupport::Unavailable,
            accessibility: ServiceSupport::Host,
        }
    }
    fn publish_semantics(&mut self, s: &SemanticSnapshot) -> Result<(), HostServiceError> {
        assert!(!s.nodes.is_empty());
        self.publications += 1;
        Ok(())
    }
    fn ime(&mut self, _: bool, _: LogicalRect) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_read(&mut self) -> Result<String, HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
    fn clipboard_write(&mut self, _: &str) -> Result<(), HostServiceError> {
        Err(HostServiceError::Unsupported)
    }
}
fn scene(revision: u64, color: [f32; 3], angle: f32) -> SceneSnapshot {
    SceneSnapshot::new(
        Revision::from_raw(revision),
        PerspectiveCamera::look_at(
            [0.; 3],
            [0., 0., -1.],
            [0., 1., 0.],
            std::f32::consts::FRAC_PI_2,
            1.,
            10.,
        )
        .unwrap(),
        vec![SceneObject {
            id: 7,
            mesh: Arc::new(
                Mesh3D::new(
                    vec![[-1., -1., 0.], [1., -1., 0.], [1., 1., 0.], [-1., 1., 0.]],
                    vec![0, 1, 2, 0, 2, 3],
                )
                .unwrap(),
            ),
            transform: Transform3D::rotation_y(angle)
                .unwrap()
                .then(Transform3D::translation([0., 0., -2.]).unwrap())
                .unwrap(),
            material: BasicMaterial::new(color).unwrap(),
        }],
    )
    .unwrap()
}
fn host_device(
    adapter: &wgpu::Adapter,
    errors: Arc<Mutex<Vec<String>>>,
) -> Result<GpuContext, Box<dyn std::error::Error>> {
    let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: 2048,
            ..Default::default()
        },
        ..Default::default()
    }))?;
    device.on_uncaptured_error(Arc::new(move |e| {
        errors.lock().unwrap().push(e.to_string())
    }));
    Ok(GpuContext::from_device(
        device,
        queue,
        DeviceErrorPolicy::HostManaged,
    )?)
}
fn target(context: &GpuContext, size: [u32; 2]) -> Result<RenderTarget, RenderError> {
    // The host allocates the texture and passes it into the adapter.
    let texture = context.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("host-owned target"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    wait(context.import_target(texture))
}
fn image(
    renderer: &mut Renderer,
    context: &GpuContext,
    target: &RenderTarget,
) -> Result<ImageId, RenderError> {
    wait(renderer.import_texture(
        target.texture().clone(),
        context.generation(),
        TextureMetadata {
            alpha: AlphaMode::Premultiplied,
            ..Default::default()
        },
    ))
}
fn quad(rect: LogicalRect, color: Color) -> Paint {
    Paint::Draw(Draw::new(Primitive::Rect(rect), color))
}
fn near(p: &Readback, x: usize, y: usize, expected: [u8; 4]) {
    let actual = &p.rgba[(y * p.size[0] as usize + x) * 4..][..4];
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "({x},{y}) {actual:?} != {expected:?}"
    );
}

// An existing engine keeps its shader/scene representation: no SceneSnapshot or
// RenderProvider implementation is required for this host-owned pass.
struct HostEngine(wgpu::RenderPipeline);
impl HostEngine {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("host engine shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
    return vec4(positions[i],0.0,1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> { return vec4(1.0,0.0,1.0,1.0); }
"#
                .into(),
            ),
        });
        Self(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("host engine pipeline"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            }),
        )
    }
    fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &RenderTarget) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.0);
        pass.draw(0..3, 0..1);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = match std::env::args().nth(1).as_deref() {
        Some("dx12") => wgpu::Backends::DX12,
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("metal") => wgpu::Backends::METAL,
        _ => return Err("specify dx12, vulkan or metal".into()),
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = wait(instance.request_adapter(&Default::default()))?;
    println!(
        "host adapter={:?}, os={}, arch={}",
        adapter.get_info(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let errors = Arc::new(Mutex::new(Vec::new()));
    let mut context = host_device(&adapter, errors.clone())?;
    let mut device_count = 1usize;
    let budgets = Budgets {
        texture_bytes: 256 << 10,
        ..Default::default()
    };
    let mut renderer = Renderer::new(
        context.clone(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        budgets,
    )?;
    let mut output = target(&context, [192, 128])?;
    let mut engine = HostEngine::new(context.device());
    let mut engine_target = target(&context, [32, 32])?;
    let mut targets = [target(&context, [64, 64])?, target(&context, [64, 64])?];
    let mut sessions = [
        ProviderSession::new(
            SceneProvider::new(scene(1, [1., 0., 0.], 0.), SceneBudgets::default())?,
            &context,
            AttachmentContract {
                format: targets[0].format(),
                size: [64, 64],
                sample_count: 1,
                isolated: true,
            },
        )?,
        ProviderSession::new(
            SceneProvider::new(scene(1, [0., 1., 0.], 0.), SceneBudgets::default())?,
            &context,
            AttachmentContract {
                format: targets[1].format(),
                size: [64, 64],
                sample_count: 1,
                isolated: true,
            },
        )?,
    ];
    let mut ui = Runtime::<(), ()>::new(16)?;
    let (_, root) = ui.open_window(
        Description::new("host root", (), |_, _, _| Dirty::NONE)
            .layout(LogicalRect::from_xywh(0., 0., 192., 128.))
            .role(Role::Container)
            .name("Host editor"),
    )?;
    let retained = ui.mount(
        root,
        Description::new("retained inspector", 42usize, |_, _, _| Dirty::NONE),
    )?;
    let immediate = ui.mount(
        root,
        Description::new("immediate inspector", (), |_, _, _| Dirty::NONE)
            .structure(Structure::Immediate),
    )?;
    ui.set_region_builder(immediate, |_, _| {
        vec![Description::new("unchanged label", (), |_, _, _| {
            Dirty::NONE
        })]
    })?;
    let owners = [
        ui.mount(
            root,
            Description::new("animated scene", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(0., 0., 64., 64.)),
        )?,
        ui.mount(
            root,
            Description::new("demand scene", (), |_, _, _| Dirty::NONE)
                .layout(LogicalRect::from_xywh(80., 0., 64., 64.)),
        )?,
    ];
    ui.flush(&mut ())?;
    let initial = ui.counters();
    let snapshot = ui.snapshot().clone();
    let mut bridge = HostBridge::new(Services { publications: 0 });
    bridge.publish(ui.snapshot()).unwrap();
    assert_eq!(
        bridge.services().clipboard_read(),
        Err(HostServiceError::Unsupported)
    );
    let mut schedule = ViewportScheduler::new(2);
    schedule.register(
        owners[0],
        FramePolicy::Continuous(Duration::from_millis(16)),
    )?;
    schedule.register(owners[1], FramePolicy::OnDemand)?;
    let mut document = BrushDocument::new(8, 64, 4);
    document.begin(CanvasPoint::new(6., 8.)?)?;
    document.append(CanvasPoint::new(10., 8.)?)?;
    document.commit()?;
    assert_eq!(document.undo_len(), 1);
    let committed = document.tiles().clone();
    document.begin(CanvasPoint::new(12., 8.)?)?;
    let canvas = CanvasView::new(CanvasPoint::new(0., 0.)?, 1., LogicalSize::new(64., 32.))?;
    let mut fonts = CosmicBackend::from_fonts(
        [include_bytes!("../../../tests/fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        2,
    )?;
    let inspector_label = Arc::new(fonts.shape(&LayoutRequest {
        text: TextSnapshot::new("Object 7", TextRevision::INITIAL),
        font_family: "Noto Sans".into(),
        font_size: 12.0,
        line_height: 16.0,
        max_width: None,
        wrap: Wrap::None,
        scale: ScaleFactor::ONE,
        font_revision: Revision::INITIAL,
    })?);
    let mut scene_frames = [0usize; 2];
    let mut compositions = 0usize;
    let mut submissions = 0usize;
    let mut readbacks = 0usize;
    let mut scale = 1u32;
    for tick in 0..12u64 {
        if tick == 6 || tick == 9 {
            scale = 2;
            if tick == 9 {
                let replacement = host_device(&adapter, errors.clone())?;
                device_count += 1;
                assert!(wait(replacement.import_target(targets[0].texture().clone())).is_err());
                context = replacement;
                engine = HostEngine::new(context.device());
                renderer = Renderer::new(
                    context.clone(),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    budgets,
                )?;
                for session in &mut sessions {
                    session.recreate(&context)?;
                }
            } else {
                renderer.clear_caches();
            }
            output = target(&context, [192 * scale, 128 * scale])?;
            engine_target = target(&context, [32 * scale, 32 * scale])?;
            targets = [
                target(&context, [64 * scale, 64 * scale])?,
                target(&context, [64 * scale, 64 * scale])?,
            ];
            for i in 0..2 {
                sessions[i].resize([64 * scale, 64 * scale])?;
                schedule.request(owners[i])?;
            }
        }
        // Simulated host occlusion/minimization, without native input or sleeps.
        let due = schedule.poll(ui.snapshot(), Duration::from_millis(tick * 16), |_| {
            tick != 4
        })?;
        if tick == 4 {
            assert!(due.is_empty());
            assert_eq!(schedule.deadline(), None);
            continue;
        }
        let mut encoder = context.device().create_command_encoder(&Default::default());
        engine.encode(&mut encoder, &engine_target);
        for i in 0..2 {
            if due.contains(&owners[i]) {
                let color = if i == 0 { [1., 0., 0.] } else { [0., 1., 0.] };
                sessions[i].provider_mut()?.set_snapshot(scene(
                    tick + 2,
                    color,
                    if i == 0 { tick as f32 * 0.02 } else { 0. },
                ))?;
                sessions[i].encode_offscreen(
                    &context,
                    &mut encoder,
                    &targets[i],
                    Revision::from_raw(tick + 2),
                )?;
                scene_frames[i] += 1;
            }
        }
        // Each published texture version is imported after previous readers retired.
        let ids = [
            image(&mut renderer, &context, &targets[0])?,
            image(&mut renderer, &context, &targets[1])?,
        ];
        let mut first = Draw::new(
            Primitive::Image {
                image: ids[0],
                rect: LogicalRect::from_xywh(0., 0., 64., 64.),
            },
            Color::WHITE,
        );
        first.clip = Some(LogicalRect::from_xywh(0., 0., 40., 64.));
        let mut items = vec![
            Paint::Draw(first),
            Paint::Draw(Draw::new(
                Primitive::Image {
                    image: ids[1],
                    rect: LogicalRect::from_xywh(80., 0., 64., 64.),
                },
                Color::WHITE,
            )),
        ];
        let engine_image = image(&mut renderer, &context, &engine_target)?;
        items.push(Paint::Draw(Draw::new(
            Primitive::Image {
                image: engine_image,
                rect: LogicalRect::from_xywh(160., 0., 32., 32.),
            },
            Color::WHITE,
        )));
        let anchor = sessions[0]
            .provider()
            .snapshot()
            .project_overlay([0., 0., -2.], LogicalRect::from_xywh(0., 0., 64., 64.))?
            .unwrap();
        items.push(quad(
            LogicalRect::from_xywh(anchor.x - 2., anchor.y - 2., 4., 4.),
            Color([1., 1., 0., 1.]),
        ));
        for ((tx, ty), bytes) in document.tiles() {
            let image = renderer.upload_image(
                [TILE as u32, TILE as u32],
                bytes,
                TextureMetadata::default(),
            )?;
            let p = canvas.project(CanvasPoint::new(
                f64::from(tx * TILE),
                f64::from(ty * TILE),
            )?)?;
            items.push(Paint::Draw(Draw::new(
                Primitive::Image {
                    image,
                    rect: LogicalRect::from_xywh(p.x, p.y + 80., TILE as f32, TILE as f32),
                },
                Color::WHITE,
            )));
        }
        for p in document.preview() {
            let p = canvas.project(*p)?;
            items.push(quad(
                LogicalRect::from_xywh(p.x - 1., p.y + 79., 2., 2.),
                Color([0., 1., 1., 1.]),
            ));
        }
        items.push(quad(
            LogicalRect::from_xywh(80., 72., 104., 48.),
            Color([0.1, 0.1, 0.1, 1.]),
        ));
        items.push(Paint::Draw(Draw::new(
            Primitive::Text {
                layout: inspector_label.clone(),
                origin: LogicalPoint::new(84., 76.),
            },
            Color::WHITE,
        )));
        items.push(quad(
            LogicalRect::from_xywh(84., 100., 64., 8.),
            Color([0.2, 0.6, 0.9, 1.]),
        ));
        let paint = PaintSnapshot {
            revision: Revision::from_raw(tick + 1),
            size: [192 * scale, 128 * scale],
            scale: ScaleFactor::new(scale as f32)?,
            items,
        };
        let frame = renderer.prepare(&paint)?;
        compositions += 1;
        let capture = if tick == 0 || tick == 6 || tick == 11 {
            Some(frame.encode_capture(&context, &mut encoder, &output, Color::TRANSPARENT)?)
        } else {
            frame.encode(&context, &mut encoder, &output, Color::TRANSPARENT)?;
            // Keep ownership until the one host submission below.
            let index = context.queue().submit([encoder.finish()]);
            submissions += 1;
            frame.submitted(&context, index.clone())?;
            for i in 0..2 {
                if due.contains(&owners[i]) {
                    sessions[i].submitted(&context, Revision::from_raw(tick + 2))?;
                }
            }
            context.device().poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: Some(Duration::from_secs(30)),
            })?;
            for session in &mut sessions {
                session.retire_completed();
            }
            ui.flush(&mut ())?;
            assert!(!bridge.publish(ui.snapshot()).unwrap());
            continue;
        };
        let index = context.queue().submit([encoder.finish()]);
        submissions += 1;
        for i in 0..2 {
            if due.contains(&owners[i]) {
                sessions[i].submitted(&context, Revision::from_raw(tick + 2))?;
            }
        }
        let pixels = capture
            .unwrap()
            .submitted(&context, index)?
            .readback(&context)?;
        readbacks += 1;
        let s = scale as usize;
        near(&pixels, 32 * s, 32 * s, [255, 255, 0, 255]);
        near(&pixels, 24 * s, 32 * s, [255, 0, 0, 255]);
        near(&pixels, 44 * s, 32 * s, [0, 0, 0, 0]);
        near(&pixels, 112 * s, 32 * s, [0, 255, 0, 255]);
        near(&pixels, 176 * s, 16 * s, [255, 0, 255, 255]);
        near(&pixels, 8 * s, 88 * s, [255, 64, 0, 255]);
        near(&pixels, 12 * s, 88 * s, [0, 255, 255, 255]);
        assert!(
            (84 * s..144 * s).any(|x| (76 * s..94 * s)
                .any(|y| pixels.rgba[(y * pixels.size[0] as usize + x) * 4] > 200)),
            "static inspector glyphs missing"
        );
        for session in &mut sessions {
            session.retire_completed();
        }
        assert_eq!(renderer.live_frames(), 0);
        assert!(renderer.cache_stats().texture_bytes <= budgets.texture_bytes);
        ui.flush(&mut ())?;
        assert!(!bridge.publish(ui.snapshot()).unwrap());
    }
    assert_eq!(scene_frames, [11, 4]);
    assert_eq!(compositions, 11);
    assert_eq!(submissions, compositions);
    assert_eq!(readbacks, 3);
    assert_eq!(ui.counters().builds, initial.builds);
    assert_eq!(ui.counters().layouts, initial.layouts);
    assert_eq!(ui.counters().commits, initial.commits);
    assert_eq!(ui.snapshot(), &snapshot);
    assert_eq!(*ui.state::<usize>(retained)?, 42);
    assert_eq!(bridge.services().publications, 1);
    assert_eq!(document.tiles(), &committed);
    document.cancel();
    assert!(document.undo());
    assert!(document.tiles().is_empty());
    assert_eq!(PRESSURE_TILT, (false, false));
    assert!(
        errors.lock().unwrap().is_empty(),
        "host GPU errors: {:?}",
        errors.lock().unwrap()
    );
    println!(
        "PASS host loop: ui_builds_delta=0 ui_layouts_delta=0 ui_commits_delta=0 scene_frames={scene_frames:?} host_engine_frames={compositions} composition_frames={compositions} host_submissions={submissions} diagnostic_readbacks={readbacks} host_devices={device_count} (one initial, one explicit replacement); clipped viewports, DPI 1x/2x, retained/immediate inspectors, gizmo, tiled brush preview and undo; no native input"
    );
    Ok(())
}
