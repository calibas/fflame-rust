//! Halayka's quaternion sets and escape paths (docs/projects/quaternions.md):
//! the presets that show them, written from the example files
//! (`assets/formulas/solid-halayka-quaternions.rhai`,
//! `variation-escape-paths.rhai`), and the test that keeps the bundled
//! presets carrying the files as they are. The examples' own tests -- the
//! sets against the paper's definition, the paths against their orbits --
//! are with the quaternions' (`lang::rhai::vec`).

use super::lang::{rhai, EntryKind};
use crate::config::escape::{CustomDialect, CustomSource};
use crate::storage::formula_store::EXAMPLES;

/// An example file's text, with LF line ends whatever the checkout gave
/// it: a source's name is its hash.
fn example(file: &str) -> String {
    EXAMPLES.iter().find(|(f, _)| *f == file).map(|(_, t)| t.replace("\r\n", "\n")).unwrap_or_else(|| panic!("no example {file}"))
}

/// Halayka's colours (2018: `HSBtoRGB(300 t, 75, 100)`), as stops: hue
/// from 360 down to 60 degrees at three quarters' saturation, the path's
/// start at the palette's.
fn halayka_palette() -> serde_json::Value {
    let stops: Vec<serde_json::Value> = (0..=10)
        .map(|k| {
            let t = k as f32 / 10.0;
            let h = ((360.0 - 300.0 * t) / 60.0).rem_euclid(6.0);
            let (s, v) = (0.75f32, 1.0f32);
            let c = v * s;
            let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
            let (r, g, b) = match h as u32 {
                0 => (c, x, 0.0),
                1 => (x, c, 0.0),
                2 => (0.0, c, x),
                3 => (0.0, x, c),
                4 => (x, 0.0, c),
                _ => (c, 0.0, x),
            };
            let m = v - c;
            serde_json::json!({ "color": [r + m, g + m, b + m], "position": t })
        })
        .collect();
    serde_json::json!({ "locked": false, "name": "Halayka", "stops": stops })
}

/// The presets: six of Halayka's (2009) sets as the Rhai solid, at the
/// paper's figures' settings (8 iterations, threshold 4), the two drawn as
/// complements in the paper as their surface, coloured by the step each
/// piece of the surface comes from; and two of his (2018) escape paths, a
/// 3D flame of the Rhai variation each.
fn presets() -> Vec<serde_json::Value> {
    let bundled: Vec<serde_json::Value> = serde_json::from_str(crate::resources::presets::PRESETS_JSON).expect("the presets parse");
    // The quaternion Julia's: the 3D look, its rig.
    let template = bundled.iter().find(|c| c["flame"]["name"] == "Quaternion Julia").expect("the Quaternion Julia preset").clone();
    let text = example("solid-halayka-quaternions.rhai");
    let solid = CustomSource { dialect: CustomDialect::Rhai, entry: rhai::title(&text).unwrap_or_default(), source: text, variations: Vec::new() };
    let solid_name = super::custom::name_of(EntryKind::Formula, &solid);
    let c1 = [0.3, 0.5, 0.4, 0.2];
    let c2 = [1.0, 1.0, 1.0, 1.0];
    // (name, step, C, draw, yaw, pitch): figures 3 to 8.
    let sets: [(&str, f64, [f64; 4], f64, f32, f32); 6] = [
        ("Halayka: sin Z + C sin Z", 1.0, c1, 0.0, 0.6, 0.35),
        ("Halayka: sin Z + C sin Z, Second C", 1.0, [-0.3, 0.5, 0.7, 0.2], 0.0, 0.6, 0.35),
        ("Halayka: cos Z + C cos Z", 2.0, c1, 0.0, 0.6, 0.35),
        ("Halayka: exp(Z^2) + C", 3.0, c1, 2.0, -0.8, 0.9),
        ("Halayka: inv(sinh Z) + C inv(sinh Z)", 4.0, c2, 0.0, -0.8, 0.9),
        ("Halayka: C inv(sinh Z) cosh Z", 5.0, c2, 2.0, 0.6, 0.35),
    ];
    let mut out: Vec<serde_json::Value> = sets
        .iter()
        .map(|(name, step, c, draw, yaw, pitch)| {
            let mut p = template.clone();
            p["flame"]["name"] = serde_json::json!(name);
            p["palette"] = halayka_palette();
            // Some ambient: the sets' rings and sheets face every way, and
            // one light leaves most of them dark.
            p["solid_shading"]["ambient"] = serde_json::json!(0.3);
            // Coloured by the solid's own `address`: the step each piece
            // of the surface comes from (its Colour, "Nearest step").
            p["escape"] = serde_json::json!({
                "cam_pitch": pitch,
                "cam_yaw": yaw,
                "coloring": "ifs_address",
                "custom_formula": solid,
                "formula": solid_name,
                "formula_params": {
                    "c_w": c[0], "c_x": c[1], "c_y": c[2], "c_z": c[3],
                    "draw": draw,
                    "levels": 8.0,
                    "step": step,
                },
                "max_iter": 256,
                "zoom_log2": -0.3,
            });
            p
        })
        .collect();

    let text = example("variation-escape-paths.rhai");
    let name = rhai::variation_name(&text).expect("the variation's name");
    let paths = CustomSource { dialect: CustomDialect::Rhai, entry: name.clone(), source: text, variations: Vec::new() };
    // The 2018 paper's figures 1 and 5.
    for (title, step, power, zoom) in [("Halayka Escape Paths: Z^2 + C", 0.0, 2.0, 1.2), ("Halayka Escape Paths: sin Z + C sin Z", 1.0, 2.0, 0.75)] {
        let mut params = serde_json::Map::new();
        params.insert(format!("{name}.step"), serde_json::json!(step));
        params.insert(format!("{name}.power"), serde_json::json!(power));
        out.push(serde_json::json!({
            "background_color": [0.0, 0.0, 0.0],
            "camera_rotation_x": 0.45,
            "camera_rotation_y": 0.6,
            "flame": {
                "custom_variations": [paths],
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
            "palette": halayka_palette(),
            "perspective_strength": 0.15,
            "render_mode": "3d",
            "solid_strength": 1.0,
            "version": 6,
            "zoom": zoom,
        }));
    }
    out
}

/// Not a test: the presets, to splice into `assets/presets.fflame`
/// (`output/halayka-presets.json`).
#[test]
#[ignore = "one-shot: writes the presets"]
fn write_presets() {
    let _ = std::fs::create_dir_all("output");
    std::fs::write("output/halayka-presets.json", serde_json::to_string_pretty(&presets()).unwrap()).unwrap();
}

/// The solid's ball follows its cut -- `prepare`'s `ball`, a box's corner
/// or the ball, a hair over -- in place of the Ball Radius parameter, so
/// the march and the camera take in a larger cut; its colourings read the
/// whole orbit (`INFO.depth`), and it is marched inside (`INFO.march`).
/// A solid's `INFO` takes only what those mean.
#[test]
fn the_solids_ball_follows_its_cut() {
    let text = example("solid-halayka-quaternions.rhai");
    let src = CustomSource { dialect: CustomDialect::Rhai, entry: rhai::title(&text).unwrap_or_default(), source: text, variations: Vec::new() };
    let name = super::custom::name_of(EntryKind::Formula, &src);
    let def = super::custom::compile_solid(&src, &name).unwrap_or_else(|e| panic!("{:?}", e.iter().map(|d| d.render(&src.source)).collect::<Vec<_>>()));
    assert!(def.march_inside);
    assert!(def.parameters.iter().all(|p| p.name != "radius"), "no Ball Radius");
    assert_eq!(def.parameters.iter().find(|p| p.name == "colour_depth").map(|p| p.default), Some(12.0));
    let mut escape = crate::config::escape::EscapeConfig { formula: name, ..Default::default() };
    for (cut, size, want) in [(0.0, 1.5, 1.5 * 3f64.sqrt()), (0.0, 3.0, 3.0 * 3f64.sqrt()), (1.0, 2.6, 2.6)] {
        escape.formula_params.insert("cut".into(), cut as f32);
        escape.formula_params.insert("size".into(), size as f32);
        let r = super::ifs::standalone_radius(def, &escape);
        assert!((r - 1.01 * want).abs() < 1e-5, "cut {cut}, size {size}: a ball of {r}");
    }

    let solid = |info: &str| {
        let src = format!(
            "const PARAMS = #{{}};\nconst INFO = #{{ title: \"t\"{info} }};\n\
             fn start(pos, p) {{ #{{ z: pos }} }}\nfn step(s, p) {{ s.z = s.z * 0.5; s }}\nfn distance(s, p) {{ length(s.z) - 1.0 }}\n"
        );
        rhai::solid(&src).is_ok()
    };
    assert!(solid(", march: \"inside\", depth: 8, radius: 2.0"));
    for bad in [", march: \"far\"", ", depth: 13", ", depth: 2.5", ", ball: 2.0"] {
        assert!(!solid(bad), "INFO {bad}");
    }
}

/// The bundled presets carry the solid and the variation as the example
/// files have them -- an edit to a file shows here until the presets are
/// written again (`write_presets`) -- and each loads: the solids name a
/// solid the renderer has, the flames carry the variation, which compiles.
#[test]
fn the_presets_carry_the_examples() {
    let bundled = crate::resources::load_presets_with_fallback();
    for want in presets() {
        let name = want["flame"]["name"].as_str().unwrap();
        let have = bundled.iter().find(|c| c.flame.name == name).unwrap_or_else(|| panic!("no preset {name}"));
        if want["render_mode"] == "escape" {
            let have_json = serde_json::to_value(&have.escape).unwrap();
            for key in ["formula", "custom_formula"] {
                assert_eq!(have_json.get(key), want["escape"].get(key), "{name}: {key} -- write the presets again");
            }
            // The config holds its parameters in f32.
            for (k, v) in want["escape"]["formula_params"].as_object().unwrap() {
                let got = have.escape.formula_params.get(k).copied().unwrap_or(f32::NAN);
                assert!((got as f64 - v.as_f64().unwrap()).abs() < 1e-6, "{name}: {k} is {got} -- write the presets again");
            }
            super::custom::ensure(&have.escape);
            assert!(super::ifs::get_ifs(&have.escape.formula).is_some(), "{name}: its solid compiles");
        } else {
            let have_json = serde_json::to_value(&have.flame.custom_variations).unwrap();
            assert_eq!(have_json, want["flame"]["custom_variations"], "{name}: the variation -- write the presets again");
            // Compiled, not registered: the global registry is every
            // test's, and a variation left in it changes what others see.
            for src in &have.flame.custom_variations {
                crate::variations::custom::compile(&src.source).unwrap_or_else(|e| panic!("{name}: {e}"));
            }
        }
    }
}
