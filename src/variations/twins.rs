//! Rhai twins of the built-in variations (docs/projects/rhai-plugins.md,
//! P7 steps 3 and 4): each a Rhai variation that does what its built-in
//! does, under the built-in's display name, with its built-in's
//! parameters exactly -- names, captions, hints, kinds, defaults, ranges,
//! a free range, a log slider (`every_variation_twin_compiles_as_its_builtin`)
//! -- and its features. Held to the built-in per point by the probe's
//! pairs (`probe::pairs`, `every_variation_twin_runs_as_its_builtin`):
//! exact where no trig is, within ulps where the language's range-reduced
//! trig meets the built-in's hardware trig (the portable side, the user's
//! rule: whatever makes WebGPU, Metal and Vulkan agree).
//!
//! Sources in `assets/variations/twins/`, one a built-in, each named
//! `rhai_<the built-in's name, lowercased>` (`INFO.name`, what `vary`
//! calls it by); [`skeleton`] writes one's `INFO` and `PARAMS` from the
//! def, with the built-in's WGSL as a comment to port from.

use crate::config::escape::CustomSource;
use crate::params::{ParamDef, ParamKind};

/// Every twin: the built-in's name, the twin's source.
pub const TWINS: &[(&str, &str)] = &[
    ("julia", include_str!("../../assets/variations/twins/julia.rhai")),
    ("rings2", include_str!("../../assets/variations/twins/rings2.rhai")),
    ("curl", include_str!("../../assets/variations/twins/curl.rhai")),
    ("flipcircle", include_str!("../../assets/variations/twins/flipcircle.rhai")),
    ("sinusoidal3d", include_str!("../../assets/variations/twins/sinusoidal3d.rhai")),
    ("exp", include_str!("../../assets/variations/twins/exp.rhai")),
    ("hyperbolic", include_str!("../../assets/variations/twins/hyperbolic.rhai")),
];

/// The twin of the built-in variation `name`, when it has one.
pub fn source(name: &str) -> Option<&'static str> {
    TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The twin as a flame carries it (`Flame::custom_variations`).
pub fn carried(name: &str) -> Option<CustomSource> {
    super::custom::carried(source(name)?)
}

/// The twin's registry key: what a transform names it by.
pub fn key(name: &str) -> Option<String> {
    super::custom::key_of(source(name)?)
}

/// A float as a Rhai literal that parses back to the same f32: the
/// shortest form, with the point Rhai wants.
fn float(v: f64) -> String {
    let s = format!("{:?}", v as f32);
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        s
    } else if let Some(e) = s.find('e') {
        format!("{}.0{}", &s[..e], &s[e..])
    } else {
        format!("{s}.0")
    }
}

fn text(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A def's parameters as a twin's `PARAMS` map, each as the built-in
/// declares it: a free range `free: true`, a log slider `exponential:
/// true`, a range of five decades that is not one `exponential: false`,
/// an angle `type: "angle"`.
pub fn params_rhai(params: &[ParamDef]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::from("const PARAMS = #{\n");
    for p in params {
        let head = match &p.kind {
            ParamKind::Float { default, min, max, free, log, angle } => {
                let mut s = format!("value: {}, min: {}, max: {}", float(*default), float(*min), float(*max));
                if *free {
                    s += ", free: true";
                }
                if *log {
                    s += ", exponential: true";
                } else if *min > 0.0 && *max / *min >= 1e5 {
                    s += ", exponential: false";
                }
                if *angle {
                    s += ", type: \"angle\"";
                }
                s
            }
            ParamKind::Int { default, min, max, free } => {
                let mut s = format!("value: {default}, min: {min}, max: {max}");
                if *free {
                    s += ", free: true";
                }
                s
            }
            ParamKind::Bool { default } => format!("value: {default}"),
            ParamKind::Choice { default, options } => {
                let names: Vec<String> = options.iter().map(text).collect();
                format!("value: {default}, choices: [{}]", names.join(", "))
            }
            ParamKind::Text { .. } | ParamKind::Color { .. } => "value: 0.0".to_string(),
        };
        out += &format!("    {}: #{{\n        {head},\n        caption: {},\n", p.name, text(&p.label));
        if !p.hint.is_empty() {
            out += &format!("        hint: {},\n", text(&p.hint));
        }
        out += "    },\n";
    }
    out += "};\n\n";
    out
}

/// A twin's beginning, from the built-in `name`'s def: its `INFO` (its
/// display name; `rhai_` and its name lowercased, what `vary` calls it)
/// and its `PARAMS`, then the built-in's WGSL as a comment to port, and a
/// body that returns its point.
pub fn skeleton(name: &str) -> Option<String> {
    let reg = super::global_registry();
    let info = reg.get(name)?;
    let mut out = format!("// {}, the built-in variation's twin.\n\n", info.display_name);
    out += &params_rhai(&info.parameters);
    out += &format!("const INFO = #{{ title: {}, name: \"rhai_{}\" }};\n\n", text(&info.display_name), name.to_ascii_lowercase());
    for (what, wgsl) in [("2D", info.wgsl_source.as_deref()), ("3D", info.wgsl_source_3d.as_deref()), ("init", info.wgsl_source_init.as_deref())] {
        if let Some(w) = wgsl.filter(|w| !w.trim().is_empty()) {
            out += &format!("// The built-in's {what} WGSL:\n");
            for line in w.trim().lines() {
                out += &format!("//   {line}\n");
            }
            out += "\n";
        }
    }
    out += "fn variation(z, p) {\n    z\n}\n";
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WRITER, not a test: `TWIN_SKELETONS=julia,rings2` writes each
    /// one's skeleton to `assets/variations/twins/<name>.rhai`, where none
    /// is yet.
    #[test]
    #[ignore = "writes files"]
    fn write_variation_twin_skeletons() {
        let Ok(names) = std::env::var("TWIN_SKELETONS") else { return };
        for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            let path = format!("assets/variations/twins/{name}.rhai");
            if std::path::Path::new(&path).exists() {
                println!("{path}: there already");
                continue;
            }
            let text = skeleton(name).unwrap_or_else(|| panic!("no built-in `{name}`"));
            std::fs::write(&path, text).unwrap();
            println!("wrote {path}");
        }
    }

    /// Each twin compiles as the app compiles a variation (its WGSL
    /// validated), under its built-in's display name, with its built-in's
    /// parameters exactly and the features that change what it draws: the
    /// colour it writes, z always written, the running sum read.
    #[test]
    fn every_variation_twin_compiles_as_its_builtin() {
        use crate::variations::Feature as F;
        let reg = super::super::global_registry();
        for (name, src) in TWINS {
            let builtin = reg.get(name).unwrap_or_else(|| panic!("no built-in `{name}`"));
            let dl = super::super::custom::compile(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(dl.display_name, builtin.display_name, "{name}: its title");
            let p = crate::lang::rhai::transformation(src).or_else(|_| crate::lang::rhai::transformation3(src)).unwrap();
            let params = p.param_defs(crate::lang::ir::ComplexParams::Split("_")).unwrap();
            assert_eq!(params.len(), builtin.parameters.len(), "{name}: its parameters");
            for (t, b) in params.iter().zip(&builtin.parameters) {
                assert_eq!((&t.name, &t.label, &t.hint, &t.kind, t.slow), (&b.name, &b.label, &b.hint, &b.kind, b.slow), "{name}.{}", b.name);
            }
            let has = |f: &str| dl.features.iter().any(|x| x == f);
            for (f, wire) in [(F::WritesColor, "writes_color"), (F::AlwaysZ, "always_z"), (F::NeedsAccum, "needs_accum")] {
                assert_eq!(has(wire), builtin.features.contains(&f), "{name}: {f:?}");
            }
            assert_eq!(dl.name, key(name).unwrap(), "{name}: its key");
        }
    }

    /// The points a twin is held at: the probe's adversarial inputs (the
    /// signed zeros, the axes, the far and the tiny) and a grid of
    /// ordinary ones, each with a z.
    fn points() -> Vec<[f32; 3]> {
        let mut pts: Vec<[f32; 3]> = crate::probe::probe_inputs().iter().map(|p| [p.x, p.y, p.z]).collect();
        for i in 0..9 {
            for j in 0..9 {
                let (x, y) = (-3.1 + 0.77 * i as f32, -2.9 + 0.73 * j as f32);
                pts.push([x, y, 0.37 * (i as f32 - j as f32)]);
            }
        }
        pts
    }

    /// Whether a twin's output `got` agrees with its built-in's `want`:
    /// both finite and within 2e-5 of the larger one's size (1 below it)
    /// -- the ulps the language's range-reduced trig and a fused
    /// multiply-add part them by -- or alike not finite.
    fn agrees(want: f32, got: f32) -> bool {
        match (want.is_finite(), got.is_finite()) {
            (true, true) => (want - got).abs() <= 2e-5 * want.abs().max(got.abs()).max(1.0),
            (false, false) => want.is_nan() == got.is_nan() && (want.is_nan() || want.signum() == got.signum()),
            _ => false,
        }
    }

    /// **Each twin runs as its built-in** (P7 step 3, V1): through the
    /// probe's pairs, the built-in and its twin at every point, in 2D and
    /// 3D, at the defaults and with each parameter at its range's ends,
    /// every output part and the colour it writes. A point past 2^24,
    /// where trig is garbage on every platform and differently
    /// (docs/accepted-divergences.txt), is not held where the built-in
    /// gives no finite answer either.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_variation_twin_runs_as_its_builtin() {
        use crate::probe::pairs::{run_pairs, Pair};
        let (device, queue) = pollster::block_on(crate::probe::run::open_device("variation twins")).expect("a GPU");
        let pts = points();
        let reg = super::super::global_registry().clone();
        let mut pairs = Vec::new();
        let mut carried = Vec::new();
        for (name, _) in TWINS {
            let builtin = reg.get(name).unwrap();
            let twin = key(name).unwrap();
            carried.push(super::carried(name).unwrap());
            pairs.push((format!("{name} at its defaults"), Pair { builtin: name.to_string(), twin: twin.clone(), params: Vec::new() }));
            for d in &builtin.parameters {
                if let Some((lo, hi)) = d.kind.bounds() {
                    for v in [lo, hi] {
                        pairs.push((format!("{name} with {} at {v}", d.name), Pair { builtin: name.to_string(), twin: twin.clone(), params: vec![(d.name.to_string(), v as f32)] }));
                    }
                }
            }
        }
        drop(reg);
        // The compare tells a different variation apart: exp's twin set
        // beside julia.
        pairs.push(("(control) julia beside exp's twin".into(), Pair { builtin: "julia".into(), twin: key("exp").unwrap(), params: Vec::new() }));
        let mut failures = Vec::new();
        for render_3d in [false, true] {
            let only: Vec<Pair> = pairs.iter().map(|(_, p)| p.clone()).collect();
            let out = run_pairs(&device, &queue, &only, &carried, &pts, render_3d).expect("the pairs ran");
            for ((what, _), o) in pairs.iter().zip(&out) {
                let mut bad = Vec::new();
                let mut exact = 0;
                for (i, (w, g)) in o.builtin.iter().zip(&o.twin).enumerate() {
                    let far = pts[i].iter().any(|c| c.abs() > 16_777_216.0);
                    for c in 0..4 {
                        exact += usize::from(w[c].to_bits() == g[c].to_bits());
                        if !agrees(w[c], g[c]) && !(far && !w[c].is_finite()) {
                            bad.push(format!("    at {:?} part {c}: built-in {} twin {}", pts[i], w[c], g[c]));
                        }
                    }
                }
                let dim = if render_3d { "3D" } else { "2D" };
                println!("{what} ({dim}): {} of {} parts apart, {exact} bit for bit", bad.len(), pts.len() * 4);
                if what.starts_with("(control)") {
                    if bad.len() * 2 < pts.len() * 2 {
                        failures.push(format!("{what} ({dim}): the compare did not tell them apart ({} apart)", bad.len()));
                    }
                    continue;
                }
                if !bad.is_empty() {
                    failures.push(format!("{what} ({dim}):
{}", bad.iter().take(6).cloned().collect::<Vec<_>>().join("
")));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("
"));
    }

    /// A 2D flame of three transforms: `variation` on a turning shrink,
    /// two linear shrinks keeping it bounded; a twin's flame carries its
    /// source, which nothing registers but the renderer.
    fn flame_with(variation: &str, carries: Option<CustomSource>) -> crate::config::FractalConfig {
        use crate::scene::transforms::{RenderMode, Transform};
        let mut cfg = crate::config::FractalConfig::default();
        cfg.render_mode = RenderMode::TwoD;
        cfg.flame.transforms.clear();
        for (i, (m, v)) in [((0.6f32, -0.3f32, 0.3f32, 0.6f32, 0.1f32, -0.2f32), variation), ((0.5, 0.0, 0.0, 0.5, 0.5, 0.3), "linear"), ((0.5, 0.0, 0.0, 0.5, -0.4, 0.4), "linear")]
            .into_iter()
            .enumerate()
        {
            let mut t = Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = m;
            t.weight = 1.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(v, 1.0);
            cfg.flame.transforms.push(t);
        }
        cfg.flame.custom_variations = carries.into_iter().collect();
        cfg.zoom = 1.6;
        cfg.levels_enabled = false;
        cfg
    }

    /// **Each twin draws its built-in's flame** (P7 step 3, V1): by
    /// density, block by block over replicates -- the chaos game parts two
    /// orbits at the first rounding, so they agree as measures, not pixel
    /// for pixel. And the measure tells two built-ins apart.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_variation_twin_draws_its_builtins_flame() {
        use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Run};
        const N: u32 = 256;
        let (device, queue) = pollster::block_on(crate::probe::run::open_device("variation twin flames")).expect("a GPU");
        let run = Run::default();
        let mut failures = Vec::new();
        let mut renders = Vec::new();
        for (name, _) in TWINS {
            let reference = render_blocks(&device, &queue, &flame_with(name, None), N, &run);
            let twin = render_blocks(&device, &queue, &flame_with(&key(name).unwrap(), Some(super::carried(name).unwrap())), N, &run);
            let c = compare_blocks(&twin.density, &reference.density, &reference.density);
            print_compared(&format!("{name}'s twin against {name}"), &c);
            let (_, _, mx, _, my, _, z) = c.pooled[0];
            if mx <= 0.0 {
                failures.push(format!("{name}'s twin drew nothing"));
            }
            if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005 {
                failures.push(format!("{name}'s twin: the whole view's density is {:.4} of the built-in's", mx / my));
            }
            if c.worst_dense > 0.05 || c.worst_middle > 0.15 {
                failures.push(format!("{name}'s twin: a block is {:.3} off (dense), {:.3} (middle)", c.worst_dense, c.worst_middle));
            }
            renders.push((name, reference));
        }
        for w in renders.windows(2) {
            let c = compare_blocks(&w[0].1.density, &w[1].1.density, &w[1].1.density);
            if c.worst_dense < 0.2 {
                failures.push(format!("{} and {} look alike to this measure: {:.3}", w[0].0, w[1].0, c.worst_dense));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("
"));
    }

    /// A parameter of every kind written as a twin's `PARAMS` reads back
    /// as the same definition.
    #[test]
    fn params_rhai_reads_back_as_the_definitions() {
        use crate::params::Spec;
        let defs = [
            ParamDef::new("a", "A", "Free \"quoted\".", Spec::float(1.0, 0.01, 5.0).free()),
            ParamDef::new("b", "B", "", Spec::float(0.5, 1e-6, 1.0)),
            ParamDef::new("c", "C", "Log.", Spec::float(0.1, 0.001, 10.0).log()),
            ParamDef::new("d", "D", "", Spec::int(3, -20, 20).free()),
            ParamDef::new("e", "E", "", Spec::int(2, 1, 9)),
            ParamDef::new("f", "F", "", Spec::angle(30.0, -360.0, 360.0)),
            ParamDef::new("g", "G", "", Spec::bool(true)),
            ParamDef::new("h", "H", "", Spec::choice(1, &["One", "Two"])),
        ];
        let src = format!("{}const INFO = #{{ name: \"rhai_params_back\" }};\nfn variation(z, p) {{ z }}\n", params_rhai(&defs));
        let p = crate::lang::rhai::transformation(&src).unwrap_or_else(|d| panic!("{src}\n{:?}", d.iter().map(|x| x.message.clone()).collect::<Vec<_>>()));
        let back = p.param_defs(crate::lang::ir::ComplexParams::Split("_")).unwrap();
        for (t, b) in back.iter().zip(&defs) {
            assert_eq!((&t.name, &t.label, &t.hint, &t.kind), (&b.name, &b.label, &b.hint, &b.kind), "{}", b.name);
        }
        assert_eq!(back.len(), defs.len());
    }
}
