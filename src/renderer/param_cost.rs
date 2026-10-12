//! What a variation parameter costs: the flame's dispatch time with a
//! parameter at its default and at its maximum. A parameter that bounds a
//! loop the variation runs every iteration is a slider trap: dragged live,
//! each frame is slower than the last, and the drag overshoots before the
//! picture can say so (`ui::slow_controls`). Measured rather than read off
//! the WGSL, because the loops sit in helpers and shared libraries a
//! source scan misses.
//!
//! ```text
//! cargo test --release --lib -- --ignored --nocapture measure_variation_param_costs
//! PARAM_COST_ONLY=glsl_ cargo test ...   # variations whose name contains it
//! ```
//!
//! Every whole-number parameter, and every other one named like a count,
//! is timed. The dispatches are small -- 32 workgroups, 32 iterations a
//! thread -- so a parameter at a ten-thousand-step maximum stays far from
//! a GPU watchdog.
#![cfg(test)]

use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::transforms::{Flame, RenderMode, Transform};
use crate::variations::ParamType;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let al = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
    limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
    limits.max_buffer_size = al.max_buffer_size;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("param cost"),
        required_features: wgpu::Features::CLEAR_TEXTURE,
        required_limits: limits,
        ..Default::default()
    }))
    .ok()
}

const WORKGROUPS: u32 = 32;
const IPT: u32 = 32;

/// The app's frame (128 workgroups x 256 iterations a thread) over one
/// of these dispatches.
const FRAME_SCALE: f64 = (128.0 * 256.0) / (WORKGROUPS as f64 * IPT as f64);

/// The median of eleven timed dispatches, after three that warm up, in ms.
fn time(r: &mut FlameRenderer, device: &wgpu::Device, queue: &wgpu::Queue, cfg: &FractalConfig) -> f64 {
    let mut samples = Vec::new();
    for k in 0..14 {
        let t0 = std::time::Instant::now();
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cost") });
        r.compute_pass(
            &mut enc, queue, device, WORKGROUPS, IPT, 0, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, cfg.rotation,
            cfg.camera_rotation_x, cfg.camera_rotation_y, cfg.camera_bank, cfg.camera_x, cfg.camera_y, cfg.camera_z,
            cfg.speed_factor, true, false,
        );
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        if k >= 3 {
            samples.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn load(r: &mut FlameRenderer, device: &wgpu::Device, queue: &wgpu::Queue, cfg: &FractalConfig) {
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
    r.load_config(device, &mut enc, queue, cfg, &cfg.palette, IPT, 0);
    queue.submit(Some(enc.finish()));
}

fn looks_like_a_count(name: &str) -> bool {
    let n = name.to_lowercase();
    ["iter", "steps", "count", "depth", "level", "loops", "octave", "samples", "num"].iter().any(|k| n.contains(k))
}

#[test]
#[ignore = "needs a GPU; prints a table, run with --nocapture"]
fn measure_variation_param_costs() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let only = std::env::var("PARAM_COST_ONLY").ok();
    let candidates: Vec<(String, Vec<(String, f32, f32)>)> = {
        let registry = crate::variations::global_registry();
        registry
            .all()
            .into_iter()
            .filter(|v| !v.name.contains("subflame"))
            .filter(|v| only.as_deref().is_none_or(|o| v.name.contains(o)))
            .map(|v| {
                let params = v
                    .parameters
                    .iter()
                    .filter(|p| matches!(p.param_type, ParamType::Integer | ParamType::UnlimitedInteger) || looks_like_a_count(&p.name))
                    .filter_map(|p| p.max_value.map(|max| (p.name.clone(), p.default_value, max)))
                    .filter(|(_, d, max)| max > d)
                    .collect::<Vec<_>>();
                (v.name.clone(), params)
            })
            .filter(|(_, ps)| !ps.is_empty())
            .collect()
    };
    eprintln!("{} variations to time", candidates.len());

    let mut rows: Vec<(f64, String)> = Vec::new();
    for (name, params) in candidates {
        let mut t = Transform::new();
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("linear", 1.0);
        t.set_variation(&name, 1.0);
        let mut cfg = FractalConfig::default();
        cfg.render_mode = RenderMode::TwoD;
        cfg.flame = Flame { transforms: vec![t], ..Flame::default() };
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut r = FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64, &cfg.flame, cfg.palette_size);
        load(&mut r, &device, &queue, &cfg);
        let base = time(&mut r, &device, &queue, &cfg);
        if let Some(e) = pollster::block_on(scope.pop()) {
            eprintln!("{name}: skipped ({})", e.to_string().lines().next().unwrap_or(""));
            r.destroy();
            continue;
        }
        for (param, default, max) in params {
            let key = format!("{name}.{param}");
            cfg.flame.transforms[0].variation_params.insert(key.clone(), max);
            load(&mut r, &device, &queue, &cfg);
            let at_max = time(&mut r, &device, &queue, &cfg);
            cfg.flame.transforms[0].variation_params.insert(key, default);
            let ratio = at_max / base.max(1e-3);
            // The app's frame is 128 workgroups at 256 iterations a thread,
            // 32 of these dispatches' work: what the maximum adds to it.
            let frame_ms = (at_max - base) * FRAME_SCALE;
            rows.push((frame_ms, format!("{name:28} {param:22} {default:>8} -> {max:<8} {base:7.2} -> {at_max:8.2} ms  x{ratio:<6.1} +{frame_ms:.0} ms a frame")));
        }
        r.destroy();
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("variation                    parameter              default -> max      at default -> at max (this dispatch), and what the max adds to an app frame");
    for (_, row) in &rows {
        println!("{row}");
    }
}
