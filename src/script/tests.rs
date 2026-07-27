//! Phase 1 tests: the sandbox does what it claims, and scripts are
//! reproducible.

use std::collections::HashMap;

use super::*;
use crate::config::fractal_config::FractalConfig;

fn run(text: &str, seed: u64) -> Result<ScriptOutcome, ScriptError> {
    ScriptHost::new().run(text, &FractalConfig::default(), seed, HashMap::new())
}

fn run_with(
    text: &str,
    seed: u64,
    params: &[(&str, ParamValue)],
) -> Result<ScriptOutcome, ScriptError> {
    let map = params
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect();
    ScriptHost::new().run(text, &FractalConfig::default(), seed, map)
}

/// The example from the feature request, verbatim in spirit: this is the
/// Phase 1 exit criterion.
const EXAMPLE: &str = r#"
script("Example", "generator");
let n = rand_int(2, 4);
for i in 0..n {
    let t = flame.add_transform();
    t.add_variation("linear", 1.0);
    t.weight = rand(0.5, 2.0);
    t.translate(rand(-1.0, 1.0), rand(-1.0, 1.0));
    t.scale(rand(0.5, 1.5));
}
if rand() > 0.5 {
    flame.add_effect("plasma");
}
"#;

#[test]
fn example_script_generates_a_flame() {
    let out = run(EXAMPLE, 42).expect("script runs");
    let n = out.config.flame.transforms.len();
    assert!((2..=4).contains(&n), "expected 2–4 transforms, got {n}");
    for t in &out.config.flame.transforms {
        assert!(t.variations.contains_key("linear"), "linear added");
        assert!((0.5..=2.0).contains(&t.weight), "weight in range: {}", t.weight);
        assert!(t.e.abs() <= 1.0 && t.f.abs() <= 1.0, "translated within ±1");
        // scale() touches the linear part only, leaving placement alone.
        assert!(t.a != 0.0 || t.b != 0.0, "linear part survived scaling");
    }
    assert_eq!(out.meta.name, "Example");
    assert_eq!(out.meta.kind, Some(ScriptKind::Generator));
    assert!(out.warnings.is_empty(), "no warnings: {:?}", out.warnings);
}

#[test]
fn same_seed_reproduces_the_same_flame() {
    // The sharing promise: script + seed is the artifact.
    //
    // Compared as serde_json::Value, not as a string: Value holds objects
    // in a BTreeMap (sorted), whereas serializing straight to text emits
    // HashMap fields — variations, variation_params, effect params — in
    // per-instance hash order. That makes .fflame TEXT unstable between
    // two runs producing the identical flame, so a string compare here
    // would test Rust's hasher, not our RNG.
    let a = serde_json::to_value(run(EXAMPLE, 8842).unwrap().config).unwrap();
    let b = serde_json::to_value(run(EXAMPLE, 8842).unwrap().config).unwrap();
    assert_eq!(a, b, "same seed must reproduce the same flame");

    let c = serde_json::to_value(run(EXAMPLE, 8843).unwrap().config).unwrap();
    assert_ne!(a, c, "a different seed should produce a different flame");

    // Adjacent seeds especially: "reroll" is seed + 1, and an earlier
    // version collapsed 8842/8843 onto one stream by forcing the low bit.
    let d = serde_json::to_value(run(EXAMPLE, 1).unwrap().config).unwrap();
    let e = serde_json::to_value(run(EXAMPLE, 2).unwrap().config).unwrap();
    assert_ne!(d, e, "consecutive seeds must give different flames");
}

#[test]
fn runaway_script_is_killed_by_the_budget() {
    // A shared script must not be able to hang the app.
    let err = run("script(\"Hang\", \"generator\"); loop { }", 1).unwrap_err();
    assert!(
        err.message.to_lowercase().contains("operation"),
        "expected an operations-budget error, got: {err}"
    );
}

#[test]
fn errors_carry_a_line_number() {
    // The audience is non-programmers: position is the difference between
    // a fixable mistake and a dead end.
    let script = "script(\"Oops\", \"generator\");\nlet t = flame.add_transform();\nt.add_variation(\"linnear\", 1.0);\n";
    let err = run(script, 1).unwrap_err();
    assert!(err.message.contains("linnear"), "names the bad variation: {err}");
    assert_eq!(err.line, Some(3), "points at the offending line: {err}");
}

#[test]
fn unknown_variation_param_is_rejected_with_suggestions() {
    let script = r#"
        script("P", "generator");
        let t = flame.add_transform();
        t.set_variation_param("julian.nope", 1.0);
    "#;
    let err = run(script, 1).unwrap_err();
    assert!(err.message.contains("nope"), "{err}");
    assert!(err.message.contains("has:"), "lists valid params: {err}");
}

#[test]
fn sandbox_has_no_io() {
    // Nothing was registered for file access, so these are parse/lookup
    // failures rather than working calls.
    for attempt in [
        "open(\"/etc/passwd\")",
        "import \"std\" as s;",
        "eval(\"1+1\")",
    ] {
        assert!(run(attempt, 1).is_err(), "`{attempt}` must not work");
    }
}

#[test]
fn params_are_collected_then_supplied() {
    let script = r#"
        script("Params", "generator");
        let n = param_int("copies", 3, 2, 8);
        let s = param("spread", 1.0, 0.0, 4.0);
        let m = param_bool("mirror", false);
        let style = param_choice("style", ["Loxodromic", "Parabolic"], 0);
        let t = flame.add_transform();
        t.add_variation("linear", 1.0);
        t.weight = n * 1.0 + s;
        if m { t.color = 1.0; }
        if style == "Parabolic" { t.color_speed = 0.5; }
    "#;

    let meta = ScriptHost::new()
        .collect(script, &FractalConfig::default())
        .expect("collect");
    assert_eq!(meta.params.len(), 4);
    assert_eq!(meta.params[0].key(), "copies");
    assert_eq!(meta.params[0].label(), "Copies", "label is humanized");
    match &meta.params[3] {
        ParamDecl::Choice { options, .. } => assert_eq!(options.len(), 2),
        other => panic!("expected a choice, got {other:?}"),
    }

    // Defaults when nothing is supplied.
    let out = run(script, 1).unwrap();
    assert_eq!(out.config.flame.transforms[0].weight, 4.0);

    // Supplied values win.
    let out = run_with(
        script,
        1,
        &[
            ("copies", ParamValue::Int(5)),
            ("spread", ParamValue::Float(2.5)),
            ("mirror", ParamValue::Bool(true)),
            ("style", ParamValue::Choice(1)),
        ],
    )
    .unwrap();
    let t = &out.config.flame.transforms[0];
    assert_eq!(t.weight, 7.5);
    assert_eq!(t.color, 1.0, "bool param applied");
    assert_eq!(t.color_speed, 0.5, "choice param applied");
}

#[test]
fn declared_params_are_clamped_and_unique() {
    let dup = r#"
        script("D", "generator");
        param("x", 1.0, 0.0, 2.0);
        param("x", 1.0, 0.0, 2.0);
    "#;
    assert!(run(dup, 1).unwrap_err().message.contains("more than once"));

    // A supplied value outside the declared range is clamped, not obeyed.
    let s = r#"
        script("C", "generator");
        let v = param("x", 1.0, 0.0, 2.0);
        let t = flame.add_transform();
        t.weight = v;
    "#;
    let out = run_with(s, 1, &[("x", ParamValue::Float(99.0))]).unwrap();
    assert_eq!(out.config.flame.transforms[0].weight, 2.0);
}

#[test]
fn script_declaration_is_validated() {
    assert!(run("script(\"A\", \"nonsense\");", 1)
        .unwrap_err()
        .message
        .contains("unknown script kind"));

    assert!(run("script(\"A\", \"generator\"); script(\"B\", \"modifier\");", 1)
        .unwrap_err()
        .message
        .contains("more than once"));

    // params before script() would leave the UI unable to label them
    assert!(
        run("param(\"x\", 1.0, 0.0, 2.0); script(\"A\", \"generator\");", 1)
            .unwrap_err()
            .message
            .contains("must come before")
    );

    // Omitting it entirely is a warning, not a failure.
    let out = run("let t = flame.add_transform();", 1).unwrap();
    assert_eq!(out.meta.kind, Some(ScriptKind::Generator));
    assert!(out.warnings.iter().any(|w| w.contains("script(name, kind)")));
}

#[test]
fn modifier_receives_the_current_flame() {
    let mut base = FractalConfig::default();
    base.flame.transforms.clear();
    for _ in 0..3 {
        base.flame.transforms.push(crate::scene::transforms::Transform::new());
    }
    let script = r#"
        script("Tint", "modifier");
        for i in 0..flame.transform_count() {
            let t = flame.transform(i);
            t.color = 0.25;
        }
    "#;
    let out = ScriptHost::new()
        .run(script, &base, 7, HashMap::new())
        .expect("modifier runs");
    assert_eq!(out.meta.kind, Some(ScriptKind::Modifier));
    assert_eq!(out.config.flame.transforms.len(), 3, "kept the existing flame");
    assert!(out.config.flame.transforms.iter().all(|t| t.color == 0.25));
}

#[test]
fn config_scalars_are_writable_by_json_key() {
    let script = r#"
        script("Cfg", "generator");
        config.set("gamma", 3.5);
        config.set("max_iterations", 1234);
        config["exposure"] = 0.75;
    "#;
    let out = run(script, 1).unwrap();
    assert_eq!(out.config.gamma, 3.5);
    assert_eq!(out.config.max_iterations, 1234);
    assert_eq!(out.config.exposure, 0.75);
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
}

#[test]
fn config_set_reports_problems() {
    // Wrong type is a hard error…
    let err = run("config.set(\"gamma\", \"bright\");", 1).unwrap_err();
    assert!(err.message.contains("invalid value"), "{err}");

    // …a name that changes nothing is a warning, not a silent success.
    let out = run("config.set(\"gama\", 2.0);", 1).unwrap();
    assert!(
        out.warnings.iter().any(|w| w.contains("gama")),
        "expected a warning, got {:?}",
        out.warnings
    );

    // The flame is off-limits here; it has a typed handle.
    let err = run("config.set(\"flame.name\", \"x\");", 1).unwrap_err();
    assert!(err.message.contains("`flame` object"), "{err}");
}

#[test]
fn transform_handles_survive_and_report_removal() {
    let script = r#"
        script("H", "generator");
        let a = flame.add_transform();
        let b = flame.add_transform();
        a.weight = 1.5;
        b.weight = 2.5;
    "#;
    let out = run(script, 1).unwrap();
    assert_eq!(out.config.flame.transforms[0].weight, 1.5);
    assert_eq!(out.config.flame.transforms[1].weight, 2.5);

    let dangling = r#"
        script("H", "generator");
        let a = flame.add_transform();
        flame.clear_transforms();
        a.weight = 1.0;
    "#;
    let err = run(dangling, 1).unwrap_err();
    assert!(err.message.contains("no longer exists"), "{err}");
}

#[test]
fn print_output_is_captured() {
    // stdout is invisible in-app and on the web.
    let out = run("script(\"P\", \"generator\"); print(\"hello\"); print(41 + 1);", 1).unwrap();
    assert_eq!(out.messages, vec!["hello".to_string(), "42".to_string()]);
}

#[test]
fn rotate_matches_an_explicit_rotation() {
    let script = r#"
        script("R", "generator");
        let t = flame.add_transform();
        t.set_affine(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        t.rotate(90.0);
    "#;
    let t = &run(script, 1).unwrap().config.flame.transforms[0];
    // Rotating identity by 90°: x' = -y, y' = x
    assert!((t.a - 0.0).abs() < 1e-6, "a = {}", t.a);
    assert!((t.b + 1.0).abs() < 1e-6, "b = {}", t.b);
    assert!((t.c - 1.0).abs() < 1e-6, "c = {}", t.c);
    assert!((t.d - 0.0).abs() < 1e-6, "d = {}", t.d);
}

#[test]
fn effects_route_to_the_right_chain() {
    let out = run(
        "script(\"E\", \"generator\"); flame.add_effect(\"plasma\"); flame.set_effect_param(\"plasma\", \"intensity\", 0.5);",
        1,
    )
    .unwrap();
    let total = out.config.density_effects.len() + out.config.color_effects.len();
    assert_eq!(total, 1, "one effect added");
    assert!(run("flame.add_effect(\"no_such_effect\");", 1)
        .unwrap_err()
        .message
        .contains("unknown effect"));
}

#[test]
fn shuffle_and_pick_are_seeded() {
    let s = r#"
        script("S", "generator");
        let items = [1, 2, 3, 4, 5, 6, 7, 8];
        let order = shuffle(items);
        let t = flame.add_transform();
        t.weight = order[0] * 1.0;
        t.color = pick(items) * 1.0;
    "#;
    let a = &run(s, 99).unwrap().config.flame.transforms[0];
    let b = &run(s, 99).unwrap().config.flame.transforms[0];
    assert_eq!((a.weight, a.color), (b.weight, b.color));
}

#[test]
fn contractiveness_is_a_whole_flame_property() {
    // The point of the metric: an EXPANSIVE transform is legitimate. One
    // map at 2x and one at 0.5x, equally weighted, average out to neutral.
    let script = r#"
        script("C", "generator");
        let a = flame.add_transform();
        a.add_variation("linear", 1.0);
        a.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        a.weight = 1.0;
        let b = flame.add_transform();
        b.add_variation("linear", 1.0);
        b.set_affine(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        b.weight = 1.0;
        print("" + flame.contractiveness());
        print("" + b.area_scale());
    "#;
    let out = run(script, 1).unwrap();
    let lambda: f64 = out.messages[0].parse().unwrap();
    assert!(lambda.abs() < 1e-6, "0.5x and 2x should cancel, got {lambda}");
    let area: f64 = out.messages[1].parse().unwrap();
    assert!((area - 4.0).abs() < 1e-6, "2x scales area 4x, got {area}");

    // Weight is a probability: make the expansive map rare and the system
    // contracts overall, without touching either transform's scale.
    let weighted = script.replace("b.weight = 1.0;", "b.weight = 0.2;");
    let out = run(&weighted, 1).unwrap();
    let lambda: f64 = out.messages[0].parse().unwrap();
    assert!(lambda < 0.0, "rarely-chosen expansion should contract, got {lambda}");
}

#[test]
fn set_contractiveness_hits_the_target() {
    let script = r#"
        script("C", "generator");
        for i in 0..3 {
            let t = flame.add_transform();
            t.add_variation("linear", 1.0);
            t.set_affine(1.4, 0.2, -0.1, 1.3, 0.0, 0.0);
            t.weight = 1.0 + i;
        }
        print("" + flame.contractiveness());
        flame.set_contractiveness(-0.25);
        print("" + flame.contractiveness());
    "#;
    let out = run(script, 1).unwrap();
    let before: f64 = out.messages[0].parse().unwrap();
    let after: f64 = out.messages[1].parse().unwrap();
    assert!(before > 0.0, "expansive to start with, got {before}");
    assert!((after + 0.25).abs() < 1e-5, "should land on -0.25, got {after}");

    // Nothing to scale is an error, not a silent no-op.
    assert!(run("script(\"C\", \"generator\"); flame.set_contractiveness(-0.2);", 1)
        .unwrap_err()
        .message
        .contains("at least one transform"));
}

fn test_palettes() -> Vec<crate::scene::palette::Palette> {
    use crate::scene::palette::{ColorStop, Palette};
    ["Ember", "Frost", "Moss"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let v = i as f32 / 3.0;
            Palette::new(
                *name,
                vec![
                    ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                    ColorStop { position: 1.0, color: [v, 1.0 - v, v * 0.5] },
                ],
            )
        })
        .collect()
}

#[test]
fn palette_choice_has_three_modes() {
    let host = ScriptHost::with_palettes(test_palettes());
    let mut base = FractalConfig::default();
    base.palette = crate::scene::palette::Palette::new(
        "MyCurrent",
        vec![crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 0.0, 0.0] }],
    );

    // 1. Say nothing -> the flame keeps the palette it came in with.
    let out = host
        .run("script(\"P\", \"generator\");", &base, 1, HashMap::new())
        .unwrap();
    assert_eq!(out.config.palette.name, "MyCurrent");

    // 2. Name one from the library.
    let out = host
        .run(
            "script(\"P\", \"generator\"); flame.set_palette(\"frost\");",
            &base,
            1,
            HashMap::new(),
        )
        .unwrap();
    assert_eq!(out.config.palette.name, "Frost", "name match is case-insensitive");

    // 3. Random pick — and it rides the SEEDED rng, so it reproduces.
    let script = "script(\"P\", \"generator\"); print(flame.random_palette());";
    let a = host.run(script, &base, 77, HashMap::new()).unwrap();
    let b = host.run(script, &base, 77, HashMap::new()).unwrap();
    assert_eq!(a.config.palette.name, b.config.palette.name);
    assert_eq!(a.messages[0], a.config.palette.name, "returns what it chose");
    assert!(test_palettes().iter().any(|p| p.name == a.config.palette.name));
}

#[test]
fn palette_errors_are_actionable() {
    let host = ScriptHost::with_palettes(test_palettes());
    let base = FractalConfig::default();
    let err = host
        .run(
            "script(\"P\", \"generator\"); flame.set_palette(\"Nope\");",
            &base,
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("palette_names()"), "points somewhere: {err}");

    // Without a library at all, say so rather than failing obscurely.
    let err = ScriptHost::new()
        .run(
            "script(\"P\", \"generator\"); flame.random_palette();",
            &base,
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("no palette library"), "{err}");
}
