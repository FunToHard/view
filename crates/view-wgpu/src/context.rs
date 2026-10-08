use crate::RenderError;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);

/// An unforgeable device epoch. A replacement device always creates a new epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceGeneration(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceErrorPolicy {
    /// Install view's bounded error sink and device-loss callback.
    Capture,
    /// Preserve host callbacks; host forwards loss with `mark_lost`.
    HostManaged,
}

#[derive(Clone)]
pub struct GpuContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    generation: DeviceGeneration,
    errors: Arc<std::sync::Mutex<Vec<String>>>,
    lost: Arc<std::sync::atomic::AtomicBool>,
}
impl GpuContext {
    pub async fn request(
        adapter: &wgpu::Adapter,
        features: wgpu::Features,
        limits: wgpu::Limits,
    ) -> Result<Self, RenderError> {
        if !adapter.features().contains(features) || !limits.check_limits(&adapter.limits()) {
            return Err(RenderError::Unsupported(
                "required device features/limits".into(),
            ));
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: features,
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;
        Self::from_device(device, queue, DeviceErrorPolicy::Capture)
    }
    /// The host must supply the queue returned with this device. wgpu validates
    /// actual resource ownership; no numeric caller-provided device ID is trusted.
    /// Callback replacement is explicit so embedding preserves host policy.
    pub fn from_device(
        device: wgpu::Device,
        queue: wgpu::Queue,
        policy: DeviceErrorPolicy,
    ) -> Result<Self, RenderError> {
        let id = NEXT_CONTEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| RenderError::Device("device namespace exhausted".into()))?;
        let errors = Arc::new(std::sync::Mutex::new(Vec::new()));
        let lost = Arc::new(std::sync::atomic::AtomicBool::new(false));
        if policy == DeviceErrorPolicy::Capture {
            let lost_flag = lost.clone();
            let lost_errors = errors.clone();
            device.set_device_lost_callback(move |reason, message| {
                lost_flag.store(true, Ordering::Release);
                if let Ok(mut messages) = lost_errors.lock()
                    && messages.len() < 64
                {
                    messages.push(format!("device lost {reason:?}: {message}"));
                }
            });
            let sink = errors.clone();
            device.on_uncaptured_error(Arc::new(move |error| {
                if let Ok(mut messages) = sink.lock()
                    && messages.len() < 64
                {
                    messages.push(error.to_string());
                }
            }));
        }
        Ok(Self {
            device,
            queue,
            generation: DeviceGeneration(id),
            errors,
            lost,
        })
    }
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
    pub fn generation(&self) -> DeviceGeneration {
        self.generation
    }
    /// Forward host-observed loss. Existing resources remain invalid even after
    /// messages are drained; create a new context to obtain a fresh generation.
    pub fn mark_lost(&self, message: impl Into<String>) {
        self.lost.store(true, Ordering::Release);
        let mut errors = self.errors.lock().unwrap_or_else(|e| e.into_inner());
        if errors.len() < 64 {
            errors.push(message.into());
        }
    }
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.lock().unwrap_or_else(|e| e.into_inner()))
    }
    pub(crate) fn check(&self, generation: DeviceGeneration) -> Result<(), RenderError> {
        if self.lost.load(Ordering::Acquire) {
            return Err(RenderError::Device(
                "device lost; recreate context and providers".into(),
            ));
        }
        if self.generation == generation {
            Ok(())
        } else {
            Err(RenderError::StaleResource)
        }
    }
    pub fn target(&self, size: [u32; 2]) -> Result<RenderTarget, RenderError> {
        self.target_format(size, wgpu::TextureFormat::Rgba8UnormSrgb)
    }
    pub fn target_format(
        &self,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Result<RenderTarget, RenderError> {
        self.check(self.generation)?;
        validate_size(size, self.device.limits().max_texture_dimension_2d)?;
        validate_format(format)?;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("view offscreen"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(RenderTarget {
            view: texture.create_view(&Default::default()),
            texture,
            generation: self.generation,
            size,
            format,
        })
    }
}
pub(crate) fn validate_size(size: [u32; 2], max: u32) -> Result<(), RenderError> {
    if size.contains(&0) || size.iter().any(|s| *s > max) {
        Err(RenderError::Invalid("target extent".into()))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_format(format: wgpu::TextureFormat) -> Result<(), RenderError> {
    if matches!(
        format,
        wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        Ok(())
    } else {
        Err(RenderError::Unsupported(
            "SDR sRGB RGBA/BGRA attachment required".into(),
        ))
    }
}
pub struct RenderTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) generation: DeviceGeneration,
    pub(crate) size: [u32; 2],
    pub(crate) format: wgpu::TextureFormat,
}
impl RenderTarget {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}
