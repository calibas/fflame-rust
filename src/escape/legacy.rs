//! Fractint's Frothy Basin, Volterra-Lotka and Unity, written in Rhai
//! (`assets/formulas/frothy-basin.rhai` with its colouring,
//! `volterra-lotka.rhai`, `unity.rhai`): tests that each runs as Fractint
//! does. The references below are Xfractint 20.04p16's loops written out
//! in f64 (common/miscfrac.c `calcfroth`, common/fractals.c `VLfpFractal`
//! and `UnityfpFractal` under common/calcfrac.c's `StandardFractal`).

use super::lang::builtins::C64;
use super::lang::interp::{run, Env, ParamValues};
use super::lang::rhai;
use crate::storage::formula_store::EXAMPLES;

fn example(file: &str) -> &'static str {
    EXAMPLES.iter().find(|(f, _)| *f == file).map(|(_, t)| *t).unwrap_or_else(|| panic!("no example {file}"))
}

/// A grid of `w` by `h` pixel centres over a Fractint view (x from `x0`
/// to `x1`, y from `y0` to `y1`).
fn grid(w: usize, h: usize, x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<(f64, f64)> {
    (0..w * h).map(|i| (x0 + (x1 - x0) * ((i % w) as f64 + 0.5) / w as f64, y0 + (y1 - y0) * ((i / w) as f64 + 0.5) / h as f64)).collect()
}

/// Fractint's frothy-basin attractor test, floating-point path; `n` the
/// attractors its setup counts from the parameter as given (Fractint's
/// double: [`froth_attractors`]).
fn froth_found(x: f64, y: f64, a: f64, twice: bool, n: u32) -> u32 {
    const CLOSE: f64 = 1e-6;
    let slope = 3.0f64.sqrt();
    let f = |x: f64| x * x - x - 3.0 * a * a / 4.0;
    let t1 = f(0.5);
    let t2 = f(t1);
    let t3 = f(t2);
    let t4 = f(t3);
    let (y0, sin_t, cos_t) = (a / 2.0, slope / 2.0, -0.5);
    let l = |t: f64| t * cos_t - y0 * sin_t;
    let r = |t: f64| t * cos_t + y0 * sin_t;
    if (y0 - y).abs() < CLOSE && x >= t1 && x <= t2 {
        if (!twice && n == 2) || (twice && n == 3) {
            1
        } else if x <= t3 {
            1
        } else if x >= t4 {
            if !twice { 1 } else { 2 }
        } else {
            0
        }
    } else if (slope * x - a - y).abs() < CLOSE && x <= r(t1) && x >= r(t2) {
        if !twice && n == 2 {
            2
        } else if twice && n == 3 {
            3
        } else if x >= r(t3) {
            if !twice { 2 } else { 4 }
        } else if x <= r(t4) {
            if !twice { 3 } else { 6 }
        } else {
            0
        }
    } else if (-slope * x - a - y).abs() < CLOSE && x <= l(t1) && x >= l(t2) {
        if !twice && n == 2 {
            2
        } else if twice && n == 3 {
            2
        } else if x >= l(t3) {
            if !twice { 3 } else { 5 }
        } else if x <= l(t4) {
            if !twice { 2 } else { 3 }
        } else {
            0
        }
    } else {
        0
    }
}

/// `froth_setup`'s count of attractors for `a` as given.
fn froth_attractors(a: f64, twice: bool) -> u32 {
    if a.abs() <= 1.028713768218725 { if twice { 6 } else { 3 } } else if twice { 3 } else { 2 }
}

/// `calcfroth`'s loop: the passes and the attractor found (0, none); `a`
/// the map's (the f32 the program reads), `n` the attractors.
fn froth_reference(px: (f64, f64), a: f64, twice: bool, n: u32, rqlim: f64, maxit: u32) -> (u32, u32) {
    let (mut x, mut y) = px;
    let (mut iter, mut found) = (0u32, 0u32);
    while found == 0 && x * x + y * y < rqlim && iter < maxit {
        let nx = x * x - y * y - x - a * y;
        y += (x + x) * y - a * x;
        x = nx;
        if twice {
            let nx = x * x - y * y - x - a * y;
            y += (x + x) * y - a * x;
            x = nx;
        }
        iter += 1;
        found = froth_found(x, y, a, twice, n);
    }
    (iter, found)
}

/// `StandardFractal`'s count for an orbit function that returns true to
/// stop: the passes counted from 1, `maxit` for one that never stops.
fn standard(maxit: u32, mut orbit: impl FnMut() -> bool) -> u32 {
    let mut coloriter = 0;
    loop {
        coloriter += 1;
        if coloriter >= maxit {
            return maxit;
        }
        if orbit() {
            return coloriter;
        }
    }
}

/// Ours: the passes and whether the pixel escaped, by the interpreter
/// (the reference the GPU is tested against).
fn ours(text: &str, slots: Option<&[f32]>, env: &Env) -> (u32, bool, C64) {
    let p = rhai::program(text).unwrap_or_else(|d| panic!("{:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>()));
    let params = slots.map_or_else(|| ParamValues::defaults(&p), |s| ParamValues::from_slots(&p, s));
    let r = run(&p, &params, None, env);
    (r.n, r.escaped, r.z)
}

/// Frothy Basin runs as `calcfroth`: on Fractint's view, at its defaults
/// and at 6 basins and at a past the critical value (2 basins), every
/// pixel's count and attractor as Fractint's -- and the colouring gives
/// Fractint's colour, its band and shade.
#[test]
fn frothy_basin_runs_as_fractint() {
    let text = example("frothy-basin.rhai");
    let coloring = example("frothy-basin-coloring.rhai");
    let cp = rhai::coloring(coloring).expect("the colouring compiles");
    let fp = rhai::program(text).expect("the formula compiles");
    let maxit = 150;
    for (a, mapping, shade) in [(1.028713768218725f64, 0u32, 0u32), (1.028713768218725, 1, 0), (1.1, 0, 1), (0.9, 1, 1)] {
        let twice = mapping == 1;
        let fslots = [a as f32, mapping as f32, 7.0];
        let cslots = [a as f32, mapping as f32, shade as f32];
        let fparams = ParamValues::from_slots(&fp, &fslots);
        let cparams = ParamValues::from_slots(&cp, &cslots);
        let (mut found_any, mut checked) = (0, 0);
        for px in grid(64, 48, -2.8, 2.8, -2.355, 1.845) {
            // The program's `a` is the f32 slot's, as the GPU's.
            let n = froth_attractors(a, twice);
            let (iter, found) = froth_reference(px, a as f32 as f64, twice, n, 7.0, maxit);
            let r = run(&fp, &fparams, Some((&cp, &cparams)), &Env::at(C64::new(px.0, px.1), maxit));
            let escaped = found != 0 || iter < maxit;
            // A pixel that starts outside the bailout Fractint counts as 0
            // passes; the engine tests after a pass, so 1 -- either way
            // outside with no attractor, the solid colour.
            let iter_ours = iter.max(1);
            assert_eq!((r.n, r.escaped), (iter_ours, escaped), "a {a}, mapping {mapping}, pixel {px:?}");
            if found != 0 {
                found_any += 1;
                let bands = if n == 6 { 6 } else { 3 };
                let shades = 255 / bands;
                let count = if shade == 0 { iter.min(shades) } else { shades * iter / maxit };
                let colour = count.max(1) + shades * (found - 1);
                assert!(!r.solid && (r.index - colour as f64 / 256.0).abs() < 1e-9, "a {a}, pixel {px:?}: index {} against colour {colour}", r.index);
            } else if r.escaped {
                assert!(r.solid, "a {a}, pixel {px:?}: no attractor, a colour");
            }
            checked += 1;
        }
        println!("frothy basin, a {a}, mapping {mapping}: {checked} pixels, {found_any} on an attractor");
        // Most of Fractint's view leaves; the basins froth in the middle
        // (16% at the defaults, 3% at a 1.1 in 150 passes).
        assert!(found_any > 0, "pixels find an attractor: {found_any} of {checked}");
    }
}

/// Volterra-Lotka runs as `VLfpFractal` under `StandardFractal`: every
/// pixel's count on Fractint's view, at its defaults and at other h and
/// p (and clamped past 1).
#[test]
fn volterra_lotka_runs_as_fractint() {
    let text = example("volterra-lotka.rhai");
    let maxit = 150;
    for (h, p) in [(0.739f64, 0.739f64), (0.3, 0.9), (1.4, -0.2)] {
        let (hc, pc) = ((h as f32 as f64).clamp(0.0, 1.0), (p as f32 as f64).clamp(0.0, 1.0));
        let (mut out, mut checked) = (0, 0);
        for px in grid(64, 48, 0.0, 6.0, 0.0, 4.5) {
            let (mut x, mut y) = px;
            let count = standard(maxit, || {
                let half = hc / 2.0;
                let xy = x * y;
                let u = x - xy;
                let w = -y + xy;
                let a = x + pc * u;
                let b = y + pc * w;
                let ab = a * b;
                let (nx, ny) = (x + half * (u + (a - ab)), y + half * (w + (-b + ab)));
                x = nx;
                y = ny;
                x * x + y * y >= 256.0
            });
            let (n, escaped, _) = ours(text, Some(&[h as f32, p as f32, 256.0]), &Env::at(C64::new(px.0, px.1), maxit));
            if count < maxit {
                assert_eq!((n, escaped), (count, true), "h {h}, p {p}, pixel {px:?}");
                out += 1;
            } else {
                assert!(!escaped || n == maxit, "h {h}, p {p}, pixel {px:?}: Fractint's inside escaped at {n}");
            }
            checked += 1;
        }
        println!("volterra-lotka, h {h}, p {p}: {checked} pixels, {out} out");
        // At p 0 (clamped from below) the scheme leaves every pixel.
        assert!(out > 0 && (pc == 0.0 || out < checked), "both inside and outside: {out} of {checked}");
    }
}

/// Unity runs as `UnityfpFractal` under `StandardFractal`: every pixel's
/// count on Fractint's view, the unit circle's test at the pixel's size
/// (Fractint's `ddelmin`; the program's Rows, the view's height over it).
#[test]
fn unity_runs_as_fractint() {
    let text = example("unity.rhai");
    let maxit = 150;
    let (w, h) = (64usize, 48usize);
    for height in [48i32, 480] {
        // Fractint's view, 3 high: the program's pixel is 4 / (magn
        // height) with magn 4 / 3.
        let ddelmin = 3.0 / height as f64;
        let (mut circle, mut checked) = (0, 0);
        for px in grid(w, h, -2.0, 2.0, -1.5, 1.5) {
            let (mut x, mut y) = px;
            let count = standard(maxit, || {
                let r2 = x * x + y * y;
                if r2 > 2.0 || (r2 - 1.0).abs() < ddelmin {
                    return true;
                }
                y = (2.0 - r2) * x;
                x = (2.0 - r2) * y;
                false
            });
            let mut env = Env::at(C64::new(px.0, px.1), maxit);
            env.magn = 4.0 / 3.0;
            let (n, escaped, z) = ours(text, Some(&[height as f32]), &env);
            if count < maxit {
                assert_eq!((n, escaped), (count, true), "height {height}, pixel {px:?}");
                // The point tested is the one before the last map.
                let _ = z;
                circle += usize::from(count > 1 && x * x + y * y <= 2.0);
            } else {
                assert!(!escaped || n == maxit, "height {height}, pixel {px:?}");
            }
            checked += 1;
        }
        println!("unity at {height} rows: {checked} pixels, {circle} settled on the circle");
        assert!(circle > 0, "some orbits settle on the unit circle");
    }
}


/// Each deep-zooms by derived equations: the renderer gives it the custom
/// perturbation tier.
#[test]
fn each_takes_the_perturbed_path() {
    for file in ["frothy-basin.rhai", "volterra-lotka.rhai", "unity.rhai"] {
        let text = example(file);
        let src = crate::config::escape::CustomSource {
            dialect: crate::config::escape::CustomDialect::Rhai,
            entry: rhai::title(text).unwrap_or_default(),
            source: text.to_string(),
            variations: Vec::new(),
        };
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = super::custom::name_of(super::lang::EntryKind::Formula, &src);
        esc.custom_formula = Some(src);
        esc.zoom_log2 = 30.0;
        super::custom::ensure(&esc);
        let tier = super::EscapeRenderer::perturb_tier(&esc);
        println!("{file}: {tier:?}");
        assert!(matches!(tier, Some(super::assembler::PerturbTier::Custom { .. })), "{file}: {tier:?}");
    }
}


/// Not a test: boundary points for the deep-zoom views, by bisection with
/// Fractint's loops in f64 between two points of a different kind --
/// Volterra-Lotka in and out, Unity escaping and settling on the circle,
/// Frothy Basin landing and leaving. Printed to paste into
/// `the_fractint_legacy_formulas_deep_zoom`.
#[test]
#[ignore = "one-shot: prints the deep-zoom views"]
fn print_boundary_views() {
    fn bisect(mut a: (f64, f64), mut b: (f64, f64), kind: &dyn Fn((f64, f64)) -> bool) -> (f64, f64) {
        assert_ne!(kind(a), kind(b), "the ends differ");
        for _ in 0..80 {
            let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            if kind(m) == kind(a) {
                a = m;
            } else {
                b = m;
            }
        }
        a
    }
    let vl = |px: (f64, f64)| {
        let (mut x, mut y) = px;
        let hc = 0.739f32 as f64;
        standard(300, || {
            let half = hc / 2.0;
            let xy = x * y;
            let (u, w) = (x - xy, -y + xy);
            let (a, b) = (x + hc * u, y + hc * w);
            let ab = a * b;
            let (nx, ny) = (x + half * (u + (a - ab)), y + half * (w + (-b + ab)));
            x = nx;
            y = ny;
            x * x + y * y >= 256.0
        }) < 300
    };
    let unity = |px: (f64, f64)| {
        let (mut x, mut y) = px;
        let tol = 3.0 / 480.0;
        let mut settled = false;
        standard(300, || {
            let r2 = x * x + y * y;
            if r2 > 2.0 {
                return true;
            }
            if (r2 - 1.0).abs() < tol {
                settled = true;
                return true;
            }
            y = (2.0 - r2) * x;
            x = (2.0 - r2) * y;
            false
        });
        settled
    };
    let a = 1.028713768218725f32 as f64;
    let froth = |px: (f64, f64)| froth_reference(px, a, false, 3, 7.0, 300).1 != 0;
    // Search a row for neighbours of a different kind.
    let find = |name: &str, kind: &dyn Fn((f64, f64)) -> bool, x0: f64, x1: f64, y: f64| {
        let n = 400;
        let mut prev = (x0, y);
        for i in 1..=n {
            let p = (x0 + (x1 - x0) * i as f64 / n as f64, y);
            if kind(p) != kind(prev) {
                let c = bisect(prev, p, kind);
                println!("{name}: (\"{:.17}\", \"{:.17}\")", c.0, c.1);
                return;
            }
            prev = p;
        }
        println!("{name}: none on the row");
    };
    // Unity on its escaping side: between escape counts, both escaping by
    // r^2 > 2 -- its circle test asks for |r^2 - 1| under a pixel, which
    // no f64 resolves at depth.
    let unity_count = |px: (f64, f64)| {
        let (mut x, mut y) = px;
        let tol = 3.0 / 480.0;
        let mut settled = false;
        let n = standard(300, || {
            let r2 = x * x + y * y;
            if r2 > 2.0 {
                return true;
            }
            if (r2 - 1.0).abs() < tol {
                settled = true;
                return true;
            }
            y = (2.0 - r2) * x;
            x = (2.0 - r2) * y;
            false
        });
        (n, settled)
    };
    {
        let n = 2000;
        let (x0, x1, y) = (-1.9, 1.9, 0.13);
        let mut prev = (x0, y);
        for i in 1..=n {
            let p = (x0 + (x1 - x0) * i as f64 / n as f64, y);
            let ((ca, sa), (cb, sb)) = (unity_count(prev), unity_count(p));
            if ca != cb && !sa && !sb && ca > 3 && cb > 3 {
                let k = ca.min(cb);
                let c = bisect(prev, p, &|q| unity_count(q).0 <= k);
                println!("unity, escaping: (\"{:.17}\", \"{:.17}\") between counts {ca} and {cb}", c.0, c.1);
                break;
            }
            prev = p;
        }
    }
    find("volterra-lotka", &vl, 0.3, 5.7, 2.0);
    find("unity", &unity, -1.9, 1.9, 0.37);
    find("frothy-basin", &froth, -2.0, 2.0, -0.6);
}

/// The three as presets (`assets/presets.fflame`): each carries its
/// formula -- and Frothy Basin its colouring -- as the example file has
/// it, under the names those sources hash to.
fn presets() -> Vec<serde_json::Value> {
    use crate::config::escape::{CustomDialect, CustomSource};
    use super::lang::EntryKind;
    let source = |file: &str| {
        let text = example(file);
        CustomSource { dialect: CustomDialect::Rhai, entry: rhai::title(text).unwrap_or_default(), source: text.to_string(), variations: Vec::new() }
    };
    let stops = |s: &[(f32, [f32; 3])]| serde_json::Value::Array(s.iter().map(|(p, c)| serde_json::json!({ "color": c, "position": p })).collect());
    // Three ramps, a basin each: bright where an orbit lands fast, darker
    // as it takes longer (Fractint's froth3 map's idea, in our colours).
    let froth = stops(&[
        (0.0, [0.0, 0.0, 0.0]),
        (0.004, [0.98, 0.16, 0.10]),
        (0.332, [0.30, 0.02, 0.02]),
        (0.336, [0.22, 0.95, 0.32]),
        (0.664, [0.02, 0.28, 0.05]),
        (0.668, [0.25, 0.48, 1.0]),
        (1.0, [0.02, 0.05, 0.32]),
    ]);
    let tide = stops(&[(0.0, [0.02, 0.05, 0.12]), (0.3, [0.05, 0.45, 0.55]), (0.6, [0.95, 0.85, 0.45]), (0.8, [0.85, 0.35, 0.12]), (1.0, [0.02, 0.05, 0.12])]);
    let ring = stops(&[(0.0, [0.05, 0.0, 0.12]), (0.35, [0.45, 0.15, 0.75]), (0.65, [0.3, 0.9, 0.95]), (1.0, [0.05, 0.0, 0.12])]);
    let cases: [(&str, &str, Option<&str>, (f64, f64, f64), f32, serde_json::Value, &str); 3] = [
        ("Frothy Basin", "frothy-basin.rhai", Some("frothy-basin-coloring.rhai"), (0.0, -0.255, 4.2), 7.0, froth, "Froth"),
        ("Volterra-Lotka", "volterra-lotka.rhai", None, (3.0, 2.25, 4.5), 256.0, tide, "Tide"),
        ("Unity", "unity.rhai", None, (0.0, 0.0, 3.0), 4.0, ring, "Ring"),
    ];
    cases
        .into_iter()
        .map(|(name, file, coloring, (cx, cy, span), bailout, palette, palette_name)| {
            let f = source(file);
            let mut esc = serde_json::json!({
                "bailout": bailout,
                "center_re": format!("{cx}"),
                "center_im": format!("{cy}"),
                "formula": super::custom::name_of(EntryKind::Formula, &f),
                "custom_formula": f,
                "coloring": "escape_count",
                "max_iter": 150,
                "zoom_log2": (4.0 / span).log2(),
            });
            if let Some(cf) = coloring {
                let c = source(cf);
                esc["coloring"] = serde_json::json!(super::custom::name_of(EntryKind::Coloring, &c));
                esc["custom_coloring"] = serde_json::json!(c);
            }
            serde_json::json!({
                "escape": esc,
                "flame": { "name": name, "transforms": [] },
                "palette": { "locked": false, "name": palette_name, "stops": palette },
                "preserve_z": false,
                "render_mode": "escape",
                "tonemap_mode": "linear",
                "version": 6,
            })
        })
        .collect()
}

/// Not a test: the three presets, to splice into `assets/presets.fflame`
/// (`output/legacy-presets.json`).
#[test]
#[ignore = "one-shot: writes the presets"]
fn write_presets() {
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/legacy-presets.json", serde_json::to_string_pretty(&presets()).unwrap()).unwrap();
}

/// The bundled presets carry the three as the example files have them:
/// an edit to a file shows here until the presets are written again
/// (`write_presets`). Each loads, and names a formula the renderer has.
#[test]
fn the_presets_carry_the_examples() {
    let bundled = crate::resources::load_presets_with_fallback();
    for want in presets() {
        let name = want["flame"]["name"].as_str().unwrap();
        let have = bundled.iter().find(|c| c.flame.name == name).unwrap_or_else(|| panic!("no preset {name}"));
        let have_json = serde_json::to_value(&have.escape).unwrap();
        for key in ["formula", "custom_formula", "coloring", "custom_coloring"] {
            assert_eq!(have_json.get(key), want["escape"].get(key), "{name}: {key} -- write the presets again");
        }
        super::custom::ensure(&have.escape);
        assert!(super::find_formula(&have.escape.formula).is_some(), "{name}: its formula compiles");
    }
}
