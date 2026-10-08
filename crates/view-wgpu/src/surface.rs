use crate::*;
use view_core::Revision;

/// Resize requests coalesce until acquisition. Suspended/occluded windows do not
/// configure or acquire. Lost surfaces require explicit host recreation.
#[derive(Clone, Debug)]
pub struct SurfaceState {
    size: [u32; 2],
    pending: Option<[u32; 2]>,
    occluded: bool,
}
impl SurfaceState {
    pub fn new(size: [u32; 2]) -> Self {
        Self {
            size,
            pending: Some(size),
            occluded: false,
        }
    }
    pub fn resize(&mut self, size: [u32; 2]) {
        self.pending = Some(size);
    }
    pub fn set_occluded(&mut self, value: bool) {
        if self.occluded && !value {
            self.pending = Some(self.pending.unwrap_or(self.size));
        }
        self.occluded = value;
    }
    pub fn apply(&mut self) -> Option<[u32; 2]> {
        let pending = self.pending.take();
        if let Some(size) = pending {
            self.size = size;
        }
        pending.filter(|_| !self.suspended())
    }
    pub fn suspended(&self) -> bool {
        self.occluded || self.size.contains(&0)
    }
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
}
pub enum Acquire<'a> {
    Ready(SurfaceFrame<'a>),
    Suspended,
    Retry,
    Reconfigure,
}
pub struct SurfaceFrame<'a> {
    texture: wgpu::SurfaceTexture,
    generation: DeviceGeneration,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    owner: std::marker::PhantomData<&'a mut ()>,
}
pub struct Presentation {
    pub revision: Revision,
    pub submission: Submission,
}
pub struct WindowSurface<'a> {
    surface: wgpu::Surface<'a>,
    config: wgpu::SurfaceConfiguration,
    state: SurfaceState,
    generation: DeviceGeneration,
    configured: bool,
}
impl<'a> WindowSurface<'a> {
    /// The surface must have participated in adapter selection through
    /// RequestAdapterOptions::compatible_surface. Native ownership stays in wgpu.
    pub fn new(
        surface: wgpu::Surface<'a>,
        adapter: &wgpu::Adapter,
        context: &GpuContext,
        size: [u32; 2],
    ) -> Result<Self, RenderError> {
        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| crate::context::validate_format(*f).is_ok())
            .ok_or_else(|| RenderError::Unsupported("surface SDR format".into()))?;
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|a| *a == wgpu::CompositeAlphaMode::Opaque)
            .ok_or_else(|| RenderError::Unsupported("opaque native surface".into()))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size[0].max(1),
            height: size[1].max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        Ok(Self {
            surface,
            config,
            state: SurfaceState::new(size),
            generation: context.generation(),
            configured: false,
        })
    }
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
    pub fn resize(&mut self, size: [u32; 2]) {
        self.state.resize(size);
    }
    pub fn set_occluded(&mut self, value: bool) {
        self.state.set_occluded(value);
    }
    pub fn acquire(&mut self, context: &GpuContext) -> Result<Acquire<'_>, RenderError> {
        context.check(self.generation)?;
        if self.state.apply().is_some() {
            self.configured = false;
        }
        if self.state.suspended() {
            return Ok(Acquire::Suspended);
        }
        if !self.configured {
            let size = self.state.size();
            crate::context::validate_size(size, context.device.limits().max_texture_dimension_2d)?;
            self.config.width = size[0];
            self.config.height = size[1];
            self.surface.configure(&context.device, &self.config);
            self.configured = true;
        }
        Ok(match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => Acquire::Ready(SurfaceFrame {
                texture,
                generation: self.generation,
                size: self.state.size(),
                format: self.config.format,
                owner: std::marker::PhantomData,
            }),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Acquire::Retry
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                self.configured = false;
                Acquire::Reconfigure
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err(RenderError::SurfaceLost),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::Device("surface acquisition validation".into()));
            }
        })
    }
}
impl SurfaceFrame<'_> {
    /// Consumes the acquired frame exactly once; dropping without this call
    /// discards it. Queue presentation acknowledgement is not scanout evidence.
    pub fn present(
        self,
        context: &GpuContext,
        prepared: PreparedFrame,
        clear: Color,
    ) -> Result<Presentation, RenderError> {
        context.check(self.generation)?;
        let view = self.texture.texture.create_view(&Default::default());
        let mut encoder = context.device.create_command_encoder(&Default::default());
        prepared.encode_view(context, &mut encoder, &view, self.size, self.format, clear)?;
        let revision = prepared.revision();
        let index = context.queue.submit([encoder.finish()]);
        let submission = prepared.submitted(context, index)?;
        context.queue.present(self.texture);
        Ok(Presentation {
            revision,
            submission,
        })
    }
}
