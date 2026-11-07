//! Ordinary flames' dispatch time, for timing a renderer change before
//! and after: the classic flame presets and the visual suite's configs
//! that stress the variation parameter block (the most transforms and
//! variations, derived slots, a subflame, 3D). Each flame on a renderer
//! of its own at each of eight placements (`FLAME_TIMING_SHIFTS`), loaded
//! once, a probe dispatch first (one past 250 ms is skipped, far from the
//! driver's watchdog), then nine rounds of sixteen dispatches after four
//! that warm up; the median over the placements of each one's median
//! round, the range, and the parameter buffer's size.
//!
//! ```text
//! FLAME_TIMING_LABEL=before cargo test --release --lib -- --ignored --nocapture time_ordinary_flames
//! ```
//!
//! Written to `output/flame-timing/<label>.txt`; `FLAME_TIMING_ONLY=a,b`
//! times the flames whose names contain one of them. A flame's speed
//! depends on where its buffers land -- which every buffer made before
//! them moves, a size changed anywhere (the parameter buffer's moved one
//! flame 24% and another -26%, the same shader's work) -- so each is
//! timed at several placements, a buffer made first shifting the
//! renderer's, and builds compare by the median over them
//! (docs/projects/variation-param-storage.md).
#![cfg(test)]

use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;

const N: u32 = 1024;
const GROUPS: u32 = 256;
const IPT: u32 = 256;

/// The flames: the eight classic presets, then the visual configs.
fn flames() -> Vec<(String, FractalConfig)> {
    let presets = FractalConfig::from_json_multi(&std::fs::read_to_string("assets/presets.fflame").expect("the presets")).expect("they parse");
    let mut out: Vec<(String, FractalConfig)> = presets.into_iter().take(8).map(|c| (c.flame.name.clone(), c)).collect();
    for path in [
        "tests/visual/configs/variations/bipolar-series-smoke.fflame",
        "tests/visual/configs/variations/classic-blades-misc-smoke.fflame",
        "tests/visual/configs/variations/misc-variations2.fflame",
        "tests/visual/configs/variations/init-migration-smoke.fflame",
        "tests/visual/configs/2d/subflame-julian.fflame",
        "tests/visual/configs/3d/swirl3D_wf-smoke.fflame",
    ] {
        let cfg = FractalConfig::from_json(&std::fs::read_to_string(path).expect("the config")).expect("it parses");
        out.push((path.rsplit('/').next().unwrap_or(path).trim_end_matches(".fflame").to_string(), cfg));
    }
    out
}

#[test]
#[ignore = "times the GPU"]
fn time_ordinary_flames() {
    let (device, queue) = pollster::block_on(crate::probe::run::open_device("flame timing")).expect("a GPU");
    let label = std::env::var("FLAME_TIMING_LABEL").unwrap_or_else(|_| "run".into());
    let wait = || {
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    };
    let dispatch = |r: &mut FlameRenderer, cfg: &FractalConfig| {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("timed") });
        r.compute_pass(
            &mut enc, &queue, &device, GROUPS, IPT, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, cfg.rotation, cfg.camera_rotation_x,
            cfg.camera_rotation_y, cfg.camera_bank, cfg.camera_x, cfg.camera_y, cfg.camera_z, cfg.speed_factor, true, false,
        );
        queue.submit(Some(enc.finish()));
    };
    let only: Option<Vec<String>> = std::env::var("FLAME_TIMING_ONLY").ok().map(|s| s.split(',').map(str::to_string).collect());
    let shifts: u64 = std::env::var("FLAME_TIMING_SHIFTS").ok().and_then(|v| v.parse().ok()).unwrap_or(8).max(1);
    let mut lines = Vec::new();
    'flames: for (name, mut cfg) in flames().into_iter().filter(|(n, _)| only.as_ref().is_none_or(|o| o.iter().any(|x| n.contains(x.as_str())))) {
        cfg.deterministic_rng = false;
        // The flame at each placement: a buffer of `s` x 192 KiB made
        // first moves every buffer of the renderer after it.
        let mut medians = Vec::new();
        let mut bytes = 0;
        for s in 0..shifts {
            let pad = (s > 0).then(|| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("placement shift"),
                    size: s * 192 * 1024,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            });
            let mut r = FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, N, N, &cfg.flame, cfg.palette_size);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
            r.load_config(&device, &mut enc, &queue, &cfg, &cfg.palette, IPT, 0);
            queue.submit(Some(enc.finish()));
            // The first dispatch builds the pipeline; the second is the probe.
            dispatch(&mut r, &cfg);
            wait();
            let start = web_time::Instant::now();
            dispatch(&mut r, &cfg);
            wait();
            let probe = start.elapsed().as_secs_f64() * 1e3;
            if probe > 250.0 {
                lines.push(format!("{name:40} skipped: one dispatch took {probe:.0} ms"));
                println!("{}", lines.last().unwrap());
                r.destroy();
                continue 'flames;
            }
            let mut rounds = Vec::new();
            for _ in 0..9 {
                for _ in 0..4 {
                    dispatch(&mut r, &cfg);
                }
                wait();
                let start = web_time::Instant::now();
                for _ in 0..16 {
                    dispatch(&mut r, &cfg);
                }
                wait();
                rounds.push(start.elapsed().as_secs_f64() * 1e3);
            }
            rounds.sort_by(|a, b| a.total_cmp(b));
            medians.push(rounds[rounds.len() / 2]);
            bytes = r.variation_params_bytes();
            r.destroy();
            if let Some(pad) = pad {
                pad.destroy();
            }
        }
        let mut sorted = medians.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        lines.push(format!(
            "{name:40} median {:8.2} ms  ({:.2} .. {:.2})  params buffer {bytes} bytes  by placement {}",
            sorted[sorted.len() / 2],
            sorted[0],
            sorted[sorted.len() - 1],
            medians.iter().map(|m| format!("{m:.1}")).collect::<Vec<_>>().join(" ")
        ));
        println!("{}", lines.last().unwrap());
    }
    let dir = std::path::Path::new("output/flame-timing");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{label}.txt")), lines.join("\n") + "\n").unwrap();
}
