//! **The viewport's upscale** (docs/projects/quality-governor.md, render
//! scale): the picture rendered below the display's size -- at a render
//! scale, or at logical points on a high-DPI screen -- drawn at the
//! display's physical size for egui to show 1:1.
//!
//! On the tone-mapped 8-bit picture (the fractal texture, or the colour
//! effects' output), which is what egui's own linear filter sampled
//! before. Bilinear is that filter. Catmull-Rom is the bicubic that
//! passes through the samples, sharper, from sixteen taps. Nearest copies
//! each source pixel into a block, crisp and blocky. FSR 1 is AMD's
//! edge-adaptive upsampling and sharpening, two passes ported to WGSL
//! (`shaders/upscale_fsr.wgsl`, MIT). All read with
//! `textureLoad`, clamped at the edges, so no sampler and no uniform
//! control flow are needed (the WebGPU rule in SHADERS.md).
//!
//! When the render is the display's size the pass does not run and the
//! picture is shown as it is: 100% on a 1x screen is the path it always
//! was.

use serde::{Deserialize, Serialize};
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, PipelineLayoutDescriptor, Queue,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension,
};

/// How the viewport's picture is drawn up to the display's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscaleFilter {
    /// The nearest pixel: each source pixel a block, no blending.
    Nearest,
    /// Linear between the four nearest pixels: what egui did.
    Bilinear,
    /// Catmull-Rom bicubic: sharper, through the samples, a slight
    /// overshoot at hard edges (clamped).
    #[default]
    CatmullRom,
    /// AMD FidelityFX Super Resolution 1: an edge-adaptive Lanczos-like
    /// upsampling (EASU), then contrast-adaptive sharpening (RCAS).
    Fsr,
}

impl UpscaleFilter {
    pub const ALL: [UpscaleFilter; 4] =
        [UpscaleFilter::Nearest, UpscaleFilter::Bilinear, UpscaleFilter::CatmullRom, UpscaleFilter::Fsr];

    /// The i18n key of its name.
    pub fn label_key(self) -> &'static str {
        match self {
            UpscaleFilter::Nearest => "menu.upscaler_nearest",
            UpscaleFilter::Bilinear => "menu.upscaler_bilinear",
            UpscaleFilter::CatmullRom => "menu.upscaler_catmull_rom",
            UpscaleFilter::Fsr => "menu.upscaler_fsr",
        }
    }
}

/// RCAS's sharpness, in stops below its strongest (`FsrRcasCon`: 0 is the
/// most, each stop halves it).
const RCAS_STOPS: f32 = 0.2;

/// The FSR passes (EASU, RCAS), from the shader file that keeps AMD's
/// licence with the port.
const FSR_SHADER: &str = include_str!("../../shaders/upscale_fsr.wgsl");

/// The render scales offered (the Rendering menu). The governor will use
/// its own steps (the plan's ladder); these are the manual setting's.
pub const RENDER_SCALES: [f32; 3] = [1.0, 0.75, 0.5];

/// The render size for a display of `base` pixels at `scale`: each side
/// rounded, never below one pixel.
pub fn scaled_size(base: (u32, u32), scale: f32) -> (u32, u32) {
    let s = if scale.is_finite() { scale.clamp(0.05, 1.0) } else { 1.0 };
    let side = |n: u32| ((n as f32 * s).round() as u32).max(1);
    (side(base.0), side(base.1))
}

const SHADER: &str = r#"
struct UpscaleParams {
    // Source pixels per destination pixel, each axis.
    ratio: vec2<f32>,
    // 0 bilinear, 1 Catmull-Rom, 2 nearest. (`filter` is reserved in
    // WGSL.)
    kind: u32,
    _pad: u32,
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> p: UpscaleParams;

fn upscale_tap(q: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(src_tex, clamp(q, vec2<i32>(0), size - vec2<i32>(1)), 0);
}

// Catmull-Rom's four weights at fraction t.
fn upscale_catmull_rom(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return vec4<f32>(
        -0.5 * t3 + t2 - 0.5 * t,
        1.5 * t3 - 2.5 * t2 + 1.0,
        -1.5 * t3 + 2.0 * t2 + 0.5 * t,
        0.5 * t3 - 0.5 * t2,
    );
}

@compute @workgroup_size(8, 8, 1)
fn upscale_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = vec2<i32>(textureDimensions(dst_tex));
    let q = vec2<i32>(i32(gid.x), i32(gid.y));
    if (q.x >= dims.x || q.y >= dims.y) {
        return;
    }
    let size = vec2<i32>(textureDimensions(src_tex));
    if (p.kind == 2u) {
        // The source pixel the destination pixel's centre falls in.
        let src = vec2<i32>(floor((vec2<f32>(q) + vec2<f32>(0.5)) * p.ratio));
        textureStore(dst_tex, q, upscale_tap(src, size));
        return;
    }
    // The destination pixel's centre in source pixels, less half a pixel:
    // the integer part is the tap left of and above it.
    let at = (vec2<f32>(q) + vec2<f32>(0.5)) * p.ratio - vec2<f32>(0.5);
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    var c = vec4<f32>(0.0);
    if (p.kind == 0u) {
        let top = mix(upscale_tap(base, size), upscale_tap(base + vec2<i32>(1, 0), size), f.x);
        let bottom = mix(upscale_tap(base + vec2<i32>(0, 1), size), upscale_tap(base + vec2<i32>(1, 1), size), f.x);
        c = mix(top, bottom, f.y);
    } else {
        let wx = upscale_catmull_rom(f.x);
        let wy = upscale_catmull_rom(f.y);
        for (var j = 0; j < 4; j = j + 1) {
            var row = vec4<f32>(0.0);
            for (var i = 0; i < 4; i = i + 1) {
                row = row + upscale_tap(base + vec2<i32>(i - 1, j - 1), size) * wx[i];
            }
            c = c + row * wy[j];
        }
    }
    textureStore(dst_tex, q, clamp(c, vec4<f32>(0.0), vec4<f32>(1.0)));
}
"#;

/// The pass and the display-sized picture it draws.
#[derive(Default)]
pub struct Upscale {
    output: Option<(Texture, TextureView)>,
    params: Option<Buffer>,
    layout: Option<BindGroupLayout>,
    /// The one-pass filters' pipeline.
    pipeline: Option<ComputePipeline>,
    /// FSR's two passes, made the first time it is chosen.
    fsr: Option<Fsr>,
}

/// A picture of `format`: written by a pass, read by the next or shown by
/// egui; COPY_SRC so a test can read it back.
fn picture(device: &Device, label: &str, format: TextureFormat, w: u32, h: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

/// Keep `slot` a `w` x `h` picture of `format`.
fn fit(slot: &mut Option<(Texture, TextureView)>, device: &Device, label: &str, format: TextureFormat, w: u32, h: u32) {
    if slot.as_ref().is_none_or(|(t, _)| t.width() != w || t.height() != h) {
        if let Some((t, _)) = slot.take() {
            t.destroy();
        }
        *slot = Some(picture(device, label, format, w, h));
    }
}

/// The layout every pass here binds: the source, the target (`format`),
/// the parameters.
fn layout(device: &Device, label: &str, format: TextureFormat) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

fn pipeline(device: &Device, layout: &BindGroupLayout, source: &str, label: &str, entry: &str) -> ComputePipeline {
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(label),
        source: ShaderSource::Wgsl(source.into()),
    });
    let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// The 16 bytes every pass here reads: source pixels per target pixel,
/// the one-pass filter's kind, RCAS's sharpness.
fn params_bytes(src: (u32, u32), dst: (u32, u32), kind: u32) -> [u8; 16] {
    let mut raw = [0u8; 16];
    raw[0..4].copy_from_slice(&(src.0.max(1) as f32 / dst.0.max(1) as f32).to_le_bytes());
    raw[4..8].copy_from_slice(&(src.1.max(1) as f32 / dst.1.max(1) as f32).to_le_bytes());
    raw[8..12].copy_from_slice(&kind.to_le_bytes());
    // RCAS's sharpness, linear (`FsrRcasCon`); padding to the rest.
    raw[12..16].copy_from_slice(&(-RCAS_STOPS).exp2().to_le_bytes());
    raw
}

/// One pass of `pipeline` from `from` into `to`, over `size`.
#[allow(clippy::too_many_arguments)]
fn dispatch(
    device: &Device,
    encoder: &mut CommandEncoder,
    layout: &BindGroupLayout,
    pipeline: &ComputePipeline,
    params: &Buffer,
    from: &TextureView,
    to: &TextureView,
    size: (u32, u32),
    label: &str,
) {
    let bg = device.create_bind_group(&BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            BindGroupEntry { binding: 0, resource: BindingResource::TextureView(from) },
            BindGroupEntry { binding: 1, resource: BindingResource::TextureView(to) },
            BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some(label), timestamp_writes: None });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bg, &[]);
    pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
}

/// What an FSR pass draws, and in what encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsrTarget {
    /// The viewport's tone-mapped `Rgba8Unorm` picture: in perceptual
    /// space already, as FSR expects.
    Display,
    /// A linear `Rgba32Float` picture that may pass 1 (a simulation's,
    /// before the tone map): encoded into [0, 1) around the two passes,
    /// `sqrt(c / (1 + c))`, so EASU and RCAS see perceptual values in the
    /// range their limiters assume, and decoded after.
    Linear,
}

impl FsrTarget {
    fn format(self) -> TextureFormat {
        match self {
            FsrTarget::Display => TextureFormat::Rgba8Unorm,
            FsrTarget::Linear => TextureFormat::Rgba32Float,
        }
    }

    /// The shader, with this target's format and encoding spliced in.
    fn source(self) -> String {
        match self {
            FsrTarget::Display => FSR_SHADER.to_string(),
            FsrTarget::Linear => {
                let splice = |s: String, from: &str, to: &str| {
                    assert_eq!(s.matches(from).count(), 1, "upscale_fsr.wgsl: `{from}` once");
                    s.replace(from, to)
                };
                let s = splice(
                    FSR_SHADER.to_string(),
                    "texture_storage_2d<rgba8unorm, write>",
                    "texture_storage_2d<rgba32float, write>",
                );
                let s = splice(
                    s,
                    "fn fsr_in(c: vec4<f32>) -> vec4<f32> { return c; }",
                    "fn fsr_in(c: vec4<f32>) -> vec4<f32> {\n    let x = max(c.rgb, vec3<f32>(0.0));\n    return vec4<f32>(sqrt(x / (vec3<f32>(1.0) + x)), c.a);\n}",
                );
                splice(
                    s,
                    "fn fsr_out(c: vec4<f32>) -> vec4<f32> { return c; }",
                    "fn fsr_out(c: vec4<f32>) -> vec4<f32> {\n    let y = min(c.rgb * c.rgb, vec3<f32>(0.999));\n    return vec4<f32>(y / (vec3<f32>(1.0) - y), c.a);\n}",
                )
            }
        }
    }
}

/// FSR 1's two passes (`shaders/upscale_fsr.wgsl`), EASU through an
/// intermediate picture, then RCAS into the caller's target.
pub struct Fsr {
    target: FsrTarget,
    layout: Option<BindGroupLayout>,
    /// EASU's and RCAS's pipelines, made on first use.
    passes: Option<(ComputePipeline, ComputePipeline)>,
    /// EASU's picture, which RCAS sharpens.
    mid: Option<(Texture, TextureView)>,
    params: Option<Buffer>,
}

impl Fsr {
    pub fn new(target: FsrTarget) -> Self {
        Fsr { target, layout: None, passes: None, mid: None, params: None }
    }

    /// Draw `src` (`src_size`) at `dst_size` into `dst`, both in the
    /// target's format.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        src: &TextureView,
        src_size: (u32, u32),
        dst: &TextureView,
        dst_size: (u32, u32),
    ) {
        let (w, h) = (dst_size.0.max(1), dst_size.1.max(1));
        let format = self.target.format();
        fit(&mut self.mid, device, "FSR EASU", format, w, h);
        let params = self.params.get_or_insert_with(|| {
            device.create_buffer(&BufferDescriptor {
                label: Some("FSR Params"),
                size: 16,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        queue.write_buffer(params, 0, &params_bytes(src_size, (w, h), 0));
        let layout = self.layout.get_or_insert_with(|| self::layout(device, "FSR Layout", format));
        let target = self.target;
        let (easu, rcas) = self.passes.get_or_insert_with(|| {
            let source = target.source();
            (
                pipeline(device, layout, &source, "FSR EASU", "easu_main"),
                pipeline(device, layout, &source, "FSR RCAS", "rcas_main"),
            )
        });
        let (_, mid) = self.mid.as_ref().expect("fitted above");
        dispatch(device, encoder, layout, easu, params, src, mid, (w, h), "FSR EASU");
        dispatch(device, encoder, layout, rcas, params, mid, dst, (w, h), "FSR RCAS");
    }

    /// Free the intermediate picture (the next `encode` makes it again).
    pub fn release(&mut self) {
        if let Some((t, _)) = self.mid.take() {
            t.destroy();
        }
    }
}

impl Upscale {
    /// Draw `src` (`src_size`, `Rgba8Unorm`) at `dst_size` into the
    /// output, with `filter`.
    pub fn run(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        src: &TextureView,
        src_size: (u32, u32),
        dst_size: (u32, u32),
        filter: UpscaleFilter,
    ) {
        let (w, h) = (dst_size.0.max(1), dst_size.1.max(1));
        fit(&mut self.output, device, "Viewport Upscale", TextureFormat::Rgba8Unorm, w, h);
        let (_, dst) = self.output.as_ref().expect("fitted above");
        if filter == UpscaleFilter::Fsr {
            self.fsr
                .get_or_insert_with(|| Fsr::new(FsrTarget::Display))
                .encode(device, queue, encoder, src, src_size, dst, (w, h));
            return;
        }
        if let Some(fsr) = self.fsr.as_mut() {
            fsr.release();
        }
        let kind: u32 = match filter {
            UpscaleFilter::Bilinear | UpscaleFilter::Fsr => 0,
            UpscaleFilter::CatmullRom => 1,
            UpscaleFilter::Nearest => 2,
        };
        let params = self.params.get_or_insert_with(|| {
            device.create_buffer(&BufferDescriptor {
                label: Some("Viewport Upscale Params"),
                size: 16,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        queue.write_buffer(params, 0, &params_bytes(src_size, (w, h), kind));
        let layout = self
            .layout
            .get_or_insert_with(|| self::layout(device, "Viewport Upscale Layout", TextureFormat::Rgba8Unorm));
        let simple = self
            .pipeline
            .get_or_insert_with(|| pipeline(device, layout, SHADER, "Viewport Upscale", "upscale_main"));
        dispatch(device, encoder, layout, simple, params, src, dst, (w, h), "Viewport Upscale");
    }

    /// The last picture drawn, and its size.
    pub fn output(&self) -> Option<(&TextureView, (u32, u32))> {
        self.output.as_ref().map(|(t, v)| (v, (t.width(), t.height())))
    }

    #[cfg(test)]
    pub(crate) fn texture(&self) -> Option<&Texture> {
        self.output.as_ref().map(|(t, _)| t)
    }

    /// Free the pictures (the next `run` makes them again).
    pub fn release(&mut self) {
        if let Some((t, _)) = self.output.take() {
            t.destroy();
        }
        if let Some(fsr) = self.fsr.as_mut() {
            fsr.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Option<(Device, Queue)> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.ok()?;
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("upscale tests"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()
        })
    }

    /// An `Rgba8Unorm` texture of `w` x `h` from RGBA bytes.
    fn image(device: &Device, queue: &Queue, w: u32, h: u32, rgba: &[u8]) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    fn read(device: &Device, queue: &Queue, tex: &Texture) -> Vec<u8> {
        let (w, h) = (tex.width(), tex.height());
        let row = (w * 4).div_ceil(256) * 256;
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        let bytes = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        (0..h).flat_map(|y| bytes[(y * row) as usize..(y * row + w * 4) as usize].to_vec()).collect()
    }

    fn upscale(device: &Device, queue: &Queue, src: (u32, u32), rgba: &[u8], dst: (u32, u32), filter: UpscaleFilter) -> Vec<u8> {
        let (_t, view) = image(device, queue, src.0, src.1, rgba);
        let mut up = Upscale::default();
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        up.run(device, queue, &mut enc, &view, src, dst, filter);
        queue.submit(std::iter::once(enc.finish()));
        let out = read(device, queue, up.texture().expect("drawn"));
        assert_eq!(up.output().map(|(_, s)| s), Some(dst));
        up.release();
        out
    }

    /// A flat picture stays flat, edges included, at any size and by
    /// either filter: the weights sum to one and the edges clamp.
    #[test]
    fn a_flat_picture_stays_flat() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let px = [40u8, 120, 200, 255];
        let src: Vec<u8> = px.iter().copied().cycle().take(5 * 3 * 4).collect();
        for filter in UpscaleFilter::ALL {
            for dst in [(10, 6), (7, 5), (13, 8)] {
                let out = upscale(&device, &queue, (5, 3), &src, dst, filter);
                for (i, v) in out.iter().enumerate() {
                    assert!(v.abs_diff(px[i % 4]) <= 1, "{filter:?} at {dst:?}: {v} against {}", px[i % 4]);
                }
            }
        }
    }

    /// Doubled, a ramp's inner pixels fall between their neighbours as a
    /// line does: both filters reproduce a linear ramp (Catmull-Rom is
    /// exact on lines), quarter of the way from each source pixel.
    #[test]
    fn a_ramp_doubled_stays_a_ramp() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (8u32, 2u32);
        let value = |x: u32| (x * 32) as u8;
        let src: Vec<u8> = (0..h).flat_map(|_| (0..w).flat_map(move |x| [value(x), 0, 0, 255])).collect();
        for filter in [UpscaleFilter::Bilinear, UpscaleFilter::CatmullRom] {
            let out = upscale(&device, &queue, (w, h), &src, (w * 2, h * 2), filter);
            // Destination x = 2i + 1 sits a quarter past source i: 32i + 8.
            // Keep away from the clamped edges.
            for i in 1..w - 2 {
                let x = 2 * i + 1;
                let got = out[(x * 4) as usize];
                let want = 32 * i + 8;
                assert!(got.abs_diff(want as u8) <= 1, "{filter:?} at x {x}: {got} against {want}");
            }
        }
    }

    /// Nearest copies each source pixel into a block: doubled, every 2x2
    /// block is its source pixel exactly.
    #[test]
    fn nearest_makes_each_pixel_a_block() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (5u32, 3u32);
        let src: Vec<u8> = (0..w * h).flat_map(|i| [(i * 17) as u8, (i * 5) as u8, 255 - i as u8, 255]).collect();
        let out = upscale(&device, &queue, (w, h), &src, (w * 2, h * 2), UpscaleFilter::Nearest);
        for y in 0..h * 2 {
            for x in 0..w * 2 {
                let d = ((y * w * 2 + x) * 4) as usize;
                let s = (((y / 2) * w + x / 2) * 4) as usize;
                assert_eq!(&out[d..d + 4], &src[s..s + 4], "pixel ({x}, {y})");
            }
        }
    }

    /// Catmull-Rom is the sharper of the two: across a hard edge, the
    /// pixels beside it are further from the middle grey than bilinear's.
    #[test]
    fn catmull_rom_keeps_an_edge_sharper() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (8u32, 1u32);
        let src: Vec<u8> = (0..w).flat_map(|x| {
            let v = if x < 4 { 0u8 } else { 255 };
            [v, v, v, 255]
        }).collect();
        let lin = upscale(&device, &queue, (w, h), &src, (w * 4, 1), UpscaleFilter::Bilinear);
        let cub = upscale(&device, &queue, (w, h), &src, (w * 4, 1), UpscaleFilter::CatmullRom);
        // The edge falls between source 3 and 4: destination 14..17.
        let spread = |o: &[u8]| (13..=18).map(|x| (o[x * 4] as i32 - 128).abs()).sum::<i32>();
        assert!(spread(&cub) > spread(&lin), "Catmull-Rom {} against bilinear {}", spread(&cub), spread(&lin));
    }

    /// FSR keeps an edge sharper than bilinear does.
    #[test]
    fn fsr_keeps_an_edge_sharper_than_bilinear() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (16u32, 4u32);
        let src: Vec<u8> = (0..w * h).flat_map(|i| {
            let v = if i % w < 8 { 40u8 } else { 215 };
            [v, v, v, 255]
        }).collect();
        let lin = upscale(&device, &queue, (w, h), &src, (w * 2, h * 2), UpscaleFilter::Bilinear);
        let fsr = upscale(&device, &queue, (w, h), &src, (w * 2, h * 2), UpscaleFilter::Fsr);
        // The edge falls between source 7 and 8: destination 14..17, row 3.
        let row = (3 * w * 2 * 4) as usize;
        let spread = |o: &[u8]| (12..=19).map(|x| (o[row + x * 4] as i32 - 128).abs()).sum::<i32>();
        assert!(spread(&fsr) > spread(&lin), "FSR {} against bilinear {}", spread(&fsr), spread(&lin));
    }

    /// A saturated colour on black: RCAS's limiters divide by zero on every
    /// channel the ring holds none of, and the port takes HLSL's answer
    /// there (`rcas_lobe`), so the empty channels stay empty and the black
    /// stays black -- no NaN's garbage, on any platform.
    #[test]
    fn fsr_keeps_a_saturated_colour_on_black_clean() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (16u32, 16u32);
        let src: Vec<u8> = (0..w * h).flat_map(|i| {
            let (x, y) = (i % w, i / w);
            if (6..10).contains(&x) && (6..10).contains(&y) { [255, 0, 0, 255] } else { [0, 0, 0, 255] }
        }).collect();
        let out = upscale(&device, &queue, (w, h), &src, (w * 2, h * 2), UpscaleFilter::Fsr);
        for (i, px) in out.chunks_exact(4).enumerate() {
            assert_eq!((px[1], px[2]), (0, 0), "pixel {i}: green and blue came from nowhere: {px:?}");
        }
        assert_eq!(&out[0..4], &[0, 0, 0, 255], "the corner is black");
        let centre = ((16 * w * 2 + 16) * 4) as usize;
        assert!(out[centre] > 200, "the square is red: {:?}", &out[centre..centre + 4]);
    }

    #[test]
    fn a_scaled_size_rounds_and_never_vanishes() {
        assert_eq!(scaled_size((1079, 1056), 1.0), (1079, 1056));
        assert_eq!(scaled_size((1079, 1056), 0.5), (540, 528));
        assert_eq!(scaled_size((1079, 1056), 0.75), (809, 792));
        assert_eq!(scaled_size((1, 1), 0.5), (1, 1));
        assert_eq!(scaled_size((100, 100), f32::NAN), (100, 100));
        assert_eq!(scaled_size((100, 100), 2.0), (100, 100));
    }

    #[test]
    fn the_filter_round_trips_and_defaults_to_catmull_rom() {
        assert_eq!(UpscaleFilter::default(), UpscaleFilter::CatmullRom);
        for f in UpscaleFilter::ALL {
            let json = serde_json::to_string(&f).unwrap();
            assert_eq!(serde_json::from_str::<UpscaleFilter>(&json).unwrap(), f);
        }
        assert_eq!(serde_json::to_string(&UpscaleFilter::CatmullRom).unwrap(), "\"catmull_rom\"");
    }
}
