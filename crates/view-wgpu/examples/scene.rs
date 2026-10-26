//! Explicit offscreen GPU test. No window, event loop or physical input.
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision, ScaleFactor};
use view_wgpu::{scene::*, *};

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
        std::thread::park_timeout(
            end.checked_duration_since(Instant::now())
                .expect("GPU timeout"),
        );
    }
}
fn camera() -> PerspectiveCamera {
    PerspectiveCamera::look_at(
        [0.0; 3],
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        std::f32::consts::FRAC_PI_2,
        1.0,
        10.0,
    )
    .unwrap()
}
fn imported_quad() -> Arc<Mesh3D> {
    Arc::new(
        import_obj_positions(
            "v -1 -1 0\nv 1 -1 0\nv 1 1 0\nv -1 1 0\nf 1 2 3 4\n",
            ObjLimits::default(),
        )
        .unwrap(),
    )
}
fn object(id: u64, z: f32, scale: f32, color: [f32; 3]) -> SceneObject {
    SceneObject {
        id,
        mesh: imported_quad(),
        transform: Transform3D::scale([scale, scale, 1.0])
            .unwrap()
            .then(Transform3D::translation([0.0, 0.0, z]).unwrap())
            .unwrap(),
        material: BasicMaterial::new(color).unwrap(),
    }
}
fn snapshot(revision: u64, reverse: bool) -> SceneSnapshot {
    let mut animated = object(1, -2.0, 1.0, [0.5, 0.25, 0.75]);
    let movement = TranslationAnimation::new(vec![
        TranslationKeyframe {
            time_seconds: 0.0,
            translation: [-0.2, 0.0, -2.0],
        },
        TranslationKeyframe {
            time_seconds: 1.0,
            translation: [0.2, 0.0, -2.0],
        },
    ])
    .unwrap();
    animated
        .apply_translation_animation(&movement, 0.5)
        .unwrap();
    let mut objects = vec![animated, object(2, -4.0, 3.0, [0.0, 0.0, 1.0])];
    if reverse {
        objects.reverse();
    }
    // These full-screen quads must be clipped, despite being drawn last.
    objects.extend([
        object(3, 2.0, 10.0, [0.0, 1.0, 0.0]),
        object(4, -0.5, 10.0, [0.0, 1.0, 0.0]),
        object(5, -11.0, 100.0, [0.0, 1.0, 0.0]),
    ]);
    SceneSnapshot::new(Revision::from_raw(revision), camera(), objects).unwrap()
}
fn lit_snapshot(revision: u64, reverse: bool) -> SceneSnapshot {
    lit_snapshot_with_light(revision, reverse, [0.0, 0.0, 1.0])
}
fn lit_snapshot_with_light(revision: u64, reverse: bool, direction: [f32; 3]) -> SceneSnapshot {
    let vertices = vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
    let indices = if reverse {
        vec![0, 2, 1]
    } else {
        vec![0, 1, 2]
    };
    let light = DirectionalLight::new(direction, [1.0; 3], 2.0).unwrap();
    SceneSnapshot::new(
        Revision::from_raw(revision),
        camera(),
        vec![SceneObject {
            id: 90,
            mesh: Arc::new(Mesh3D::new(vertices, indices).unwrap()),
            transform: Transform3D::translation([0.0, 0.0, -2.0]).unwrap(),
            material: BasicMaterial::metallic_roughness([0.8, 0.35, 0.15], 0.0, 0.5, light)
                .unwrap(),
        }],
    )
    .unwrap()
}
fn render(
    context: &GpuContext,
    session: &mut ProviderSession<SceneProvider>,
    target: &RenderTarget,
) -> Result<Readback, RenderError> {
    let revision = session.provider().snapshot().revision();
    let mut encoder = context.device().create_command_encoder(&Default::default());
    assert!(
        session
            .encode_direct(context, &mut encoder, target, revision)
            .is_err()
    );
    session.encode_offscreen(context, &mut encoder, target, revision)?;
    assert!(session.provider_mut().is_err());
    assert!(session.resize([32, 32]).is_err());
    context.queue().submit([encoder.finish()]);
    session.submitted(context, revision)?;
    assert!(session.submitted(context, revision).is_err());
    let mut renderer = Renderer::new(context.clone(), target.format(), Budgets::default())?;
    let image = wait(renderer.import_texture(
        target.texture().clone(),
        context.generation(),
        TextureMetadata {
            alpha: AlphaMode::Premultiplied,
            ..Default::default()
        },
    ))?;
    let size = target.size();
    let paint = PaintSnapshot {
        revision,
        size,
        scale: ScaleFactor::ONE,
        items: vec![Paint::Draw(Draw::new(
            Primitive::Image {
                image,
                rect: LogicalRect::new(
                    LogicalPoint::ZERO,
                    LogicalSize::new(size[0] as f32, size[1] as f32),
                ),
            },
            Color::WHITE,
        ))],
    };
    let output = context.target(size)?;
    let pixels = renderer
        .prepare(&paint)?
        .submit_capture(context, &output, Color::TRANSPARENT)?
        .readback(context)?;
    session.retire_completed();
    assert!(session.provider_mut().is_ok());
    Ok(pixels)
}
fn pixel(pixels: &Readback, x: usize, y: usize, expected: [u8; 4]) {
    let actual = &pixels.rgba[(y * pixels.size[0] as usize + x) * 4..][..4];
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
        "({x},{y}): {actual:?} expected {expected:?}"
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
    let context = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        wgpu::Limits::default(),
    ))?;
    let target = context.target([64, 64])?;
    let mut session = ProviderSession::new(
        SceneProvider::new(snapshot(1, false), SceneBudgets::default())?,
        &context,
        AttachmentContract {
            format: target.format(),
            size: target.size(),
            sample_count: 1,
            isolated: true,
        },
    )?;
    let pixels = render(&context, &mut session, &target)?;
    pixel(&pixels, 32, 32, [128, 64, 191, 255]);
    pixel(&pixels, 10, 32, [0, 0, 255, 255]);
    pixel(&pixels, 2, 2, [0, 0, 0, 0]);
    session.provider_mut()?.set_snapshot(snapshot(2, true))?;
    let reversed = render(&context, &mut session, &target)?;
    assert_eq!(
        pixels.rgba, reversed.rgba,
        "depth must not depend on draw order"
    );
    // New frame clears depth: an empty scene clears color too.
    session.provider_mut()?.set_snapshot(SceneSnapshot::new(
        Revision::from_raw(3),
        camera(),
        vec![],
    )?)?;
    let empty = render(&context, &mut session, &target)?;
    assert!(empty.rgba.iter().all(|byte| *byte == 0));
    session.provider_mut()?.set_snapshot(snapshot(4, false))?;
    session.resize([128, 64])?;
    let wide = context.target([128, 64])?;
    let resized = render(&context, &mut session, &wide)?;
    pixel(&resized, 64, 32, [128, 64, 191, 255]);
    pixel(&resized, 42, 32, [0, 0, 255, 255]);
    pixel(&resized, 30, 32, [0, 0, 0, 0]);
    assert!(session.resize([0, 64]).is_err());
    let mut encoder = context.device().create_command_encoder(&Default::default());
    assert!(
        session
            .encode_offscreen(&context, &mut encoder, &target, Revision::from_raw(4))
            .is_err()
    );
    assert!(
        session
            .encode_offscreen(&context, &mut encoder, &wide, Revision::from_raw(999))
            .is_err()
    );
    // A replacement device requires explicit recreation and new targets.
    let other = wait(GpuContext::request(
        &adapter,
        wgpu::Features::empty(),
        wgpu::Limits::default(),
    ))?;
    let replacement = other.target([128, 64])?;
    let mut encoder = other.device().create_command_encoder(&Default::default());
    assert!(matches!(
        session.encode_offscreen(&other, &mut encoder, &replacement, Revision::from_raw(4)),
        Err(RenderError::StaleResource)
    ));
    session.recreate(&other)?;
    let recreated = render(&other, &mut session, &replacement)?;
    assert_eq!(resized.rgba, recreated.rgba);
    session
        .provider_mut()?
        .set_snapshot(lit_snapshot(5, false))?;
    let lit = render(&other, &mut session, &replacement)?;
    session
        .provider_mut()?
        .set_snapshot(lit_snapshot(6, true))?;
    let unlit_face = render(&other, &mut session, &replacement)?;
    let center = (32 * lit.size[0] as usize + 64) * 4;
    assert!(
        lit.rgba[center] > unlit_face.rgba[center] + 60,
        "directional light should brighten the correctly oriented face: lit={:?}, back={:?}",
        &lit.rgba[center..center + 4],
        &unlit_face.rgba[center..center + 4]
    );
    session.resize([129, 65])?;
    session
        .provider_mut()?
        .set_snapshot(lit_snapshot_with_light(7, false, [0.0, 0.0, -1.0]))?;
    let opposed = render(&other, &mut session, &other.target([129, 65])?)?;
    let center = (32 * 129 + 64) * 4;
    // At this odd-sized target's center the view/light halfway vector is zero.
    // Back lighting contributes no direct radiance; the analytic result is ambient.
    for (channel, color) in [0.8_f64, 0.35, 0.15].into_iter().enumerate() {
        let linear = ((color + 0.055) / 1.055).powf(2.4) * 0.03;
        let encoded = if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        let expected = (encoded * 255.0).round() as u8;
        assert!(opposed.rgba[center + channel].abs_diff(expected) <= 2);
    }
    assert_eq!(opposed.rgba[center + 3], 255);
    assert!(context.take_errors().is_empty());
    assert!(other.take_errors().is_empty());
    std::fs::create_dir_all("target/verification")?;
    let mut ppm = b"P6\n64 64\n255\n".to_vec();
    for p in pixels.rgba.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write("target/verification/m3b-scene.ppm", ppm)?;
    println!(
        "PASS scene depth/order, imported asset, animated transform, derived normals, directional metallic-roughness shading, clipping, sRGB, composition, resize, revision and recreation; pixel tolerance 2/255"
    );
    Ok(())
}
