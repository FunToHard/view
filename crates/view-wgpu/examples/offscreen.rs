//! Explicit GPU fixture. Never opens a window or takes physical input.
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision, ScaleFactor, Transform2D};
use view_text::{CosmicBackend, LayoutRequest, TextBackend, TextRevision, TextSnapshot, Wrap};
use view_wgpu::*;
struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<T>(future: impl Future<Output = T>) -> T {
    let end = Instant::now() + Duration::from_secs(30);
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        let remaining = end
            .checked_duration_since(Instant::now())
            .expect("GPU request timeout");
        std::thread::park_timeout(remaining);
    }
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
}
fn quad(r: LogicalRect, c: Color) -> Paint {
    Paint::Draw(Draw::new(Primitive::Rect(r), c))
}
fn pixel(readback: &Readback, x: usize, y: usize) -> [u8; 4] {
    readback.rgba[(y * readback.size[0] as usize + x) * 4..][..4]
        .try_into()
        .unwrap()
}
fn near(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "pixel {actual:?}, expected {expected:?}, tolerance 2/255"
    );
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
        "os={} arch={} adapter={:?}; CPU device type denotes software",
        std::env::consts::OS,
        std::env::consts::ARCH,
        adapter.get_info()
    );
    let limits = wgpu::Limits {
        max_texture_dimension_2d: 2048,
        ..Default::default()
    };
    let context = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        limits,
    ))?;
    assert!(context.target([0, 64]).is_err());
    assert!(
        context
            .target_format([64, 64], wgpu::TextureFormat::Rgba16Float)
            .is_err()
    );
    assert!(
        wait(GpuContext::request(
            &adapter,
            wgpu::Features::empty(),
            wgpu::Limits {
                max_texture_dimension_2d: u32::MAX,
                ..Default::default()
            }
        ))
        .is_err()
    );
    let mut renderer = Renderer::new(
        context.clone(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Budgets {
            texture_bytes: 16,
            glyph_bytes: 22 << 20,
            ..Default::default()
        },
    )?;
    let target = context.target([128, 96])?;
    let red = Color([1.0, 0.0, 0.0, 1.0]);
    let green = Color([0.0, 1.0, 0.0, 1.0]);
    let blue = Color([0.0, 0.0, 1.0, 1.0]);
    let image = renderer.upload_image([1, 1], &[255, 0, 255, 255], TextureMetadata::default())?;
    let mut clipped = Draw::new(Primitive::Rect(rect(0.0, 0.0, 24.0, 24.0)), green);
    clipped.transform = Transform2D::translation(32.0, 0.0);
    clipped.clip = Some(rect(36.0, 4.0, 8.0, 8.0));
    let mut path = lyon::path::Path::builder();
    path.begin(lyon::math::point(64.0, 0.0));
    path.line_to(lyon::math::point(88.0, 0.0));
    path.line_to(lyon::math::point(64.0, 24.0));
    path.close();
    let path = path.build();
    let mut fonts = CosmicBackend::from_fonts(
        [include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()],
        2,
    )?;
    let layout = Arc::new(fonts.shape(&LayoutRequest {
        text: TextSnapshot::new("Hello", TextRevision::INITIAL),
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        max_width: None,
        wrap: Wrap::None,
        scale: ScaleFactor::ONE,
        font_revision: Revision::INITIAL,
    })?);
    let snapshot = PaintSnapshot {
        revision: Revision::INITIAL,
        size: [128, 96],
        scale: ScaleFactor::ONE,
        items: vec![
            quad(rect(0.0, 0.0, 24.0, 24.0), red),
            Paint::Draw(clipped),
            Paint::Draw(Draw::new(Primitive::Fill(path.clone()), blue)),
            Paint::Draw(Draw::new(
                Primitive::Image {
                    image,
                    rect: rect(96.0, 0.0, 24.0, 24.0),
                },
                Color::WHITE,
            )),
            Paint::Layer {
                opacity: 0.5,
                items: vec![
                    quad(rect(0.0, 32.0, 24.0, 24.0), red),
                    quad(rect(12.0, 32.0, 24.0, 24.0), blue),
                ],
            },
            Paint::Draw(Draw::new(
                Primitive::Text {
                    layout: layout.clone(),
                    origin: LogicalPoint::new(40.0, 32.0),
                },
                Color::WHITE,
            )),
            quad(rect(40.0, 32.0, 2.0, 26.0), green),
        ],
    };
    let prepared = renderer.prepare(&snapshot)?;
    let bindings_before = renderer.cache_stats().sampled_bindings_created;
    // Existing solid/image resources share bindings; each isolated layer alone
    // needs a new binding because it owns a fresh target texture.
    for _ in 0..100 {
        drop(renderer.prepare(&snapshot)?);
    }
    assert_eq!(
        renderer.cache_stats().sampled_bindings_created,
        bindings_before + 100
    );
    // Evict the sampled image after preparation: the old prepared bind group
    // must retain its original texture, while new preparation rejects the ID.
    for _ in 0..5 {
        renderer.upload_image([1, 1], &[0, 255, 255, 255], TextureMetadata::default())?;
    }
    assert!(matches!(
        renderer.prepare(&snapshot),
        Err(RenderError::StaleResource)
    ));
    let capture = prepared.submit_capture(&context, &target, Color::TRANSPARENT)?;
    // Target reuse must not change already captured pixels or their revision.
    let blank = PaintSnapshot {
        revision: Revision::INITIAL.checked_next()?,
        items: vec![],
        ..snapshot.clone()
    };
    renderer
        .prepare(&blank)?
        .submit(&context, &target, Color::WHITE)?;
    let pixels = capture.readback(&context)?;
    assert_eq!(pixels.revision, Revision::INITIAL);
    near(pixel(&pixels, 8, 8), [255, 0, 0, 255]);
    near(pixel(&pixels, 38, 6), [0, 255, 0, 255]);
    near(pixel(&pixels, 34, 6), [0, 0, 0, 0]);
    near(pixel(&pixels, 68, 4), [0, 0, 255, 255]);
    near(pixel(&pixels, 84, 20), [0, 0, 0, 0]);
    near(pixel(&pixels, 100, 8), [255, 0, 255, 255]);
    near(pixel(&pixels, 4, 40), [188, 0, 0, 128]);
    near(pixel(&pixels, 16, 40), [0, 0, 188, 128]);
    near(pixel(&pixels, 40, 40), [0, 255, 0, 255]);
    assert!(
        (44..96).any(|x| (34..58).any(|y| pixel(&pixels, x, y)[3] > 0)),
        "glyph raster missing"
    );
    // Reusing geometry counts a cache hit, including after text/font reload.
    let shapes = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(Primitive::Fill(path), red))],
        ..blank.clone()
    };
    renderer.prepare(&shapes)?;
    assert!(renderer.cache_stats().geometry_hits > 0);
    fonts.reload_fonts([include_bytes!("../fixtures/fonts/NotoSans-Regular.ttf").to_vec()])?;
    let text = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(
            Primitive::Text {
                layout,
                origin: LogicalPoint::ZERO,
            },
            Color::WHITE,
        ))],
        ..blank
    };
    let text_pixels = renderer
        .prepare(&text)?
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    assert!(text_pixels.rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
    renderer.prepare(&text)?;
    assert!(renderer.cache_stats().glyph_hits > 0);
    // Controls wrap cached shaping results in fresh Arcs on every paint.
    // Those wrappers must not create a new GPU text entry each frame.
    let before = renderer.cache_stats();
    for _ in 0..100 {
        let mut repainted = text.clone();
        if let Paint::Draw(draw) = &mut repainted.items[0]
            && let Primitive::Text { layout, .. } = &mut draw.primitive
        {
            *layout = Arc::new((**layout).clone());
        }
        renderer.prepare(&repainted)?;
    }
    let after = renderer.cache_stats();
    assert_eq!(after.glyph_bytes, before.glyph_bytes);
    assert_eq!(after.glyph_hits, before.glyph_hits + 100);
    let mut bounded = Renderer::new(
        context.clone(),
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Budgets {
            max_text_entries: 2,
            ..Default::default()
        },
    )?;
    let retained = bounded.prepare(&text)?;
    for x in 0..100 {
        let mut changed = text.clone();
        if let Paint::Draw(draw) = &mut changed.items[0] {
            draw.transform = Transform2D::translation(x as f32, 0.);
        }
        bounded.prepare(&changed)?;
        assert!(bounded.cache_stats().text_entries <= 2);
    }
    // Entry eviction must not invalidate a frame already prepared by its owner.
    let retained_pixels = retained
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    assert_eq!(retained_pixels.rgba, text_pixels.rgba);
    let retained_text = renderer.prepare(&text)?;
    let replacement = Arc::new(fonts.shape(&LayoutRequest {
        text: TextSnapshot::new("New fonts", TextRevision::INITIAL),
        font_family: "Noto Sans".into(),
        font_size: 20.0,
        line_height: 26.0,
        max_width: None,
        wrap: Wrap::None,
        scale: ScaleFactor::ONE,
        font_revision: fonts.font_revision(),
    })?);
    let replacement_paint = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(
            Primitive::Text {
                layout: replacement.clone(),
                origin: LogicalPoint::ZERO,
            },
            Color::WHITE,
        ))],
        ..shapes.clone()
    };
    renderer.prepare(&replacement_paint)?;
    assert!(renderer.cache_stats().glyph_evictions > 0);
    renderer.clear_caches();
    let retained_pixels = retained_text
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    assert_eq!(
        retained_pixels.rgba, text_pixels.rgba,
        "glyph eviction changed prepared glyphs"
    );
    let labels = PaintSnapshot {
        items: (0..100)
            .map(|i| {
                Paint::Draw(Draw::new(
                    Primitive::Text {
                        layout: replacement.clone(),
                        origin: LogicalPoint::new((i % 10) as f32 * 10.0, (i / 10) as f32 * 9.0),
                    },
                    Color::WHITE,
                ))
            })
            .collect(),
        ..shapes.clone()
    };
    let labels_frame = renderer.prepare(&labels)?;
    assert!(
        labels_frame.reserved_bytes() < 22 << 20,
        "labels should share their font atlas"
    );
    labels_frame
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;

    // Aggregate frame slots remain occupied until completion callbacks are polled.
    let one = renderer.prepare(&shapes)?;
    let two = renderer.prepare(&shapes)?;
    let three = renderer.prepare(&shapes)?;
    assert!(matches!(
        renderer.prepare(&shapes),
        Err(RenderError::BudgetExceeded)
    ));
    drop((one, two, three));
    assert_eq!(renderer.live_frames(), 0);

    // Stroke, fractional clips, and a 2x logical-to-physical transform.
    let mut stroke = lyon::path::Path::builder();
    stroke.begin(lyon::math::point(4.0, 4.0));
    stroke.line_to(lyon::math::point(20.0, 4.0));
    stroke.end(false);
    let scaled = PaintSnapshot {
        scale: ScaleFactor::new(2.0)?,
        items: vec![Paint::Draw(Draw::new(
            Primitive::Stroke {
                path: stroke.build(),
                width: 2.0,
            },
            green,
        ))],
        ..shapes.clone()
    };
    let scaled_pixels = renderer
        .prepare(&scaled)?
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    near(pixel(&scaled_pixels, 16, 8), [0, 255, 0, 255]);
    near(pixel(&scaled_pixels, 16, 12), [0, 0, 0, 0]);
    let mut fraction = Draw::new(Primitive::Rect(rect(0.0, 0.0, 20.0, 20.0)), red);
    fraction.clip = Some(rect(0.75, 0.75, 2.0, 2.0));
    let fractional = PaintSnapshot {
        items: vec![Paint::Draw(fraction)],
        ..shapes.clone()
    };
    let fractional_pixels = renderer
        .prepare(&fractional)?
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    near(pixel(&fractional_pixels, 0, 0), [0, 0, 0, 0]);
    near(pixel(&fractional_pixels, 1, 1), [255, 0, 0, 255]);
    near(pixel(&fractional_pixels, 3, 3), [0, 0, 0, 0]);

    // Texture origin and straight-alpha conversion are independent metadata.
    let bottom = renderer.upload_image(
        [1, 2],
        &[255, 0, 0, 128, 0, 0, 255, 255],
        TextureMetadata {
            origin: Origin::BottomLeft,
            ..Default::default()
        },
    )?;
    let origin_fixture = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(
            Primitive::Image {
                image: bottom,
                rect: rect(0.0, 0.0, 8.0, 8.0),
            },
            Color::WHITE,
        ))],
        ..shapes.clone()
    };
    let origin_pixels = renderer
        .prepare(&origin_fixture)?
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    near(pixel(&origin_pixels, 1, 0), [0, 0, 255, 255]);
    near(pixel(&origin_pixels, 1, 7), [188, 0, 0, 128]);
    let filter_fixture = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(
            Primitive::Image {
                image: bottom,
                rect: rect(0.0, 0.0, 1.0, 3.0),
            },
            Color::WHITE,
        ))],
        ..shapes.clone()
    };
    let filtered = renderer
        .prepare(&filter_fixture)?
        .submit_capture(&context, &target, Color::TRANSPARENT)?
        .readback(&context)?;
    near(pixel(&filtered, 0, 1), [137, 0, 188, 192]);

    // Same-device offscreen contribution and metadata validation.
    let source = context.target([1, 1])?;
    let source_paint = PaintSnapshot {
        size: [1, 1],
        items: vec![],
        ..shapes.clone()
    };
    renderer
        .prepare(&source_paint)?
        .submit(&context, &source, red)?;
    let imported = wait(renderer.import_texture(
        source.texture().clone(),
        context.generation(),
        TextureMetadata {
            alpha: AlphaMode::Premultiplied,
            ..Default::default()
        },
    ))?;
    let imported_paint = PaintSnapshot {
        items: vec![Paint::Draw(Draw::new(
            Primitive::Image {
                image: imported,
                rect: rect(0.0, 0.0, 32.0, 32.0),
            },
            Color::WHITE,
        ))],
        ..shapes.clone()
    };
    assert!(
        renderer
            .prepare(&PaintSnapshot {
                size: [1, 1],
                ..imported_paint.clone()
            })?
            .encode(
                &context,
                &mut context.device().create_command_encoder(&Default::default()),
                &source,
                Color::TRANSPARENT
            )
            .is_err(),
        "target feedback must reject before encoding"
    );
    near(
        pixel(
            &renderer
                .prepare(&imported_paint)?
                .submit_capture(&context, &target, Color::TRANSPARENT)?
                .readback(&context)?,
            8,
            8,
        ),
        [255, 0, 0, 255],
    );
    assert!(
        wait(renderer.import_texture(
            source.texture().clone(),
            context.generation(),
            TextureMetadata {
                transfer: Transfer::Linear,
                ..Default::default()
            }
        ))
        .is_err()
    );
    let other = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        wgpu::Limits {
            max_texture_dimension_2d: 2048,
            ..Default::default()
        },
    ))?;
    let foreign = other.target([1, 1])?;
    assert!(matches!(
        wait(renderer.import_texture(
            foreign.texture().clone(),
            other.generation(),
            TextureMetadata::default()
        )),
        Err(RenderError::StaleResource)
    ));
    assert!(
        wait(renderer.import_texture(
            foreign.texture().clone(),
            context.generation(),
            TextureMetadata::default()
        ))
        .is_err(),
        "forged epoch must not bypass real device validation"
    );
    assert!(matches!(
        renderer.prepare(&shapes)?.encode(
            &other,
            &mut other.device().create_command_encoder(&Default::default()),
            &foreign,
            Color::TRANSPARENT
        ),
        Err(RenderError::StaleResource)
    ));

    buffer_cache_fixture(&context, &target)?;
    provider_fixture(&context, &target)?;
    assert!(
        context.take_errors().is_empty(),
        "uncaptured GPU validation errors"
    );
    std::fs::create_dir_all("target/verification")?;
    let mut ppm = format!("P6\n{} {}\n255\n", pixels.size[0], pixels.size[1]).into_bytes();
    for pixel in pixels.rgba.as_chunks::<4>().0.iter() {
        ppm.extend_from_slice(&pixel[..3]);
    }
    std::fs::write("target/verification/m3-offscreen.ppm", ppm)?;
    println!(
        "PASS actual GPU shapes, paths, clips, SDR alpha, isolated layers, images, glyph raster/order/font lifetime, cache eviction and immutable revision capture; tolerance 2/255. {:?}",
        renderer.cache_stats()
    );
    Ok(())
}

fn buffer_cache_fixture(context: &GpuContext, target: &RenderTarget) -> Result<(), RenderError> {
    for limit in [0, 2] {
        let mut renderer = Renderer::new(
            context.clone(),
            target.format(),
            Budgets {
                max_buffer_entries: limit,
                ..Budgets::default()
            },
        )?;
        let mut snapshot = PaintSnapshot {
            revision: Revision::INITIAL,
            size: target.size(),
            scale: ScaleFactor::ONE,
            items: vec![Paint::Draw(Draw::new(
                Primitive::Rect(rect(10.0, 10.0, 20.0, 20.0)),
                Color::WHITE,
            ))],
        };
        let retained = renderer.prepare(&snapshot)?;
        let expected = renderer
            .prepare(&snapshot)?
            .submit_capture(context, target, Color::TRANSPARENT)?
            .readback(context)?;
        for i in 0..1000 {
            snapshot.size = [128 + i, 96 + i];
            drop(renderer.prepare(&snapshot)?);
            assert!(renderer.cache_stats().buffer_entries <= limit);
        }
        assert_eq!(renderer.cache_stats().sampled_bindings_created, 1);
        if limit > 0 {
            assert_eq!(renderer.cache_stats().buffer_entries, 1);
            assert_eq!(renderer.cache_stats().buffer_hits, 1001);
        }
        let resized_target = context.target(snapshot.size)?;
        let resized = renderer
            .prepare(&snapshot)?
            .submit_capture(context, &resized_target, Color::TRANSPARENT)?
            .readback(context)?;
        near(pixel(&resized, 15, 15), [255; 4]);
        near(pixel(&resized, 35, 15), [0; 4]);
        // Actual geometry changes still evict; size-only changes above must not.
        for i in 0..100 {
            snapshot.items = vec![quad(rect(i as f32, 0.0, 20.0, 20.0), Color::WHITE)];
            drop(renderer.prepare(&snapshot)?);
            assert!(renderer.cache_stats().buffer_entries <= limit);
        }
        let actual = retained
            .submit_capture(context, target, Color::TRANSPARENT)?
            .readback(context)?;
        assert_eq!(
            actual.rgba, expected.rgba,
            "eviction must preserve prepared geometry"
        );
    }
    Ok(())
}

struct FixtureProvider {
    events: Arc<std::sync::Mutex<Vec<&'static str>>>,
    pipeline: Option<wgpu::RenderPipeline>,
    format: wgpu::TextureFormat,
}
impl RenderProvider for FixtureProvider {
    fn negotiate(&mut self, contract: AttachmentContract) -> Result<ContributionMode, RenderError> {
        self.events.lock().unwrap().push("negotiate");
        self.format = contract.format;
        Ok(ContributionMode::Direct)
    }
    fn prepare(&mut self, _: &GpuContext, _: Revision) -> Result<(), RenderError> {
        self.events.lock().unwrap().push("prepare");
        Ok(())
    }
    fn encode(&mut self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), RenderError> {
        self.events.lock().unwrap().push("encode");
        pass.insert_debug_marker("scoped provider fixture");
        pass.set_pipeline(self.pipeline.as_ref().unwrap());
        pass.draw(0..3, 0..1);
        Ok(())
    }
    fn retire(&mut self, _: Revision) {
        self.events.lock().unwrap().push("retire");
    }
    fn resize(&mut self, _: [u32; 2]) -> Result<(), RenderError> {
        self.events.lock().unwrap().push("resize");
        Ok(())
    }
    fn recreate(&mut self, context: &GpuContext) -> Result<(), RenderError> {
        self.events.lock().unwrap().push("recreate");
        let shader=context.device().create_shader_module(wgpu::ShaderModuleDescriptor {label:Some("provider green fixture"),source:wgpu::ShaderSource::Wgsl("@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0));return vec4<f32>(p[i],0.0,1.0);} @fragment fn fs()->@location(0) vec4<f32> {return vec4<f32>(0.0,1.0,0.0,1.0);}".into())});
        self.pipeline = Some(context.device().create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("provider fixture"),
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
                        format: self.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            },
        ));
        Ok(())
    }
    fn failed(&mut self, _: &RenderError) {
        self.events.lock().unwrap().push("failed");
    }
}
fn provider_fixture(context: &GpuContext, target: &RenderTarget) -> Result<(), RenderError> {
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let contract = AttachmentContract {
        format: target.format(),
        size: target.size(),
        sample_count: 1,
        isolated: false,
    };
    let mut session = ProviderSession::new(
        FixtureProvider {
            events: events.clone(),
            pipeline: None,
            format: target.format(),
        },
        context,
        contract,
    )?;
    assert!(session.submitted(context, Revision::INITIAL).is_err());
    let mut encoder = context.device().create_command_encoder(&Default::default());
    session.encode_direct(context, &mut encoder, target, Revision::INITIAL)?;
    assert!(session.resize([10, 10]).is_err());
    let index = context.queue().submit([encoder.finish()]);
    session.submitted(context, Revision::INITIAL)?;
    assert!(!events.lock().unwrap().contains(&"retire"));
    context
        .device()
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: Some(Duration::from_secs(30)),
        })
        .map_err(|e| RenderError::Device(e.to_string()))?;
    session.retire_completed();
    assert_eq!(
        &*events.lock().unwrap(),
        &["negotiate", "recreate", "prepare", "encode", "retire"]
    );
    session.resize(target.size())?;
    session.recreate(context)?;
    let mut isolated = ProviderSession::new(
        FixtureProvider {
            events,
            pipeline: None,
            format: target.format(),
        },
        context,
        AttachmentContract {
            isolated: true,
            ..contract
        },
    )?;
    assert_eq!(isolated.mode(), ContributionMode::Offscreen);
    let mut encoder = context.device().create_command_encoder(&Default::default());
    assert!(
        isolated
            .encode_direct(context, &mut encoder, target, Revision::INITIAL)
            .is_err()
    );
    isolated.encode_offscreen(context, &mut encoder, target, Revision::INITIAL)?;
    let index = context.queue().submit([encoder.finish()]);
    isolated.submitted(context, Revision::INITIAL)?;
    context
        .device()
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: Some(Duration::from_secs(30)),
        })
        .map_err(|e| RenderError::Device(e.to_string()))?;
    isolated.retire_completed();
    let mut renderer = Renderer::new(context.clone(), target.format(), Budgets::default())?;
    let imported = wait(renderer.import_texture(
        target.texture().clone(),
        context.generation(),
        TextureMetadata {
            alpha: AlphaMode::Premultiplied,
            ..Default::default()
        },
    ))?;
    let output = context.target(target.size())?;
    let snapshot = PaintSnapshot {
        revision: Revision::INITIAL,
        size: target.size(),
        scale: ScaleFactor::ONE,
        items: vec![Paint::Draw(Draw::new(
            Primitive::Image {
                image: imported,
                rect: rect(0.0, 0.0, 128.0, 96.0),
            },
            Color::WHITE,
        ))],
    };
    let pixels = renderer
        .prepare(&snapshot)?
        .submit_capture(context, &output, Color::TRANSPARENT)?
        .readback(context)?;
    near(pixel(&pixels, 50, 50), [0, 255, 0, 255]);
    Ok(())
}
