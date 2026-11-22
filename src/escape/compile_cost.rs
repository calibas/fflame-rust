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
        entry_point: None,
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
