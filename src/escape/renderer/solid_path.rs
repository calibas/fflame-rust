//! Mode D's path tracer (docs/projects/heightfield-3d.md, T3c): a
//! solid's distance field through the core the terrain renders with
//! (`crate::escape::path_core`), on the same camera and lights as the
//! walk.
//!
//! A sample costs several walks -- the camera ray's march, a shadow
//! march a light, a bounce's march, each surface's normal and colouring
//! -- and a walk at 1080p is already hundreds of milliseconds. So the
//! pass is BANDED as the walk is: a sample is a pass over the frame's
//! rows in dispatches the watchdog never notices, sized by the walk's
//! own cost model with the path's extra marches counted in. Where a
//! whole frame fits one dispatch, one dispatch takes several samples.

use crate::config::escape::RenderTier;
use crate::config::FractalConfig;
use crate::escape::path_core::{self, PathParamsGpu, PathSum, PATH_SHOW_SAMPLES};
use egui_wgpu::wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor, Device,
    PipelineLayoutDescriptor, Queue, SamplerBindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    Texture, TextureSampleType, TextureView, TextureViewDimension,
};

use super::{assembler, EscapeParamsGpu, EscapeRenderer};

/// How much smaller than an export's a viewport band is: the walk's
/// band budget is about 300 ms of GPU, and a frame the UI stays usable
/// through wants a fraction of that.
pub const VIEWPORT_BAND_DIVISOR: u64 = 6;

/// What the solid's path tracer keeps between frames.
pub(super) struct SolidPath {
    sum: PathSum,
    output: (Texture, TextureView),
    width: u32,
    height: u32,
    /// Group 0 of the path pipelines: the escape uniform, the palette
    /// and its sampler, the sum and the path's uniform.
    layout: BindGroupLayout,
    /// The escape uniform at the output's size.
    params: Buffer,
    /// Rows of the pass in progress already traced: a sample is a pass
    /// over every row.
    row: u32,
    /// Whether the picture shown is this one, not the walk's.
    shown: bool,
}

impl SolidPath {
    fn new(device: &Device, w: u32, h: u32) -> Self {
        let entry = |binding: u32, ty: BindingType| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let uniform = BindingType::Buffer { ty: BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Solid Path"),
            entries: &[
                entry(0, uniform),
                entry(
                    2,
                    BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                ),
                entry(3, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(
                    8,
                    BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(9, uniform),
            ],
        });
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Solid Path Params"),
            size: std::mem::size_of::<EscapeParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SolidPath {
            sum: PathSum::new(device, w * h),
            output: EscapeRenderer::create_output(device, w, h),
            width: w,
            height: h,
            layout,
            params,
            row: 0,
            shown: false,
        }
    }

    /// The output's size; the sum starts over when it changes.
    fn resize(&mut self, device: &Device, w: u32, h: u32) {
        if (w, h) == (self.width, self.height) {
            return;
        }
        self.output.0.destroy();
        self.output = EscapeRenderer::create_output(device, w, h);
        self.sum.ensure(device, w * h);
        self.width = w;
        self.height = h;
        self.row = 0;
        self.shown = false;
    }

    pub(super) fn view(&self) -> Option<&TextureView> {
        self.shown.then_some(&self.output.1)
    }

    pub(super) fn destroy(&self) {
        self.sum.destroy();
        self.output.0.destroy();
        self.params.destroy();
    }
}

impl EscapeRenderer {
    /// Start the solid's path-traced picture over, and show the walk's.
    pub fn reset_solid_path(&mut self) {
        if let Some(p) = self.solid_path.as_mut() {
            p.sum.reset();
            p.row = 0;
            p.shown = false;
        }
    }

    /// Complete path-traced samples of the solid so far.
    pub fn solid_path_samples(&self) -> u32 {
        self.solid_path.as_ref().map_or(0, |p| p.sum.count())
    }

    /// Samples so far with a part-done pass counted as its share of the
    /// rows.
    pub fn solid_path_progress(&self) -> f32 {
        self.solid_path
            .as_ref()
            .map_or(0.0, |p| p.sum.count() as f32 + p.row as f32 / p.height.max(1) as f32)
    }

    /// Whether the output is the path tracer's picture.
    pub fn solid_path_shown(&self) -> bool {
        self.solid_path.as_ref().is_some_and(|p| p.shown)
    }

    /// Show the path tracer's picture, or the walk's.
    pub fn show_solid_path(&mut self, on: bool) {
        if let Some(p) = self.solid_path.as_mut() {
            p.shown = on;
        }
    }

    /// One frame of a solid's viewport in the config's tier (plan
    /// section 8): Auto path traces once the walk has settled -- the
    /// caller asks only then -- and shows the path tracer's picture from
    /// [`PATH_SHOW_SAMPLES`]; Path Traced shows it from the first band.
    /// True while there is more to do.
    pub fn trace_solid_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, palette_view: &TextureView) -> bool {
        let tier = config.escape.solid_tier;
        if tier == RenderTier::Lit {
            self.show_solid_path(false);
            return false;
        }
        let target = config.escape.path.samples.max(1);
        let more = self.trace_solid(device, queue, config, palette_view, target, VIEWPORT_BAND_DIVISOR);
        let show = tier == RenderTier::PathTraced || self.solid_path_samples() >= PATH_SHOW_SAMPLES.min(target);
        self.show_solid_path(show);
        more
    }

    /// The export's picture: `samples` path-traced samples of the solid,
    /// in bands of the full budget, `wait` called between them (a
    /// blocking poll on the desktop). Shown once done.
    pub fn render_solid_still(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        palette_view: &TextureView,
        samples: u32,
        mut wait: impl FnMut(),
    ) {
        self.reset_solid_path();
        while self.trace_solid(device, queue, config, palette_view, samples.max(1), 1) {
            wait();
        }
        wait();
        let any = self.solid_path_samples() > 0;
        self.show_solid_path(any);
    }

    /// One dispatch of the solid's path tracer: a band of rows at one
    /// sample, or -- where the whole frame fits the budget -- every row
    /// at as many samples as fit, up to `target`; then the mean into the
    /// output. The band is the walk's budget over `divisor`. Submits its
    /// own work. True while the sum holds fewer than `target` samples.
    pub fn trace_solid(
        &mut self,
        device: &Device,
        queue: &Queue,
        config: &FractalConfig,
        palette_view: &TextureView,
        target: u32,
        divisor: u64,
    ) -> bool {
        let escape = &config.escape;
        let Some(def) = crate::escape::ifs::get_ifs(&escape.formula).filter(|d| d.solid) else {
            return false;
        };
        if self.ifs.is_none() {
            return false;
        }
        let (w, h) = (self.out_width.max(1), self.out_height.max(1));
        match self.solid_path.as_mut() {
            Some(p) => p.resize(device, w, h),
            None => self.solid_path = Some(SolidPath::new(device, w, h)),
        }
        if self.solid_path_samples() >= target {
            return false;
        }
        // What the walk has before its first band.
        self.ensure_lens(device, queue, escape);
        self.ensure_ifs_seeds(escape);
        self.upload_ifs(device, queue, true);
        self.upload_ifs_chain(device, queue);
        let key = self.ensure_solid_path_pipeline(device, escape, def);

        // The escape uniform at the output's size: the path traces
        // display pixels, its jitter the antialiasing.
        let mut params = self.params_for(escape);
        params.width = w;
        params.height = h;
        params.stride = 1;
        params.tile_y0 = 0;
        params.flags &= !8;
        // The eye's offset from the target, and the field of view.
        let eye = params.fdata[2];
        let distance = (eye[0] * eye[0] + eye[1] * eye[1] + eye[2] * eye[2]).sqrt();
        let settings = path_core::path_settings(config, distance);
        let per_ray = 2.0 * (eye[3] * 0.5).tan() / h as f32;
        let param = |name: &str, fallback: f32| {
            escape.formula_params.get(name).copied().unwrap_or_else(|| {
                def.parameters.iter().find(|p| p.name == name).map_or(fallback, |p| p.default)
            })
        };
        let radius = path_core::light_radius(param("shadow_sharpness", 12.0));
        let rows_all = self.solid_path_rows(escape, def, settings.bounces, w, divisor);
        let rows = rows_all.min(h).max(1);

        let ifs_group = self.ifs_bind_group(device);
        let path = self.solid_path.as_ref().expect("made above");
        queue.write_buffer(&path.params, 0, bytemuck::bytes_of(&params));
        let count = path.sum.count();
        let (y0, band, n) = if path.row == 0 && rows >= h {
            // A whole frame a dispatch: as many samples as fit.
            (0, h, (rows_all / h).clamp(1, 64).min(target - count))
        } else {
            (path.row, rows.min(h - path.row), 1)
        };
        let uniform = PathParamsGpu::new(&settings, count, n, per_ray, param("shadow", 0.7), radius, (y0, band)).buffer(device, queue);
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Solid Path"),
            layout: &path.layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: path.params.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(palette_view) },
                BindGroupEntry { binding: 3, resource: BindingResource::Sampler(&self.palette_sampler) },
                BindGroupEntry { binding: 8, resource: path.sum.buffer().as_entire_binding() },
                BindGroupEntry { binding: 9, resource: uniform.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Escape Solid Path") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Escape Solid Path"), timestamp_writes: None });
            pass.set_pipeline(&self.pipelines[&key]);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &ifs_group, &[]);
            if let Some(l) = self.lens_gpu.as_ref() {
                pass.set_bind_group(2, l.bind_group(), &[]);
            }
            pass.dispatch_workgroups(w.div_ceil(8), band.div_ceil(8), 1);
        }
        queue.submit(std::iter::once(enc.finish()));

        let path = self.solid_path.as_mut().expect("made above");
        path.row = y0 + band;
        if path.row >= h {
            path.row = 0;
            path.sum.add(n);
        }
        path.sum.resolve(device, queue, &path.output.1, w, h, path.row);
        path.sum.count() < target
    }

    /// Rows of a `w`-wide frame one dispatch of one sample may cover:
    /// the walk's model (`direct_rows_per_dispatch`) with a sample's
    /// marches counted -- at each of the camera's surface and `bounces`
    /// more, a shadow march a light, the next march, and the normal's six
    /// distances and the colouring's walk -- under the walk's budget, its
    /// session shrink, and `divisor`.
    fn solid_path_rows(&self, escape: &crate::config::escape::EscapeConfig, def: &crate::escape::ifs::IfsDef, bounces: u32, w: u32, divisor: u64) -> u32 {
        super::tuning::ensure_loaded();
        let shift = super::DIRECT_BUDGET_SHIFT.load(std::sync::atomic::Ordering::Relaxed);
        let param = |name: &str, fallback: f32| {
            escape.formula_params.get(name).copied().unwrap_or_else(|| {
                def.parameters.iter().find(|p| p.name == name).map_or(fallback, |p| p.default)
            })
        };
        let steps = param("steps", 96.0).max(4.0) as u64;
        let (sh, _, _, _) = &self.solid_lighting;
        let lights = if crate::config::SolidShadingSettings::is_default(sh) {
            1
        } else {
            sh.lights.iter().filter(|l| l.enabled && l.intensity > 0.0).count() as u64
        };
        let per_sample = (1 + bounces as u64) * (steps * (1 + lights) + 7);
        let maps = self.ifs.as_ref().and_then(|p| p.solid.as_ref()).map_or(1, |(_, r)| r.len()) as u64;
        // `ifs_rows_per_dispatch`'s arithmetic, unclamped by a height: the
        // caller wants to know how many FRAMES fit as well.
        let per_pixel = (param("levels", 24.0).max(1.0) as u64)
            .saturating_mul(param("beam", 8.0).max(1.0) as u64)
            .saturating_mul(per_sample)
            .saturating_mul(maps.max(1));
        let per_row = (w.max(1) as u64).saturating_mul(per_pixel);
        let rows = ((super::IFS_SOLID_BUDGET >> shift) / divisor.max(1)) / per_row.max(1);
        rows.clamp(1, u32::MAX as u64) as u32
    }

    /// The path pipeline for this formula, colouring, beam and lens, as
    /// `ensure_pipeline` keys the walk's.
    fn ensure_solid_path_pipeline(
        &mut self,
        device: &Device,
        escape: &crate::config::escape::EscapeConfig,
        def: &crate::escape::ifs::IfsDef,
    ) -> String {
        let coloring = crate::escape::ifs::get_ifs_coloring(&escape.coloring, def);
        let beam = escape
            .formula_params
            .get("beam")
            .copied()
            .unwrap_or_else(|| def.parameters.iter().find(|p| p.name == "beam").map_or(1.0, |p| p.default))
            .clamp(1.0, assembler::IFS_MAX_BEAM as f32) as u32;
        let registry = crate::variations::global_registry();
        let lens_src = crate::escape::lens::lens_source(escape, &registry);
        let lens_id = crate::escape::lens::lens_key(escape, &registry);
        let sums = self.ifs.as_ref().is_some_and(|p| p.rows.iter().any(|r| r.kind == 7.0));
        let key = format!("ifs_path|{}|{}|b{beam}|{lens_id}|{sums}", def.name, coloring.name);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_ifs_path(def, coloring, beam, lens_src.as_deref(), sums);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let path = self.solid_path.as_ref().expect("made before the pipeline");
            let mut groups = vec![Some(&path.layout), Some(&self.ifs_bind_group_layout)];
            if lens_src.is_some() {
                groups.push(Some(&self.lens_bind_group_layout));
            }
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Solid Path Pipeline Layout"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    /// Mode D's group 1: the maps, the chain, the geometry cache, the
    /// coarse pass, the transition graph and the reference.
    fn ifs_bind_group(&self, device: &Device) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Solid Path IFS"),
            layout: &self.ifs_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: self.ifs_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: self.ifs_chain_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.ifs_geom_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: self.ifs_coarse_buffer.as_entire_binding() },
                BindGroupEntry { binding: 4, resource: self.ifs_xaos_buffer.as_entire_binding() },
                BindGroupEntry { binding: 5, resource: self.ifs_ref_buffer.as_entire_binding() },
            ],
        })
    }

    /// The path tracer's sum, for a test to read back.
    #[cfg(test)]
    pub(crate) fn solid_path_sum_for_test(&self) -> Option<&Buffer> {
        self.solid_path.as_ref().map(|p| p.sum.buffer())
    }

    /// The path tracer's output texture, for a test to read back.
    #[cfg(test)]
    pub(crate) fn solid_path_texture_for_test(&self) -> Option<&Texture> {
        self.solid_path.as_ref().map(|p| &p.output.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::escape::RenderTier;
    use crate::scene::transforms::{Flame, RenderMode, Transform};
    use egui_wgpu::wgpu::{
        Backends, Extent3d, Features, Instance, InstanceDescriptor, MapMode, MemoryHints, PowerPreference,
        RequestAdapterOptions, TexelCopyBufferInfo, TexelCopyBufferLayout, TextureDescriptor, TextureDimension,
        TextureFormat, TextureUsages, TextureViewDescriptor,
    };
    use std::collections::HashMap;

    fn device() -> Option<(Device, Queue)> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&egui_wgpu::wgpu::DeviceDescriptor {
            label: Some("solid path tests"),
            required_features: adapter.features() & Features::CLEAR_TEXTURE,
            required_limits: adapter.limits(),
            memory_hints: MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()?;
        device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu error in solid path tests: {e}")));
        Some((device, queue))
    }

    fn wait(device: &Device) {
        let _ = device.poll(egui_wgpu::wgpu::PollType::Wait { submission_index: None, timeout: None });
    }

    fn read_buffer(device: &Device, queue: &Queue, src: &Buffer, size: u64) -> Vec<u8> {
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(MapMode::Read, |_| {});
        wait(device);
        let out = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        out
    }

    /// An Rgba32Float texture's texels.
    fn read_texture(device: &Device, queue: &Queue, tex: &Texture, w: u32, h: u32) -> Vec<[f32; 4]> {
        let row = (w * 16).div_ceil(256) * 256;
        let staging = device.create_buffer(&BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &staging,
                layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        staging.slice(..).map_async(MapMode::Read, |_| {});
        wait(device);
        let bytes = staging.slice(..).get_mapped_range().to_vec();
        staging.unmap();
        let mut out = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let o = (y * row + x * 16) as usize;
                let f: &[f32] = bytemuck::cast_slice(&bytes[o..o + 16]);
                out.push([f[0], f[1], f[2], f[3]]);
            }
        }
        out
    }

    /// A white palette, so a surface's albedo is its colouring's
    /// brightness alone.
    fn white_palette(device: &Device, queue: &Queue) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("white palette"),
            size: Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            tex.as_image_copy(),
            &[255u8; 256 * 4],
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(1) },
            Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// The unit cube as an IFS: eight half-scale maps to its corners. A
    /// solid with no concavity, so a ray leaving its surface never meets
    /// it again.
    fn cube() -> Flame {
        let mut fl = Flame::default();
        fl.transforms.clear();
        for k in 0..8u32 {
            let mut t = Transform::default();
            t.a = 1.0;
            t.b = 0.0;
            t.c = 0.0;
            t.d = 1.0;
            t.e = (k & 1) as f32;
            t.f = ((k >> 1) & 1) as f32;
            t.g = ((k >> 2) & 1) as f32;
            t.color = k as f32 / 8.0;
            t.variations = HashMap::from([("linear3D".to_string(), 0.5)]);
            t.variation_order = vec!["linear3D".to_string()];
            fl.transforms.push(t);
        }
        fl.final_transforms.clear();
        fl.xaos = None;
        fl
    }

    /// The cube in mode D, coloured flat (address colouring, no halo: a
    /// brightness of one), lit by nothing until a test says otherwise.
    fn cube_config() -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.flame = cube();
        c.escape.formula = "ifs_flame_3d".to_string();
        c.escape.coloring = "ifs_address".to_string();
        c.escape.coloring_params.insert("reach".to_string(), 0.0);
        c.escape.zoom_log2 = 0.0;
        c.escape.cam_yaw = 0.9;
        c.escape.cam_pitch = 0.42;
        c.escape.solid_tier = RenderTier::PathTraced;
        c.gamma = 1.0;
        c.exposure = 1.0;
        c.background_color = [0.0; 3];
        c.solid_shading.shading_strength = 1.0;
        c.solid_shading.ambient = 0.0;
        c.solid_shading.specular = 0.0;
        c.solid_shading.ssao_strength = 0.0;
        for l in c.solid_shading.lights.iter_mut() {
            l.enabled = false;
        }
        c.escape.path.gloss = 0.0;
        c
    }

    fn renderer_for(device: &Device, c: &FractalConfig, w: u32, h: u32) -> EscapeRenderer {
        let mut r = EscapeRenderer::new(device, w, h);
        r.resize(device, w, h, 1);
        let def = crate::escape::ifs::get_ifs(&c.escape.formula).expect("solid");
        let registry = crate::variations::global_registry();
        let packed = crate::escape::ifs::pack_for(def, c, &registry);
        assert!(packed.is_some(), "the cube does not qualify");
        r.set_ifs(packed);
        r.set_solid_lighting(&c.solid_shading, (c.fog_strength, c.fog_start, c.background_color));
        r
    }

    /// The white furnace on a solid: albedo 1, a uniform sky L and no
    /// light. The cube is convex, so every bounce escapes to the sky and
    /// every surface reads L -- whatever the bounce count, exactly but
    /// for Russian roulette's noise past the first.
    #[test]
    fn the_white_furnace_on_a_solid() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (48u32, 32u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let l = [0.5f32, 0.25, 0.75];
        for bounces in [1u32, 3] {
            let mut c = cube_config();
            c.background_color = l;
            c.escape.path.bounces = bounces;
            let mut r = renderer_for(&device, &c, w, h);
            r.render_solid_still(&device, &queue, &c, &palette, 64, || wait(&device));
            assert_eq!(r.solid_path_samples(), 64);
            let out = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
            let hit: Vec<&[f32; 4]> = out.iter().filter(|p| p[3] > 0.0).collect();
            assert!(hit.len() > (w * h / 8) as usize, "the cube covers {} pixels", hit.len());
            for k in 0..3 {
                let mean = hit.iter().map(|p| p[k] as f64).sum::<f64>() / hit.len() as f64;
                let worst = hit.iter().map(|p| (p[k] - l[k]).abs()).fold(0.0f32, f32::max);
                println!("furnace, {bounces} bounces, channel {k}: mean {mean:.5} against {}, worst {worst:.4}", l[k]);
                assert!((mean - l[k] as f64).abs() < 0.01 * l[k] as f64, "{bounces} bounces, channel {k}: {mean}");
            }
            r.destroy();
        }
    }

    /// Sun only, through a pinhole, with no bounce: the path tracer's
    /// face is the lit tier's `albedo I diffuse cos`, its relight of the
    /// walk -- the two tiers light a solid alike.
    #[test]
    fn a_sunlit_solid_is_the_lit_tiers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.solid_shading.lights[0].enabled = true;
        c.solid_shading.lights[0].azimuth = 30.0;
        c.solid_shading.lights[0].elevation = 35.0;
        c.solid_shading.lights[0].intensity = 1.5;
        c.solid_shading.lights[0].color = [1.0, 0.5, 0.25];
        c.escape.formula_params.insert("shadow".to_string(), 0.0);
        c.escape.formula_params.insert("shadow_sharpness".to_string(), 1.0e4);
        c.escape.path.bounces = 0;
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(&device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let lit = read_texture(&device, &queue, &r.output_texture, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 4, || wait(&device));
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        // A face's inside: lit there and alike to all eight neighbours --
        // a jittered sample anywhere in the pixel is on the same face, and
        // clear of the normal's blur across an edge (its step is a
        // pixel) -- with all the path's samples on it.
        let at = |x: u32, y: u32| lit[(y * w + x) as usize];
        let mut n = 0;
        let mut worst = 0.0f32;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let (p, q) = (at(x, y), path[(y * w + x) as usize]);
                let flat = (0..9u32).all(|i| {
                    let o = at(x + i % 3 - 1, y + i / 3 - 1);
                    (0..4).all(|k| (p[k] - o[k]).abs() < 1e-4)
                });
                if p[3] < 1.0 || q[3] < 1.0 || !flat || p[0] <= 0.0 {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    worst = worst.max((p[k] - q[k]).abs() / p[k].max(1e-3));
                }
            }
        }
        println!("sunlit solid: {n} face pixels, worst relative difference {worst:.2e}");
        assert!(n > 10, "only {n} face pixels");
        assert!(worst < 1e-3, "{worst}");
        r.destroy();
    }

    /// The path tracer covers what the lit tier covers. Its camera rays
    /// are jittered over the pixel where the walk's go through its
    /// centre, so on a fractal full of sub-pixel holes the two agree on
    /// AVERAGE coverage, and inside a patch the walk sees as solid all
    /// round the path tracer leaves nothing of the background showing. A
    /// ray lost to the march -- out of steps, or stopped short -- would
    /// show as the background's colour through the solid.
    #[test]
    fn the_path_tracer_covers_what_the_lit_tier_covers() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_pt, palette) = white_palette(&device, &queue);
        let (w, h) = (320u32, 180u32);
        // The Sierpinski tetrahedron, the shipped preset's view.
        let mut c = cube_config();
        c.flame.transforms.truncate(4);
        for (t, v) in c.flame.transforms.iter_mut().zip([[0.0f32, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]) {
            t.e = v[0];
            t.f = v[1];
            t.g = v[2];
        }
        c.escape.cam_yaw = 2.6;
        c.escape.cam_pitch = 0.25;
        c.solid_shading.lights[0].enabled = true;
        c.escape.solid_tier = RenderTier::Lit;
        let mut r = renderer_for(&device, &c, w, h);
        loop {
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
            let settled = r.render(&device, &queue, &mut enc, &c.escape, &palette, 1);
            queue.submit(std::iter::once(enc.finish()));
            if settled {
                break;
            }
        }
        let lit = read_texture(&device, &queue, &r.output_texture, w, h);
        r.render_solid_still(&device, &queue, &c, &palette, 16, || wait(&device));
        let path = read_texture(&device, &queue, r.solid_path_texture_for_test().expect("traced"), w, h);
        let mean = |img: &[[f32; 4]]| img.iter().map(|p| p[3] as f64).sum::<f64>() / img.len() as f64;
        let at = |x: u32, y: u32| lit[(y * w + x) as usize][3];
        let (mut n, mut cov) = (0usize, 0.0f64);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                if (0..9u32).all(|i| at(x + i % 3 - 1, y + i / 3 - 1) >= 1.0) {
                    n += 1;
                    cov += path[(y * w + x) as usize][3] as f64;
                }
            }
        }
        let (ml, mp, inside) = (mean(&lit), mean(&path), cov / n.max(1) as f64);
        println!("coverage: lit {ml:.4}, path {mp:.4}; inside {n} solid patch pixels the path covers {inside:.4}");
        assert!(n > 500, "only {n} patch pixels");
        assert!((mp - ml).abs() < 0.02 * ml, "mean coverage {mp} against {ml}");
        assert!(inside > 0.995, "the background shows through a solid patch: {inside}");
        r.destroy();
    }

    /// The same samples in any banding are the same bits: three samples
    /// as whole frames and as one-row bands, a sky and a sun, three
    /// bounces and a coat -- and again.
    #[test]
    fn solid_samples_are_band_invariant() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (40u32, 24u32);
        let (_pt, palette) = white_palette(&device, &queue);
        let mut c = cube_config();
        c.background_color = [0.3, 0.4, 0.6];
        c.solid_shading.lights[0].enabled = true;
        c.escape.path.bounces = 3;
        c.escape.path.gloss = 0.04;
        let mut r = renderer_for(&device, &c, w, h);
        let mut sums = Vec::new();
        for divisor in [1u64, u64::MAX, 1, u64::MAX] {
            r.reset_solid_path();
            let mut guard = 0;
            while r.trace_solid(&device, &queue, &c, &palette, 3, divisor) {
                guard += 1;
                assert!(guard < 10_000);
            }
            assert_eq!(r.solid_path_samples(), 3);
            sums.push(read_buffer(&device, &queue, r.solid_path_sum_for_test().expect("traced"), (w * h * 16) as u64));
        }
        assert!(sums.iter().all(|s| *s == sums[0]), "the banding moved the sum");
        r.destroy();
    }
}
