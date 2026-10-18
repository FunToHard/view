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
fn object(id: u64, z: f32, scale: f32, color: [f32; 3]) -> SceneObject {
    SceneObject {
        id,
        mesh: Arc::new(
            Mesh3D::new(
                vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                ],
                vec![0, 1, 2, 0, 2, 3],
            )
            .unwrap(),
        ),
        transform: Transform3D::scale([scale, scale, 1.0])
            .unwrap()
            .then(Transform3D::translation([0.0, 0.0, z]).unwrap())
            .unwrap(),
        material: BasicMaterial::new(color).unwrap(),
    }
}
fn snapshot(revision: u64, reverse: bool) -> SceneSnapshot {
    let mut objects = vec![
        object(1, -2.0, 1.0, [0.5, 0.25, 0.75]),
        object(2, -4.0, 3.0, [0.0, 0.0, 1.0]),
    ];
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
    assert!(context.take_errors().is_empty());
    assert!(other.take_errors().is_empty());
    std::fs::create_dir_all("target/verification")?;
    let mut ppm = b"P6\n64 64\n255\n".to_vec();
    for p in pixels.rgba.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write("target/verification/m3b-scene.ppm", ppm)?;
    println!(
        "PASS scene depth/order, model transforms, near/far/behind clipping, sRGB material, same-device composition, clear, aspect/resize, revision rejection and device recreation; pixel tolerance 2/255"
    );
    Ok(())
}
