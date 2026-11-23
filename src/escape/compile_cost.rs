//! **What a solid's shaders cost to compile** (the compile-times branch):
//! a measurement, not a check. For every built-in solid, its Rhai twin and
//! each example Rhai solid, the lit walk and the path tracer as the
//! renderer assembles them, timed in three parts -- naga's parse and
//! validation, naga's SPIR-V back end, and the whole pipeline creation on
//! the desktop's driver (which runs naga again, then the driver's own
//! compiler) -- and written to `output/compile-cost/` with a TSV of the
//! numbers, for `scripts/shader_inline_weight.py` to read.
//!
//! `cargo test --release --lib -- --ignored compile_cost --nocapture`

use crate::config::escape::{CustomDialect, CustomSource};
use crate::escape::assembler::{assemble_ifs_path_user, assemble_ifs_user};
use crate::escape::ifs::{get_ifs, get_ifs_coloring, IfsDef, UserKernels};
use crate::lang::{rhai, EntryKind};

/// The solids measured: (label, def).
fn solids() -> Vec<(String, &'static IfsDef)> {
    let mut out = Vec::new();
    for (name, _) in crate::escape::twins::SOLID_TWINS {
        out.push((format!("{name} (WGSL)"), get_ifs(name).expect("a built-in solid")));
        let src = crate::escape::twins::solid_custom_source(name).expect("a twin");
        let key = crate::escape::custom::name_of(EntryKind::Solid, &src);
        let def = crate::escape::custom::compile_solid(&src, &key).unwrap_or_else(|_| panic!("{name}'s twin"));
        out.push((format!("{name} (Rhai twin)"), def));
    }
    for (file, text) in crate::storage::formula_store::EXAMPLES {
        if !file.starts_with("solid-") {
            continue;
        }
        let text = text.replace("\r\n", "\n");
        let src = CustomSource { dialect: CustomDialect::Rhai, entry: rhai::title(&text).unwrap_or_default(), source: text, variations: Vec::new() };
        let key = crate::escape::custom::name_of(EntryKind::Formula, &src);
        match crate::escape::custom::compile_solid(&src, &key) {
            Ok(def) => out.push((format!("{file} (Rhai)"), def)),
            Err(_) => println!("{file}: does not compile as a solid; skipped"),
        }
    }
    out
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>().split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_")
}

/// (naga front ms, naga SPIR-V ms, pipeline ms) for one source.
///
/// The driver keeps compiled pipelines on disk, keyed by what it is
/// given: a source measured twice compiled in milliseconds the second
/// time. An uncalled function holding this run's nanoseconds makes every
/// source new to it without changing what runs.
fn time(device: &wgpu::Device, src: &str) -> (f64, f64, f64) {
    time_entry(device, src, None)
}

/// [`time`] with the entry point named (a module with several).
fn time_entry(device: &wgpu::Device, src: &str, entry: Option<&str>) -> (f64, f64, f64) {
    use wgpu::naga;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u32);
    let src = &format!("{src}
fn compile_cost_nonce() -> u32 {{ return {nonce}u; }}
");
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    let module = naga::front::wgsl::parse_str(src).expect("parses");
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("validates");
    let front = ms(t);
    let t = std::time::Instant::now();
    let _ = naga::back::spv::write_vec(&module, &info, &naga::back::spv::Options::default(), None).expect("SPIR-V");
    let spv = ms(t);
    let t = std::time::Instant::now();
    let m = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
    let p = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &m,
        entry_point: entry,
        compilation_options: Default::default(),
        cache: None,
    });
    drop(p);
    (front, spv, ms(t))
}

#[test]
#[ignore = "a measurement: writes output/compile-cost/"]
fn compile_cost() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
        println!("no adapter");
        return;
    };
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let dir = std::path::Path::new("output/compile-cost");
    std::fs::create_dir_all(dir).unwrap();
    let mut tsv = String::from("shader\tbytes\tlines\tnaga_front_ms\tnaga_spv_ms\tpipeline_ms\n");
    let none = UserKernels::default();
    let only = std::env::var("COMPILE_COST_ONLY").ok();
    for (label, def) in solids() {
        if only.as_deref().is_some_and(|o| !label.contains(o)) {
            continue;
        }
        let col = get_ifs_coloring(def.default_coloring, def);
        // A solid that calls variations has them spliced in as maps.
        let escape = crate::config::escape::EscapeConfig { formula: def.name.to_string(), ..Default::default() };
        let lens = crate::escape::lens::lens_source(&escape, &crate::variations::global_registry());
        let shaders = [
            ("lit", assemble_ifs_user(def, col, 4, lens.as_deref(), false, false, false, &none, &none)),
            ("path", assemble_ifs_path_user(def, col, 4, lens.as_deref(), false, false, &none)),
        ];
        for (what, src) in shaders {
            let name = format!("{} {what}", label);
            std::fs::write(dir.join(format!("{}.wgsl", slug(&name))), &src).unwrap();
            let (front, spv, pipe) = time(&device, &src);
            println!("{name:<48} {:>7} B {:>5} lines  front {front:>6.0} ms  spv {spv:>6.0} ms  pipeline {pipe:>7.0} ms", src.len(), src.lines().count());
            tsv.push_str(&format!("{name}\t{}\t{}\t{front:.1}\t{spv:.1}\t{pipe:.1}\n", src.len(), src.lines().count()));
        }
    }
    std::fs::write(dir.join("compile-cost.tsv"), tsv).unwrap();
}

/// A/B for a change to the path tracer's shader: every solid and terrain
/// preset path traced small, its pixels hashed, to `output/pt-ab/<tag>.txt`
/// (`PT_AB_TAG`); run before and after, and diff the two files.
#[test]
#[ignore = "an A/B measurement: writes output/pt-ab/"]
fn path_traced_presets_hash() {
    use crate::config::escape::RenderTier;
    let Ok(tag) = std::env::var("PT_AB_TAG") else { return };
    let (device, queue) = pollster::block_on(crate::probe::run::open_device("pt a/b")).expect("a GPU");
    let mut out = String::new();
    for cfg in crate::scene::presets::global_preset_library().presets() {
        use crate::scene::transforms::RenderMode;
        let mut c = cfg.clone();
        c.ensure_sources();
        let (solid, terrain) = match c.render_mode {
            RenderMode::Escape => (get_ifs(&c.escape.formula).is_some_and(|d| d.solid), c.escape.terrain.enabled),
            RenderMode::Simulation => (false, c.sim.terrain.enabled),
            _ => (false, false),
        };
        if !solid && !terrain {
            continue;
        }
        // `PT_AB_TIER=lit` for the lit tier (the walk's own shader).
        let tier = if std::env::var("PT_AB_TIER").as_deref() == Ok("lit") { RenderTier::Lit } else { RenderTier::PathTraced };
        c.escape.solid_tier = tier;
        c.escape.terrain.tier = tier;
        c.sim.terrain.tier = tier;
        c.escape.path.samples = 3;
        let job = crate::renderer::RenderJob::new(&c, 96, 64);
        let px = match pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)) {
            Ok(o) => o.rgba_data,
            Err(e) => {
                out.push_str(&format!("{}\terror {e}\n", c.flame.name));
                continue;
            }
        };
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        px.hash(&mut h);
        let lit = px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 0).count();
        out.push_str(&format!("{}\t{}\t{:016x}\tlit {lit}\n", c.flame.name, if solid { "solid" } else { "terrain" }, h.finish()));
    }
    std::fs::create_dir_all("output/pt-ab").unwrap();
    std::fs::write(format!("output/pt-ab/{tag}.txt"), &out).unwrap();
    print!("{out}");
}

/// The 2D escape formulas' compile (mode A), each built-in beside its Rhai
/// twin where it has one, with the default colouring: what a Catalog
/// preview or a formula switch compiles. `COMPILE_COST_2D=1`.
#[test]
#[ignore = "a measurement"]
fn compile_cost_2d() {
    if std::env::var("COMPILE_COST_2D").is_err() {
        return;
    }
    let (device, _queue) = pollster::block_on(crate::probe::run::open_device("compile cost 2d")).expect("a GPU");
    let coloring = crate::escape::get_coloring("smooth");
    let mut rows = Vec::new();
    for f in crate::escape::FORMULAS {
        let src = crate::escape::assembler::assemble(f, coloring, false);
        let (_, _, ms) = time(&device, &src);
        rows.push((format!("{} (built-in)", f.name), src.len(), ms));
        if let Some(cs) = crate::escape::twins::custom_source(f.name) {
            let key = crate::escape::custom::name_of(EntryKind::Formula, &cs);
            if let Ok(def) = crate::escape::custom::compile_formula(&cs, &key) {
                let src = crate::escape::assembler::assemble(def, coloring, false);
                let (_, _, ms) = time(&device, &src);
                rows.push((format!("{} (Rhai twin)", f.name), src.len(), ms));
            }
        }
    }
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    let total: f64 = rows.iter().map(|r| r.2).sum();
    println!("{} shaders, {total:.0} ms in all; the slowest:", rows.len());
    for (name, bytes, ms) in rows.iter().take(15) {
        println!("  {name:<40} {bytes:>7} B  {ms:>7.0} ms");
    }
    let med = rows[rows.len() / 2].2;
    println!("median {med:.0} ms");
}

/// The flame's chaos-game shader for every flame preset, as the shader
/// cache builds it (its active variations, the config's constants), timed
/// and written to `output/compile-cost/flame-*.wgsl`. `COMPILE_COST_FLAMES=1`.
#[test]
#[ignore = "a measurement"]
fn compile_cost_flames() {
    if std::env::var("COMPILE_COST_FLAMES").is_err() {
        return;
    }
    use crate::scene::transforms::RenderMode;
    let (device, _queue) = pollster::block_on(crate::probe::run::open_device("compile cost flames")).expect("a GPU");
    let dir = std::path::Path::new("output/compile-cost");
    std::fs::create_dir_all(dir).unwrap();
    let builder = crate::shader_builder_v2::ShaderBuilder::new(crate::variations::global_registry().clone());
    let mut rows = Vec::new();
    for cfg in crate::scene::presets::global_preset_library().presets() {
        if !matches!(cfg.render_mode, RenderMode::TwoD | RenderMode::ThreeD) {
            continue;
        }
        let mut c = cfg.clone();
        c.ensure_sources();
        let active = c.flame.extract_active_variations();
        let constants = crate::shader_cache::ShaderCache::constants_from_config(&c);
        let src = builder.build_from_template(&c.flame, &active, c.render_mode == RenderMode::ThreeD, false, c.flame.has_xaos(), true, &constants);
        std::fs::write(dir.join(format!("flame-{}.wgsl", slug(&c.flame.name))), &src).unwrap();
        let (_, _, ms) = time_entry(&device, &src, Some("main"));
        rows.push((format!("{} ({:?}, {} variations)", c.flame.name, c.render_mode, active.len()), src.len(), ms));
    }
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    let total: f64 = rows.iter().map(|r| r.2).sum();
    println!("{} flames, {total:.0} ms in all; median {:.0} ms; the slowest:", rows.len(), rows[rows.len() / 2].2);
    for (name, bytes, ms) in rows.iter().take(15) {
        println!("  {name:<60} {bytes:>7} B  {ms:>7.0} ms");
    }
}

/// Every registered variation alone in a one-transform 2D flame, its
/// shader's compile timed: which variations cost the compile.
/// `COMPILE_COST_VARIATIONS=1` (about a minute).
#[test]
#[ignore = "a measurement"]
fn compile_cost_variations() {
    if std::env::var("COMPILE_COST_VARIATIONS").is_err() {
        return;
    }
    let (device, _queue) = pollster::block_on(crate::probe::run::open_device("compile cost variations")).expect("a GPU");
    let reg = crate::variations::global_registry().clone();
    let builder = crate::shader_builder_v2::ShaderBuilder::new(reg.clone());
    let names: Vec<String> = reg.all().iter().filter(|v| v.wgsl_source.is_some()).map(|v| v.name.to_string()).collect();
    // The flame with no variation but linear: the floor every row sits on.
    let flame_of = |name: &str| {
        let mut flame = crate::scene::transforms::Flame::new();
        let mut xf = crate::scene::transforms::Transform::new();
        xf.set_variation(name, 1.0);
        flame.transforms.push(xf);
        flame
    };
    let mut cfg = crate::config::FractalConfig::default();
    let mut rows = Vec::new();
    for name in std::iter::once("linear".to_string()).chain(names) {
        cfg.flame = flame_of(&name);
        let active = cfg.flame.extract_active_variations();
        let constants = crate::shader_cache::ShaderCache::constants_from_config(&cfg);
        let src = builder.build_from_template(&cfg.flame, &active, false, false, false, true, &constants);
        let (_, _, ms) = time_entry(&device, &src, Some("main"));
        rows.push((name, src.len(), ms));
    }
    let floor = rows[0].2;
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    println!("{} variations; linear alone {floor:.0} ms; the slowest:", rows.len());
    for (name, bytes, ms) in rows.iter().take(25) {
        println!("  {name:<28} {bytes:>7} B  {ms:>7.0} ms");
    }
    let over = rows.iter().filter(|r| r.2 > floor + 200.0).count();
    println!("{over} variations add more than 200 ms");
}

/// One flame of ten heavier variations, plain, with a final transform, and
/// with linked and final ones: what `apply_variations`' call sites cost.
/// `COMPILE_COST_SITES=1`.
#[test]
#[ignore = "a measurement"]
fn compile_cost_flame_sites() {
    if std::env::var("COMPILE_COST_SITES").is_err() {
        return;
    }
    use crate::scene::transforms::{Flame, Transform};
    let (device, _queue) = pollster::block_on(crate::probe::run::open_device("compile cost sites")).expect("a GPU");
    let builder = crate::shader_builder_v2::ShaderBuilder::new(crate::variations::global_registry().clone());
    // Ten of the heaviest variations by default (a stress test, not a
    // typical flame); `COMPILE_COST_SITES_VARS=a,b,...` for any ten.
    let given = std::env::var("COMPILE_COST_SITES_VARS").ok();
    let default = ["electron_orbital", "chladni_disc", "crackle", "synth", "honeycomb", "quaternion", "sphere_packing", "waves2b", "dc_perlin", "complex"];
    let heavy: Vec<&str> = match &given {
        Some(v) => v.split(',').collect(),
        None => default.to_vec(),
    };
    assert!(heavy.len() >= 2, "at least two variations");
    let half = heavy.len() / 2;
    let xf = |vars: &[&str]| {
        let mut t = Transform::new();
        for v in vars {
            t.set_variation(v, 1.0);
        }
        t
    };
    for (label, linked, fin) in [("plain", false, false), ("final", false, true), ("linked + final", true, true)] {
        let mut flame = Flame::new();
        flame.transforms.push(xf(&heavy[..half]));
        flame.transforms.push(xf(&heavy[half..]));
        if linked {
            flame.linked_transforms.push(xf(&heavy));
            for t in &mut flame.transforms {
                t.linked_attachments.push(0);
            }
        }
        if fin {
            flame.final_transforms.push(xf(&heavy));
            for t in &mut flame.transforms {
                t.final_attachments.push(0);
            }
        }
        let mut cfg = crate::config::FractalConfig::default();
        cfg.flame = flame;
        let active = cfg.flame.extract_active_variations();
        let constants = crate::shader_cache::ShaderCache::constants_from_config(&cfg);
        let src = builder.build_from_template(&cfg.flame, &active, false, false, false, true, &constants);
        std::fs::write(format!("output/compile-cost/flame-sites-{}.wgsl", slug(label)), &src).unwrap();
        let (_, _, ms) = time_entry(&device, &src, Some("main"));
        println!("  {label:<16} {:>7} B  {ms:>7.0} ms", src.len());
    }
}

/// A/B for a change to the flame shader's chain: the visual configs with
/// linked or final transforms, and two flames of the sites measurement,
/// rendered at a fixed iteration count and hashed, to
/// `output/pt-ab/flame-<tag>.txt` (`FLAME_AB_TAG`).
#[test]
#[ignore = "an A/B measurement"]
fn flame_chain_hash() {
    use crate::scene::transforms::{Flame, Transform};
    let Ok(tag) = std::env::var("FLAME_AB_TAG") else { return };
    let (device, queue) = pollster::block_on(crate::probe::run::open_device("flame a/b")).expect("a GPU");
    let mut cfgs: Vec<(String, crate::config::FractalConfig)> = Vec::new();
    for f in ["3d/stereogram-smoke", "variations/multi_kaleidoscope-smoke", "variations/spray_blur-normal-xform-smoke", "variations/spray_blur-smoke"] {
        let text = std::fs::read_to_string(format!("tests/visual/configs/{f}.fflame")).unwrap();
        cfgs.push((f.to_string(), crate::config::FractalConfig::from_json(&text).unwrap()));
    }
    let xf = |vars: &[&str], a: f32| {
        let mut t = Transform::new();
        for v in vars {
            t.set_variation(v, 0.5);
        }
        t.a = a;
        t.d = a;
        t
    };
    for (label, linked) in [("final", false), ("linked + final", true)] {
        let mut flame = Flame::new();
        flame.transforms.push(xf(&["spherical", "swirl"], 0.6));
        flame.transforms.push(xf(&["julia", "disc"], 0.7));
        if linked {
            flame.linked_transforms.push(xf(&["sinusoidal"], 0.9));
            flame.transforms[0].linked_attachments.push(0);
        }
        flame.final_transforms.push(xf(&["bubble"], 1.0));
        for t in &mut flame.transforms {
            t.final_attachments.push(0);
        }
        let mut c = crate::config::FractalConfig::default();
        c.deterministic_rng = true;
        c.flame = flame;
        cfgs.push((label.to_string(), c));
    }
    let mut out = String::new();
    for (name, mut c) in cfgs {
        c.ensure_sources();
        let job = crate::renderer::RenderJob::new(&c, 96, 64).with_iterations(2_000_000);
        let px = pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data;
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        px.hash(&mut h);
        let lit = px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 0).count();
        out.push_str(&format!("{name}\t{:016x}\tlit {lit}\n", h.finish()));
    }
    std::fs::create_dir_all("output/pt-ab").unwrap();
    std::fs::write(format!("output/pt-ab/flame-{tag}.txt"), &out).unwrap();
    print!("{out}");
}

/// Everything else that compiles on a user's choice, beside the flame and
/// the solids: each simulation model's step and each colouring (built-in
/// beside its Rhai twin), each escape field, each formula's perturbed
/// (deep zoom) shader, and the terrains' walk and path tracer.
/// `COMPILE_COST_REST=1`.
#[test]
#[ignore = "a measurement"]
fn compile_cost_rest() {
    if std::env::var("COMPILE_COST_REST").is_err() {
        return;
    }
    use crate::config::sim::{SimBoundary, SimDownscale, SimUpscale};
    let (device, _queue) = pollster::block_on(crate::probe::run::open_device("compile cost rest")).expect("a GPU");
    let dir = std::path::Path::new("output/compile-cost");
    std::fs::create_dir_all(dir).unwrap();
    let mut rows: Vec<(String, usize, f64)> = Vec::new();
    // Each compute entry point is a pipeline of its own.
    let entries = |src: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = src;
        while let Some(at) = rest.find("@compute") {
            rest = &rest[at..];
            if let Some(f) = rest.find("fn ") {
                let name: String = rest[f + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                out.push(name);
                rest = &rest[f + 3..];
            } else {
                break;
            }
        }
        out
    };
    let mut measure = |rows: &mut Vec<(String, usize, f64)>, name: String, src: String| {
        std::fs::write(dir.join(format!("rest-{}.wgsl", slug(&name))), &src).unwrap();
        let es = entries(&src);
        if es.len() <= 1 {
            let (_, _, ms) = time(&device, &src);
            rows.push((name, src.len(), ms));
        } else {
            for e in es {
                let (_, _, ms) = time_entry(&device, &src, Some(&e));
                rows.push((format!("{name} [{e}]"), src.len(), ms));
            }
        }
    };
    let b = SimBoundary::default();
    for m in crate::sim::MODELS {
        measure(&mut rows, format!("sim model {} (built-in)", m.name), crate::sim::assembler::assemble_step(m, b, 0));
    }
    for (name, src) in crate::sim::custom::SHIPPED_MODELS {
        if let Ok(def) = crate::sim::custom::compile_model(src, &format!("cc-{name}")) {
            let def: &'static crate::sim::ModelDef = Box::leak(Box::new(def));
            measure(&mut rows, format!("sim model {name} (Rhai twin)"), crate::sim::assembler::assemble_step(def, b, 0));
        }
    }
    for c in crate::sim::COLORINGS {
        measure(&mut rows, format!("sim colouring {} (built-in)", c.name), crate::sim::assembler::assemble_color(c, b, SimUpscale::default(), SimDownscale::default(), false));
    }
    for (name, src) in crate::sim::custom::SHIPPED {
        if let Ok(def) = crate::sim::custom::compile(src, &format!("cc-{name}")) {
            measure(&mut rows, format!("sim colouring {name} (Rhai twin)"), crate::sim::assembler::assemble_color(&def, b, SimUpscale::default(), SimDownscale::default(), false));
        }
    }
    for f in crate::escape::fields::FIELDS {
        let col = crate::escape::fields::FIELD_COLORINGS[0];
        measure(&mut rows, format!("field {}", f.name), crate::escape::assembler::assemble_field(f, col));
    }
    let smooth = crate::escape::get_coloring("smooth");
    for f in crate::escape::FORMULAS {
        let escape = crate::config::escape::EscapeConfig { formula: f.name.to_string(), ..Default::default() };
        let escape = crate::escape::custom::resolve(&escape).into_owned();
        if let Some(tier) = crate::escape::custom::perturb_tier(&escape) {
            for fe in [false, true] {
                let src = crate::escape::assembler::assemble_perturbed_full(smooth, None, fe, tier, None, None);
                measure(&mut rows, format!("perturbed {}{}", f.name, if fe { " (floatexp)" } else { "" }), src);
            }
        }
    }
    for placed in [false, true] {
        measure(&mut rows, format!("terrain walk{}", if placed { " (placed lights)" } else { "" }), crate::escape::terrain::assemble_walk(placed));
        measure(&mut rows, format!("terrain path{}", if placed { " (placed lights)" } else { "" }), crate::escape::terrain::assemble_path(placed));
    }
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    println!("{} shaders; median {:.0} ms; the slowest:", rows.len(), rows[rows.len() / 2].2);
    for (name, bytes, ms) in rows.iter().take(30) {
        println!("  {name:<48} {bytes:>7} B  {ms:>7.0} ms");
    }
}
