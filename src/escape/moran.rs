//! Level-dependent flames (docs/projects/level-dependent-flames.md): the
//! presets that show them, written from the example file
//! (`assets/formulas/variation-moran.rhai`), the test that keeps the
//! bundled presets carrying the file as it is, and the tests of what the
//! file draws -- each set in the unit square, the levels composed with
//! the coarsest outermost.

use super::lang::builtins::C64;
use super::lang::interp::{ParamValues, VariationRun};
use super::lang::rhai;
use crate::config::escape::{CustomDialect, CustomSource};
use crate::storage::formula_store::EXAMPLES;

/// An example file's text, with LF line ends whatever the checkout gave
/// it: a source's name is its hash.
fn example(file: &str) -> String {
    EXAMPLES.iter().find(|(f, _)| *f == file).map(|(_, t)| t.replace("\r\n", "\n")).unwrap_or_else(|| panic!("no example {file}"))
}

/// The sets, as the file numbers them.
const TRIANGLE: f64 = 0.0;
const CARPET: f64 = 1.0;
const FERN: f64 = 2.0;
const DRAGON: f64 = 3.0;
const LEVY: f64 = 4.0;

/// The schedules, as the file numbers them.
const SPLIT: f64 = 0.0;
const ALTERNATE: f64 = 1.0;
const STURMIAN: f64 = 2.0;

/// The presets: a flame of one transform holding only the variation, its
/// colours the coarsest level's map, the unit square in view.
fn presets() -> Vec<serde_json::Value> {
    let text = example("variation-moran.rhai");
    let name = rhai::variation_name(&text).expect("the variation's name");
    let moran = CustomSource { dialect: CustomDialect::Rhai, entry: name.clone(), source: text, variations: Vec::new() };
    let palette = serde_json::to_value(crate::config::FractalConfig::default().palette).unwrap();
    // (title, coarse, fine, schedule, split).
    let flames: [(&str, f64, f64, f64, f64); 6] = [
        ("Moran: Triangle of Ferns", TRIANGLE, FERN, SPLIT, 2.0),
        ("Moran: Triangle of Ferns, Four Levels", TRIANGLE, FERN, SPLIT, 4.0),
        ("Moran: Carpet of Dragons", CARPET, DRAGON, SPLIT, 1.0),
        ("Moran: Dragon of Carpets", DRAGON, CARPET, SPLIT, 4.0),
        ("Moran: Carpet and Levy C, Sturmian", CARPET, LEVY, STURMIAN, 3.0),
        ("Moran: Dragon and Levy C, Sturmian", DRAGON, LEVY, STURMIAN, 3.0),
    ];
    flames
        .iter()
        .map(|(title, coarse, fine, schedule, split)| {
            let mut params = serde_json::Map::new();
            for (key, v) in [("coarse", coarse), ("fine", fine), ("schedule", schedule), ("split", split), ("depth", &24.0)] {
                params.insert(format!("{name}.{key}"), serde_json::json!(v));
            }
            serde_json::json!({
                "flame": {
                    "custom_variations": [moran],
                    "name": title,
                    "transforms": [{
                        "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0,
                        "color": 0.0,
                        "color_speed": 0.0,
                        "direct_color": 1.0,
                        "opacity": 1.0,
                        "variation_params": params,
                        "variations": { name.clone(): 1.0 },
                        "weight": 1.0,
                    }],
                },
                "pan_x": 0.5,
                "pan_y": 0.5,
                "palette": palette,
                "render_mode": "2d",
                "version": 6,
                "zoom": 3.0,
            })
        })
        .collect()
}

/// Not a test: the presets, to splice into `assets/presets.fflame`
/// (`output/moran-presets.json`).
#[test]
#[ignore = "one-shot: writes the presets"]
fn write_presets() {
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/moran-presets.json", serde_json::to_string_pretty(&presets()).unwrap()).unwrap();
}

/// The bundled presets carry the variation as the example file has it --
/// an edit to the file shows here until the presets are written again
/// (`write_presets`) -- with their parameters, and the variation compiles.
#[test]
fn the_presets_carry_the_example() {
    let bundled = crate::resources::load_presets_with_fallback();
    for want in presets() {
        let name = want["flame"]["name"].as_str().unwrap();
        let have = bundled.iter().find(|c| c.flame.name == name).unwrap_or_else(|| panic!("no preset {name}"));
        let have_json = serde_json::to_value(&have.flame.custom_variations).unwrap();
        assert_eq!(have_json, want["flame"]["custom_variations"], "{name}: the variation -- write the presets again");
        let params = &have.flame.transforms[0].variation_params;
        for (k, v) in want["flame"]["transforms"][0]["variation_params"].as_object().unwrap() {
            let got = params.get(k).copied().unwrap_or(f32::NAN);
            assert!((got as f64 - v.as_f64().unwrap()).abs() < 1e-6, "{name}: {k} is {got} -- write the presets again");
        }
        // Compiled, not registered: the global registry is every test's,
        // and a variation left in it changes what others see.
        for src in &have.flame.custom_variations {
            crate::variations::custom::compile(&src.source).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
}

/// `n` points of the variation at the given parameters, each with the
/// colour it wrote, back in the file's own coordinates (+y up).
fn points(set: &[(&str, f64)], n: usize) -> Vec<(C64, f64)> {
    let src = example("variation-moran.rhai");
    let p = rhai::transformation(&src).unwrap_or_else(|e| panic!("{e:?}"));
    let mut params = ParamValues::defaults(&p);
    for (name, v) in set {
        let i = p.params.iter().position(|x| x.name == *name).unwrap_or_else(|| panic!("no parameter {name}"));
        params.values[i] = (*v, 0.0);
    }
    let mut run = VariationRun::new(&p, &params, 7);
    (0..n)
        .map(|_| {
            let (z, colour) = run.call(C64::new(0.0, 0.0), C64::new(0.0, 0.0), 1.0);
            (C64::new(z.re, 1.0 - z.im), colour.expect("a colour"))
        })
        .collect()
}

/// Each set alone (coarse and fine the same) is drawn in the unit square
/// and fills most of it, as the file says.
#[test]
fn every_set_is_drawn_in_the_unit_square() {
    for set in [TRIANGLE, CARPET, FERN, DRAGON, LEVY] {
        let pts = points(&[("coarse", set), ("fine", set)], 3000);
        let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for (z, colour) in &pts {
            assert!((0.0..=1.0).contains(colour), "set {set}: colour {colour}");
            lo = (lo.0.min(z.re), lo.1.min(z.im));
            hi = (hi.0.max(z.re), hi.1.max(z.im));
        }
        assert!(lo.0 >= -0.01 && lo.1 >= -0.01 && hi.0 <= 1.01 && hi.1 <= 1.01, "set {set}: {lo:?} to {hi:?}");
        assert!(hi.0 - lo.0 > 0.45 && hi.1 - lo.1 > 0.45, "set {set} fills the square: {lo:?} to {hi:?}");
    }
}

/// The inverse of the triangle's map `i`, and of the carpet's.
fn triangle_inverse(i: usize, z: C64) -> C64 {
    let o = [(0.0, 0.0), (0.5, 0.0), (0.25, 0.4330127)][i];
    C64::new((z.re - o.0) * 2.0, (z.im - o.1) * 2.0)
}

fn carpet_inverse(i: usize, z: C64) -> C64 {
    let n = if i >= 4 { i + 1 } else { i };
    C64::new(z.re * 3.0 - (n % 3) as f64, z.im * 3.0 - (n / 3) as f64)
}

/// Whether `z` is the image, under one map a level of `levels` (the
/// coarsest first, triangle or carpet), of a point in the unit square:
/// both sets map the square into itself, so every level on the way in
/// stays in it.
fn peels(z: C64, levels: &[f64]) -> bool {
    const TOL: f64 = 1e-9;
    let inside = |z: C64| (-TOL..=1.0 + TOL).contains(&z.re) && (-TOL..=1.0 + TOL).contains(&z.im);
    if !inside(z) {
        return false;
    }
    let Some((&set, rest)) = levels.split_first() else { return true };
    if set == TRIANGLE {
        (0..3).any(|i| peels(triangle_inverse(i, z), rest))
    } else {
        (0..8).any(|i| peels(carpet_inverse(i, z), rest))
    }
}

/// The levels compose with the coarsest outermost, each taking its set
/// from the schedule: a triangle-and-carpet point peels back through the
/// schedule's sets level by level, the coarsest level's map the one its
/// colour names. In the other order (a variation composing it would peel
/// every point) most do not: the sets' cells overlap, so some still do.
#[test]
fn the_levels_compose_coarsest_outermost() {
    let (t, c) = (TRIANGLE, CARPET);
    // The Sturmian schedule's first levels: the golden ratio's word.
    for (schedule, split, levels) in [
        (SPLIT, 2.0, [t, t, c, c, c]),
        (ALTERNATE, 0.0, [t, c, t, c, t]),
        (STURMIAN, 0.0, [t, c, t, t, c]),
    ] {
        let pts = points(&[("coarse", t), ("fine", c), ("schedule", schedule), ("split", split), ("depth", 12.0)], 400);
        for (z, colour) in &pts {
            let first = (colour * 3.0).floor() as usize;
            assert!(peels(triangle_inverse(first, *z), &levels[1..]), "schedule {schedule}: {z:?} (colour {colour}) does not peel as {levels:?}");
        }
        if schedule == SPLIT {
            let reversed = [c, c, c, t, t];
            let peeled = pts.iter().filter(|(z, _)| peels(*z, &reversed)).count();
            assert!(peeled * 2 < pts.len(), "the other order peels {peeled} of {}", pts.len());
        }
    }
}
