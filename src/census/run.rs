//! Run the census on real flames and decode the counter tail.
//!
//! Two modes: one flame with exact numbers to stdout, and the corpus
//! sweep that writes the committed report. See the design doc.

use super::{component_class_name, SEL_XFORMS, STRIDE, VOUT_BASE, VPP_BASE, XIN_BASE};
use crate::config::fractal_config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use std::collections::BTreeMap;
use std::path::Path;

/// Census render size. Small on purpose: the histogram exists only so
/// the real pipeline runs unmodified — nobody looks at the pixels.
const W: u32 = 800;
const H: u32 = 600;
const NUM_WORKGROUPS: u32 = 128;
const THREADS_PER_WORKGROUP: u64 = 64;
const IPT: u32 = 256;

/// Meta-slot in the class stride (never written by the shader): the
/// aggregator records "this variation was exercised at all" here, so
/// `rank` can tell "exercised, nothing interesting" from "absent from
/// the corpus" — a probe divergence at an ORDINARY input is reachable
/// for any exercised variation, and unknown for an absent one.
const EXERCISED_CLASS: usize = 95;

/// Raw decoded tail: everything the aggregator needs, no formatting.
pub struct RawCensus {
    pub per_xform_calls: Vec<u64>,
    /// [xform][class] normal-phase input counts.
    pub xin: Vec<Vec<u64>>,
    /// [local variation idx][class] output counts.
    pub vout: Vec<Vec<u64>>,
    /// [local variation idx][class] pre/post chained-input counts.
    pub vpp: Vec<Vec<u64>>,
    /// Local idx -> variation name.
    pub names: Vec<Option<String>>,
}

/// One interesting (who, kind, class) observation with exact numbers —
/// the single-flame view.
#[derive(Debug, Clone)]
pub struct Observation {
    pub kind: &'static str,
    pub who: String,
    pub class: String,
    pub count: u64,
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
            "({},{})",
            component_class_name((idx / 9) as u32),
            component_class_name((idx % 9) as u32)
        )
    } else if idx < 90 {
        format!("z:{}", component_class_name((idx - 81) as u32))
    } else if idx == EXERCISED_CLASS {
        "(exercised)".to_string()
    } else {
        format!("spare{idx}")
    }
}

// ---------------------------------------------------------------- device

async fn create_device() -> Result<(wgpu::Device, wgpu::Queue, String), String> {
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
    let info = adapter.get_info();
    let adapter_line = format!("{} / {:?}", info.name, info.backend);
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
    Ok((device, queue, adapter_line))
}

// ------------------------------------------------------------ one flame

fn census_config(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    config: &FractalConfig,
    iterations: u64,
) -> Result<RawCensus, String> {
    if config.solid_strength > 0.0 {
        return Err("solid rendering — excluded from the census (v1)".into());
    }

    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        W,
        H,
        &config.flame,
        config.palette_size,
    );
    renderer.enable_census(device);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Census Config"),
    });
    renderer.load_config(device, &mut encoder, queue, config, &config.palette, IPT, 20);
    // The tail is excluded from every ordinary clear; zero it explicitly
    // rather than trusting recycled allocations (see clear_census_tail).
    renderer.clear_census_tail(&mut encoder);
    queue.submit(std::iter::once(encoder.finish()));

    let per_pass = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * IPT as u64;
    let passes = iterations.div_ceil(per_pass).max(1);
    for i in 0..passes {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Census Pass"),
        });
        renderer.compute_pass(
            &mut enc,
            queue,
            device,
            NUM_WORKGROUPS,
            IPT,
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
        .read_census_blocking(device, queue)
        .ok_or("census readback failed")?;

    let id_map = config.flame.get_id_mapping();
    let mut names: Vec<Option<String>> = vec![None; super::MAX_VARS];
    for (name, idx) in &id_map {
        if (*idx as usize) < names.len() {
            names[*idx as usize] = Some(name.clone());
        }
    }

    let table = |base: usize, rows: usize| -> Vec<Vec<u64>> {
        (0..rows)
            .map(|r| {
                (0..STRIDE)
                    .map(|c| words[base + r * STRIDE + c] as u64)
                    .collect()
            })
            .collect()
    };
    Ok(RawCensus {
        per_xform_calls: (0..SEL_XFORMS).map(|x| words[x] as u64).collect(),
        xin: table(XIN_BASE, SEL_XFORMS),
        vout: table(VOUT_BASE, super::MAX_VARS),
        vpp: table(VPP_BASE, super::MAX_VARS),
        names,
    })
}

// ------------------------------------------------------- single-flame CLI

pub fn run_single(path: &Path, iterations: u64) -> Result<Report, String> {
    let config = FractalConfig::load_from_file(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    pollster::block_on(async {
        let (device, queue, _) = create_device().await?;
        let raw = census_config(&device, &queue, &config, iterations)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(report_from_raw(&config, raw))
    })
}

fn report_from_raw(config: &FractalConfig, raw: RawCensus) -> Report {
    let flame = &config.flame;
    let total_calls: u64 = raw.per_xform_calls.iter().sum();
    let mut observations = Vec::new();

    for x in 0..SEL_XFORMS {
        let calls = raw.per_xform_calls[x];
        if calls == 0 {
            continue;
        }
        for c in 0..STRIDE {
            let n = raw.xin[x][c];
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
    for (v, name) in raw.names.iter().enumerate() {
        let Some(name) = name else { continue };
        let calls: u64 = flame
            .transforms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.variations.get(name).map_or(false, |w| *w != 0.0))
            .map(|(i, _)| raw.per_xform_calls.get(i).copied().unwrap_or(0))
            .sum();
        let denom = calls.max(1);
        for (tbl, kind) in [(&raw.vout, "out"), (&raw.vpp, "pp")] {
            for c in 0..STRIDE {
                let n = tbl[v][c];
                if n > 0 {
                    observations.push(Observation {
                        kind,
                        who: name.clone(),
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
        per_xform_calls: raw.per_xform_calls,
        observations,
    }
}

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
                println!(
                    "{:<4} {:<22} {:<22} {:>14} {:>10}",
                    "io", "who", "class", "count", "fraction"
                );
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

// ------------------------------------------------------------ the corpus

struct Agg {
    max_fraction: f64,
    total: u64,
    worst: String,
}

/// Fractions are stochastic; the committed report buckets them so a
/// regeneration only diffs when reachability actually changes, not
/// because the chaos game rolled differently. Boundaries an order of
/// magnitude apart make boundary-straddling churn rare.
fn bucket(f: f64) -> &'static str {
    if f >= 0.1 {
        "dominant"
    } else if f >= 1e-3 {
        "common"
    } else if f >= 1e-6 {
        "rare"
    } else {
        "trace"
    }
}

/// Aggregate one flame's raw census into the corpus tables.
///
/// The per-xform input table is attributed to the xform's normal-phase
/// variations here, where the flame is known. Pre/post variations have
/// their own chained-input table; `Any` variations with fx_priority
/// overrides are attributed as normal-phase (the rare override to
/// pre/post makes this attribution conservative, not wrong — the input
/// they saw is still real).
fn aggregate(
    config: &FractalConfig,
    raw: &RawCensus,
    label: &str,
    agg: &mut BTreeMap<(String, &'static str, usize), Agg>,
) {
    let registry = crate::variations::global_registry();
    let flame = &config.flame;

    let mut note = |who: &str, kind: &'static str, class: usize, n: u64, denom: u64| {
        if n == 0 {
            return;
        }
        let f = n as f64 / denom.max(1) as f64;
        let e = agg
            .entry((who.to_string(), kind, class))
            .or_insert(Agg { max_fraction: 0.0, total: 0, worst: label.to_string() });
        e.total += n;
        // Deterministic tie-break: exact-tie fractions are common (a
        // variation alone on a one-transform flame is exactly 1.0 in
        // dozens of random flames), and first-encountered ties made the
        // `worst` column flip between runs. Smaller label wins.
        if f > e.max_fraction || (f == e.max_fraction && label < e.worst.as_str()) {
            e.max_fraction = f;
            e.worst = label.to_string();
        }
    };

    // Inputs: attribute each xform's class counts to its normal-phase
    // variations.
    for (x, t) in flame.transforms.iter().enumerate() {
        let calls = raw.per_xform_calls.get(x).copied().unwrap_or(0);
        if calls == 0 {
            continue;
        }
        for (name, w) in &t.variations {
            if *w == 0.0 {
                continue;
            }
            let normalish = registry.get(name).map_or(true, |info| {
                use crate::variations::VariationPhase as P;
                matches!(info.phase, P::Normal | P::Any)
            });
            if !normalish {
                continue;
            }
            for c in 0..STRIDE {
                note(name, "in", c, raw.xin[x][c], calls);
            }
        }
    }

    // Outputs and pre/post inputs are already per variation.
    for (v, name) in raw.names.iter().enumerate() {
        let Some(name) = name else { continue };
        let calls: u64 = flame
            .transforms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.variations.get(name).map_or(false, |w| *w != 0.0))
            .map(|(i, _)| raw.per_xform_calls.get(i).copied().unwrap_or(0))
            .sum();
        for c in 0..STRIDE {
            note(name, "out", c, raw.vout[v][c], calls);
            note(name, "pp", c, raw.vpp[v][c], calls);
        }
        // The exercised marker: fraction = this variation's share of the
        // flame's calls, so `worst` names the flame where it is most
        // central — the best reproducer for anything found later.
        let flame_total: u64 = raw.per_xform_calls.iter().sum();
        note(name, "use", EXERCISED_CLASS, calls, flame_total);
    }
}

/// The corpus: shipped presets, the visual-regression configs, and
/// seeded random flames — the generator reaches parameter space the
/// curated sets never touch, and the seed in the label makes any row
/// reproducible.
///
/// Every config gets `deterministic_rng` forced on: without it the
/// chaos game reseeds per run, heavy-tailed classes (how often a walk
/// escapes past 1e16) swing across bucket boundaries, and a
/// regeneration diffed ~150 of ~1160 rows with nothing changed.
/// Deterministic trajectories make the report byte-stable per machine —
/// atomic-add sums are order-independent, so scheduling can't perturb
/// counts.
fn corpus(seeds: u32) -> Vec<(String, FractalConfig)> {
    let mut out = Vec::new();

    for preset in crate::resources::load_presets_with_fallback() {
        let label = format!("preset:{}", preset.flame.name);
        out.push((label, preset));
    }

    let mut visual: Vec<_> = walkdir("tests/visual/configs");
    visual.sort();
    for p in visual {
        if let Ok(c) = FractalConfig::load_from_file(Path::new(&p)) {
            out.push((p, c));
        }
    }

    for seed in 1..=seeds {
        use rand::SeedableRng;
        let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(seed as u64);
        let settings = crate::scene::randomize::RandomGeneratorSettings::default();
        let flame =
            crate::scene::randomize::generate_random_flame_with_rng(&settings, &mut rng);
        let mut config = FractalConfig::default();
        config.flame = flame;
        out.push((format!("random:{seed}"), config));
    }

    out
}

fn walkdir(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("fflame") {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    out
}

pub fn run_corpus_cli(iterations: u64, seeds: u32, out_path: &Path) -> i32 {
    match run_corpus(iterations, seeds, out_path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("census: {e}");
            1
        }
    }
}

fn run_corpus(iterations: u64, seeds: u32, out_path: &Path) -> Result<(), String> {
    pollster::block_on(async {
        // Shared device. A device-per-flame variant was tried while
        // chasing run-to-run count drift and did NOT reduce it — see
        // the stability notes in the design doc.
        let (device, queue, adapter_line) = create_device().await?;
        let mut flames = corpus(seeds);
        for (_, config) in &mut flames {
            config.deterministic_rng = true;
        }
        let n = flames.len();
        let mut agg: BTreeMap<(String, &'static str, usize), Agg> = BTreeMap::new();
        let mut run = 0usize;
        let mut skipped: Vec<String> = Vec::new();
        let started = web_time::Instant::now();

        for (i, (label, config)) in flames.iter().enumerate() {
            match census_config(&device, &queue, config, iterations) {
                Ok(raw) => {
                    aggregate(config, &raw, label, &mut agg);
                    run += 1;
                }
                Err(e) => skipped.push(format!("{label} ({e})")),
            }
            if (i + 1) % 25 == 0 || i + 1 == n {
                eprintln!("  {}/{} flames ({} skipped)", i + 1, n, skipped.len());
            }
        }

        let version = crate::version::get_version_info();
        let mut w = String::new();
        w.push_str("# Fractal Art Editor — variation reachability census\n");
        w.push_str("# schema 1\n#\n");
        w.push_str("# This file is generated. See src/census/ for what it means.\n");
        w.push_str(&format!(
            "# Regenerate: cargo run --release --bin variation_probe -- census --corpus\n#\n"
        ));
        w.push_str(&format!("# build    {} ({})\n", version.version, version.git_hash));
        w.push_str(&format!("# os       {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
        w.push_str(&format!("# adapter  {adapter_line}\n"));
        w.push_str(&format!(
            "# corpus   presets + tests/visual/configs + random seed 1..{seeds}  ({run} run, {} skipped)\n",
            skipped.len()
        ));
        w.push_str(&format!("# iterations per flame: {iterations}\n"));
        for s in &skipped {
            w.push_str(&format!("# skipped: {s}\n"));
        }
        w.push_str("#\n");
        w.push_str(
            "# One line per (variation, io, class) observed anywhere in the\n\
             # corpus. `fraction` is bucketed (dominant >= 0.1 > common >= 1e-3\n\
             # > rare >= 1e-6 > trace) so a regeneration only diffs when\n\
             # reachability changes, not when the chaos game rolls differently.\n\
             # `worst` names the flame with the highest exact fraction — the\n\
             # reproducer. io: in = normal-phase input (attributed), out =\n\
             # variation contribution, pp = pre/post chained input.\n#\n",
        );
        for ((who, kind, class), a) in &agg {
            w.push_str(&format!(
                "{:<24} {:<4} {:<24} {:<9} {}\n",
                who,
                kind,
                pair_name(*class),
                bucket(a.max_fraction),
                a.worst,
            ));
        }
        std::fs::write(out_path, &w).map_err(|e| format!("write {}: {e}", out_path.display()))?;
        eprintln!(
            "census: {} rows -> {} ({} flames, {:.0}s)",
            agg.len(),
            out_path.display(),
            run,
            started.elapsed().as_secs_f64()
        );
        Ok(())
    })
}
