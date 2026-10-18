//! Opt-in hidden native surface fixture; does not activate or seize input.
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_core::{Revision, ScaleFactor};
use view_wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<T>(future: impl Future<Output = T>) -> Result<T, Box<dyn std::error::Error>> {
    let end = Instant::now() + Duration::from_secs(30);
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return Ok(value);
        }
        let remaining = end
            .checked_duration_since(Instant::now())
            .ok_or("GPU request timeout")?;
        std::thread::park_timeout(remaining);
    }
}
#[derive(Default)]
struct App {
    result: Option<Result<(), String>>,
}
impl App {
    fn exercise(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("view hidden renderer verification")
                    .with_visible(false)
                    .with_active(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(64, 64)),
            )?,
        );
        let backend = match std::env::args().nth(2).as_deref() {
            Some("dx12") => wgpu::Backends::DX12,
            Some("vulkan") => wgpu::Backends::VULKAN,
            Some("metal") => wgpu::Backends::METAL,
            _ => return Err("specify backend after --run-hidden".into()),
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                event_loop.owned_display_handle(),
            ))
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))??;
        println!("hidden native surface adapter={:?}", adapter.get_info());
        let context = wait(GpuContext::request(
            &adapter,
            wgpu::Features::empty(),
            wgpu::Limits::default(),
        ))??;
        let mut surface = WindowSurface::new(surface, &adapter, &context, [64, 64])?;
        let mut renderer = Renderer::new(context.clone(), surface.format(), Budgets::default())?;
        for size in [[64, 64], [96, 80], [32, 48]] {
            surface.resize([20, 20]);
            surface.resize(size);
            let snapshot = PaintSnapshot {
                revision: Revision::INITIAL,
                size,
                scale: ScaleFactor::ONE,
                items: vec![],
            };
            match surface.acquire(&context)? {
                Acquire::Ready(frame) => {
                    let receipt = frame.present(
                        &context,
                        renderer.prepare(&snapshot)?,
                        Color([0.2, 0.3, 0.4, 1.0]),
                    )?;
                    assert_eq!(receipt.revision, Revision::INITIAL);
                    context.device().poll(wgpu::PollType::Wait {
                        submission_index: Some(receipt.submission.index),
                        timeout: Some(Duration::from_secs(30)),
                    })?;
                }
                _ => return Err("hidden surface unavailable; no presentation certification".into()),
            }
            surface.resize([0, 0]);
            assert!(matches!(surface.acquire(&context)?, Acquire::Suspended));
            surface.resize(size);
            surface.set_occluded(true);
            assert!(matches!(surface.acquire(&context)?, Acquire::Suspended));
            surface.set_occluded(false);
        }
        assert!(context.take_errors().is_empty());
        println!(
            "PASS hidden native surface acquire/submit/present, coalesced resize, zero-size and occlusion suspension; no visible scanout or physical-input certification"
        );
        Ok(())
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.result = Some(self.exercise(event_loop).map_err(|e| e.to_string()));
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("--run-hidden") {
        return Err("explicit --run-hidden required".into());
    }
    let mut app = App::default();
    EventLoop::new()?.run_app(&mut app)?;
    app.result.ok_or("no native callback")?.map_err(Into::into)
}
