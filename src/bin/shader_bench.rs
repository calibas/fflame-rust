//! Phase 0 measurements for sticky shader compilation.
//!
//!     cargo run --release --bin shader_bench
//!
//! Three measurements, written for
//! `docs/projects/sticky-shader-compilation.md` (which carries the
//! results):
//!
//! 1. **Baseline** — 20 random seeds rendered on one persistent
//!    renderer, the gallery's shape. Every seed picks a different
//!    variation subset, so every `load_config` recompiles. Reported:
//!    per-seed wall time, shader rebuild count, total compile ms.
//! 2. **Superset proxy** — the same 20 seeds, but every flame is
//!    augmented so its *compiled* variation set (and its order) is the
//!    full generator pool, with the flame's unused entries at weight 0,
//!    and the transform count pinned. This is Layer B emulated with
//!    zero new machinery: weight-0 variations enter the compiled set,
//!    and the dispatcher's `w != 0.0` gate keeps them dead. Rebuilds
//!    collapse to one; the delta against baseline is the payoff.
//! 3. **Dead-cost curve** — one flame's sustained throughput with N
//!    dead variations compiled in, N ∈ {0, 15, 30, 60, 95}. Dead
//!    variations cost a uniform branch per transform per iteration plus
//!    code size; this curve is what sets the sticky cap. Compile time
//!    per N is reported too (it is the thing being amortized).
//!
//! Caveat recorded with the results: the curve's dead variations are
//! drawn from the registry filtered to small parameter counts (the 1600
//! param-slot ceiling binds long before 95 arbitrary variations), so it
//! measures branch-and-code-size cost, not heavyweight-body cost.

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer;
use fractal_flame_wgpu::renderer::render::{render_with, NoProgress, RenderJob};
use fractal_flame_wgpu::scene::randomize::{
    generate_random_flame_with_rng, RandomGeneratorSettings,
};
use fractal_flame_wgpu::scene::transforms::Flame;
use rand::SeedableRng;

const SEEDS: u64 = 20;
const SIZE: u32 = 512;
const ITERS_PER_SEED: u64 = 10_000_000;
const CURVE_ITERS: u64 = 50_000_000;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let (device, queue) = pollster::block_on(create_device());

    println!("=== Phase 0: sticky shader compilation measurements ===");
    println!("device ready; {SEEDS} seeds at {SIZE}x{SIZE}, {ITERS_PER_SEED} iters each\n");

    run_batch(&device, &queue, "baseline (natural flames)", false, false);
    run_batch(&device, &queue, "baseline (pinned transform count)", true, false);
    run_batch(&device, &queue, "superset proxy (pinned + pool at w=0)", true, true);

    dead_cost_curve(&device, &queue);
}

/// The generator's default pool, in registry (canonical) order — the
/// order Layer B's map will use for sticky extras.
fn pool_in_canonical_order() -> Vec<String> {
    let settings = RandomGeneratorSettings::default();
    let reg = fractal_flame_wgpu::variations::global_registry();
    reg.names()
        .iter()
        .filter(|n| settings.enabled_variations.contains(*n))
        .cloned()
        .collect()
}

/// Emulate the sticky superset on an existing flame: every transform's
/// compiled set becomes `pool` in canonical order, its own weights
/// preserved and everything else at 0. Rewriting `variation_order` too
/// is what makes the local index map — and therefore the shader —
/// identical across flames, which is exactly Layer B's contract.
fn supersetize(flame: &mut Flame, pool: &[String]) {
    for t in &mut flame.transforms {
        for name in pool {
            t.variations.entry(name.clone()).or_insert(0.0);
        }
        t.variation_order = pool.to_vec();
    }
}

fn seed_config(seed: u64, pin_transforms: bool) -> FractalConfig {
    let mut settings = RandomGeneratorSettings::default();
    if pin_transforms {
        settings.transform_count_min = 4;
        settings.transform_count_max = 4;
    }
    let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(seed);
    let flame = generate_random_flame_with_rng(&settings, &mut rng);
    let mut config = FractalConfig::default();
    config.flame = flame;
    config.deterministic_rng = true;
    config
}

fn run_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    pin_transforms: bool,
    superset: bool,
) {
    let pool = pool_in_canonical_order();

    // Fresh renderer per configuration so rebuild counters start clean.
    let first = seed_config(1, pin_transforms);
    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        SIZE,
        SIZE,
        &first.flame,
        first.palette_size,
    );

    let mut per_seed = Vec::new();
    let started = std::time::Instant::now();
    for seed in 1..=SEEDS {
        let mut config = seed_config(seed, pin_transforms);
        if superset {
            supersetize(&mut config.flame, &pool);
        }
        let t = std::time::Instant::now();
        let job = RenderJob::new(&config, SIZE, SIZE).with_iterations(ITERS_PER_SEED);
        let out = pollster::block_on(render_with(&mut renderer, device, queue, job, &mut NoProgress));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        match out {
            Ok(_) => per_seed.push(ms),
            Err(e) => {
                println!("  seed {seed} FAILED: {e}");
                per_seed.push(ms);
            }
        }
    }
    let total = started.elapsed().as_secs_f64() * 1000.0;
    let (rebuilds, compile_ms) = renderer.shader_rebuild_stats();
    renderer.destroy();

    per_seed.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = per_seed[per_seed.len() / 2];
    println!("--- {label} ---");
    println!(
        "  total {total:7.0} ms   median/seed {median:6.1} ms   shader rebuilds {rebuilds:2}   compile {compile_ms:7.1} ms ({:.0}% of total)",
        100.0 * compile_ms / total
    );
    println!();
}

/// One flame, N dead variations compiled in, sustained throughput.
fn dead_cost_curve(device: &wgpu::Device, queue: &wgpu::Queue) {
    println!("--- dead-variation cost curve ({CURVE_ITERS} iters, second render of each) ---");

    // Small-footprint dead candidates: registry order, parameterless or
    // nearly so, so 95 of them stay under the 1600 param-slot ceiling.
    let reg = fractal_flame_wgpu::variations::global_registry();
    let candidates: Vec<String> = reg
        .names()
        .iter()
        .filter(|n| reg.get(n).map(|i| i.slot_count() <= 2 && i.state_count == 0).unwrap_or(false))
        .cloned()
        .collect();
    drop(reg);

    for n_dead in [0usize, 15, 30, 60, 95] {
        let mut config = seed_config(3, true); // 4 transforms, a few live variations
        config.max_iterations = CURVE_ITERS;

        let live: std::collections::HashSet<String> = config
            .flame
            .transforms
            .iter()
            .flat_map(|t| t.variations.keys().cloned())
            .collect();
        let dead: Vec<String> = candidates
            .iter()
            .filter(|c| !live.contains(*c))
            .take(n_dead)
            .cloned()
            .collect();
        for t in &mut config.flame.transforms {
            for name in &dead {
                t.variations.entry(name.clone()).or_insert(0.0);
                t.variation_order.push(name.clone());
            }
        }

        let mut renderer = FlameRenderer::with_palette_size(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            SIZE,
            SIZE,
            &config.flame,
            config.palette_size,
        );

        // First render pays the compile; the second is pure throughput.
        let job = RenderJob::new(&config, SIZE, SIZE).with_iterations(CURVE_ITERS);
        let _ = pollster::block_on(render_with(&mut renderer, device, queue, job, &mut NoProgress));
        let (_, compile_ms) = renderer.shader_rebuild_stats();

        let job = RenderJob::new(&config, SIZE, SIZE).with_iterations(CURVE_ITERS);
        let t = std::time::Instant::now();
        let out = pollster::block_on(render_with(&mut renderer, device, queue, job, &mut NoProgress));
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        renderer.destroy();

        match out {
            Ok(o) => {
                let miters = o.total_iterations as f64 / 1000.0 / ms;
                println!(
                    "  dead {n_dead:3}   {miters:7.1} Miter/s   render {ms:7.0} ms   compile {compile_ms:6.1} ms   ({} live vars in flame)",
                    live.len()
                );
            }
            Err(e) => println!("  dead {n_dead:3}   FAILED: {e}"),
        }
    }
}

async fn create_device() -> (wgpu::Device, wgpu::Queue) {
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
        .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);

    let adapter_limits = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    limits.max_buffer_size = adapter_limits.max_buffer_size;
    limits.max_storage_buffers_per_shader_stage = adapter_limits.max_storage_buffers_per_shader_stage;

    let mut features = wgpu::Features::CLEAR_TEXTURE;
    if adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
        features |= wgpu::Features::FLOAT32_FILTERABLE;
    }
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("shader bench"),
            required_features: features,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await
        .expect("device creation failed")
}
