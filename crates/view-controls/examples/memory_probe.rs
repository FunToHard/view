//! Opt-in staged GPU memory probe. Hidden window, no injected native input.
use std::{
    future::Future,
    io::Write,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};
use view_controls::{Button, Label, Style, TextField, frame};
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision, ScaleFactor};
use view_text::{CosmicBackend, EditorConfig};
use view_wgpu::{
    Acquire, Budgets, Color, GpuContext, PaintSnapshot, Renderer, WindowSurface, wgpu,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park_timeout(Duration::from_millis(10)),
        }
    }
}
fn checkpoint(name: &str) {
    println!("STAGE {name}");
    std::io::stdout().flush().expect("probe stdout");
    // Diagnostic-only hold: external sampler measures a quiet cumulative stage.
    std::thread::sleep(Duration::from_secs(3));
    // Keep next-stage initialization out of this quiet stage's samples.
    println!("STAGE transition");
    std::io::stdout().flush().expect("probe stdout");
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
    LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
}
fn settle(context: &GpuContext) -> Result<()> {
    context.device().poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(30)),
    })?;
    if !context.take_errors().is_empty() {
        return Err("GPU validation/device error".into());
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct FrameTiming {
    prepare_ms: f64,
    submit_ms: f64,
}
fn report_times(workload: &str, stage: &str, samples: &[f64]) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |percent: usize| sorted[(sorted.len() * percent).div_ceil(100) - 1];
    println!(
        "TIMING {{\"workload\":\"{workload}\",\"stage\":\"{stage}\",\"samples\":{},\"first_ms\":{:.6},\"p50_ms\":{:.6},\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"max_ms\":{:.6}}}",
        samples.len(),
        samples[0],
        percentile(50),
        percentile(95),
        percentile(99),
        sorted[sorted.len() - 1],
    );
}
fn check_cache(renderer: &Renderer, fonts: &CosmicBackend, budgets: &Budgets) -> Result<()> {
    let stats = renderer.cache_stats();
    if stats.text_entries > budgets.max_text_entries
        || stats.buffer_entries > budgets.max_buffer_entries
        || stats.texture_bytes > budgets.texture_bytes
        || stats.buffer_bytes > budgets.buffer_bytes
        || stats.geometry_bytes > budgets.geometry_bytes
        || stats.glyph_bytes > budgets.glyph_bytes
        || fonts.cached_layouts() > 128
    {
        return Err(format!("retained cache exceeded configured budget: {stats:?}").into());
    }
    Ok(())
}
fn present(
    context: &GpuContext,
    surface: &mut WindowSurface<'_>,
    renderer: &mut Renderer,
    snapshot: &PaintSnapshot,
) -> Result<FrameTiming> {
    context.device().poll(wgpu::PollType::Poll)?;
    let started = Instant::now();
    let mut prepared = Some(renderer.prepare(snapshot)?);
    let prepare_ms = started.elapsed().as_secs_f64() * 1000.0;
    let submit_started = Instant::now();
    surface.resize(snapshot.size);
    for _ in 0..8 {
        match surface.acquire(context)? {
            Acquire::Ready(frame) => {
                frame.present(
                    context,
                    prepared.take().unwrap(),
                    Color([0.06, 0.075, 0.1, 1.0]),
                )?;
                return Ok(FrameTiming {
                    prepare_ms,
                    submit_ms: submit_started.elapsed().as_secs_f64() * 1000.0,
                });
            }
            Acquire::Retry | Acquire::Reconfigure => std::thread::sleep(Duration::from_millis(2)),
            Acquire::Suspended => return Err("probe unexpectedly suspended".into()),
        }
    }
    Err("probe surface unavailable after retries".into())
}
fn form(
    field: &mut TextField,
    buttons: &[Button; 2],
    fonts: &mut CosmicBackend,
    size: [u32; 2],
    now: u64,
) -> Result<PaintSnapshot> {
    let style = Style::default();
    let bounds = rect(20.0, 20.0, size[0] as f32 - 40.0, 44.0);
    let mut items = frame(bounds, &style);
    items.extend(
        field
            .prepare(fonts, &style, bounds, ScaleFactor::ONE, now)?
            .paint(&style)?,
    );
    for (i, button) in buttons.iter().enumerate() {
        let bounds = rect(20.0 + i as f32 * 130.0, 90.0, 110.0, 36.0);
        items.extend(frame(bounds, &style));
        items.push(Label::new(&button.label).paint(
            fonts,
            &style,
            rect(bounds.origin.x + 8.0, 98.0, 94.0, 22.0),
            ScaleFactor::ONE,
        )?);
    }
    Ok(PaintSnapshot {
        revision: Revision::INITIAL,
        size,
        scale: ScaleFactor::ONE,
        items,
    })
}
#[derive(Default)]
struct App {
    result: Option<std::result::Result<(), String>>,
    memory_policy: String,
}
impl App {
    fn exercise(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("view GPU memory probe")
                    .with_visible(false)
                    .with_active(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(640, 360)),
            )?,
        );
        checkpoint("native_window");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                event_loop.owned_display_handle(),
            ))
        });
        let raw_surface = instance.create_surface(window.clone())?;
        let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&raw_surface),
            ..Default::default()
        }))?;
        println!("ADAPTER {:?}", adapter.get_info());
        if !matches!(
            adapter.get_info().device_type,
            wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::DiscreteGpu
        ) {
            return Err("a positively identified hardware adapter is required".into());
        }
        checkpoint("adapter_surface");
        let memory_hints = match self.memory_policy.as_str() {
            "default" => wgpu::MemoryHints::Performance,
            "usage" => wgpu::MemoryHints::MemoryUsage,
            "manual16" => wgpu::MemoryHints::Manual {
                suballocated_device_memory_block_size: 16 * 1024 * 1024..64 * 1024 * 1024,
            },
            other => return Err(format!("unsupported memory policy {other}").into()),
        };
        let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints,
            ..Default::default()
        }))?;
        let context =
            GpuContext::from_device(device, queue, view_wgpu::DeviceErrorPolicy::Capture)?;
        checkpoint("gpu_device");
        report_allocator(&context, "gpu_device");
        let mut surface = WindowSurface::new(raw_surface, &adapter, &context, [640, 360])?;
        match surface.acquire(&context)? {
            Acquire::Ready(frame) => drop(frame),
            _ => return Err("initial swapchain unavailable".into()),
        }
        settle(&context)?;
        checkpoint("swapchain");
        let budgets = Budgets::default();
        let mut renderer = Renderer::new(context.clone(), surface.format(), budgets)?;
        let empty = PaintSnapshot {
            revision: Revision::INITIAL,
            size: [640, 360],
            scale: ScaleFactor::ONE,
            items: vec![],
        };
        present(&context, &mut surface, &mut renderer, &empty)?;
        settle(&context)?;
        checkpoint("renderer_clear");
        let mut fonts = CosmicBackend::system(128);
        checkpoint("system_fonts");
        let label = Label::new("GPU memory probe").paint(
            &mut fonts,
            &Style::default(),
            rect(20.0, 20.0, 400.0, 40.0),
            ScaleFactor::ONE,
        )?;
        checkpoint("shaped_text");
        present(
            &context,
            &mut surface,
            &mut renderer,
            &PaintSnapshot {
                items: vec![label],
                ..empty
            },
        )?;
        settle(&context)?;
        checkpoint("rendered_text");
        let mut field = TextField::new("Text", "Hello", EditorConfig::default())?;
        field.set_focused(true, 0);
        let buttons = [Button::new("Apply"), Button::new("Cancel")];
        present(
            &context,
            &mut surface,
            &mut renderer,
            &form(&mut field, &buttons, &mut fonts, [640, 360], 0)?,
        )?;
        settle(&context)?;
        checkpoint("controls_idle");
        report_allocator(&context, "controls_idle");
        println!("STAGE resizing");
        let resize_started = Instant::now();
        let mut total_times = Vec::with_capacity(1000);
        let mut prepare_times = Vec::with_capacity(1000);
        let mut submit_times = Vec::with_capacity(1000);
        for i in 0..1000 {
            let frame_started = Instant::now();
            let phase = i % 400;
            let step = if phase < 200 { phase } else { 399 - phase };
            let _ =
                window.request_inner_size(winit::dpi::PhysicalSize::new(640 + step, 360 + step));
            let size = window.inner_size();
            let timing = present(
                &context,
                &mut surface,
                &mut renderer,
                &form(
                    &mut field,
                    &buttons,
                    &mut fonts,
                    [size.width, size.height],
                    i as u64,
                )?,
            )?;
            total_times.push(frame_started.elapsed().as_secs_f64() * 1000.0);
            prepare_times.push(timing.prepare_ms);
            submit_times.push(timing.submit_ms);
            check_cache(&renderer, &fonts, &budgets)?;
        }
        report_times("resize", "cpu_total", &total_times);
        report_times("resize", "renderer_prepare", &prepare_times);
        report_times("resize", "surface_submit", &submit_times);
        report_times("resize", "cpu_total_warm", &total_times[100..]);
        report_times("resize", "renderer_prepare_warm", &prepare_times[100..]);
        report_times("resize", "surface_submit_warm", &submit_times[100..]);
        println!(
            "WORKLOAD resize_iterations=1000 elapsed_ms={}",
            resize_started.elapsed().as_millis()
        );
        settle(&context)?;
        println!("COUNTERS {:?}", renderer.cache_stats());
        checkpoint("after_1000_resizes");
        println!("STAGE editing");
        let size = window.inner_size();
        let edit_started = Instant::now();
        total_times.clear();
        prepare_times.clear();
        submit_times.clear();
        for i in 0..1000 {
            let frame_started = Instant::now();
            if i % 2 == 0 {
                field.insert_text("x", i)?;
            } else {
                field.editor_mut().delete(true, false, i)?;
            }
            let timing = present(
                &context,
                &mut surface,
                &mut renderer,
                &form(
                    &mut field,
                    &buttons,
                    &mut fonts,
                    [size.width, size.height],
                    i,
                )?,
            )?;
            total_times.push(frame_started.elapsed().as_secs_f64() * 1000.0);
            prepare_times.push(timing.prepare_ms);
            submit_times.push(timing.submit_ms);
            check_cache(&renderer, &fonts, &budgets)?;
        }
        report_times("edit", "cpu_total", &total_times);
        report_times("edit", "renderer_prepare", &prepare_times);
        report_times("edit", "surface_submit", &submit_times);
        report_times("edit", "cpu_total_warm", &total_times[100..]);
        report_times("edit", "renderer_prepare_warm", &prepare_times[100..]);
        report_times("edit", "surface_submit_warm", &submit_times[100..]);
        println!(
            "WORKLOAD edit_iterations=1000 elapsed_ms={}",
            edit_started.elapsed().as_millis()
        );
        assert_eq!(field.snapshot().text.text(), "Hello");
        settle(&context)?;
        println!("COUNTERS {:?}", renderer.cache_stats());
        checkpoint("after_1000_edits");
        report_allocator(&context, "after_1000_edits");
        drop((
            field, buttons, fonts, renderer, surface, context, adapter, instance,
        ));
        checkpoint("released_gpu_and_controls");
        println!(
            "PASS hardware DX12 stages; 1000 resizes; 500 insert/delete pairs; no physical input"
        );
        Ok(())
    }
}
fn report_allocator(context: &GpuContext, stage: &str) {
    match context.device().generate_allocator_report() {
        Some(report) => {
            println!(
                "ALLOCATOR {stage} blocks={} allocations={} allocated_bytes={} reserved_bytes={}",
                report.blocks.len(),
                report.allocations.len(),
                report.total_allocated_bytes,
                report.total_reserved_bytes
            );
            let mut allocations = report.allocations;
            allocations.sort_by_key(|allocation| std::cmp::Reverse(allocation.size));
            for allocation in allocations.iter().take(12) {
                println!(
                    "GPU_ALLOCATION {stage} name={:?} bytes={}",
                    allocation.name, allocation.size
                );
            }
        }
        None => println!("ALLOCATOR {stage} unavailable"),
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.result = Some(self.exercise(event_loop).map_err(|e| e.to_string()));
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--run-hidden") {
        return Err("explicit --run-hidden required".into());
    }
    checkpoint("process");
    let policy = args.get(2).map(String::as_str).unwrap_or("default");
    let mut app = App {
        result: None,
        memory_policy: policy.to_owned(),
    };
    EventLoop::new()?.run_app(&mut app)?;
    app.result
        .ok_or("missing native callback")?
        .map_err(Into::into)
}
