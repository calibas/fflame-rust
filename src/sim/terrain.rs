//! A simulation's 3D terrain (docs/projects/heightfield-3d.md, T4 and
//! section 6): the grid as a height field -- one channel of a layer,
//! smoothed by the relief stage -- coloured by the colour stack once a
//! cell, seen through a solid camera over the grid and drawn by the
//! escape engine's terrain renderer, in its tiers.
//!
//! The world is the grid's cells: x right, y up the picture (the
//! picture's top is north), z the height, `height` grid widths a field
//! unit. The ground is made again whenever the field or how it is drawn
//! changes -- every step of a running simulation, so a run shows the lit
//! tier, and the path tracer gathers once it rests.

use crate::config::escape::RenderTier;
use crate::config::FractalConfig;
use crate::escape::ifs::{solid_frame, SolidCamera};
use crate::escape::path_core::{path_settings_from, PathSettings};
use crate::escape::terrain::{TerrainRenderer, TerrainView};
use crate::escape::terrain_tiers::{TerrainTiers, TierInputs};
use wgpu::{Device, Queue, TextureView};

/// The lit tier's antialiasing grid, a side.
const SUPERSAMPLE: u32 = 2;

/// Field units the slab reaches above and below zero: the height has no
/// bound a config states, and the slab only clips rays.
const FIELD_BOUND: f32 = 64.0;

/// The camera over a `gw x gh` grid: the solid camera's angles (yaw 0
/// looking north), its target on the grid at half a field unit's height,
/// `cam_distance` grid widths away.
pub fn sim_terrain_camera(config: &FractalConfig, gw: u32, gh: u32) -> SolidCamera {
    let t = &config.sim.terrain;
    let (right, up, forward) = solid_frame(
        t.cam_pitch as f64,
        t.cam_yaw as f64 - std::f64::consts::FRAC_PI_2,
        t.cam_bank as f64,
        0.0,
    );
    let w = gw.max(2) as f64;
    let distance = t.cam_distance.max(1.0e-3) as f64 * w;
    let target = [
        t.target_x as f64 * (gw.max(2) - 1) as f64,
        t.target_y as f64 * (gh.max(2) - 1) as f64,
        0.5 * t.height as f64 * w,
    ];
    let eye_rel = [-forward[0] * distance, -forward[1] * distance, -forward[2] * distance];
    SolidCamera {
        eye: [target[0] + eye_rel[0], target[1] + eye_rel[1], target[2] + eye_rel[2]],
        target,
        forward,
        right,
        up,
        fov: t.cam_fov.clamp(0.05, 3.0),
        distance,
        eye_rel,
    }
}

/// How the terrain is seen and lit: the camera, the Solid lighting
/// (world-fixed), and -- on a repeated ground -- the fog toward `far`.
pub fn sim_terrain_view(config: &FractalConfig, gw: u32, gh: u32, jitter: [f32; 2]) -> TerrainView {
    let t = &config.sim.terrain;
    let w = gw.max(2) as f32;
    let camera = sim_terrain_camera(config, gw, gh);
    let (fog, start, far) = if t.tiling == crate::config::sim::SimTiling::Repeat {
        let far = t.far.max(0.5) * w;
        let haze = t.haze.clamp(0.0, 1.0);
        if haze > 0.0 {
            let start = far * (1.0 - 0.65 * haze);
            (4.6 / (far - start).max(1.0e-6), start, far)
        } else {
            (0.0, 0.0, far)
        }
    } else {
        // The grid alone: everything in it, nothing beyond.
        (0.0, 0.0, (t.cam_distance.max(0.0) + 4.0) * w)
    };
    TerrainView {
        camera,
        shading: config.solid_shading.clone(),
        fog: (fog, start, config.background_color),
        shadow: t.shadow,
        softness: t.shadow_sharpness,
        occlusion_reach: t.occlusion * w,
        jitter,
        height: t.height * w,
        width: 1.0,
        samples_per_axis: SUPERSAMPLE,
        far,
        sky: crate::escape::path_core::sky_seen(&t.path, config),
    }
}

/// The colour by the height: when the colour stack is the palette over
/// the height's own channel -- one Channel colouring (the base, or the
/// one enabled layer at full opacity, Normal) of the terrain's layer and
/// channel -- the palette coordinate per unit of the ground's height
/// (the colouring's scale over `height_scale`, the ground's cells a
/// field unit), its offset, and 1 to wrap. Then the terrain colours
/// each point by the palette at its own height, and the bands follow
/// the height's contours down the steepest face (heightfield plan, T4).
/// None for any other stack: its colours, a cell's each.
pub fn height_colour(config: &FractalConfig, height_scale: f32) -> Option<[f32; 3]> {
    let s = &config.sim;
    let t = &s.terrain;
    let (coloring, params, source) = if s.color_layers.is_empty() {
        (&s.coloring, &s.coloring_params, 0usize)
    } else {
        let mut on = s.color_layers.iter().filter(|l| l.enabled);
        let layer = on.next()?;
        if on.next().is_some() || layer.gather || layer.opacity < 1.0 || layer.blend != crate::config::sim::SimBlend::Normal {
            return None;
        }
        (&layer.coloring, &layer.coloring_params, layer.source)
    };
    if coloring != "channel" || source != t.layer as usize || !(height_scale.abs() > 1.0e-20) {
        return None;
    }
    let def = crate::sim::COLORINGS.iter().find(|c| c.name == "channel")?;
    let param = |name: &str| {
        params.get(name).copied().unwrap_or_else(|| def.parameters.iter().find(|p| p.name == name).map_or(0.0, |p| p.default))
    };
    if param("channel").round().clamp(0.0, 3.0) as u32 != t.channel {
        return None;
    }
    Some([param("scale") / height_scale, param("offset"), if param("wrap") >= 0.5 { 1.0 } else { 0.0 }])
}

/// The path tracer's settings: the terrain's own block, the lens in
/// the target's terms.
pub fn sim_path_settings(config: &FractalConfig, gw: u32) -> PathSettings {
    let t = &config.sim.terrain;
    path_settings_from(&t.path, config, t.cam_distance.max(1.0e-3) * gw.max(2) as f32, 0.05)
}

/// What the ground is made from: the run's step, what is drawn of it (the
/// simulation's config with the terrain's view and rendering taken out)
/// and the palette's generation.
fn ground_key(config: &FractalConfig, step: u32, palette_generation: u64) -> String {
    let mut sim = config.sim.clone();
    let t = &config.sim.terrain;
    let kept = (t.enabled, t.layer, t.channel, t.softness, t.height, t.tiling);
    sim.terrain = Default::default();
    (sim.terrain.enabled, sim.terrain.layer, sim.terrain.channel, sim.terrain.softness, sim.terrain.height, sim.terrain.tiling) = kept;
    format!("{step}|{palette_generation}|{:?}", sim)
}

/// A simulation's terrain: the renderer, its tiers, and what its ground
/// was made from.
pub struct SimTerrain {
    terrain: TerrainRenderer,
    tiers: TerrainTiers,
    made: Option<String>,
    grid: (u32, u32),
}

impl SimTerrain {
    pub fn new(device: &Device, out_w: u32, out_h: u32) -> Self {
        SimTerrain { terrain: TerrainRenderer::new(device, out_w, out_h), tiers: TerrainTiers::default(), made: None, grid: (0, 0) }
    }

    /// The output's size.
    pub fn resize(&mut self, device: &Device, out_w: u32, out_h: u32) {
        if self.terrain.resize(device, out_w, out_h) {
            self.tiers.invalidate();
        }
    }

    /// Make the ground from the simulation's field, when the field or how
    /// it is drawn has changed since the last time. Call after the
    /// simulation's `color`. Submits its own work; true when it made one.
    pub fn update(
        &mut self,
        device: &Device,
        queue: &Queue,
        sim: &mut super::SimRenderer,
        config: &FractalConfig,
        palette_view: &TextureView,
        palette_generation: u64,
    ) -> bool {
        let key = ground_key(config, sim.step_index(), palette_generation);
        let (gw, gh) = sim.grid_size();
        if self.made.as_deref() == Some(key.as_str()) && self.grid == (gw, gh) {
            return false;
        }
        let t = &config.sim.terrain;
        let repeat = t.tiling == crate::config::sim::SimTiling::Repeat;
        let scale = t.height * gw.max(2) as f32;
        let bound = FIELD_BOUND * scale.abs() + gw as f32;
        let (relief, albedo) = sim.terrain_inputs(device, queue, &config.sim, palette_view);
        self.terrain.set_grid(device, queue, relief, albedo, gw, gh, scale, bound, repeat);
        self.terrain.set_height_colour(palette_view, height_colour(config, scale));
        self.made = Some(key);
        self.grid = (gw, gh);
        true
    }

    fn inputs<'a>(&self, config: &FractalConfig, view: &'a dyn Fn([f32; 2]) -> TerrainView) -> TierInputs<'a> {
        let t = &config.sim.terrain;
        TierInputs {
            view,
            settings: sim_path_settings(config, self.grid.0),
            tier: t.tier,
            samples: t.path.samples,
            supersample: SUPERSAMPLE,
            filling: false,
        }
    }

    /// One frame of the viewport, in the terrain's tier. True while there
    /// is more to do.
    pub fn render_viewport(&mut self, device: &Device, queue: &Queue, config: &FractalConfig) -> bool {
        let (gw, gh) = self.grid;
        let view = |jitter| sim_terrain_view(config, gw, gh, jitter);
        let inputs = self.inputs(config, &view);
        self.tiers.viewport(&mut self.terrain, device, queue, &inputs)
    }

    /// The export's picture: path traced at `samples` unless the tier is
    /// Lit. `wait` between batches.
    pub fn render_still(&mut self, device: &Device, queue: &Queue, config: &FractalConfig, frame: (u32, u32), wait: impl FnMut()) {
        let (gw, gh) = self.grid;
        let view = |jitter| sim_terrain_view(config, gw, gh, jitter);
        let inputs = self.inputs(config, &view);
        self.tiers.still(&mut self.terrain, device, queue, &inputs, frame, wait);
    }

    pub fn output_view(&self) -> &TextureView {
        self.tiers.output_view(&self.terrain)
    }

    /// Path-traced samples so far, and whether the picture is theirs.
    pub fn path_progress(&self) -> (u32, bool) {
        self.tiers.path_progress(&self.terrain)
    }

    /// Whether the tier draws the path tracer at all.
    pub fn path_traces(config: &FractalConfig) -> bool {
        config.sim.terrain.tier != RenderTier::Lit
    }

    pub fn destroy(&self) {
        self.terrain.destroy();
        self.tiers.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::sim::SimGrid;
    use crate::scene::transforms::RenderMode;

    fn device() -> Option<(Device, Queue)> {
        crate::escape::terrain::gpu_tests::device()
    }

    fn white_palette(device: &Device, queue: &Queue) -> (wgpu::Texture, TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white palette"),
            size: wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            tex.as_image_copy(),
            &[255u8; 256 * 4],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    /// Gray-Scott on a fixed grid with its terrain on.
    fn config(n: u32, steps: u32) -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Simulation;
        c.sim.grid = SimGrid::Fixed { width: n, height: n };
        c.sim.steps = steps;
        c.sim.terrain.enabled = true;
        c.background_color = [0.4, 0.5, 0.6];
        c
    }

    /// The colour by the height applies exactly when the colour stack is
    /// the palette over the height's own channel: one Channel colouring of
    /// the terrain's layer and channel, at full opacity and Normal. Its
    /// mapping is the colouring's scale over the ground's height scale,
    /// the offset and the wrap, with the colouring's defaults.
    #[test]
    fn colour_by_height_applies_to_the_heights_own_channel() {
        let mut c = config(64, 10);
        c.sim.terrain.channel = 1;
        c.sim.coloring = "channel".into();
        c.sim.coloring_params.clear();
        // The defaults: channel 1, scale 3, offset 0, clamp.
        assert_eq!(height_colour(&c, 12.0), Some([3.0 / 12.0, 0.0, 0.0]));
        c.sim.coloring_params.insert("scale".into(), 2.0);
        c.sim.coloring_params.insert("offset".into(), 0.25);
        c.sim.coloring_params.insert("wrap".into(), 1.0);
        assert_eq!(height_colour(&c, 4.0), Some([0.5, 0.25, 1.0]));
        // Another channel, another colouring, another layer: its colours.
        c.sim.coloring_params.insert("channel".into(), 0.0);
        assert_eq!(height_colour(&c, 4.0), None);
        c.sim.coloring_params.insert("channel".into(), 1.0);
        c.sim.coloring = "two_channel".into();
        assert_eq!(height_colour(&c, 4.0), None);
        c.sim.coloring = "channel".into();
        c.sim.terrain.layer = 1;
        assert_eq!(height_colour(&c, 4.0), None);
        c.sim.terrain.layer = 0;
        // A stack: one enabled Channel layer of the terrain's layer.
        let layer = crate::config::sim::SimColorLayer {
            coloring: "channel".into(),
            coloring_params: [("channel".to_string(), 1.0), ("scale".to_string(), 5.0)].into_iter().collect(),
            ..Default::default()
        };
        c.sim.color_layers = vec![layer.clone()];
        assert_eq!(height_colour(&c, 10.0), Some([0.5, 0.0, 0.0]));
        c.sim.color_layers = vec![crate::config::sim::SimColorLayer { opacity: 0.5, ..layer.clone() }];
        assert_eq!(height_colour(&c, 10.0), None, "a translucent layer is a blend");
        c.sim.color_layers = vec![layer.clone(), layer.clone()];
        assert_eq!(height_colour(&c, 10.0), None, "two layers are a blend");
        c.sim.color_layers = vec![layer.clone(), crate::config::sim::SimColorLayer { enabled: false, ..layer }];
        assert_eq!(height_colour(&c, 10.0), Some([0.5, 0.0, 0.0]), "a disabled layer is not drawn");
    }

    /// A simulation's terrain renders through `render_with` -- lit, and
    /// path traced -- and is not the flat picture: the ground covers the
    /// middle of the frame and leaves the background at its top.
    #[test]
    fn a_sim_terrain_renders() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (96u32, 64u32);
        let mut flat = config(64, 400);
        flat.sim.terrain.enabled = false;
        let render = |c: &FractalConfig| {
            pollster::block_on(crate::renderer::render(&device, &queue, crate::renderer::RenderJob::new(c, w, h), &mut crate::renderer::NoProgress))
                .expect("render")
                .rgba_data
        };
        let plain = render(&flat);
        let sky = [0.4f32, 0.5, 0.6].map(|v| v * 255.0);
        for tier in [RenderTier::Lit, RenderTier::PathTraced] {
            let mut c = config(64, 400);
            c.sim.terrain.tier = tier;
            c.sim.terrain.path.samples = 8;
            let out = render(&c);
            assert_ne!(out, plain, "{tier:?}: the terrain is not the flat picture");
            let is_sky = |x: u32, y: u32| {
                let p = &out[((y * w + x) * 4) as usize..][..3];
                (0..3).all(|k| (p[k] as f32 - sky[k]).abs() < 3.0)
            };
            assert!(is_sky(w / 2, 0), "{tier:?}: the background above the ground");
            assert!(!is_sky(w / 2, h / 2), "{tier:?}: the ground in the middle");
        }
    }

    /// The viewport as the app drives it: while the run steps, every frame
    /// makes a new ground and restarts the tiers (the lit tier shows);
    /// once it rests, the path tracer gathers to its target.
    #[test]
    fn a_resting_run_path_traces() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (_p, palette) = white_palette(&device, &queue);
        let (w, h) = (160u32, 96u32);
        let mut c = config(64, 10_000);
        c.sim.terrain.path.samples = 12;
        let mut sim = crate::sim::SimRenderer::new(&device, &c.sim, w, h);
        let mut t = SimTerrain::new(&device, w, h);
        let wait = || {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        };
        for _ in 0..12 {
            sim.render_frame(&device, &queue, &c.sim, &palette, 20);
            assert!(t.update(&device, &queue, &mut sim, &c, &palette, 1), "a step makes a new ground");
            t.render_viewport(&device, &queue, &c);
            wait();
            assert_eq!(t.path_progress().0, 0, "a running simulation is not path traced");
        }
        let mut frames = 0;
        loop {
            sim.render_frame(&device, &queue, &c.sim, &palette, 0);
            assert!(!t.update(&device, &queue, &mut sim, &c, &palette, 1), "a resting run keeps its ground");
            let more = t.render_viewport(&device, &queue, &c);
            wait();
            frames += 1;
            if !more {
                break;
            }
            assert!(frames < 400, "still busy at {:?}", t.path_progress());
        }
        assert_eq!(t.path_progress(), (12, true));
        t.destroy();
    }

    /// Renders config files for inspection, as the CLI's export does:
    /// `INSPECT="a.fflame=a.png;b.fflame=b.png"`, `INSPECT_SIZE=WxH`.
    /// For when the release executable is busy.
    #[test]
    #[ignore = "inspection; needs a GPU and INSPECT"]
    fn render_configs_for_inspection() {
        let (device, queue) = device().expect("gpu");
        let list = std::env::var("INSPECT").unwrap_or_default();
        let (w, h) = std::env::var("INSPECT_SIZE")
            .ok()
            .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
            .unwrap_or((1920u32, 1080u32));
        for pair in list.split(';').filter(|p| !p.is_empty()) {
            let (src, dst) = pair.split_once('=').expect("in=out");
            let c = FractalConfig::from_json(&std::fs::read_to_string(src).expect("read")).expect("parse");
            let out = pollster::block_on(crate::renderer::render(&device, &queue, crate::renderer::RenderJob::new(&c, w, h), &mut crate::renderer::NoProgress))
                .expect("render");
            image::save_buffer(dst, &out.rgba_data, out.width, out.height, image::ColorType::Rgba8).expect("write");
            println!("{src} -> {dst} in {:.0} ms", out.render_time_ms);
        }
    }

    /// The interactive budget (plan T4's gate), measured: a running
    /// simulation's frame at 1080p -- a step batch, its colour, the ground
    /// made again and the lit tier's frame.
    #[test]
    #[ignore = "measurement; needs a GPU"]
    fn a_running_terrain_frame_at_1080p() {
        let (device, queue) = device().expect("gpu");
        let (_p, palette) = white_palette(&device, &queue);
        let (w, h) = (1920u32, 1080u32);
        let mut c = config(64, 100_000);
        c.sim.grid = SimGrid::Viewport { scale: 1.0 };
        c.sim.terrain.tier = RenderTier::Lit;
        let mut sim = crate::sim::SimRenderer::new(&device, &c.sim, w, h);
        let mut t = SimTerrain::new(&device, w, h);
        let wait = || {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        };
        for k in 0..40 {
            let t0 = std::time::Instant::now();
            sim.render_frame(&device, &queue, &c.sim, &palette, 4);
            wait();
            let t1 = std::time::Instant::now();
            t.update(&device, &queue, &mut sim, &c, &palette, 1);
            t.render_viewport(&device, &queue, &c);
            wait();
            if k >= 30 {
                println!(
                    "grid {:?}: simulation {:.1} ms, terrain {:.1} ms",
                    sim.grid_size(),
                    (t1 - t0).as_secs_f32() * 1000.0,
                    t1.elapsed().as_secs_f32() * 1000.0
                );
            }
        }
        t.destroy();
    }
}
