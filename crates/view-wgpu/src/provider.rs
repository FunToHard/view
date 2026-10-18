use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use view_core::Revision;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceId(pub u64);
#[derive(Clone, Debug)]
pub struct PassDeclaration {
    pub id: u64,
    pub after: Vec<u64>,
    pub reads: Vec<ResourceId>,
    pub writes: Vec<ResourceId>,
}
/// Validates a small external pass graph. Each resource has one writer; imports
/// may be read, but cannot also be written. Dependencies must be explicit.
pub fn validate_passes(
    passes: &[PassDeclaration],
    imports: &[ResourceId],
) -> Result<Vec<u64>, RenderError> {
    let mut ids = BTreeMap::new();
    let mut writers = BTreeMap::new();
    let imports: BTreeSet<_> = imports.iter().copied().collect();
    for p in passes {
        if ids.insert(p.id, p).is_some() {
            return Err(RenderError::Invalid("duplicate pass".into()));
        }
        for resource in &p.writes {
            if imports.contains(resource) || writers.insert(*resource, p.id).is_some() {
                return Err(RenderError::Invalid("multiple resource writers".into()));
            }
        }
        if p.reads.iter().any(|r| p.writes.contains(r)) {
            return Err(RenderError::Invalid("read/write feedback".into()));
        }
    }
    let mut ordered = Vec::new();
    let mut done = BTreeSet::new();
    while ordered.len() < passes.len() {
        let mut progress = false;
        for p in passes {
            if done.contains(&p.id) {
                continue;
            }
            if p.after.iter().any(|id| !ids.contains_key(id)) {
                return Err(RenderError::Invalid("unknown dependency".into()));
            }
            if p.after.iter().all(|id| done.contains(id)) {
                done.insert(p.id);
                ordered.push(p.id);
                progress = true;
            }
        }
        if !progress {
            return Err(RenderError::Invalid("pass dependency cycle".into()));
        }
    }
    fn ancestors(id: u64, ids: &BTreeMap<u64, &PassDeclaration>, found: &mut BTreeSet<u64>) {
        for parent in &ids[&id].after {
            if found.insert(*parent) {
                ancestors(*parent, ids, found);
            }
        }
    }
    for p in passes {
        let mut deps = BTreeSet::new();
        ancestors(p.id, &ids, &mut deps);
        for r in &p.reads {
            if imports.contains(r) {
                continue;
            }
            let writer = writers
                .get(r)
                .ok_or_else(|| RenderError::Invalid("undeclared resource".into()))?;
            if !deps.contains(writer) {
                return Err(RenderError::Invalid("undeclared read dependency".into()));
            }
        }
    }
    Ok(ordered)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContributionMode {
    Offscreen,
    Direct,
}
#[derive(Clone, Copy, Debug)]
pub struct AttachmentContract {
    pub format: wgpu::TextureFormat,
    pub size: [u32; 2],
    pub sample_count: u32,
    pub isolated: bool,
}
impl AttachmentContract {
    pub fn negotiate(self, requested: ContributionMode) -> Result<ContributionMode, RenderError> {
        crate::context::validate_format(self.format)?;
        if self.size.contains(&0) {
            return Err(RenderError::Invalid("provider zero extent".into()));
        }
        if self.sample_count != 1 {
            return Err(RenderError::Unsupported(
                "resolve provider output before composition".into(),
            ));
        }
        Ok(if self.isolated {
            ContributionMode::Offscreen
        } else {
            requested
        })
    }
}
/// A provider must not submit or present through the shared context. Direct contributions are
/// scoped to one compatible render pass; complex effects use offscreen output.
pub trait RenderProvider {
    /// Request a private single-sample Depth32Float attachment, cleared to 1.0.
    /// Depth providers are offscreen-only; the session owns attachment lifetime.
    fn requires_depth(&self) -> bool {
        false
    }
    fn negotiate(&mut self, contract: AttachmentContract) -> Result<ContributionMode, RenderError>;
    fn prepare(&mut self, context: &GpuContext, revision: Revision) -> Result<(), RenderError>;
    fn encode(&mut self, pass: &mut wgpu::RenderPass<'_>) -> Result<(), RenderError>;
    fn retire(&mut self, revision: Revision);
    fn resize(&mut self, size: [u32; 2]) -> Result<(), RenderError>;
    fn recreate(&mut self, context: &GpuContext) -> Result<(), RenderError>;
    fn failed(&mut self, error: &RenderError);
}
/// Coordinates provider callbacks and completion. Polling is caller-controlled;
/// preparation and retirement never imply presentation.
pub struct ProviderSession<P> {
    provider: P,
    generation: DeviceGeneration,
    contract: AttachmentContract,
    mode: ContributionMode,
    pending: Vec<(Revision, std::sync::mpsc::Receiver<()>)>,
    awaiting_submission: Option<Revision>,
    depth: Option<([u32; 2], wgpu::TextureView)>,
    requires_depth: bool,
}
impl<P: RenderProvider> ProviderSession<P> {
    pub fn new(
        mut provider: P,
        context: &GpuContext,
        contract: AttachmentContract,
    ) -> Result<Self, RenderError> {
        contract.negotiate(ContributionMode::Offscreen)?;
        let requested = provider.negotiate(contract)?;
        let mode = contract.negotiate(requested)?;
        let requires_depth = provider.requires_depth();
        if requires_depth && mode != ContributionMode::Offscreen {
            return Err(RenderError::Unsupported(
                "depth providers require offscreen composition".into(),
            ));
        }
        provider.recreate(context)?;
        Ok(Self {
            provider,
            generation: context.generation(),
            contract,
            mode,
            pending: Vec::new(),
            awaiting_submission: None,
            depth: None,
            requires_depth,
        })
    }
    pub fn mode(&self) -> ContributionMode {
        self.mode
    }
    /// Inspect provider state without changing encoded work.
    pub fn provider(&self) -> &P {
        &self.provider
    }
    /// Update a provider only after encoded work is discarded or acknowledged
    /// and completed submissions are retired. Attachment requirements must stay
    /// fixed for this session; create a new session to renegotiate them.
    pub fn provider_mut(&mut self) -> Result<&mut P, RenderError> {
        if self.awaiting_submission.is_some() || !self.pending.is_empty() {
            return Err(RenderError::Invalid("provider has outstanding work".into()));
        }
        Ok(&mut self.provider)
    }
    pub fn resize(&mut self, size: [u32; 2]) -> Result<(), RenderError> {
        if self.awaiting_submission.is_some() {
            return Err(RenderError::Invalid(
                "submit or discard encoded provider work before resize".into(),
            ));
        }
        if size.contains(&0) {
            return Err(RenderError::Invalid("provider suspended".into()));
        }
        self.provider.resize(size)?;
        self.contract.size = size;
        Ok(())
    }
    pub fn recreate(&mut self, context: &GpuContext) -> Result<(), RenderError> {
        if self.awaiting_submission.is_some() || !self.pending.is_empty() {
            self.provider.failed(&RenderError::StaleResource);
        }
        self.provider.recreate(context)?;
        self.generation = context.generation();
        self.pending.clear();
        self.awaiting_submission = None;
        self.depth = None;
        Ok(())
    }
    /// Encode an offscreen contribution. Caller composes the resolved target via
    /// Renderer::import_texture with premultiplied alpha and submits this encoder
    /// before the composition encoder (or uses the same encoder).
    pub fn encode_offscreen(
        &mut self,
        context: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &RenderTarget,
        revision: Revision,
    ) -> Result<(), RenderError> {
        context.check(target.generation)?;
        if target.size != self.contract.size || target.format != self.contract.format {
            return Err(RenderError::Invalid("provider attachment mismatch".into()));
        }
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })];
        self.prepare(context, revision)?;
        if self.requires_depth
            && self
                .depth
                .as_ref()
                .is_none_or(|(size, _)| *size != target.size)
        {
            let texture = context.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("view provider depth"),
                size: wgpu::Extent3d {
                    width: target.size[0],
                    height: target.size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((target.size, texture.create_view(&Default::default())));
        }
        let depth_stencil_attachment =
            self.depth
                .as_ref()
                .map(|(_, view)| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &attachments,
            depth_stencil_attachment,
            ..Default::default()
        });
        let result = self.provider.encode(&mut pass);
        if let Err(error) = &result {
            self.provider.failed(error);
        }
        if result.is_ok() {
            self.awaiting_submission = Some(revision);
        }
        result
    }
    fn prepare(&mut self, context: &GpuContext, revision: Revision) -> Result<(), RenderError> {
        context.check(self.generation)?;
        if self.provider.requires_depth() != self.requires_depth {
            return Err(RenderError::Invalid(
                "provider attachment requirements changed".into(),
            ));
        }
        if self.pending.len() >= 3 || self.awaiting_submission.is_some() {
            return Err(RenderError::BudgetExceeded);
        }
        let result = self.provider.prepare(context, revision);
        if let Err(error) = &result {
            self.provider.failed(error);
        }
        result
    }
    pub fn encode_direct(
        &mut self,
        context: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        target: &RenderTarget,
        revision: Revision,
    ) -> Result<(), RenderError> {
        if self.mode != ContributionMode::Direct || self.contract.isolated {
            return Err(RenderError::Unsupported(
                "direct contribution requires compatible non-isolated attachment".into(),
            ));
        }
        context.check(target.generation)?;
        if target.size != self.contract.size || target.format != self.contract.format {
            return Err(RenderError::Invalid("direct attachment mismatch".into()));
        }
        self.prepare(context, revision)?;
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view: &target.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &attachments,
            ..Default::default()
        });
        let result = self.provider.encode(&mut pass);
        if let Err(error) = &result {
            self.provider.failed(error);
        }
        if result.is_ok() {
            self.awaiting_submission = Some(revision);
        }
        result
    }
    /// Call after the host submits the encoder containing this contribution.
    pub fn submitted(
        &mut self,
        context: &GpuContext,
        revision: Revision,
    ) -> Result<(), RenderError> {
        context.check(self.generation)?;
        if self.awaiting_submission != Some(revision) {
            return Err(RenderError::Invalid(
                "provider submission revision/order".into(),
            ));
        }
        if self.pending.len() >= 3 {
            return Err(RenderError::BudgetExceeded);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        context.queue.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        self.pending.push((revision, rx));
        self.awaiting_submission = None;
        Ok(())
    }
    /// Caller must also discard the encoder containing the abandoned work.
    pub fn discard_encoded(&mut self) {
        self.awaiting_submission = None;
    }
    pub fn retire_completed(&mut self) {
        self.pending.retain(|(revision, rx)| {
            if rx.try_recv().is_ok() {
                self.provider.retire(*revision);
                false
            } else {
                true
            }
        });
    }
}
