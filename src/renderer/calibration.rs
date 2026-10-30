//! **The governor's calibration** (docs/projects/quality-governor.md,
//! phase 1): each scene rendered at each rung of its ladder, timed, and
//! written for `scripts/frame_metrics.py loss` to compare against the
//! scene's reference.
//!
//! ```text
//! cargo test --release --lib -- --ignored governor_calibration --nocapture
//! ```
//! `CALIBRATE_OUT` (default `output/governor`), `CALIBRATE_SIZE` (`WxH`,
//! default 1920x1080), `CALIBRATE_ONLY` (scene names, comma separated),
//! `CALIBRATE_REPEATS` (default 3). Run it with the machine otherwise
//! idle: the times are wall clock.
//!
//! A time is the export path's own (`RenderOutput::render_time_ms`): a
//! whole settled picture, the median of the repeats after a warm-up
//! render that compiles the shaders. Render scale does not exist yet, so
//! a rung below full size is rendered smaller and the script upscales it.
//! A simulation has no reference -- its field is content, so a smaller
//! grid is another run -- and is timed per step instead, at each field
//! scale of a bound grid.

use crate::config::FractalConfig;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Supersampling, then render scale.
    Escape,
    /// Mode D's solid, lit (the picture shown while anything moves).
    EscapeSolid,
    /// Samples a frame, at each render scale.
    Flame,
    /// A step's cost at each field scale.
    Sim,
}

struct Scene {
    name: &'static str,
    path: &'static str,
    kind: Kind,
}

const SCENES: &[Scene] = &[
    Scene { name: "escape-seahorse", path: "tests/visual/configs/escape/mandelbrot-seahorse-zoom.fflame", kind: Kind::Escape },
    Scene { name: "escape-deep", path: "tests/visual/configs/escape/deep-zoom-30.fflame", kind: Kind::Escape },
    Scene { name: "escape-newton", path: "tests/visual/configs/escape/newton-3.fflame", kind: Kind::Escape },
    Scene { name: "escape-solid", path: "tests/visual/configs/escape/ifs-solid-tetrahedron-sky.fflame", kind: Kind::EscapeSolid },
    Scene { name: "flame-2d", path: "tests/visual/configs/2d/plastic-sierpinski.fflame", kind: Kind::Flame },
    Scene { name: "flame-3d", path: "tests/visual/configs/3d/bubble-3d.fflame", kind: Kind::Flame },
    Scene { name: "sim-gray-scott", path: "tests/visual/configs/sim/gray-scott-bilinear-upscale.fflame", kind: Kind::Sim },
    Scene { name: "sim-mccabe", path: "tests/visual/configs/sim/mccabe-coarse.fflame", kind: Kind::Sim },
    Scene { name: "sim-cyclic-ca", path: "tests/visual/configs/sim/cyclic-ca-1-1-14.fflame", kind: Kind::Sim },
    Scene { name: "sim-sandpile", path: "tests/visual/configs/sim/sandpile-moore.fflame", kind: Kind::Sim },
    Scene { name: "sim-physarum", path: "tests/visual/configs/sim/physarum-coarse.fflame", kind: Kind::Sim },
    Scene { name: "sim-lenia", path: "tests/visual/configs/sim/lenia-soup.fflame", kind: Kind::Sim },
    Scene { name: "sim-smoothlife", path: "tests/visual/configs/sim/smoothlife-glider.fflame", kind: Kind::Sim },
    Scene { name: "sim-cahn-hilliard", path: "tests/visual/configs/sim/cahn-hilliard-age.fflame", kind: Kind::Sim },
    Scene { name: "sim-dla", path: "tests/visual/configs/sim/dla-age.fflame", kind: Kind::Sim },
];

/// Escape's reference supersampling, and its rungs: (supersampling,
/// render scale).
const ESCAPE_REFERENCE_SS: u32 = 6;
const ESCAPE_RUNGS: &[(u32, f32)] = &[(4, 1.0), (3, 1.0), (2, 1.0), (1, 1.0), (2, 0.75), (1, 0.75), (2, 0.5), (1, 0.5)];

/// A flame's reference, in samples a pixel at full size; its budgets, in
/// samples a FULL-size pixel, each rendered at every render scale --
/// the same samples landing in fewer, larger pixels.
const FLAME_REFERENCE_SPP: f64 = 1024.0;
const FLAME_BUDGETS: &[f64] = &[0.25, 1.0, 4.0, 16.0, 64.0];
const SCALES: &[f32] = &[1.0, 0.75, 0.5];
/// The threads of the render loop's dispatch (`render::NUM_WORKGROUPS`
/// workgroups of 64).
const FLAME_THREADS: u64 = 128 * 64;

/// A bound simulation grid's field scales, and the two step counts whose
/// difference is timed.
const SIM_SCALES: &[f32] = &[1.0, 0.75, 0.5, 0.375, 0.25, 0.1875, 0.125];
const SIM_STEPS: (u32, u32) = (20, 220);

fn env_size() -> (u32, u32) {
    std::env::var("CALIBRATE_SIZE")
        .ok()
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or((1920, 1080))
}

fn scaled(size: (u32, u32), r: f32) -> (u32, u32) {
    (((size.0 as f32 * r).round() as u32).max(1), ((size.1 as f32 * r).round() as u32).max(1))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    out: std::path::PathBuf,
    repeats: usize,
    csv: std::fs::File,
}

impl Harness {
    /// Render `config` at `size` (with `iterations` for a flame), `repeats`
    /// times; the median time and the samples plotted, and the last
    /// picture written to `file`.
    fn time(&self, config: &FractalConfig, size: (u32, u32), iterations: Option<u64>, file: Option<&str>) -> (f64, u64) {
        let mut times = Vec::new();
        let mut last = None;
        for _ in 0..self.repeats {
            let mut job = crate::renderer::RenderJob::new(config, size.0, size.1);
            if let Some(n) = iterations {
                // A dispatch is FLAME_THREADS threads of `ipt` iterations
                // and the loop stops at the first dispatch past the target,
                // so a budget below one default dispatch (2.1 million)
                // would render a whole one: shorten the dispatches to fit.
                let dispatches = n.div_ceil(FLAME_THREADS * 256).max(1);
                let ipt = n.div_ceil(FLAME_THREADS * dispatches).clamp(1, 256) as u32;
                job = job.with_iterations(n).with_iterations_per_thread(ipt);
            }
            let out = pollster::block_on(crate::renderer::render(&self.device, &self.queue, job, &mut crate::renderer::NoProgress))
                .expect("render");
            times.push(out.render_time_ms);
            last = Some(out);
        }
        let out = last.expect("a render");
        if let Some(file) = file {
            let path = self.out.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
            image::save_buffer(&path, &out.rgba_data, out.width, out.height, image::ColorType::Rgba8).expect("write");
        }
        (median(times), out.total_iterations)
    }

    fn row(&mut self, fields: &[String]) {
        use std::io::Write;
        writeln!(self.csv, "{}", fields.join(",")).expect("csv");
        println!("{}", fields.join("  "));
    }
}

#[test]
#[ignore = "measurement; needs a GPU and an idle machine"]
fn governor_calibration() {
    let (device, queue) = pollster::block_on(crate::app::export::headless_device()).expect("gpu");
    let size = env_size();
    let out = std::path::PathBuf::from(std::env::var("CALIBRATE_OUT").unwrap_or_else(|_| "output/governor".into()));
    std::fs::create_dir_all(&out).expect("mkdir");
    let only: Vec<String> = std::env::var("CALIBRATE_ONLY")
        .map(|s| s.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect())
        .unwrap_or_default();
    let repeats = std::env::var("CALIBRATE_REPEATS").ok().and_then(|s| s.parse().ok()).unwrap_or(3usize).max(1);
    let csv = std::fs::File::create(out.join("calibration.csv")).expect("csv");
    let mut h = Harness { device, queue, out, repeats, csv };
    h.row(&["scene,kind,rung,ss,scale,width,height,samples,ms,file".into()]);

    for scene in SCENES.iter().filter(|s| only.is_empty() || only.iter().any(|n| n == s.name)) {
        let mut config = FractalConfig::from_json(&std::fs::read_to_string(scene.path).expect(scene.path)).expect("parse");
        let file = |rung: &str| format!("{}/{rung}.png", scene.name);
        match scene.kind {
            Kind::Escape | Kind::EscapeSolid => {
                if scene.kind == Kind::EscapeSolid {
                    config.escape.solid_tier = crate::config::escape::RenderTier::Lit;
                }
                let at = |h: &mut Harness, rung: &str, ss: u32, r: f32| {
                    let mut c = config.clone();
                    c.escape.supersample = ss;
                    let s = scaled(size, r);
                    let (ms, _) = h.time(&c, s, None, Some(&file(rung)));
                    h.row(&[
                        scene.name.into(), "escape".into(), rung.into(), ss.to_string(), r.to_string(),
                        s.0.to_string(), s.1.to_string(), String::new(), format!("{ms:.2}"), file(rung),
                    ]);
                };
                // The warm-up, then the reference and the rungs.
                let mut warm = config.clone();
                warm.escape.supersample = 1;
                h.time(&warm, scaled(size, 0.5), None, None);
                at(&mut h, "ref", ESCAPE_REFERENCE_SS, 1.0);
                for &(ss, r) in ESCAPE_RUNGS {
                    at(&mut h, &format!("ss{ss}-r{}", (r * 100.0).round()), ss, r);
                }
            }
            Kind::Flame => {
                let full = size.0 as f64 * size.1 as f64;
                let at = |h: &mut Harness, rung: &str, spp: f64, r: f32| {
                    let s = scaled(size, r);
                    let (ms, samples) = h.time(&config, s, Some((spp * full).round() as u64), Some(&file(rung)));
                    h.row(&[
                        scene.name.into(), "flame".into(), rung.into(), String::new(), r.to_string(),
                        s.0.to_string(), s.1.to_string(), samples.to_string(), format!("{ms:.2}"), file(rung),
                    ]);
                };
                h.time(&config, scaled(size, 0.5), Some(full as u64), None);
                at(&mut h, "ref", FLAME_REFERENCE_SPP, 1.0);
                for &spp in FLAME_BUDGETS {
                    for &r in SCALES {
                        at(&mut h, &format!("spp{spp}-r{}", (r * 100.0).round()), spp, r);
                    }
                }
            }
            Kind::Sim => {
                // The steps' cost alone: the difference between two step
                // counts from the same seed, so the seed, the colouring and
                // the readback cancel.
                let at = |h: &mut Harness, scale: f32| {
                    let mut c = config.clone();
                    c.sim.grid = crate::config::sim::SimGrid::Viewport { scale };
                    let (gw, gh) = c.sim.grid.cells_for(size.0, size.1);
                    c.sim.steps = SIM_STEPS.0;
                    let (t0, _) = h.time(&c, size, None, None);
                    c.sim.steps = SIM_STEPS.1;
                    let rung = format!("scale{}", (scale * 1000.0).round() / 10.0);
                    let (t1, _) = h.time(&c, size, None, Some(&file(&rung)));
                    let per_step = (t1 - t0) / (SIM_STEPS.1 - SIM_STEPS.0) as f64;
                    h.row(&[
                        scene.name.into(), "sim".into(), rung.clone(), String::new(), scale.to_string(),
                        gw.to_string(), gh.to_string(), (SIM_STEPS.1 - SIM_STEPS.0).to_string(),
                        format!("{per_step:.4}"), file(&rung),
                    ]);
                };
                let mut warm = config.clone();
                warm.sim.grid = crate::config::sim::SimGrid::Viewport { scale: 0.125 };
                warm.sim.steps = SIM_STEPS.0;
                h.time(&warm, size, None, None);
                for &scale in SIM_SCALES {
                    at(&mut h, scale);
                }
            }
        }
    }
}
