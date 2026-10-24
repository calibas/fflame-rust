//! The Bristorbrot and the Quaternionbrot (docs/projects/bristorbrot.md):
//! the example solids (`assets/formulas/solid-bristorbrot.rhai`,
//! `solid-quaternionbrot.rhai`) against their algebras written here in
//! f64 -- the Bristorian product from its rule, not from the script's
//! expanded parts -- their march against sampling, and the presets,
//! written from the files and kept to them.

use super::lang::interp::{self, ParamValues};
use super::lang::ir::Program;
use super::lang::{rhai, EntryKind};
use crate::config::escape::{CustomDialect, CustomSource};
use crate::storage::formula_store::EXAMPLES;

/// Four parts: real, then e1, e2, e3 (i, j, k).
type Q = [f64; 4];

/// Bristor's rule (bristorbrot.org): 1 the identity, e_a e_a = -1, and
/// e_a e_b = sgn(b - a) e_b for a != b, extended bilinearly.
fn bristor(a: Q, b: Q) -> Q {
    let mut out = [0.0; 4];
    for (i, ai) in a.iter().enumerate() {
        for (j, bj) in b.iter().enumerate() {
            let v = ai * bj;
            match (i, j) {
                (0, _) => out[j] += v,
                (_, 0) => out[i] += v,
                _ if i == j => out[0] -= v,
                _ => out[j] += if j > i { v } else { -v },
            }
        }
    }
    out
}

fn hamilton(a: Q, b: Q) -> Q {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn add(a: Q, b: Q) -> Q {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]
}

fn scale(a: Q, s: f64) -> Q {
    a.map(|x| x * s)
}

fn norm(a: Q) -> f64 {
    a.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// The two algebras' maps, without c, and their derivatives along `v`
/// (the product rule, which needs no associativity).
#[derive(Clone, Copy, Debug)]
enum Map {
    /// The Bristorian `z^2`, `(zz)z`, `z(zz)` (the file's Formula 0, 1, 2).
    Bristor(u8),
    /// Hamilton's `q^n` for a whole `n`.
    Quat(u32),
}

impl Map {
    fn f(self, z: Q) -> Q {
        match self {
            Map::Bristor(0) => bristor(z, z),
            Map::Bristor(1) => bristor(bristor(z, z), z),
            Map::Bristor(_) => bristor(z, bristor(z, z)),
            Map::Quat(n) => (1..n).fold(z, |q, _| hamilton(q, z)),
        }
    }

    fn along(self, z: Q, v: Q) -> Q {
        match self {
            Map::Bristor(form) => {
                let sq = add(bristor(z, v), bristor(v, z));
                match form {
                    0 => sq,
                    1 => add(bristor(sq, z), bristor(bristor(z, z), v)),
                    _ => add(bristor(v, bristor(z, z)), bristor(z, sq)),
                }
            }
            Map::Quat(_) => unreachable!("the Quaternionbrot carries a scalar derivative"),
        }
    }
}

/// The window into the four dimensions: (real, i, cos a j + sin a k), the
/// hidden axis carrying the offset `w`.
fn section(pos: [f64; 3], roll_deg: f64, w: f64) -> Q {
    let (s, c) = roll_deg.to_radians().sin_cos();
    [pos[0], pos[1], c * pos[2] - s * w, s * pos[2] + c * w]
}

/// One set, as the files define them.
#[derive(Clone, Copy, Debug)]
struct Set {
    map: Map,
    julia: bool,
    c: Q,
    roll: f64,
    w: f64,
    /// The Bristorbrot's Distance "Bristor's" (the default): the running
    /// derivative; else the Jacobian's Frobenius norm.
    scalar: bool,
}

const BAIL: f64 = 1.0e4;
/// The longest reading the files give: a tenth of their ball.
const CAP: f64 = 0.25;

impl Set {
    /// Whether `pos` stays within the bailout for `levels` steps, and the
    /// distance the file gives: the Bristorbrot's over the Frobenius norm
    /// of the orbit's Jacobian along the window's axes, the
    /// Quaternionbrot's over its running derivative, at most `CAP`. In
    /// f64, unscaled.
    fn run(&self, pos: [f64; 3], levels: u32) -> (bool, f64) {
        let mut z = section(pos, self.roll, self.w);
        let c = if self.julia { self.c } else { z };
        let k = if self.julia { 0.0 } else { 1.0 };
        let axes = [section([1.0, 0.0, 0.0], self.roll, 0.0), section([0.0, 1.0, 0.0], self.roll, 0.0), section([0.0, 0.0, 1.0], self.roll, 0.0)];
        let mut m = axes;
        let mut dr = 1.0;
        for _ in 0..levels {
            match self.map {
                Map::Bristor(_) => {
                    for (mi, ai) in m.iter_mut().zip(&axes) {
                        *mi = add(self.map.along(z, *mi), scale(*ai, k));
                    }
                }
                Map::Quat(_) => {}
            }
            // The running derivative: the Quaternionbrot's, and Bristor's.
            let n = match self.map {
                Map::Bristor(0) | Map::Quat(2) => 2,
                _ => 3,
            };
            dr = n as f64 * norm(z).powi(n - 1) * dr + k;
            z = add(self.map.f(z), c);
            if norm(z) > BAIL {
                break;
            }
        }
        let r = norm(z);
        if r <= BAIL {
            return (true, 0.0);
        }
        let stretch = match self.map {
            Map::Bristor(_) if !self.scalar => m.iter().map(|x| x.iter().map(|v| v * v).sum::<f64>()).sum::<f64>().sqrt(),
            _ => dr,
        };
        (false, (0.5 * r * r.ln() / stretch.max(1e-30)).min(CAP))
    }

    /// The file and its parameters for this set.
    fn solid(&self) -> (Program, ParamValues) {
        let file = match self.map {
            Map::Bristor(_) => "solid-bristorbrot.rhai",
            Map::Quat(_) => "solid-quaternionbrot.rhai",
        };
        let src = example(file);
        let p = rhai::solid(&src).unwrap_or_else(|d| panic!("{file}: {:?}", d.iter().map(|x| x.render(&src)).collect::<Vec<_>>()));
        let mut params = ParamValues::defaults(&p);
        let mut set = |name: &str, v: f64| {
            let i = p.params.iter().position(|x| x.name == name).unwrap_or_else(|| panic!("{file}: no parameter {name}"));
            params.values[i] = (v, 0.0);
        };
        match self.map {
            Map::Bristor(form) => set("formula", form as f64),
            Map::Quat(n) => set("power", n as f64),
        }
        set("julia", if self.julia { 1.0 } else { 0.0 });
        for (name, v) in ["c_w", "c_x", "c_y", "c_z"].iter().zip(self.c) {
            set(name, v);
        }
        set("roll", self.roll);
        set("w", self.w);
        if matches!(self.map, Map::Bristor(_)) {
            set("estimate", if self.scalar { 0.0 } else { 1.0 });
        }
        (p, params)
    }
}

/// An example file's text, with LF line ends whatever the checkout gave
/// it: a source's name is its hash.
fn example(file: &str) -> String {
    EXAMPLES.iter().find(|(f, _)| *f == file).map(|(_, t)| t.replace("\r\n", "\n")).unwrap_or_else(|| panic!("no example {file}"))
}

/// The sets the tests walk: each formula and power, Mandelbrot and Julia,
/// the 3D set and a section rolled and offset; the Julia constants
/// Bristor's viewer starts from.
fn sets() -> Vec<Set> {
    let quad: Q = [-0.7, -0.35, 0.05, 0.0];
    let cube: Q = [0.15, 0.4, 0.05, 0.0];
    let mut out = Vec::new();
    for (map, c) in [(Map::Bristor(0), quad), (Map::Bristor(1), cube), (Map::Bristor(2), cube), (Map::Quat(2), [-0.2, 0.65, 0.0, 0.0]), (Map::Quat(3), [0.35, 0.62, 0.0, 0.0])] {
        for julia in [false, true] {
            for (roll, w) in [(0.0, 0.0), (27.0, 0.15)] {
                out.push(Set { map, julia, c, roll, w, scalar: true });
            }
        }
    }
    out
}

/// Bristor's witnesses (bristorbrot.org): the rule is not commutative
/// (`i j = j`, `j i = -i`) nor associative, `(-1, 1, 2)` squares to
/// `(-4, -4, -2)` (Hamilton's square is `(-4, -2, -4)`), and at `i + j`
/// the cubes are `-3i - 3j` and `-i - j`. And the file's product, run,
/// is the rule's.
#[test]
fn bristors_rule_gives_his_witnesses() {
    let (i, j) = ([0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(bristor(i, j), j);
    assert_eq!(bristor(j, i), scale(i, -1.0));
    assert_eq!(bristor(bristor(i, i), j), scale(j, -1.0));
    assert_eq!(bristor(i, bristor(i, j)), j);
    let z = [-1.0, 1.0, 2.0, 0.0];
    assert_eq!(bristor(z, z), [-4.0, -4.0, -2.0, 0.0]);
    assert_eq!(hamilton(z, z), [-4.0, -2.0, -4.0, 0.0]);
    let z = add(i, j);
    assert_eq!(Map::Bristor(1).f(z), [0.0, -3.0, -3.0, 0.0]);
    assert_eq!(Map::Bristor(2).f(z), [0.0, -1.0, -1.0, 0.0]);
    // The 3D Bristorbrot's square, the file's parts written out.
    let z = [0.3, -0.7, 1.1, 0.0];
    let (x, y, w) = (z[0], z[1], z[2]);
    assert_eq!(bristor(z, z), [x * x - y * y - w * w, y * (2.0 * x - w), w * (2.0 * x + y), 0.0]);
}

/// The solids run as their algebras say: at points in and out of each
/// set, the file's estimate is the one written here from the rule (and
/// Hamilton's product) -- none in the set, the estimate out of it.
#[test]
fn the_solids_run_as_their_algebras() {
    let jacobian = sets().into_iter().filter(|s| matches!(s.map, Map::Bristor(_))).map(|s| Set { scalar: false, ..s });
    for set in sets().into_iter().chain(jacobian) {
        let (p, params) = set.solid();
        let (mut held, mut out) = (0, 0);
        let centre = if set.julia { 0.0 } else { -0.5 };
        for k in 0..512 {
            let pos = [centre + 1.4 * ((k % 8) as f64 / 3.5 - 1.0), 1.4 * ((k / 8 % 8) as f64 / 3.5 - 1.0), 1.4 * ((k / 64) as f64 / 3.5 - 1.0)];
            let (inside, want) = set.run(pos, 24);
            let got = interp::solid(&p, &params, pos, 24);
            if inside {
                held += 1;
                assert_eq!(got, 0.0, "{set:?} at {pos:?}: in the set, but {got}");
            } else {
                out += 1;
                assert!((got - want).abs() <= 1e-6 * want.abs(), "{set:?} at {pos:?}: {got} against {want}");
            }
        }
        assert!(held > 0 && out > 0, "{set:?}: the points hold both ({held} in)");
    }
}

/// Rays around `set`, each marched by the file's estimate as mode D's
/// near march is -- a hit where the reading is under a hair, and a step
/// that lands in the set bisected back to where it entered
/// (`ifs_refine`) -- against the first point along it (two thousandths
/// apart) that stays: how many rays met the set, and those the march
/// carried through a part of it and out. The march reads `levels`
/// iterations.
fn march_through(set: &Set, rays: usize) -> (usize, Vec<String>) {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    const LEVELS: u32 = 24;
    const SAMPLE: f64 = 0.002;
    let (p, params) = set.solid();
    let centre = if set.julia { 0.0 } else { -0.5 };
    let (mut traced, mut through) = (0, Vec::new());
    for _ in 0..rays {
        // From around the set, at a point near its core.
        let o = [centre + (next() * 2.0 - 1.0) * 1.6, (next() * 2.0 - 1.0) * 1.6, (next() * 2.0 - 1.0) * 1.6];
        let aim = [centre + (next() - 0.5) * 0.6, (next() - 0.5) * 0.6, (next() - 0.5) * 0.6];
        let mut d = [aim[0] - o[0], aim[1] - o[1], aim[2] - o[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        d = d.map(|x| x / len);
        let at = |t: f64| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
        if set.run(o, LEVELS).0 {
            continue;
        }
        let mut first = f64::INFINITY;
        let mut t = 0.0;
        while t < 4.0 {
            if set.run(at(t), LEVELS).0 {
                first = t;
                break;
            }
            t += SAMPLE;
        }
        if !first.is_finite() {
            continue;
        }
        traced += 1;
        let mut t = 0.0;
        let mut before = 0.0;
        let mut hit = f64::INFINITY;
        for _ in 0..4000 {
            let e = interp::solid(&p, &params, at(t), LEVELS);
            if e <= 0.0 {
                let (mut lo, mut hi) = (before, t);
                for _ in 0..40 {
                    let mid = 0.5 * (lo + hi);
                    if set.run(at(mid), LEVELS).0 {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                hit = hi;
                break;
            }
            if e < 1e-4 {
                hit = t;
                break;
            }
            before = t;
            t += e;
            if t > 4.0 {
                break;
            }
        }
        if hit > first + SAMPLE {
            through.push(format!("from {o:?} along {d:?} the march stopped at {hit}, past the set at {first}"));
        }
    }
    (traced, through)
}

/// Marched by its estimate, no ray passes through a part of either set:
/// each formula and power, Mandelbrot and Julia, the 3D set and a rolled,
/// offset section, the Bristorbrot by either Distance. (Uncapped,
/// Bristor's reading carried rays through its Julia sets; the cap is what
/// this holds.)
#[test]
fn neither_is_stepped_through() {
    let jacobian = sets().into_iter().filter(|s| matches!(s.map, Map::Bristor(_))).map(|s| Set { scalar: false, ..s });
    for set in sets().into_iter().chain(jacobian) {
        let (traced, through) = march_through(&set, 60);
        assert!(traced > 8, "{set:?}: rays that meet the set: {traced}");
        assert!(through.is_empty(), "{set:?}: {}", through.join("; "));
    }
}

/// A preset: its name, the file, the parameters (besides `levels`, the
/// iterations), the camera (pitch, yaw, zoom stops, the target's x) and
/// the colouring.
struct Preset {
    name: &'static str,
    file: &'static str,
    params: &'static [(&'static str, f64)],
    levels: f64,
    camera: (f32, f32, f64, &'static str),
    coloring: &'static str,
}

/// The presets: Bristor's sets at his viewer's constants -- the 3D
/// Bristorbrot, the square's Julia set at his README's picture (C =
/// (-0.7, -0.35, 0.05, 0), rolled 27 degrees), the two cubes' Julia sets,
/// and the quaternion sets beside them, q^2 + c and q^3 + c, Mandelbrot
/// and Julia. (A quaternion set offset along the hidden axis is only
/// smaller: every turn about the real axis is a symmetry.)
const PRESETS: &[Preset] = &[
    Preset {
        name: "Bristorbrot",
        file: "solid-bristorbrot.rhai",
        params: &[("formula", 0.0), ("julia", 0.0)],
        levels: 24.0,
        camera: (0.3, 1.6, 0.3, "-0.5"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Bristorbrot Julia",
        file: "solid-bristorbrot.rhai",
        params: &[("formula", 0.0), ("julia", 1.0), ("c_w", -0.7), ("c_x", -0.35), ("c_y", 0.05), ("c_z", 0.0), ("roll", 27.0)],
        levels: 48.0,
        camera: (0.6, 2.2, 0.6, "0"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Bristorbrot Julia, (zz)z",
        file: "solid-bristorbrot.rhai",
        params: &[("formula", 1.0), ("julia", 1.0), ("c_w", 0.15), ("c_x", 0.4), ("c_y", 0.05), ("c_z", 0.0), ("roll", 27.0)],
        levels: 48.0,
        camera: (0.6, 2.2, 0.6, "0"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Bristorbrot Julia, z(zz)",
        file: "solid-bristorbrot.rhai",
        params: &[("formula", 2.0), ("julia", 1.0), ("c_w", 0.15), ("c_x", 0.4), ("c_y", 0.05), ("c_z", 0.0), ("roll", 27.0)],
        levels: 48.0,
        camera: (0.6, 2.2, 0.6, "0"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Quaternionbrot",
        file: "solid-quaternionbrot.rhai",
        params: &[("power", 2.0), ("julia", 0.0)],
        levels: 24.0,
        camera: (0.3, 1.6, 0.3, "-0.5"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Quaternionbrot Julia",
        file: "solid-quaternionbrot.rhai",
        params: &[("power", 2.0), ("julia", 1.0), ("c_w", -0.2), ("c_x", 0.65), ("c_y", 0.0), ("c_z", 0.0), ("roll", 27.0)],
        levels: 48.0,
        camera: (0.6, 2.2, 0.6, "0"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Quaternionbrot Cubed",
        file: "solid-quaternionbrot.rhai",
        params: &[("power", 3.0), ("julia", 0.0)],
        levels: 24.0,
        camera: (0.3, 1.6, 0.5, "0"),
        coloring: "ifs_level",
    },
    Preset {
        name: "Quaternionbrot Cubed Julia",
        file: "solid-quaternionbrot.rhai",
        params: &[("power", 3.0), ("julia", 1.0), ("c_w", 0.35), ("c_x", 0.62), ("c_y", 0.0), ("c_z", 0.0), ("roll", 27.0)],
        levels: 48.0,
        camera: (0.6, 2.2, 0.6, "0"),
        coloring: "ifs_level",
    },
];

/// The presets as configs, on the Quaternion Julia preset's 3D look.
fn presets() -> Vec<serde_json::Value> {
    let bundled: Vec<serde_json::Value> = serde_json::from_str(crate::resources::presets::PRESETS_JSON).expect("the presets parse");
    let template = bundled.iter().find(|c| c["flame"]["name"] == "Quaternion Julia").expect("the Quaternion Julia preset").clone();
    PRESETS
        .iter()
        .map(|preset| {
            let text = example(preset.file);
            let solid = CustomSource { dialect: CustomDialect::Rhai, entry: rhai::title(&text).unwrap_or_default(), source: text, variations: Vec::new() };
            let name = super::custom::name_of(EntryKind::Formula, &solid);
            let mut params = serde_json::Map::new();
            for (k, v) in preset.params {
                params.insert(k.to_string(), serde_json::json!(v));
            }
            params.insert("levels".to_string(), serde_json::json!(preset.levels));
            let (pitch, yaw, zoom_log2, target_x) = preset.camera;
            let mut p = template.clone();
            p["flame"]["name"] = serde_json::json!(preset.name);
            p["escape"] = serde_json::json!({
                "cam_pitch": pitch,
                "cam_target_x": target_x,
                "cam_yaw": yaw,
                "coloring": preset.coloring,
                "custom_formula": solid,
                "formula": name,
                "formula_params": params,
                "max_iter": 256,
                "zoom_log2": zoom_log2,
            });
            p
        })
        .collect()
}

/// Not a test: the presets, to splice into `assets/presets.fflame`
/// (`output/bristor-presets.json`).
#[test]
#[ignore = "one-shot: writes the presets"]
fn write_presets() {
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/bristor-presets.json", serde_json::to_string_pretty(&presets()).unwrap()).unwrap();
}

/// The bundled presets carry the solids as the example files have them --
/// an edit to a file shows here until the presets are written again
/// (`write_presets`) -- with their parameters, and each names a solid the
/// renderer has.
#[test]
fn the_presets_carry_the_examples() {
    let bundled = crate::resources::load_presets_with_fallback();
    for want in presets() {
        let name = want["flame"]["name"].as_str().unwrap();
        let have = bundled.iter().find(|c| c.flame.name == name).unwrap_or_else(|| panic!("no preset {name}"));
        let have_json = serde_json::to_value(&have.escape).unwrap();
        for key in ["formula", "custom_formula"] {
            assert_eq!(have_json.get(key), want["escape"].get(key), "{name}: {key} -- write the presets again");
        }
        for (k, v) in want["escape"]["formula_params"].as_object().unwrap() {
            let got = have.escape.formula_params.get(k).copied().unwrap_or(f32::NAN);
            assert!((got as f64 - v.as_f64().unwrap()).abs() < 1e-6, "{name}: {k} is {got} -- write the presets again");
        }
        super::custom::ensure(&have.escape);
        assert!(super::ifs::get_ifs(&have.escape.formula).is_some(), "{name}: its solid compiles");
    }
}
