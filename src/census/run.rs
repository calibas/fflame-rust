//! Run the census on real flames and decode the counter tail.
//!
//! Phase 1: one flame at a time, table to stdout. The corpus runner and
//! committed report land in phase 2 (see the design doc).

use super::{class, component_class_name, SEL_XFORMS, STRIDE, VOUT_BASE, VPP_BASE, XIN_BASE};
use crate::config::fractal_config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use std::path::Path;

/// Census render size. Small on purpose: the histogram exists only so
/// the real pipeline runs unmodified — nobody looks at the pixels.
const W: u32 = 800;
const H: u32 = 600;
const NUM_WORKGROUPS: u32 = 128;
const THREADS_PER_WORKGROUP: u64 = 64;

/// One interesting (table, class) observation, decoded.
#[derive(Debug, Clone)]
pub struct Observation {
    /// "in" (normal-phase input, attributed from the xform table),
    /// "out" (variation contribution), "pp" (pre/post chained input).
    pub kind: &'static str,
    /// Variation name (or the xform label for raw in-table rows).
    pub who: String,
    /// Human-readable class, e.g. "(+0, +0)" or "z:nan".
    pub class: String,
    pub count: u64,
    /// Fraction of that owner's calls.
    pub fraction: f64,
}

pub struct Report {
    pub flame_name: String,
    pub total_calls: u64,
    pub per_xform_calls: Vec<u64>,
    pub observations: Vec<Observation>,
}

fn pair_name(idx: usize) -> String {
    if idx < 81 {
        format!(
            "({}, {})",
            component_class_name((idx / 9) as u32),
            component_class_name((idx % 9) as u32)
        )
    } else if idx < 90 {
        format!("z:{}", component_class_name((idx - 81) as u32))
    } else {
        format!("spare{idx}")
    }
}

/// Run one flame for `iterations` chaos-game iterations and decode the
/// tail. Returns Err for flames the v1 instrument excludes.
pub fn run_single(path: &Path, iterations: u64) -> Result<Report, String> {
    let config = FractalConfig::load_from_file(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if config.solid_strength > 0.0 {
        return Err(format!(
            "{}: solid rendering — excluded from the census (v1)",
            path.display()
        ));
    }

    pollster::block_on(run_config(&config, iterations))
}

async fn run_config(config: &FractalConfig, iterations: u64) -> Result<Report, String> {
    // Headless device, same shape as the probe's.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| format!("no GPU adapter: {e:?}"))?;
    let adapter_limits = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    limits.max_buffer_size = adapter_limits.max_buffer_size;
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("variation census"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await
        .map_err(|e| format!("device creation failed: {e:?}"))?;

    let mut renderer = FlameRenderer::with_palette_size(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8Unorm,
        W,
        H,
        &config.flame,
        config.palette_size,
    );
    renderer.enable_census(&device);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Census Config"),
    });
    let ipt = 256u32;
    renderer.load_config(&device, &mut encoder, &queue, config, &config.palette, ipt, 20);
    queue.submit(std::iter::once(encoder.finish()));

    let per_pass = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * ipt as u64;
    let passes = iterations.div_ceil(per_pass).max(1);
    for i in 0..passes {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Census Pass"),
        });
        renderer.compute_pass(
            &mut enc,
            &queue,
            &device,
            NUM_WORKGROUPS,
            ipt,
            20,
            config.zoom,
            config.pan_x,
            config.pan_y,
            config.rotation,
            config.camera_rotation_x,
            config.camera_rotation_y,
            config.camera_bank,
            config.camera_x,
            config.camera_y,
            config.camera_z,
            config.speed_factor,
            i == 0, // clear once; the tail survives clears anyway
            i == 0,
        );
        queue.submit(std::iter::once(enc.finish()));
    }

    let words = renderer
        .read_census_blocking(&device, &queue)
        .ok_or("census readback failed")?;

    Ok(decode(config, &words))
}

fn decode(config: &FractalConfig, words: &[u32]) -> Report {
    let flame = &config.flame;
    // Local index -> variation name, from the same mapping the shader
    // builder used.
    let id_map = flame.get_id_mapping();
    let mut names: Vec<Option<&String>> = vec![None; super::MAX_VARS];
    for (name, idx) in &id_map {
        if (*idx as usize) < names.len() {
            names[*idx as usize] = Some(name);
        }
    }

    let per_xform_calls: Vec<u64> = (0..SEL_XFORMS)
        .map(|x| words[x] as u64)
        .collect();
    let total_calls: u64 = per_xform_calls.iter().sum();

    let mut observations = Vec::new();

    // Normal-phase inputs: per-xform table, attributed to the transform
    // (variation attribution happens against the flame's per-xform
    // active sets when reports aggregate — phase 2; the raw row is
    // already actionable).
    for x in 0..SEL_XFORMS {
        let calls = per_xform_calls[x];
        if calls == 0 {
            continue;
        }
        for c in 0..STRIDE {
            let n = words[XIN_BASE + x * STRIDE + c] as u64;
            if n > 0 {
                observations.push(Observation {
                    kind: "in",
                    who: format!("xform{x}"),
                    class: pair_name(c),
                    count: n,
                    fraction: n as f64 / calls as f64,
                });
            }
        }
    }

    // Per-variation tables. The denominator is the summed calls of the
    // xforms that carry the variation with nonzero weight.
    for (v, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        let calls: u64 = flame
            .transforms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.variations.get(*name).map_or(false, |w| *w != 0.0))
            .map(|(i, _)| per_xform_calls.get(i).copied().unwrap_or(0))
            .sum();
        let denom = calls.max(1);
        for (base, kind) in [(VOUT_BASE, "out"), (VPP_BASE, "pp")] {
            for c in 0..STRIDE {
                let n = words[base + v * STRIDE + c] as u64;
                if n > 0 {
                    observations.push(Observation {
                        kind,
                        who: (*name).clone(),
                        class: pair_name(c),
                        count: n,
                        fraction: n as f64 / denom as f64,
                    });
                }
            }
        }
    }

    observations.sort_by(|a, b| b.fraction.total_cmp(&a.fraction));
    Report {
        flame_name: flame.name.clone(),
        total_calls,
        per_xform_calls,
        observations,
    }
}

/// CLI entry: run and print.
pub fn run_single_cli(path: &Path, iterations: u64) -> i32 {
    match run_single(path, iterations) {
        Err(e) => {
            eprintln!("census: {e}");
            1
        }
        Ok(r) => {
            println!("# census — {} ({} calls)", r.flame_name, r.total_calls);
            let active: Vec<String> = r
                .per_xform_calls
                .iter()
                .enumerate()
                .filter(|(_, c)| **c > 0)
                .map(|(i, c)| format!("x{i}:{c}"))
                .collect();
            println!("# calls per xform: {}", active.join(" "));
            if r.observations.is_empty() {
                println!("no interesting inputs or outputs observed");
            } else {
                println!("{:<4} {:<22} {:<22} {:>14} {:>10}", "io", "who", "class", "count", "fraction");
                for o in &r.observations {
                    println!(
                        "{:<4} {:<22} {:<22} {:>14} {:>10.6}",
                        o.kind, o.who, o.class, o.count, o.fraction
                    );
                }
            }
            0
        }
    }
}

// The NAN class constant is referenced by rank (phase 3); silence the
// unused warning until then without losing the export.
#[allow(unused)]
const _: u32 = class::NAN;
