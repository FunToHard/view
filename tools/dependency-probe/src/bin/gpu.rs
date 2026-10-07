//! Explicit opt-in GPU compute/readback qualification, never part of cargo test.
use std::future::Future;
use std::sync::{Arc, mpsc};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn wait<T>(future: impl Future<Output = T>) -> Result<T, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Ok(value);
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("GPU request timed out")?;
        std::thread::park_timeout(remaining);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = match std::env::args().nth(1).as_deref() {
        Some("dx12") => wgpu::Backends::DX12,
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("metal") => wgpu::Backends::METAL,
        _ => return Err("Specify one backend: dx12, vulkan or metal".into()),
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = wait(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))??;
    let info = adapter.get_info();
    println!(
        "os={} arch={} adapter={info:?}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "adapter_features={:?}\nadapter_limits={:?}",
        adapter.features(),
        adapter.limits()
    );
    println!(
        "device_type={:?}; CPU denotes software; Other/Unknown is not certified hardware",
        info.device_type
    );
    let (device, queue) = wait(adapter.request_device(&wgpu::DeviceDescriptor::default()))??;
    // Construct glyphon's GPU objects against the exact same device/queue types.
    let _text_objects = dependency_probe::text_renderer_compatibility(&device, &queue);
    let storage = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sequence-output"),
        size: 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sequence-readback"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("owned-sequence-fixture"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!("../../../../tests/fixtures/shaders/sequence.wgsl").into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("sequence"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage.as_entire_binding(),
        }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&storage, 0, &readback, 0, 16);
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    rx.recv_timeout(Duration::from_secs(30))??;
    let mapped = readback.slice(..).get_mapped_range()?;
    let actual: Vec<u32> = mapped
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_le_bytes(*bytes))
        .collect();
    if actual != [7, 10, 13, 16] {
        return Err(format!("GPU mismatch: {actual:?}").into());
    }
    drop(mapped);
    readback.unmap();
    println!(
        "PASS WGSL dispatch + readback [7, 10, 13, 16]; no window, text raster or performance certification"
    );
    Ok(())
}
