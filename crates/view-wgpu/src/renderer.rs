use crate::*;
use std::{collections::VecDeque, sync::Arc};
use view_core::{LogicalPoint, LogicalRect, LogicalSize, Revision};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    straight: f32,
}
const _: () = assert!(std::mem::size_of::<Vertex>() == 36);

#[derive(Clone, Copy, Debug)]
pub struct Budgets {
    pub texture_bytes: u64,
    pub geometry_bytes: u64,
    pub buffer_bytes: u64,
    pub glyph_bytes: u64,
    /// Conservative per-frame allocation ceiling (including isolated layers).
    pub frame_bytes: u64,
    pub max_draws: usize,
    pub max_layer_depth: usize,
    /// Bounds simultaneously prepared/submitted frames, including retired cache
    /// resources held by their immutable bind groups. Poll the device to retire.
    pub max_frames: usize,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            texture_bytes: 64 << 20,
            geometry_bytes: 16 << 20,
            buffer_bytes: 16 << 20,
            glyph_bytes: 512 << 20,
            frame_bytes: 1 << 30,
            max_draws: 4096,
            max_layer_depth: 16,
            max_frames: 3,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheStats {
    pub texture_bytes: u64,
    pub geometry_bytes: u64,
    pub texture_evictions: u64,
    pub geometry_hits: u64,
    pub buffer_bytes: u64,
    pub buffer_hits: u64,
    pub glyph_bytes: u64,
    pub glyph_hits: u64,
    pub glyph_evictions: u64,
}
struct Image {
    id: ImageId,
    texture: wgpu::Texture,
    metadata: TextureMetadata,
    bytes: u64,
}
#[derive(Clone)]
struct Mesh {
    vertices: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
struct Geometry {
    key: Vec<u32>,
    mesh: Mesh,
    bytes: u64,
}
struct CachedBuffers {
    key: Vec<u8>,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    bytes: u64,
}
struct CachedText {
    layout: Arc<view_text::ShapedText>,
    key: Vec<u32>,
    text: Arc<TextDraw>,
}
struct TextDraw {
    renderer: glyphon::TextRenderer,
    atlas: Arc<std::sync::Mutex<glyphon::TextAtlas>>,
    viewport: glyphon::Viewport,
    clip: [u32; 4],
}
struct AtlasEpoch {
    layout: Arc<view_text::ShapedText>,
    atlas: Arc<std::sync::Mutex<glyphon::TextAtlas>>,
}

/// A renderer caches immutable images/geometry. GPU buffers and glyph atlases are
/// frame-owned: wgpu retains encoded resources until submitted work completes.
pub struct Renderer {
    context: GpuContext,
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    white: wgpu::Texture,
    images: VecDeque<Image>,
    geometry: VecDeque<Geometry>,
    buffers: std::cell::RefCell<VecDeque<CachedBuffers>>,
    buffer_stats: std::cell::Cell<(u64, u64)>,
    texts: VecDeque<CachedText>,
    atlases: Vec<AtlasEpoch>,
    serial: u64,
    budgets: Budgets,
    stats: CacheStats,
    glyph_cache: glyphon::Cache,
    live_frames: Arc<std::sync::atomic::AtomicUsize>,
}
impl Renderer {
    pub fn new(
        context: GpuContext,
        format: wgpu::TextureFormat,
        budgets: Budgets,
    ) -> Result<Self, RenderError> {
        crate::context::validate_format(format)?;
        let device = &context.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("view 2D"),
            source: wgpu::ShaderSource::Wgsl(include_str!("quad.wgsl").into()),
        });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4,3=>Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("view linear premultiplied"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("view linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = upload(
            &context,
            [1, 1],
            &[255; 4],
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let glyph_cache = glyphon::Cache::new(device);
        Ok(Self {
            context,
            format,
            pipeline,
            sampler,
            white,
            images: VecDeque::new(),
            geometry: VecDeque::new(),
            buffers: Default::default(),
            buffer_stats: Default::default(),
            texts: VecDeque::new(),
            atlases: Vec::new(),
            serial: 0,
            budgets,
            stats: CacheStats::default(),
            glyph_cache,
            live_frames: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }
    pub fn context(&self) -> &GpuContext {
        &self.context
    }
    pub fn cache_stats(&self) -> CacheStats {
        let (buffer_bytes, buffer_hits) = self.buffer_stats.get();
        CacheStats {
            buffer_bytes,
            buffer_hits,
            ..self.stats
        }
    }
    pub fn live_frames(&self) -> usize {
        self.live_frames.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn clear_caches(&mut self) {
        self.images.clear();
        self.geometry.clear();
        self.buffers.get_mut().clear();
        self.buffer_stats.set((0, self.buffer_stats.get().1));
        self.texts.clear();
        self.stats.glyph_evictions += self.atlases.len() as u64;
        self.atlases.clear();
        self.stats.glyph_bytes = 0;
        self.stats.texture_bytes = 0;
        self.stats.geometry_bytes = 0;
    }
    pub fn upload_image(
        &mut self,
        size: [u32; 2],
        rgba: &[u8],
        metadata: TextureMetadata,
    ) -> Result<ImageId, RenderError> {
        crate::context::validate_size(size, self.context.device.limits().max_texture_dimension_2d)?;
        let bytes = u64::from(size[0]) * u64::from(size[1]) * 4;
        if rgba.len() as u64 != bytes {
            return Err(RenderError::Invalid("RGBA byte length".into()));
        }
        if bytes > self.budgets.texture_bytes {
            return Err(RenderError::BudgetExceeded);
        }
        if metadata.alpha == AlphaMode::Opaque
            && rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
        {
            return Err(RenderError::Invalid("opaque image alpha".into()));
        }
        let format = match metadata.transfer {
            Transfer::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Transfer::Linear => wgpu::TextureFormat::Rgba8Unorm,
        };
        let texture = upload(&self.context, size, rgba, format);
        self.insert_texture(texture, metadata)
    }
    /// Imports a same-device texture. wgpu validates the actual device during
    /// binding; the context epoch also rejects resources from replaced devices.
    /// Premultiplied sRGB bytes must encode *linear-premultiplied* RGB channels.
    pub async fn import_texture(
        &mut self,
        texture: wgpu::Texture,
        generation: DeviceGeneration,
        metadata: TextureMetadata,
    ) -> Result<ImageId, RenderError> {
        self.context.check(generation)?;
        if !texture
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || texture.sample_count() != 1
            || texture.dimension() != wgpu::TextureDimension::D2
            || texture.depth_or_array_layers() != 1
            || texture.mip_level_count() != 1
        {
            return Err(RenderError::Unsupported(
                "sampled D2 single-layer resolved texture required".into(),
            ));
        }
        let valid = match metadata.transfer {
            Transfer::Srgb => matches!(
                texture.format(),
                wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb
            ),
            Transfer::Linear => matches!(
                texture.format(),
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
            ),
        };
        if !valid {
            return Err(RenderError::Unsupported(
                "texture transfer/format mismatch".into(),
            ));
        }
        // A caller can obtain our epoch but still pass a foreign wgpu object.
        // Validate the real backend ownership under an error scope before caching.
        let scope = self
            .context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let view = texture.create_view(&Default::default());
        let _validation = self
            .context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("view import ownership validation"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
        if let Some(error) = scope.pop().await {
            return Err(RenderError::Device(error.to_string()));
        }
        self.insert_texture(texture, metadata)
    }
    fn insert_texture(
        &mut self,
        texture: wgpu::Texture,
        metadata: TextureMetadata,
    ) -> Result<ImageId, RenderError> {
        let bytes = u64::from(texture.width()) * u64::from(texture.height()) * 4;
        if bytes > self.budgets.texture_bytes {
            return Err(RenderError::BudgetExceeded);
        }
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or(RenderError::StaleResource)?;
        while self.stats.texture_bytes + bytes > self.budgets.texture_bytes {
            let old = self.images.pop_front().ok_or(RenderError::BudgetExceeded)?;
            self.stats.texture_bytes -= old.bytes;
            self.stats.texture_evictions += 1;
        }
        let id = ImageId {
            generation: self.context.generation(),
            serial: self.serial,
        };
        self.images.push_back(Image {
            id,
            texture,
            metadata,
            bytes,
        });
        self.stats.texture_bytes += bytes;
        Ok(id)
    }
    pub fn prepare(&mut self, snapshot: &PaintSnapshot) -> Result<PreparedFrame, RenderError> {
        self.context.check(self.context.generation())?;
        self.live_frames
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |n| (n < self.budgets.max_frames).then_some(n + 1),
            )
            .map_err(|_| RenderError::BudgetExceeded)?;
        let lease = Arc::new(FrameLease(self.live_frames.clone()));
        crate::context::validate_size(
            snapshot.size,
            self.context.device.limits().max_texture_dimension_2d,
        )?;
        let mut accounting = Accounting {
            bytes: 0,
            draws: 0,
            atlases: Vec::new(),
        };
        let nodes = self.prepare_items(&snapshot.items, snapshot, 0, &mut accounting)?;
        Ok(PreparedFrame {
            generation: self.context.generation(),
            revision: snapshot.revision,
            size: snapshot.size,
            format: self.format,
            nodes,
            pipeline: self.pipeline.clone(),
            bytes: accounting.bytes,
            lease,
        })
    }
    fn prepare_items(
        &mut self,
        items: &[Paint],
        snapshot: &PaintSnapshot,
        depth: usize,
        accounting: &mut Accounting,
    ) -> Result<Vec<Node>, RenderError> {
        if depth > self.budgets.max_layer_depth {
            return Err(RenderError::BudgetExceeded);
        }
        let mut nodes = Vec::new();
        for item in items {
            accounting.draws += 1;
            if accounting.draws > self.budgets.max_draws {
                return Err(RenderError::BudgetExceeded);
            }
            match item {
                Paint::Layer { opacity, items } => {
                    if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                        return Err(RenderError::Invalid("layer opacity".into()));
                    }
                    accounting.add(
                        u64::from(snapshot.size[0]) * u64::from(snapshot.size[1]) * 4,
                        self.budgets.frame_bytes,
                    )?;
                    let target = self.context.target_format(snapshot.size, self.format)?;
                    let children = self.prepare_items(items, snapshot, depth + 1, accounting)?;
                    let rect = LogicalRect::new(
                        LogicalPoint::ZERO,
                        LogicalSize::new(
                            snapshot.size[0] as f32 / snapshot.scale.get(),
                            snapshot.size[1] as f32 / snapshot.scale.get(),
                        ),
                    );
                    let draw = Draw::new(Primitive::Rect(rect), Color([1.0, 1.0, 1.0, *opacity]));
                    let mesh = rect_mesh(rect)?;
                    let composite = self.prepare_mesh(
                        &draw,
                        mesh,
                        &target.texture,
                        TextureMetadata {
                            alpha: AlphaMode::Premultiplied,
                            ..Default::default()
                        },
                        snapshot,
                        accounting,
                    )?;
                    nodes.push(Node::Layer {
                        target: Box::new(target),
                        children,
                        composite,
                    });
                }
                Paint::Draw(draw) => {
                    draw.color.linear_premultiplied()?;
                    if !draw.transform.is_valid() || draw.clip.is_some_and(|c| !c.is_valid()) {
                        return Err(RenderError::Invalid("draw geometry".into()));
                    }
                    let clip = scissor(draw.clip, snapshot)?;
                    if clip[2] == 0 || clip[3] == 0 {
                        continue;
                    }
                    match &draw.primitive {
                        Primitive::Text { layout, origin } => {
                            let [a, b, c, d, _, _] = draw.transform.matrix();
                            if a <= 0.0 || a != d || b != 0.0 || c != 0.0 || !origin.is_valid() {
                                return Err(RenderError::Unsupported(
                                    "text requires positive uniform scale/translation".into(),
                                ));
                            }
                            // glyphon can grow two atlases to the device limit. Reserve
                            // their worst-case bytes before allowing any allocation.
                            let side =
                                u64::from(self.context.device.limits().max_texture_dimension_2d);
                            let glyph_count = layout.with_raster_source(|buffer, _| {
                                buffer
                                    .layout_runs()
                                    .map(|run| run.glyphs.len())
                                    .sum::<usize>()
                            });
                            let atlas_bytes = side * side * 5;
                            let text_bytes =
                                (glyph_count as u64 * 64).next_power_of_two().max(4096);
                            accounting.add(text_bytes, self.budgets.frame_bytes)?;
                            let mut key = draw.transform.matrix().map(f32::to_bits).to_vec();
                            key.extend([
                                origin.x.to_bits(),
                                origin.y.to_bits(),
                                snapshot.scale.get().to_bits(),
                                snapshot.size[0],
                                snapshot.size[1],
                            ]);
                            key.extend(draw.color.0.map(f32::to_bits));
                            key.extend(clip);
                            if let Some(i) = self.texts.iter().position(|cached| {
                                Arc::ptr_eq(&cached.layout, layout) && cached.key == key
                            }) {
                                let cached = self.texts.remove(i).unwrap();
                                accounting.atlas(
                                    &cached.text.atlas,
                                    atlas_bytes,
                                    self.budgets.frame_bytes,
                                )?;
                                nodes.push(Node::Text(cached.text.clone()));
                                self.texts.push_back(cached);
                                self.stats.glyph_hits += 1;
                                continue;
                            }
                            if text_bytes + atlas_bytes > self.budgets.glyph_bytes {
                                return Err(RenderError::BudgetExceeded);
                            }
                            let mut existing = self
                                .atlases
                                .iter()
                                .position(|epoch| epoch.layout.shares_font_collection(layout));
                            if self.stats.glyph_bytes
                                + text_bytes
                                + if existing.is_some() { 0 } else { atlas_bytes }
                                > self.budgets.glyph_bytes
                            {
                                self.stats.glyph_evictions += self.atlases.len() as u64;
                                self.texts.clear();
                                self.atlases.clear();
                                self.stats.glyph_bytes = 0;
                                existing = None;
                            }
                            let atlas_handle = if let Some(i) = existing {
                                self.atlases[i].atlas.clone()
                            } else {
                                let atlas =
                                    Arc::new(std::sync::Mutex::new(glyphon::TextAtlas::new(
                                        &self.context.device,
                                        &self.context.queue,
                                        &self.glyph_cache,
                                        self.format,
                                    )));
                                self.atlases.push(AtlasEpoch {
                                    layout: layout.clone(),
                                    atlas: atlas.clone(),
                                });
                                self.stats.glyph_bytes += atlas_bytes;
                                atlas
                            };
                            accounting.atlas(
                                &atlas_handle,
                                atlas_bytes,
                                self.budgets.frame_bytes,
                            )?;
                            // No trimming or slot reuse within a live epoch. Whole
                            // epoch eviction preserves old prepared glyph references.
                            let mut atlas = atlas_handle.lock().unwrap_or_else(|e| e.into_inner());
                            let mut text = glyphon::TextRenderer::new(
                                &mut atlas,
                                &self.context.device,
                                Default::default(),
                                None,
                            );
                            let mut viewport =
                                glyphon::Viewport::new(&self.context.device, &self.glyph_cache);
                            viewport.update(
                                &self.context.queue,
                                glyphon::Resolution {
                                    width: snapshot.size[0],
                                    height: snapshot.size[1],
                                },
                            );
                            let point = draw
                                .transform
                                .transform_point(*origin)
                                .to_physical(snapshot.scale);
                            let color = draw.color.0.map(|v| (v * 255.0).round() as u8);
                            let prepared = layout.with_raster_source(|buffer, fonts| {
                                text.prepare(
                                    &self.context.device,
                                    &self.context.queue,
                                    fonts,
                                    &mut atlas,
                                    &viewport,
                                    [glyphon::TextArea {
                                        buffer,
                                        left: point.x,
                                        top: point.y,
                                        scale: a * snapshot.scale.get(),
                                        bounds: glyphon::TextBounds {
                                            left: clip[0] as i32,
                                            top: clip[1] as i32,
                                            right: (clip[0] + clip[2]) as i32,
                                            bottom: (clip[1] + clip[3]) as i32,
                                        },
                                        default_color: glyphon::Color::rgba(
                                            color[0], color[1], color[2], color[3],
                                        ),
                                        custom_glyphs: &[],
                                    }],
                                    &mut glyphon::SwashCache::new(),
                                )
                            });
                            drop(atlas);
                            if prepared.is_err() {
                                self.stats.glyph_evictions += self.atlases.len() as u64;
                                self.texts.clear();
                                self.atlases.clear();
                                self.stats.glyph_bytes = 0;
                                return Err(RenderError::BudgetExceeded);
                            }
                            let text = Arc::new(TextDraw {
                                renderer: text,
                                atlas: atlas_handle,
                                viewport,
                                clip,
                            });
                            self.texts.push_back(CachedText {
                                layout: layout.clone(),
                                key,
                                text: text.clone(),
                            });
                            self.stats.glyph_bytes += text_bytes;
                            nodes.push(Node::Text(text));
                        }
                        Primitive::Image { image, rect } => {
                            let mesh = rect_mesh(*rect)?;
                            self.context.check(image.generation)?;
                            let index = self
                                .images
                                .iter()
                                .position(|i| i.id == *image)
                                .ok_or(RenderError::StaleResource)?;
                            let image = self.images.remove(index).unwrap();
                            let result = self.prepare_mesh(
                                draw,
                                mesh,
                                &image.texture,
                                image.metadata,
                                snapshot,
                                accounting,
                            );
                            self.images.push_back(image);
                            nodes.push(Node::Mesh(result?));
                        }
                        primitive => {
                            let mesh = self.geometry(primitive)?;
                            nodes.push(Node::Mesh(self.prepare_mesh(
                                draw,
                                mesh,
                                &self.white,
                                TextureMetadata::default(),
                                snapshot,
                                accounting,
                            )?));
                        }
                    }
                }
            }
        }
        Ok(nodes)
    }
    fn geometry(&mut self, primitive: &Primitive) -> Result<Mesh, RenderError> {
        let (path, width) = match primitive {
            Primitive::Rect(rect) => return rect_mesh(*rect),
            Primitive::Fill(path) => (path, None),
            Primitive::Stroke { path, width } => {
                if !width.is_finite() || *width <= 0.0 {
                    return Err(RenderError::Invalid("stroke width".into()));
                }
                (path, Some(*width))
            }
            _ => return Err(RenderError::Invalid("mesh primitive".into())),
        };
        let mut key = vec![width.map_or(0, f32::to_bits)];
        for event in path.iter() {
            use lyon::path::Event;
            let (tag, points) = match event {
                Event::Begin { at } => (0, vec![at]),
                Event::Line { from, to } => (1, vec![from, to]),
                Event::Quadratic { from, ctrl, to } => (2, vec![from, ctrl, to]),
                Event::Cubic {
                    from,
                    ctrl1,
                    ctrl2,
                    to,
                } => (3, vec![from, ctrl1, ctrl2, to]),
                Event::End { last, first, close } => (if close { 4 } else { 5 }, vec![last, first]),
            };
            key.push(tag);
            for p in points {
                if !p.x.is_finite() || !p.y.is_finite() {
                    return Err(RenderError::Invalid("path point".into()));
                }
                key.extend([p.x.to_bits(), p.y.to_bits()]);
            }
        }
        if let Some(i) = self.geometry.iter().position(|g| g.key == key) {
            let entry = self.geometry.remove(i).unwrap();
            let mesh = entry.mesh.clone();
            self.geometry.push_back(entry);
            self.stats.geometry_hits += 1;
            return Ok(mesh);
        }
        use lyon::tessellation::*;
        let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
        if let Some(width) = width {
            StrokeTessellator::new().tessellate_path(
                path,
                &StrokeOptions::default().with_line_width(width),
                &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex<'_, '_>| {
                    v.position().to_array()
                }),
            )
        } else {
            FillTessellator::new().tessellate_path(
                path,
                &FillOptions::default(),
                &mut BuffersBuilder::new(&mut buffers, |v: FillVertex<'_>| v.position().to_array()),
            )
        }
        .map_err(|e| RenderError::Invalid(format!("tessellation: {e:?}")))?;
        let mesh = Mesh {
            vertices: buffers.vertices,
            indices: buffers.indices,
        };
        let bytes = (mesh.vertices.len() * 8 + mesh.indices.len() * 4 + key.len() * 4) as u64;
        if bytes > self.budgets.geometry_bytes {
            return Err(RenderError::BudgetExceeded);
        }
        while self.stats.geometry_bytes + bytes > self.budgets.geometry_bytes {
            self.stats.geometry_bytes -= self
                .geometry
                .pop_front()
                .ok_or(RenderError::BudgetExceeded)?
                .bytes;
        }
        self.stats.geometry_bytes += bytes;
        self.geometry.push_back(Geometry {
            key,
            mesh: mesh.clone(),
            bytes,
        });
        Ok(mesh)
    }
    fn prepare_mesh(
        &self,
        draw: &Draw,
        mesh: Mesh,
        texture: &wgpu::Texture,
        metadata: TextureMetadata,
        snapshot: &PaintSnapshot,
        accounting: &mut Accounting,
    ) -> Result<GpuMesh, RenderError> {
        let color = draw.color.linear_premultiplied()?;
        let rect = match &draw.primitive {
            Primitive::Rect(r) | Primitive::Image { rect: r, .. } => Some(*r),
            _ => None,
        };
        let mut vertices = Vec::with_capacity(mesh.vertices.len());
        for p in &mesh.vertices {
            let point = draw
                .transform
                .transform_point(LogicalPoint::new(p[0], p[1]))
                .to_physical(snapshot.scale);
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(RenderError::Invalid("transformed vertex".into()));
            }
            let mut uv = rect.map_or([0.5; 2], |r| {
                [
                    (p[0] - r.origin.x) / r.size.width.max(f32::MIN_POSITIVE),
                    (p[1] - r.origin.y) / r.size.height.max(f32::MIN_POSITIVE),
                ]
            });
            if metadata.origin == Origin::BottomLeft {
                uv[1] = 1.0 - uv[1];
            }
            vertices.push(Vertex {
                position: [
                    point.x / snapshot.size[0] as f32 * 2.0 - 1.0,
                    1.0 - point.y / snapshot.size[1] as f32 * 2.0,
                ],
                uv,
                color,
                straight: if metadata.alpha == AlphaMode::Straight {
                    1.0
                } else {
                    0.0
                },
            });
        }
        let bytes = (vertices.len() * 36 + mesh.indices.len() * 4) as u64;
        accounting.add(bytes, self.budgets.frame_bytes)?;
        if bytes > self.context.device.limits().max_buffer_size {
            return Err(RenderError::BudgetExceeded);
        }
        let device = &self.context.device;
        let mut key = bytemuck::cast_slice(&vertices).to_vec();
        key.extend_from_slice(bytemuck::cast_slice(&mesh.indices));
        // Include the split point; identical concatenation must not alias distinct layouts.
        key.extend_from_slice(&(vertices.len() as u64).to_le_bytes());
        let mut cache = self.buffers.borrow_mut();
        let (mut resident, mut hits) = self.buffer_stats.get();
        let cached = cache
            .iter()
            .position(|entry| entry.key == key)
            .map(|i| cache.remove(i).unwrap());
        let (vertex, index) = if let Some(entry) = cached {
            hits += 1;
            let pair = (entry.vertex.clone(), entry.index.clone());
            cache.push_back(entry);
            pair
        } else {
            let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("view vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("view indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            // Key bytes and GPU bytes both count toward this cache's budget.
            let resident_bytes = bytes + key.len() as u64;
            if resident_bytes <= self.budgets.buffer_bytes {
                while resident + resident_bytes > self.budgets.buffer_bytes {
                    resident -= cache.pop_front().ok_or(RenderError::BudgetExceeded)?.bytes;
                }
                resident += resident_bytes;
                cache.push_back(CachedBuffers {
                    key,
                    vertex: vertex.clone(),
                    index: index.clone(),
                    bytes: resident_bytes,
                });
            }
            (vertex, index)
        };
        self.buffer_stats.set((resident, hits));
        let view = texture.create_view(&Default::default());
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view sampled image"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Ok(GpuMesh {
            vertex,
            index,
            bindings,
            count: mesh.indices.len() as u32,
            clip: scissor(draw.clip, snapshot)?,
            texture: texture.clone(),
        })
    }
}
fn upload(
    context: &GpuContext,
    size: [u32; 2],
    rgba: &[u8],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let texture = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("view image"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    context.queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * 4),
            rows_per_image: Some(size[1]),
        },
        texture.size(),
    );
    texture
}
fn rect_mesh(rect: LogicalRect) -> Result<Mesh, RenderError> {
    if !rect.is_valid() {
        return Err(RenderError::Invalid("rectangle".into()));
    }
    let x = rect.origin.x;
    let y = rect.origin.y;
    let r = x + rect.size.width;
    let b = y + rect.size.height;
    Ok(Mesh {
        vertices: vec![[x, y], [r, y], [r, b], [x, b]],
        indices: vec![0, 1, 2, 0, 2, 3],
    })
}
fn scissor(clip: Option<LogicalRect>, snapshot: &PaintSnapshot) -> Result<[u32; 4], RenderError> {
    let Some(c) = clip else {
        return Ok([0, 0, snapshot.size[0], snapshot.size[1]]);
    };
    if !c.is_valid() {
        return Err(RenderError::Invalid("clip".into()));
    }
    let s = snapshot.scale.get();
    // The committed clip is half-open. A pixel belongs when its center lies
    // inside that clip; rounding outward would leak pixels across fractional DPI.
    let x = (c.origin.x * s - 0.5)
        .ceil()
        .clamp(0.0, snapshot.size[0] as f32) as u32;
    let y = (c.origin.y * s - 0.5)
        .ceil()
        .clamp(0.0, snapshot.size[1] as f32) as u32;
    let r = ((c.origin.x + c.size.width) * s - 0.5)
        .ceil()
        .clamp(0.0, snapshot.size[0] as f32) as u32;
    let b = ((c.origin.y + c.size.height) * s - 0.5)
        .ceil()
        .clamp(0.0, snapshot.size[1] as f32) as u32;
    Ok([
        x,
        y,
        if c.size.width == 0.0 {
            0
        } else {
            r.saturating_sub(x)
        },
        if c.size.height == 0.0 {
            0
        } else {
            b.saturating_sub(y)
        },
    ])
}
struct Accounting {
    bytes: u64,
    draws: usize,
    atlases: Vec<Arc<std::sync::Mutex<glyphon::TextAtlas>>>,
}
impl Accounting {
    fn atlas(
        &mut self,
        atlas: &Arc<std::sync::Mutex<glyphon::TextAtlas>>,
        bytes: u64,
        budget: u64,
    ) -> Result<(), RenderError> {
        if !self.atlases.iter().any(|other| Arc::ptr_eq(other, atlas)) {
            self.add(bytes, budget)?;
            self.atlases.push(atlas.clone());
        }
        Ok(())
    }
    fn add(&mut self, bytes: u64, budget: u64) -> Result<(), RenderError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(RenderError::BudgetExceeded)?;
        if self.bytes > budget {
            Err(RenderError::BudgetExceeded)
        } else {
            Ok(())
        }
    }
}
struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    count: u32,
    clip: [u32; 4],
    texture: wgpu::Texture,
}
enum Node {
    Mesh(GpuMesh),
    Text(Arc<TextDraw>),
    Layer {
        target: Box<RenderTarget>,
        children: Vec<Node>,
        composite: GpuMesh,
    },
}
pub struct PreparedFrame {
    generation: DeviceGeneration,
    revision: Revision,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    nodes: Vec<Node>,
    pipeline: wgpu::RenderPipeline,
    bytes: u64,
    lease: Arc<FrameLease>,
}
struct FrameLease(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for FrameLease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
impl PreparedFrame {
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn reserved_bytes(&self) -> u64 {
        self.bytes
    }
    pub fn encode(
        &self,
        context: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &RenderTarget,
        clear: Color,
    ) -> Result<(), RenderError> {
        context.check(target.generation)?;
        fn samples(nodes: &[Node], texture: &wgpu::Texture) -> bool {
            nodes.iter().any(|node| match node {
                Node::Mesh(mesh) => mesh.texture == *texture,
                Node::Layer { children, .. } => samples(children, texture),
                Node::Text(_) => false,
            })
        }
        if samples(&self.nodes, &target.texture) {
            return Err(RenderError::Invalid(
                "paint target sampled by its own frame".into(),
            ));
        }
        self.encode_view(
            context,
            encoder,
            &target.view,
            target.size,
            target.format,
            clear,
        )
    }
    pub(crate) fn encode_view(
        &self,
        context: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        clear: Color,
    ) -> Result<(), RenderError> {
        context.check(self.generation)?;
        if size != self.size || format != self.format {
            return Err(RenderError::Invalid("prepared target mismatch".into()));
        }
        encode_nodes(&self.nodes, &self.pipeline, encoder, view, clear)
    }
    pub fn submit(
        self,
        context: &GpuContext,
        target: &RenderTarget,
        clear: Color,
    ) -> Result<Submission, RenderError> {
        let mut encoder = context.device.create_command_encoder(&Default::default());
        self.encode(context, &mut encoder, target, clear)?;
        let index = context.queue.submit([encoder.finish()]);
        self.submitted(context, index)
    }
    /// Host-owned submission acknowledgement. Call immediately after submitting
    /// the encoder produced by `encode`. GPU retirement releases the frame slot.
    pub fn submitted(
        self,
        context: &GpuContext,
        index: wgpu::SubmissionIndex,
    ) -> Result<Submission, RenderError> {
        context.check(self.generation)?;
        let lease = self.lease.clone();
        context.queue.on_submitted_work_done(move || drop(lease));
        Ok(Submission {
            revision: self.revision,
            index,
            generation: self.generation,
        })
    }
    /// Copies pixels in the same command buffer as painting, before any later
    /// target reuse. The capture's revision therefore identifies its actual pixels.
    pub fn submit_capture(
        self,
        context: &GpuContext,
        target: &RenderTarget,
        clear: Color,
    ) -> Result<Capture, RenderError> {
        let mut encoder = context.device.create_command_encoder(&Default::default());
        let capture = self.encode_capture(context, &mut encoder, target, clear)?;
        let index = context.queue.submit([encoder.finish()]);
        capture.submitted(context, index)
    }
    /// Record painting and diagnostic readback in the host's encoder without
    /// submitting it. Acknowledge the host's submission once with the result.
    pub fn encode_capture(
        self,
        context: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &RenderTarget,
        clear: Color,
    ) -> Result<EncodedCapture, RenderError> {
        self.encode(context, encoder, target, clear)?;
        let row = (target.size[0] * 4).div_ceil(256) * 256;
        let bytes = u64::from(row) * u64::from(target.size[1]);
        if bytes > context.device.limits().max_buffer_size {
            return Err(RenderError::BudgetExceeded);
        }
        let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("view revision capture"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(target.size[1]),
                },
            },
            target.texture.size(),
        );
        Ok(EncodedCapture {
            frame: self,
            buffer,
            row,
            size: target.size,
            format: target.format,
        })
    }
}
/// Host-recorded readback. Dropping requires discarding its unsubmitted encoder.
pub struct EncodedCapture {
    frame: PreparedFrame,
    buffer: wgpu::Buffer,
    row: u32,
    size: [u32; 2],
    format: wgpu::TextureFormat,
}
impl EncodedCapture {
    /// Call after the host submits the encoder; this method never submits.
    pub fn submitted(
        self,
        context: &GpuContext,
        index: wgpu::SubmissionIndex,
    ) -> Result<Capture, RenderError> {
        Ok(Capture {
            submission: self.frame.submitted(context, index)?,
            buffer: self.buffer,
            row: self.row,
            size: self.size,
            format: self.format,
        })
    }
}
/// Submission is not presentation. Readback carries the committed paint revision.
pub struct Submission {
    pub revision: Revision,
    pub index: wgpu::SubmissionIndex,
    pub(crate) generation: DeviceGeneration,
}
pub struct Readback {
    pub revision: Revision,
    pub size: [u32; 2],
    pub rgba: Vec<u8>,
}
pub struct Capture {
    pub submission: Submission,
    buffer: wgpu::Buffer,
    row: u32,
    size: [u32; 2],
    format: wgpu::TextureFormat,
}
impl Capture {
    pub fn readback(self, context: &GpuContext) -> Result<Readback, RenderError> {
        context.check(self.submission.generation)?;
        let buffer = self.buffer;
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(self.submission.index),
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .map_err(|e| RenderError::Device(e.to_string()))?;
        rx.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|e| RenderError::Device(e.to_string()))?
            .map_err(|e| RenderError::Device(e.to_string()))?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| RenderError::Device(e.to_string()))?;
        let mut rgba = Vec::new();
        for row in mapped.chunks_exact(self.row as usize) {
            rgba.extend_from_slice(&row[..self.size[0] as usize * 4]);
        }
        if self.format == wgpu::TextureFormat::Bgra8UnormSrgb {
            for p in rgba.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(Readback {
            revision: self.submission.revision,
            size: self.size,
            rgba,
        })
    }
}
fn encode_nodes(
    nodes: &[Node],
    pipeline: &wgpu::RenderPipeline,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    clear: Color,
) -> Result<(), RenderError> {
    let color = clear.linear_premultiplied()?;
    for node in nodes {
        if let Node::Layer {
            target, children, ..
        } = node
        {
            encode_nodes(
                children,
                pipeline,
                encoder,
                &target.view,
                Color::TRANSPARENT,
            )?;
        }
    }
    let attachments = [Some(wgpu::RenderPassColorAttachment {
        view: target,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color {
                r: color[0] as f64,
                g: color[1] as f64,
                b: color[2] as f64,
                a: color[3] as f64,
            }),
            store: wgpu::StoreOp::Store,
        },
    })];
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &attachments,
        ..Default::default()
    });
    for node in nodes {
        match node {
            Node::Mesh(mesh)
            | Node::Layer {
                composite: mesh, ..
            } => {
                if mesh.count == 0 || mesh.clip[2] == 0 || mesh.clip[3] == 0 {
                    continue;
                }
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &mesh.bindings, &[]);
                pass.set_vertex_buffer(0, mesh.vertex.slice(..));
                pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
                let [x, y, w, h] = mesh.clip;
                pass.set_scissor_rect(x, y, w, h);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
            Node::Text(text) => {
                let [x, y, w, h] = text.clip;
                pass.set_scissor_rect(x, y, w, h);
                let atlas = text.atlas.lock().unwrap_or_else(|e| e.into_inner());
                text.renderer
                    .render(&atlas, &text.viewport, &mut pass)
                    .map_err(|e| RenderError::Device(e.to_string()))?;
            }
        }
    }
    Ok(())
}
