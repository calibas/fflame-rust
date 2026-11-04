//! A wgpu custom backend whose every call is a command for the host.
//!
//! Objects are integer handles the module allocates; the host keeps the
//! real ones (wgpu-py's) in a table under the same numbers. A command
//! encoder records on this side, so a whole command buffer crosses to the
//! host once, at submit. Buffer maps run when the device is polled, as
//! wgpu-core runs them, and so do submitted-work callbacks.

use std::cell::{Cell, RefCell};
use std::future::ready;
use std::ops::Range;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wgpu::custom::*;

use crate::host;

thread_local! {
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
    static PENDING_MAPS: RefCell<Vec<PendingMap>> = const { RefCell::new(Vec::new()) };
    static WORK_DONE: RefCell<Vec<BoxSubmittedWorkDoneCallback>> = const { RefCell::new(Vec::new()) };
    /// Buffers mapped now, and whether for writing.
    static MAPPED: RefCell<Vec<(u32, bool)>> = const { RefCell::new(Vec::new()) };
    static SUBMISSION: Cell<u64> = const { Cell::new(0) };
    static ERROR_SCOPES: Cell<u32> = const { Cell::new(0) };
}

fn next_id() -> u32 {
    NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}

/// A wgpu enum by its WebGPU name (wgpu-types' serde names are WebGPU's).
fn name<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).expect("a wgpu enum serializes")
}

fn label(l: Option<&str>) -> Value {
    l.map(Value::from).unwrap_or(Value::Null)
}

struct PendingMap {
    buffer: u32,
    write: bool,
    range: Range<u64>,
    callback: BufferMapCallback,
}

/// Runs the queued maps and work-done callbacks. The device's poll does,
/// and so does the module's executor while a future is still pending.
pub fn flush() {
    let maps = PENDING_MAPS.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for m in maps {
        host::send(&json!({
            "op": "map", "buffer": m.buffer, "write": m.write,
            "offset": m.range.start, "size": m.range.end - m.range.start,
        }));
        MAPPED.with(|v| v.borrow_mut().push((m.buffer, m.write)));
        (m.callback)(Ok(()));
    }
    let done = WORK_DONE.with(|w| std::mem::take(&mut *w.borrow_mut()));
    if !done.is_empty() {
        host::send(&json!({ "op": "wait" }));
        for cb in done {
            cb();
        }
    }
}

// ---------------------------------------------------------------------
// Objects that are only a handle
// ---------------------------------------------------------------------

#[derive(Debug)]
pub struct Handle(pub u32);

impl Drop for Handle {
    fn drop(&mut self) {
        host::send(&json!({ "op": "drop", "id": self.0 }));
    }
}

fn handle<T: std::fmt::Debug>(x: Option<&Handle>, what: &T) -> u32 {
    x.unwrap_or_else(|| panic!("not this backend's: {what:?}")).0
}

impl BindGroupLayoutInterface for Handle {}
impl BindGroupInterface for Handle {}
impl TextureViewInterface for Handle {}
impl SamplerInterface for Handle {}
impl QuerySetInterface for Handle {}
impl PipelineLayoutInterface for Handle {}
impl TlasInterface for Handle {}
impl RenderBundleInterface for Handle {}

impl ShaderModuleInterface for Handle {
    fn get_compilation_info(&self) -> Pin<Box<dyn ShaderCompilationInfoFuture>> {
        Box::pin(ready(wgpu::CompilationInfo { messages: Vec::new() }))
    }
}

fn pipeline_bind_group_layout(pipeline: u32, index: u32) -> DispatchBindGroupLayout {
    let id = next_id();
    host::send(&json!({ "op": "get_bind_group_layout", "id": id, "pipeline": pipeline, "index": index }));
    DispatchBindGroupLayout::custom(Handle(id))
}

impl ComputePipelineInterface for Handle {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        pipeline_bind_group_layout(self.0, index)
    }
}

impl RenderPipelineInterface for Handle {
    fn get_bind_group_layout(&self, index: u32) -> DispatchBindGroupLayout {
        pipeline_bind_group_layout(self.0, index)
    }
}

impl PipelineCacheInterface for Handle {
    fn get_data(&self) -> Option<Vec<u8>> {
        None
    }
}

impl ExternalTextureInterface for Handle {
    fn destroy(&self) {}
}

impl BlasInterface for Handle {
    fn prepare_compact_async(&self, _callback: BlasCompactCallback) {
        unimplemented!("acceleration structures")
    }
    fn ready_for_compaction(&self) -> bool {
        false
    }
}

fn buffer_id(b: &wgpu::Buffer) -> u32 {
    b.as_custom::<Buf>().expect("this backend's buffer").id
}
fn dbuffer_id(b: &DispatchBuffer) -> u32 {
    b.as_custom::<Buf>().expect("this backend's buffer").id
}
fn texture_id(t: &wgpu::Texture) -> u32 {
    t.as_custom::<Tex>().expect("this backend's texture").id
}
fn view_id(v: &wgpu::TextureView) -> u32 {
    handle(v.as_custom::<Handle>(), v)
}

// ---------------------------------------------------------------------
// Buffers and textures
// ---------------------------------------------------------------------

#[derive(Debug)]
pub struct Buf {
    id: u32,
}

impl Drop for Buf {
    fn drop(&mut self) {
        host::send(&json!({ "op": "drop", "id": self.id }));
    }
}

fn mapped_for_write(buffer: u32) -> Option<bool> {
    MAPPED.with(|v| v.borrow().iter().find(|(b, _)| *b == buffer).map(|(_, w)| *w))
}

impl BufferInterface for Buf {
    fn map_async(&self, mode: wgpu::MapMode, range: Range<wgpu::BufferAddress>, callback: BufferMapCallback) {
        let write = matches!(mode, wgpu::MapMode::Write);
        PENDING_MAPS.with(|p| p.borrow_mut().push(PendingMap { buffer: self.id, write, range, callback }));
    }

    fn get_mapped_range(&self, sub_range: Range<wgpu::BufferAddress>) -> DispatchBufferMappedRange {
        if mapped_for_write(self.id).is_none() {
            flush();
        }
        let write = mapped_for_write(self.id).expect("get_mapped_range on a buffer that is not mapped");
        let size = (sub_range.end - sub_range.start) as usize;
        let data = if write {
            vec![0u8; size]
        } else {
            let bytes = host::call_with(
                &json!({ "op": "read_mapped", "buffer": self.id, "offset": sub_range.start, "size": size }),
                &[],
            );
            assert_eq!(bytes.len(), size, "the host read the whole range");
            bytes
        };
        DispatchBufferMappedRange::custom(Mapped { buffer: self.id, offset: sub_range.start, data, write })
    }

    fn unmap(&self) {
        host::send(&json!({ "op": "unmap", "buffer": self.id }));
        MAPPED.with(|v| v.borrow_mut().retain(|(b, _)| *b != self.id));
    }

    fn destroy(&self) {
        host::send(&json!({ "op": "destroy", "id": self.id }));
    }
}

/// A mapped range: a copy here. A write mapping's bytes go to the host
/// when the range is dropped, which wgpu requires before the unmap.
#[derive(Debug)]
pub struct Mapped {
    buffer: u32,
    offset: u64,
    data: Vec<u8>,
    write: bool,
}

impl Drop for Mapped {
    fn drop(&mut self) {
        if self.write {
            host::call_with(&json!({ "op": "write_mapped", "buffer": self.buffer, "offset": self.offset }), &self.data);
        }
    }
}

impl BufferMappedRangeInterface for Mapped {
    fn len(&self) -> usize {
        self.data.len()
    }
    unsafe fn read_slice(&self) -> &[u8] {
        &self.data
    }
    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        wgpu::WriteOnly::from_mut(self.data.as_mut_slice())
    }
}

#[derive(Debug)]
pub struct Tex {
    id: u32,
}

impl Drop for Tex {
    fn drop(&mut self) {
        host::send(&json!({ "op": "drop", "id": self.id }));
    }
}

impl TextureInterface for Tex {
    fn create_view(&self, desc: &wgpu::TextureViewDescriptor<'_>) -> DispatchTextureView {
        let id = next_id();
        host::send(&json!({
            "op": "create_view", "id": id, "texture": self.id,
            "label": label(desc.label),
            "format": desc.format.map(name),
            "dimension": desc.dimension.map(name),
            "aspect": name(desc.aspect),
            "base_mip_level": desc.base_mip_level,
            "mip_level_count": desc.mip_level_count,
            "base_array_layer": desc.base_array_layer,
            "array_layer_count": desc.array_layer_count,
        }));
        DispatchTextureView::custom(Handle(id))
    }

    fn destroy(&self) {
        host::send(&json!({ "op": "destroy", "id": self.id }));
    }
}

// ---------------------------------------------------------------------
// The device
// ---------------------------------------------------------------------

#[derive(Debug)]
pub struct Dev {
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
    pub info: wgpu::AdapterInfo,
}

fn bind_group_layout_entry(e: &wgpu::BindGroupLayoutEntry) -> Value {
    use wgpu::{BindingType as B, BufferBindingType as BB, SamplerBindingType as SB, TextureSampleType as TS};
    let mut v = json!({ "binding": e.binding, "visibility": e.visibility.bits() });
    match e.ty {
        B::Buffer { ty, has_dynamic_offset, min_binding_size } => {
            let ty = match ty {
                BB::Uniform => "uniform",
                BB::Storage { read_only: true } => "read-only-storage",
                BB::Storage { read_only: false } => "storage",
            };
            v["buffer"] = json!({
                "type": ty, "has_dynamic_offset": has_dynamic_offset,
                "min_binding_size": min_binding_size.map(|s| s.get()).unwrap_or(0),
            });
        }
        B::Sampler(ty) => {
            let ty = match ty {
                SB::Filtering => "filtering",
                SB::NonFiltering => "non-filtering",
                SB::Comparison => "comparison",
            };
            v["sampler"] = json!({ "type": ty });
        }
        B::Texture { sample_type, view_dimension, multisampled } => {
            let sample_type = match sample_type {
                TS::Float { filterable: true } => "float",
                TS::Float { filterable: false } => "unfilterable-float",
                TS::Depth => "depth",
                TS::Sint => "sint",
                TS::Uint => "uint",
            };
            v["texture"] = json!({
                "sample_type": sample_type, "view_dimension": name(view_dimension), "multisampled": multisampled,
            });
        }
        B::StorageTexture { access, format, view_dimension } => {
            v["storage_texture"] = json!({
                "access": name(access), "format": name(format), "view_dimension": name(view_dimension),
            });
        }
        ref other => unimplemented!("binding type {other:?}"),
    }
    v
}

fn bind_group_entry(e: &wgpu::BindGroupEntry<'_>) -> Value {
    use wgpu::BindingResource as R;
    let resource = match &e.resource {
        R::Buffer(b) => json!({
            "buffer": buffer_id(b.buffer), "offset": b.offset, "size": b.size.map(|s| s.get()),
        }),
        R::Sampler(s) => json!({ "sampler": handle(s.as_custom::<Handle>(), s) }),
        R::TextureView(v) => json!({ "texture_view": view_id(v) }),
        other => unimplemented!("binding resource {other:?}"),
    };
    json!({ "binding": e.binding, "resource": resource })
}

fn constants(c: &[(&str, f64)]) -> Value {
    Value::Object(c.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())
}

fn stage(module: &wgpu::ShaderModule, entry_point: Option<&str>, opts: &wgpu::PipelineCompilationOptions<'_>) -> Value {
    json!({
        "module": handle(module.as_custom::<Handle>(), module),
        "entry_point": entry_point,
        "constants": constants(opts.constants),
    })
}

fn blend_component(c: &wgpu::BlendComponent) -> Value {
    json!({ "src_factor": name(c.src_factor), "dst_factor": name(c.dst_factor), "operation": name(c.operation) })
}

impl DeviceInterface for Dev {
    fn features(&self) -> wgpu::Features {
        self.features
    }
    fn limits(&self) -> wgpu::Limits {
        self.limits.clone()
    }
    fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.info.clone()
    }

    fn create_shader_module(
        &self,
        desc: wgpu::ShaderModuleDescriptor<'_>,
        _checks: wgpu::ShaderRuntimeChecks,
    ) -> DispatchShaderModule {
        let code = match desc.source {
            wgpu::ShaderSource::Wgsl(code) => code,
            _ => unimplemented!("shader source other than WGSL"),
        };
        let id = next_id();
        host::call_with(&json!({ "op": "create_shader_module", "id": id, "label": label(desc.label) }), code.as_bytes());
        DispatchShaderModule::custom(Handle(id))
    }

    unsafe fn create_shader_module_passthrough(
        &self,
        _desc: &wgpu::ShaderModuleDescriptorPassthrough<'_>,
    ) -> DispatchShaderModule {
        unimplemented!("passthrough shaders")
    }

    fn create_bind_group_layout(&self, desc: &wgpu::BindGroupLayoutDescriptor<'_>) -> DispatchBindGroupLayout {
        let id = next_id();
        let entries: Vec<Value> = desc.entries.iter().map(bind_group_layout_entry).collect();
        host::send(&json!({ "op": "create_bind_group_layout", "id": id, "label": label(desc.label), "entries": entries }));
        DispatchBindGroupLayout::custom(Handle(id))
    }

    fn create_bind_group(&self, desc: &wgpu::BindGroupDescriptor<'_>) -> DispatchBindGroup {
        let id = next_id();
        let entries: Vec<Value> = desc.entries.iter().map(bind_group_entry).collect();
        host::send(&json!({
            "op": "create_bind_group", "id": id, "label": label(desc.label),
            "layout": handle(desc.layout.as_custom::<Handle>(), desc.layout), "entries": entries,
        }));
        DispatchBindGroup::custom(Handle(id))
    }

    fn create_pipeline_layout(&self, desc: &wgpu::PipelineLayoutDescriptor<'_>) -> DispatchPipelineLayout {
        let id = next_id();
        let layouts: Vec<Value> = desc
            .bind_group_layouts
            .iter()
            .map(|l| match l {
                Some(l) => json!(handle(l.as_custom::<Handle>(), l)),
                None => Value::Null,
            })
            .collect();
        host::send(&json!({
            "op": "create_pipeline_layout", "id": id, "label": label(desc.label), "bind_group_layouts": layouts,
        }));
        DispatchPipelineLayout::custom(Handle(id))
    }

    fn create_render_pipeline(&self, desc: &wgpu::RenderPipelineDescriptor<'_>) -> DispatchRenderPipeline {
        assert!(desc.vertex.buffers.is_empty(), "vertex buffers are not forwarded yet");
        assert!(desc.depth_stencil.is_none(), "depth/stencil is not forwarded yet");
        let id = next_id();
        let p = &desc.primitive;
        let fragment = desc.fragment.as_ref().map(|f| {
            let targets: Vec<Value> = f
                .targets
                .iter()
                .map(|t| match t {
                    Some(t) => json!({
                        "format": name(t.format),
                        "blend": t.blend.map(|b| json!({ "color": blend_component(&b.color), "alpha": blend_component(&b.alpha) })),
                        "write_mask": t.write_mask.bits(),
                    }),
                    None => Value::Null,
                })
                .collect();
            let mut s = stage(f.module, f.entry_point, &f.compilation_options);
            s["targets"] = Value::Array(targets);
            s
        });
        host::send(&json!({
            "op": "create_render_pipeline", "id": id, "label": label(desc.label),
            "layout": desc.layout.map(|l| handle(l.as_custom::<Handle>(), l)),
            "vertex": stage(desc.vertex.module, desc.vertex.entry_point, &desc.vertex.compilation_options),
            "primitive": {
                "topology": name(p.topology),
                "strip_index_format": p.strip_index_format.map(name),
                "front_face": name(p.front_face),
                "cull_mode": p.cull_mode.map(name).unwrap_or(json!("none")),
            },
            "multisample": {
                "count": desc.multisample.count, "mask": desc.multisample.mask,
                "alpha_to_coverage_enabled": desc.multisample.alpha_to_coverage_enabled,
            },
            "fragment": fragment,
        }));
        DispatchRenderPipeline::custom(Handle(id))
    }

    fn create_mesh_pipeline(&self, _desc: &wgpu::MeshPipelineDescriptor<'_>) -> DispatchRenderPipeline {
        unimplemented!("mesh pipelines")
    }

    fn create_compute_pipeline(&self, desc: &wgpu::ComputePipelineDescriptor<'_>) -> DispatchComputePipeline {
        let id = next_id();
        host::send(&json!({
            "op": "create_compute_pipeline", "id": id, "label": label(desc.label),
            "layout": desc.layout.map(|l| handle(l.as_custom::<Handle>(), l)),
            "compute": stage(desc.module, desc.entry_point, &desc.compilation_options),
        }));
        DispatchComputePipeline::custom(Handle(id))
    }

    unsafe fn create_pipeline_cache(&self, _desc: &wgpu::PipelineCacheDescriptor<'_>) -> DispatchPipelineCache {
        DispatchPipelineCache::custom(Handle(0))
    }

    fn create_buffer(&self, desc: &wgpu::BufferDescriptor<'_>) -> DispatchBuffer {
        let id = next_id();
        host::send(&json!({
            "op": "create_buffer", "id": id, "label": label(desc.label),
            "size": desc.size, "usage": desc.usage.bits(), "mapped_at_creation": desc.mapped_at_creation,
        }));
        if desc.mapped_at_creation {
            MAPPED.with(|v| v.borrow_mut().push((id, true)));
        }
        DispatchBuffer::custom(Buf { id })
    }

    fn create_texture(&self, desc: &wgpu::TextureDescriptor<'_>) -> DispatchTexture {
        let id = next_id();
        let s = desc.size;
        host::send(&json!({
            "op": "create_texture", "id": id, "label": label(desc.label),
            "size": [s.width, s.height, s.depth_or_array_layers],
            "mip_level_count": desc.mip_level_count, "sample_count": desc.sample_count,
            "dimension": name(desc.dimension), "format": name(desc.format), "usage": desc.usage.bits(),
            "view_formats": desc.view_formats.iter().map(|f| name(*f)).collect::<Vec<_>>(),
        }));
        DispatchTexture::custom(Tex { id })
    }

    fn create_external_texture(
        &self,
        _desc: &wgpu::ExternalTextureDescriptor<'_>,
        _planes: &[&wgpu::TextureView],
    ) -> DispatchExternalTexture {
        unimplemented!("external textures")
    }

    fn create_blas(
        &self,
        _desc: &wgpu::CreateBlasDescriptor<'_>,
        _sizes: wgpu::BlasGeometrySizeDescriptors,
    ) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures")
    }

    fn create_tlas(&self, _desc: &wgpu::CreateTlasDescriptor<'_>) -> DispatchTlas {
        unimplemented!("acceleration structures")
    }

    fn create_sampler(&self, desc: &wgpu::SamplerDescriptor<'_>) -> DispatchSampler {
        let id = next_id();
        host::send(&json!({
            "op": "create_sampler", "id": id, "label": label(desc.label),
            "address_mode_u": name(desc.address_mode_u),
            "address_mode_v": name(desc.address_mode_v),
            "address_mode_w": name(desc.address_mode_w),
            "mag_filter": name(desc.mag_filter),
            "min_filter": name(desc.min_filter),
            "mipmap_filter": name(desc.mipmap_filter),
            "lod_min_clamp": desc.lod_min_clamp, "lod_max_clamp": desc.lod_max_clamp,
            "compare": desc.compare.map(name), "max_anisotropy": desc.anisotropy_clamp,
        }));
        DispatchSampler::custom(Handle(id))
    }

    fn create_query_set(&self, desc: &wgpu::QuerySetDescriptor<'_>) -> DispatchQuerySet {
        let id = next_id();
        let ty = match desc.ty {
            wgpu::QueryType::Occlusion => "occlusion",
            wgpu::QueryType::Timestamp => "timestamp",
            other => unimplemented!("query type {other:?}"),
        };
        host::send(&json!({ "op": "create_query_set", "id": id, "label": label(desc.label), "type": ty, "count": desc.count }));
        DispatchQuerySet::custom(Handle(id))
    }

    fn create_command_encoder(&self, _desc: &wgpu::CommandEncoderDescriptor<'_>) -> DispatchCommandEncoder {
        DispatchCommandEncoder::custom(Enc { cmds: Rc::default(), keep: Rc::default() })
    }

    fn create_render_bundle_encoder(
        &self,
        _desc: &wgpu::RenderBundleEncoderDescriptor<'_>,
    ) -> DispatchRenderBundleEncoder {
        unimplemented!("render bundles")
    }

    fn set_device_lost_callback(&self, _callback: BoxDeviceLostCallback) {}

    fn on_uncaptured_error(&self, _handler: Arc<dyn wgpu::UncapturedErrorHandler>) {}

    fn push_error_scope(&self, _filter: wgpu::ErrorFilter) -> u32 {
        ERROR_SCOPES.with(|n| {
            let i = n.get();
            n.set(i + 1);
            i
        })
    }

    fn pop_error_scope(&self, _index: u32) -> Pin<Box<dyn PopErrorScopeFuture>> {
        ERROR_SCOPES.with(|n| n.set(n.get().saturating_sub(1)));
        Box::pin(ready(None))
    }

    unsafe fn start_graphics_debugger_capture(&self) {}
    unsafe fn stop_graphics_debugger_capture(&self) {}

    fn poll(&self, poll_type: wgpu::wgt::PollType<u64>) -> Result<wgpu::PollStatus, wgpu::PollError> {
        if matches!(poll_type, wgpu::wgt::PollType::Wait { .. }) {
            host::send(&json!({ "op": "wait" }));
        }
        flush();
        Ok(wgpu::PollStatus::QueueEmpty)
    }

    fn get_internal_counters(&self) -> wgpu::InternalCounters {
        Default::default()
    }

    fn generate_allocator_report(&self) -> Option<wgpu::AllocatorReport> {
        None
    }

    fn destroy(&self) {
        host::send(&json!({ "op": "destroy_device" }));
    }
}

// ---------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------

#[derive(Debug)]
pub struct Que;

#[derive(Debug)]
pub struct Staging(Vec<u8>);

impl QueueWriteBufferInterface for Staging {
    fn len(&self) -> usize {
        self.0.len()
    }
    unsafe fn write_slice(&mut self) -> wgpu::WriteOnly<'_, [u8]> {
        wgpu::WriteOnly::from_mut(self.0.as_mut_slice())
    }
}

fn texel_texture(t: &wgpu::TexelCopyTextureInfo<'_>) -> Value {
    json!({
        "texture": texture_id(t.texture), "mip_level": t.mip_level,
        "origin": [t.origin.x, t.origin.y, t.origin.z], "aspect": name(t.aspect),
    })
}

fn texel_layout(l: &wgpu::TexelCopyBufferLayout) -> Value {
    json!({ "offset": l.offset, "bytes_per_row": l.bytes_per_row, "rows_per_image": l.rows_per_image })
}

fn texel_buffer(b: &wgpu::TexelCopyBufferInfo<'_>) -> Value {
    let mut v = texel_layout(&b.layout);
    v["buffer"] = json!(buffer_id(b.buffer));
    v
}

fn extent(e: wgpu::Extent3d) -> Value {
    json!([e.width, e.height, e.depth_or_array_layers])
}

impl QueueInterface for Que {
    fn write_buffer(&self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress, data: &[u8]) {
        host::call_with(&json!({ "op": "write_buffer", "buffer": dbuffer_id(buffer), "offset": offset }), data);
    }

    fn create_staging_buffer(&self, size: wgpu::BufferSize) -> Option<DispatchQueueWriteBuffer> {
        Some(DispatchQueueWriteBuffer::custom(Staging(vec![0; size.get() as usize])))
    }

    fn validate_write_buffer(
        &self,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _size: wgpu::BufferSize,
    ) -> Option<()> {
        Some(())
    }

    fn write_staging_buffer(
        &self,
        buffer: &DispatchBuffer,
        offset: wgpu::BufferAddress,
        staging_buffer: &DispatchQueueWriteBuffer,
    ) {
        let staging = staging_buffer.as_custom::<Staging>().expect("this backend's staging buffer");
        self.write_buffer(buffer, offset, &staging.0);
    }

    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        data_layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        host::call_with(
            &json!({
                "op": "write_texture", "texture": texel_texture(&texture),
                "layout": texel_layout(&data_layout), "size": extent(size),
            }),
            data,
        );
    }

    fn submit(&self, command_buffers: &mut dyn Iterator<Item = DispatchCommandBuffer>) -> u64 {
        let mut keep = Vec::new();
        let buffers: Vec<Value> = command_buffers
            .map(|cb| {
                let cmds = cb.as_custom::<CmdBuf>().expect("this backend's command buffer");
                keep.push(std::mem::take(&mut *cmds.1.borrow_mut()));
                Value::Array(std::mem::take(&mut *cmds.0.borrow_mut()))
            })
            .collect();
        host::send(&json!({ "op": "submit", "buffers": buffers }));
        drop(keep);
        SUBMISSION.with(|s| {
            let n = s.get() + 1;
            s.set(n);
            n
        })
    }

    fn get_timestamp_period(&self) -> f32 {
        1.0
    }

    fn on_submitted_work_done(&self, callback: BoxSubmittedWorkDoneCallback) {
        WORK_DONE.with(|w| w.borrow_mut().push(callback));
    }

    fn compact_blas(&self, _blas: &DispatchBlas) -> (Option<u64>, DispatchBlas) {
        unimplemented!("acceleration structures")
    }
}

// ---------------------------------------------------------------------
// Command encoding: recorded here, replayed by the host at submit
// ---------------------------------------------------------------------

type Commands = Rc<RefCell<Vec<Value>>>;

/// What recorded commands name, kept alive until the host has replayed
/// them: the host's objects go when the handles drop, and wgpu lets a
/// caller drop a bind group (say) as soon as a pass has used it.
#[derive(Default)]
pub struct KeepAlive(Vec<Box<dyn std::any::Any>>);

impl std::fmt::Debug for KeepAlive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeepAlive({})", self.0.len())
    }
}

type Keep = Rc<RefCell<KeepAlive>>;

fn hold<T: Clone + 'static>(keep: &Keep, x: &T) {
    keep.borrow_mut().0.push(Box::new(x.clone()));
}

#[derive(Debug)]
pub struct Enc {
    cmds: Commands,
    keep: Keep,
}

#[derive(Debug)]
pub struct CmdBuf(RefCell<Vec<Value>>, RefCell<KeepAlive>);

impl CommandBufferInterface for CmdBuf {}

impl Enc {
    fn push(&self, v: Value) {
        self.cmds.borrow_mut().push(v);
    }
}

impl CommandEncoderInterface for Enc {
    fn copy_buffer_to_buffer(
        &self,
        source: &DispatchBuffer,
        source_offset: wgpu::BufferAddress,
        destination: &DispatchBuffer,
        destination_offset: wgpu::BufferAddress,
        copy_size: Option<wgpu::BufferAddress>,
    ) {
        hold(&self.keep, source);
        hold(&self.keep, destination);
        self.push(json!({
            "op": "copy_buffer_to_buffer", "src": dbuffer_id(source), "src_offset": source_offset,
            "dst": dbuffer_id(destination), "dst_offset": destination_offset, "size": copy_size,
        }));
    }

    fn copy_buffer_to_texture(
        &self,
        source: wgpu::TexelCopyBufferInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        hold(&self.keep, source.buffer);
        hold(&self.keep, destination.texture);
        self.push(json!({
            "op": "copy_buffer_to_texture", "src": texel_buffer(&source),
            "dst": texel_texture(&destination), "size": extent(copy_size),
        }));
    }

    fn copy_texture_to_buffer(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyBufferInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        hold(&self.keep, source.texture);
        hold(&self.keep, destination.buffer);
        self.push(json!({
            "op": "copy_texture_to_buffer", "src": texel_texture(&source),
            "dst": texel_buffer(&destination), "size": extent(copy_size),
        }));
    }

    fn copy_texture_to_texture(
        &self,
        source: wgpu::TexelCopyTextureInfo<'_>,
        destination: wgpu::TexelCopyTextureInfo<'_>,
        copy_size: wgpu::Extent3d,
    ) {
        hold(&self.keep, source.texture);
        hold(&self.keep, destination.texture);
        self.push(json!({
            "op": "copy_texture_to_texture", "src": texel_texture(&source),
            "dst": texel_texture(&destination), "size": extent(copy_size),
        }));
    }

    fn begin_compute_pass(&self, desc: &wgpu::ComputePassDescriptor<'_>) -> DispatchComputePass {
        DispatchComputePass::custom(Pass::new(
            self.cmds.clone(),
            self.keep.clone(),
            json!({ "op": "compute_pass", "label": label(desc.label) }),
        ))
    }

    fn begin_render_pass(&self, desc: &wgpu::RenderPassDescriptor<'_>) -> DispatchRenderPass {
        assert!(desc.depth_stencil_attachment.is_none(), "depth/stencil is not forwarded yet");
        let colors: Vec<Value> = desc
            .color_attachments
            .iter()
            .map(|a| match a {
                Some(a) => {
                    hold(&self.keep, a.view);
                    if let Some(r) = a.resolve_target {
                        hold(&self.keep, r);
                    }
                    let (load_op, clear_value) = match a.ops.load {
                        wgpu::LoadOp::Clear(c) => ("clear", json!([c.r, c.g, c.b, c.a])),
                        _ => ("load", Value::Null),
                    };
                    let store_op = match a.ops.store {
                        wgpu::StoreOp::Store => "store",
                        wgpu::StoreOp::Discard => "discard",
                    };
                    json!({
                        "view": view_id(a.view),
                        "resolve_target": a.resolve_target.map(view_id),
                        "load_op": load_op, "clear_value": clear_value, "store_op": store_op,
                    })
                }
                None => Value::Null,
            })
            .collect();
        DispatchRenderPass::custom(Pass::new(
            self.cmds.clone(),
            self.keep.clone(),
            json!({ "op": "render_pass", "label": label(desc.label), "color_attachments": colors }),
        ))
    }

    fn finish(&mut self) -> DispatchCommandBuffer {
        DispatchCommandBuffer::custom(CmdBuf(
            RefCell::new(std::mem::take(&mut *self.cmds.borrow_mut())),
            RefCell::new(std::mem::take(&mut *self.keep.borrow_mut())),
        ))
    }

    fn clear_texture(&self, texture: &DispatchTexture, range: &wgpu::ImageSubresourceRange) {
        hold(&self.keep, texture);
        self.push(json!({
            "op": "clear_texture", "texture": texture.as_custom::<Tex>().expect("this backend's texture").id,
            "aspect": name(range.aspect), "base_mip_level": range.base_mip_level,
            "mip_level_count": range.mip_level_count, "base_array_layer": range.base_array_layer,
            "array_layer_count": range.array_layer_count,
        }));
    }

    fn clear_buffer(&self, buffer: &DispatchBuffer, offset: wgpu::BufferAddress, size: Option<wgpu::BufferAddress>) {
        hold(&self.keep, buffer);
        self.push(json!({ "op": "clear_buffer", "buffer": dbuffer_id(buffer), "offset": offset, "size": size }));
    }

    fn insert_debug_marker(&self, _label: &str) {}
    fn push_debug_group(&self, _label: &str) {}
    fn pop_debug_group(&self) {}

    fn write_timestamp(&self, _query_set: &DispatchQuerySet, _query_index: u32) {}

    fn resolve_query_set(
        &self,
        query_set: &DispatchQuerySet,
        first_query: u32,
        query_count: u32,
        destination: &DispatchBuffer,
        destination_offset: wgpu::BufferAddress,
    ) {
        hold(&self.keep, query_set);
        hold(&self.keep, destination);
        self.push(json!({
            "op": "resolve_query_set", "query_set": handle(query_set.as_custom::<Handle>(), query_set),
            "first": first_query, "count": query_count,
            "dst": dbuffer_id(destination), "dst_offset": destination_offset,
        }));
    }

    fn mark_acceleration_structures_built<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::Blas>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
        unimplemented!("acceleration structures")
    }

    fn build_acceleration_structures<'a>(
        &self,
        _blas: &mut dyn Iterator<Item = &'a wgpu::BlasBuildEntry<'a>>,
        _tlas: &mut dyn Iterator<Item = &'a wgpu::Tlas>,
    ) {
        unimplemented!("acceleration structures")
    }

    fn transition_resources<'a>(
        &mut self,
        _buffer_transitions: &mut dyn Iterator<Item = wgpu::wgt::BufferTransition<&'a DispatchBuffer>>,
        _texture_transitions: &mut dyn Iterator<Item = wgpu::wgt::TextureTransition<&'a DispatchTexture>>,
    ) {
    }
}

/// A compute or render pass: its commands, appended to the encoder's
/// when the pass ends (is dropped).
#[derive(Debug)]
pub struct Pass {
    into: Commands,
    keep: Keep,
    head: Value,
    cmds: Vec<Value>,
    /// The pass's state, so a call setting it again to what it already
    /// is is not recorded: a simulation sets the same pipeline before
    /// each of thousands of steps, and each recorded call is a call the
    /// host makes into wgpu-py.
    pipeline: Option<u32>,
    bind_groups: Vec<Option<(Option<u32>, Vec<u32>)>>,
}

impl Drop for Pass {
    fn drop(&mut self) {
        let mut head = std::mem::take(&mut self.head);
        head["cmds"] = Value::Array(std::mem::take(&mut self.cmds));
        self.into.borrow_mut().push(head);
    }
}

impl Pass {
    fn new(into: Commands, keep: Keep, head: Value) -> Self {
        Pass { into, keep, head, cmds: Vec::new(), pipeline: None, bind_groups: Vec::new() }
    }

    /// Records a pipeline unless it is the one already set (whose handle
    /// the pass already holds).
    fn pipeline<T: Clone + 'static>(&mut self, id: u32, pipeline: &T) {
        if self.pipeline != Some(id) {
            self.pipeline = Some(id);
            hold(&self.keep, pipeline);
            self.cmds.push(json!(["pipeline", id]));
        }
    }

    fn bind_group(&mut self, index: u32, bind_group: Option<&DispatchBindGroup>, offsets: &[wgpu::DynamicOffset]) {
        let id = bind_group.map(|b| handle(b.as_custom::<Handle>(), b));
        let state = Some((id, offsets.to_vec()));
        let i = index as usize;
        if self.bind_groups.len() <= i {
            self.bind_groups.resize(i + 1, None);
        }
        if self.bind_groups[i] == state {
            return;
        }
        self.bind_groups[i] = state;
        if let Some(b) = bind_group {
            hold(&self.keep, b);
        }
        self.cmds.push(json!(["bind_group", index, id, offsets]));
    }
}

impl ComputePassInterface for Pass {
    fn set_pipeline(&mut self, pipeline: &DispatchComputePipeline) {
        self.pipeline(handle(pipeline.as_custom::<Handle>(), pipeline), pipeline);
    }
    fn set_bind_group(&mut self, index: u32, bind_group: Option<&DispatchBindGroup>, offsets: &[wgpu::DynamicOffset]) {
        self.bind_group(index, bind_group, offsets);
    }
    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediates")
    }
    fn insert_debug_marker(&mut self, _label: &str) {}
    fn push_debug_group(&mut self, _group_label: &str) {}
    fn pop_debug_group(&mut self) {}
    fn write_timestamp(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {}
    fn begin_pipeline_statistics_query(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {}
    fn end_pipeline_statistics_query(&mut self) {}
    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.cmds.push(json!(["dispatch", x, y, z]));
    }
    fn dispatch_workgroups_indirect(&mut self, indirect_buffer: &DispatchBuffer, indirect_offset: wgpu::BufferAddress) {
        hold(&self.keep, indirect_buffer);
        self.cmds.push(json!(["dispatch_indirect", dbuffer_id(indirect_buffer), indirect_offset]));
    }
}

impl RenderPassInterface for Pass {
    fn set_pipeline(&mut self, pipeline: &DispatchRenderPipeline) {
        self.pipeline(handle(pipeline.as_custom::<Handle>(), pipeline), pipeline);
    }
    fn set_bind_group(&mut self, index: u32, bind_group: Option<&DispatchBindGroup>, offsets: &[wgpu::DynamicOffset]) {
        self.bind_group(index, bind_group, offsets);
    }
    fn set_index_buffer(
        &mut self,
        _buffer: &DispatchBuffer,
        _index_format: wgpu::IndexFormat,
        _offset: wgpu::BufferAddress,
        _size: Option<wgpu::BufferSize>,
    ) {
        unimplemented!("index buffers")
    }
    fn set_vertex_buffer(
        &mut self,
        _slot: u32,
        _buffer: &DispatchBuffer,
        _offset: wgpu::BufferAddress,
        _size: Option<wgpu::BufferSize>,
    ) {
        unimplemented!("vertex buffers")
    }
    fn set_immediates(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("immediates")
    }
    fn set_blend_constant(&mut self, color: wgpu::Color) {
        self.cmds.push(json!(["blend_constant", [color.r, color.g, color.b, color.a]]));
    }
    fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.cmds.push(json!(["scissor", x, y, width, height]));
    }
    fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32, min_depth: f32, max_depth: f32) {
        self.cmds.push(json!(["viewport", x, y, width, height, min_depth, max_depth]));
    }
    fn set_stencil_reference(&mut self, reference: u32) {
        self.cmds.push(json!(["stencil_reference", reference]));
    }
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.cmds.push(json!([
            "draw", vertices.end - vertices.start, instances.end - instances.start, vertices.start, instances.start
        ]));
    }
    fn draw_indexed(&mut self, _indices: Range<u32>, _base_vertex: i32, _instances: Range<u32>) {
        unimplemented!("indexed draws")
    }
    fn draw_mesh_tasks(&mut self, _x: u32, _y: u32, _z: u32) {
        unimplemented!("mesh shaders")
    }
    fn draw_indirect(&mut self, _indirect_buffer: &DispatchBuffer, _indirect_offset: wgpu::BufferAddress) {
        unimplemented!("indirect draws")
    }
    fn draw_indexed_indirect(&mut self, _indirect_buffer: &DispatchBuffer, _indirect_offset: wgpu::BufferAddress) {
        unimplemented!("indirect draws")
    }
    fn draw_mesh_tasks_indirect(&mut self, _indirect_buffer: &DispatchBuffer, _indirect_offset: wgpu::BufferAddress) {
        unimplemented!("mesh shaders")
    }
    fn multi_draw_indirect(&mut self, _b: &DispatchBuffer, _o: wgpu::BufferAddress, _count: u32) {
        unimplemented!("indirect draws")
    }
    fn multi_draw_indexed_indirect(&mut self, _b: &DispatchBuffer, _o: wgpu::BufferAddress, _count: u32) {
        unimplemented!("indirect draws")
    }
    fn multi_draw_indirect_count(
        &mut self,
        _b: &DispatchBuffer,
        _o: wgpu::BufferAddress,
        _cb: &DispatchBuffer,
        _co: wgpu::BufferAddress,
        _max: u32,
    ) {
        unimplemented!("indirect draws")
    }
    fn multi_draw_mesh_tasks_indirect(&mut self, _b: &DispatchBuffer, _o: wgpu::BufferAddress, _count: u32) {
        unimplemented!("mesh shaders")
    }
    fn multi_draw_indexed_indirect_count(
        &mut self,
        _b: &DispatchBuffer,
        _o: wgpu::BufferAddress,
        _cb: &DispatchBuffer,
        _co: wgpu::BufferAddress,
        _max: u32,
    ) {
        unimplemented!("indirect draws")
    }
    fn multi_draw_mesh_tasks_indirect_count(
        &mut self,
        _b: &DispatchBuffer,
        _o: wgpu::BufferAddress,
        _cb: &DispatchBuffer,
        _co: wgpu::BufferAddress,
        _max: u32,
    ) {
        unimplemented!("mesh shaders")
    }
    fn insert_debug_marker(&mut self, _label: &str) {}
    fn push_debug_group(&mut self, _group_label: &str) {}
    fn pop_debug_group(&mut self) {}
    fn write_timestamp(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {}
    fn begin_occlusion_query(&mut self, _query_index: u32) {}
    fn end_occlusion_query(&mut self) {}
    fn begin_pipeline_statistics_query(&mut self, _query_set: &DispatchQuerySet, _query_index: u32) {}
    fn end_pipeline_statistics_query(&mut self) {}
    fn execute_bundles(&mut self, _render_bundles: &mut dyn Iterator<Item = &DispatchRenderBundle>) {
        unimplemented!("render bundles")
    }
}

// ---------------------------------------------------------------------
// Making the device
// ---------------------------------------------------------------------

/// The host's limits (WebGPU's names, kebab-case) over wgpu's defaults:
/// matched to wgpu's own serde names with case and separators ignored.
fn limits_from_host(host: &serde_json::Map<String, Value>) -> wgpu::Limits {
    let norm = |k: &str| k.replace(['-', '_'], "").to_lowercase();
    let mut base = serde_json::to_value(wgpu::Limits::default()).expect("limits serialize");
    if let Some(obj) = base.as_object_mut() {
        for (k, v) in obj.iter_mut() {
            if let Some((_, hv)) = host.iter().find(|(hk, _)| norm(hk) == norm(k)) {
                *v = hv.clone();
            }
        }
    }
    serde_json::from_value(base).expect("limits deserialize")
}

/// The device and queue the host made (`init`).
pub fn device() -> Result<(wgpu::Device, wgpu::Queue), String> {
    let reply = host::query(&json!({ "op": "init" }));
    if let Some(e) = reply.get("error").and_then(Value::as_str) {
        return Err(e.to_string());
    }
    let mut features = wgpu::Features::CLEAR_TEXTURE;
    let host_features: Vec<&str> = reply["features"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if host_features.contains(&"float32-filterable") {
        features |= wgpu::Features::FLOAT32_FILTERABLE;
    }
    let limits = limits_from_host(reply["limits"].as_object().ok_or("the host sent no limits")?);
    let info = &reply["info"];
    let s = |k: &str| info[k].as_str().unwrap_or("").to_string();
    let backend = match info["backend_type"].as_str().unwrap_or("") {
        "Vulkan" => wgpu::Backend::Vulkan,
        "Metal" => wgpu::Backend::Metal,
        "D3D12" => wgpu::Backend::Dx12,
        "OpenGL" | "OpenGLES" => wgpu::Backend::Gl,
        _ => wgpu::Backend::Noop,
    };
    let info = wgpu::AdapterInfo {
        name: s("device"),
        vendor: info["vendor_id"].as_u64().unwrap_or(0) as u32,
        device: info["device_id"].as_u64().unwrap_or(0) as u32,
        device_type: wgpu::DeviceType::Other,
        device_pci_bus_id: String::new(),
        driver: s("description"),
        driver_info: String::new(),
        backend,
        subgroup_min_size: 4,
        subgroup_max_size: 128,
        transient_saves_memory: false,
    };
    let device = wgpu::Device::from_custom(Dev { features, limits, info });
    let queue = wgpu::Queue::from_custom(Que);
    Ok((device, queue))
}
