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

/// A flame with real structure, to mutate.
fn seeded_flame() -> FractalConfig {
    let host = ScriptHost::new();
    let script = r#"
        script("Base", "generator");
        for i in 0..3 {
            let t = flame.add_transform();
            t.add_variation("linear", 1.0);
            t.add_variation("spherical", 0.5);
            t.set_affine(0.6, 0.1, -0.1, 0.6, 0.2, -0.3);
            t.weight = 1.0;
            t.color = 0.4;
        }
    "#;
    host.run(script, &FractalConfig::default(), 1, HashMap::new())
        .unwrap()
        .config
}

#[test]
fn mutate_varies_the_flame_without_wrecking_it() {
    let base = seeded_flame();
    let source = include_str!("../../assets/scripts/modifiers/mutate.rhai");
    let host = ScriptHost::new();

    let before_balance = {
        // Same measure the script preserves.
        let out = host
            .run(
                "script(\"M\", \"modifier\"); print(\"\" + flame.contractiveness());",
                &base,
                1,
                HashMap::new(),
            )
            .unwrap();
        out.messages[0].parse::<f64>().unwrap()
    };

    let mut seen = std::collections::HashSet::new();
    for seed in 1u64..=6 {
        let out = host.run(source, &base, seed, HashMap::new()).unwrap();
        let cfg = out.config;

        // Structure survives: same transforms, still weighted, still curved.
        assert_eq!(cfg.flame.transforms.len(), base.flame.transforms.len());
        for t in &cfg.flame.transforms {
            assert!(t.weight >= 0.05, "seed {seed}: transform faded out");
            assert!(t.a.is_finite() && t.d.is_finite(), "seed {seed}: bad affine");
            assert!((0.0..=1.0).contains(&t.color), "seed {seed}: colour off-palette");
            assert!(!t.variations.is_empty(), "seed {seed}: lost its variations");
        }

        // Balance is held, so mutations explore shape rather than drifting
        // into an over-expanded haze.
        let after = host
            .run(
                "script(\"M\", \"modifier\"); print(\"\" + flame.contractiveness());",
                &cfg,
                1,
                HashMap::new(),
            )
            .unwrap()
            .messages[0]
            .parse::<f64>()
            .unwrap();
        assert!(
            (after - before_balance).abs() < 1e-4,
            "seed {seed}: balance drifted {before_balance} -> {after}"
        );

        // Each seed is a DIFFERENT mutation, and none is the original.
        let json = serde_json::to_string(&serde_json::to_value(&cfg).unwrap()).unwrap();
        assert!(seen.insert(json), "seed {seed} repeated an earlier mutation");
        assert_ne!(
            serde_json::to_value(&cfg).unwrap(),
            serde_json::to_value(&base).unwrap(),
            "seed {seed} changed nothing"
        );
    }
}

#[test]
fn mutate_modes_limit_what_changes() {
    let base = seeded_flame();
    let source = include_str!("../../assets/scripts/modifiers/mutate.rhai");
    let host = ScriptHost::new();

    // Colours-only leaves the geometry alone.
    let out = host
        .run(
            source,
            &base,
            5,
            [("mode".to_string(), ParamValue::Choice(4))].into_iter().collect(),
        )
        .unwrap();
    let (a, b) = (&out.config.flame.transforms[0], &base.flame.transforms[0]);
    assert_eq!((a.a, a.b, a.c, a.d, a.e, a.f), (b.a, b.b, b.c, b.d, b.e, b.f));
    assert_eq!(a.weight, b.weight, "weights untouched in Colours mode");

    // Shape-only leaves colour alone.
    let out = host
        .run(
            source,
            &base,
            5,
            [("mode".to_string(), ParamValue::Choice(1))].into_iter().collect(),
        )
        .unwrap();
    let a = &out.config.flame.transforms[0];
    assert_eq!(a.color, base.flame.transforms[0].color);
}

#[test]
fn contractiveness_counts_variation_weights() {
    // Regression: a variation's weight MULTIPLIES the transform's output
    // (`result = Sum w_v * f_v(A*p)`), so `linear` at 2.0 is a doubling.
    // A metric that reads only the affine called this balanced, so
    // set_contractiveness "restored" a balance that had really drifted —
    // which showed up as a mutated flame rendering to haze.
    let probe = |weight: f64| {
        let script = format!(
            r#"
                script("W", "generator");
                let t = flame.add_transform();
                t.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
                t.add_variation("linear", {weight});
                t.weight = 1.0;
                print("" + flame.contractiveness());
            "#
        );
        run(&script, 1).unwrap().messages[0].parse::<f64>().unwrap()
    };

    // 0.5x affine alone: ln(0.5).
    let base = probe(1.0);
    assert!((base - 0.5f64.ln()).abs() < 1e-6, "got {base}");
    // Doubling the variation weight cancels it exactly.
    let doubled = probe(2.0);
    assert!(doubled.abs() < 1e-6, "0.5x affine x2 weight is neutral, got {doubled}");
    // And halving compounds it.
    let halved = probe(0.5);
    assert!((halved - 0.25f64.ln()).abs() < 1e-6, "got {halved}");
}

#[test]
fn contractiveness_ignores_non_summing_variations() {
    // Pre/post variations compose rather than joining the weighted sum,
    // so their weights must not be read as output scaling. `pre_blur` is
    // a Pre-phase variation; `linear` carries the sum.
    let script = r#"
        script("P", "generator");
        let t = flame.add_transform();
        t.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        t.add_variation("linear", 1.0);
        t.weight = 1.0;
        print("" + flame.contractiveness());
        t.add_variation("pre_blur", 3.0);
        print("" + flame.contractiveness());
    "#;
    let out = run(script, 1).unwrap();
    let (before, after) = (
        out.messages[0].parse::<f64>().unwrap(),
        out.messages[1].parse::<f64>().unwrap(),
    );
    assert!(
        (before - after).abs() < 1e-9,
        "a pre-phase weight changed the reading: {before} -> {after}"
    );
}

#[test]
fn set_contractiveness_stays_sane_on_a_degenerate_transform() {
    // A transform with nothing in the weighted sum collapses to the
    // origin. It must not drag the mean to -27 and trigger an
    // astronomical rescale (an earlier version asked for k = 1e12).
    let script = r#"
        script("D", "generator");
        let a = flame.add_transform();
        a.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        a.add_variation("linear", 1.0);
        a.weight = 1.0;
        let b = flame.add_transform();
        b.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        b.weight = 1.0;
        let k = flame.set_contractiveness(-0.25);
        print("" + k);
    "#;
    let out = run(script, 1).unwrap();
    let k: f64 = out.messages[0].parse().unwrap();
    assert!(k > 1e-3 && k < 1e3, "rescale factor out of sane range: {k}");
    for t in &out.config.flame.transforms {
        assert!(t.a.is_finite() && t.a.abs() < 1e6, "affine blew up: {}", t.a);
    }
}

#[test]
fn whole_numbers_work_wherever_decimals_do() {
    // Rhai does not coerce 1 to 1.0 for registered functions, so without
    // explicit handling every one of these fails with "function not
    // found" — a wall for the non-programmers this feature targets.
    let script = r#"
        script("N", "generator");
        let s = param("spread", 1, 0, 3);
        let t = flame.add_transform();
        t.add_variation("linear", 1);
        t.weight = 2;
        t.color = 1;
        t.translate(1, -1);
        t.scale(2);
        t.scale_xy(1, 1);
        t.rotate(90);
        t.set_affine(1, 0, 0, 1, 0, 0);
        t.set_variation_param("julian.power", 3);
        let r = rand(0, 2);
        if chance(1) { t.color = 0; }
        flame.set_contractiveness(0);
    "#;
    let out = run(script, 1).expect("whole numbers accepted throughout");
    let t = &out.config.flame.transforms[0];
    assert_eq!(t.weight, 2.0);
    assert_eq!(t.color, 0.0, "chance(1) is always true");

    // Mixed int/float in one call is fine too.
    assert!(run(
        "script(\"N\", \"generator\"); let t = flame.add_transform(); t.set_affine(1, 0.0, 0, 1.0, 0, 0.5);",
        1
    )
    .is_ok());

    // A genuine non-number still gets a clear message, not "not found".
    let err = run(
        "script(\"N\", \"generator\"); let t = flame.add_transform(); t.weight = \"heavy\";",
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("expects a number"), "{err}");
}

/// Golden values pinning the random stream.
///
/// The sharing promise is that a script plus a seed names one exact
/// flame, on desktop, in the browser, and from Python. That only holds
/// if every draw uses FIXED-WIDTH arithmetic: `usize` is 64-bit on
/// desktop and 32-bit on wasm32, and rand dispatches to a different
/// integer implementation for each, so a `gen_range(0..len)` silently
/// forked the stream between platforms (found by comparing a real WASM
/// build against desktop).
///
/// If this test fails, the stream moved. That is a breaking change for
/// every script anyone has already shared — treat it as such rather than
/// updating the numbers.
#[test]
fn random_stream_is_pinned() {
    let script = r#"
        script("RNG", "generator");
        print("" + rand());
        print("" + rand(-2.0, 5.0));
        print("" + rand_int(0, 1000000));
        print("" + pick([10, 20, 30, 40, 50, 60, 70]));
        print("" + shuffle([1,2,3,4,5,6,7,8]));
        print("" + chance(0.5));
    "#;
    let out = run(script, 12345).unwrap();
    assert_eq!(
        out.messages,
        vec![
            "0.46722037666755534",
            "1.6016749570971562",
            "24093",
            "40",
            "[3, 5, 8, 1, 2, 4, 7, 6]",
            "false",
        ],
        "the seeded stream changed — see this test's docs before touching it"
    );
}

#[test]
fn contractiveness_does_not_depend_on_hash_order() {
    // mean_log_scale sums variation weights; float addition isn't
    // associative, so summing in HashMap order would make the result
    // depend on the hasher seed (which differs per build and platform).
    // Many variations, added in varying orders, must agree exactly.
    let build = |names: &[&str]| {
        let adds: String = names
            .iter()
            .map(|n| format!("t.add_variation(\"{n}\", 0.37);
"))
            .collect();
        let script = format!(
            r#"
                script("H", "generator");
                let t = flame.add_transform();
                t.set_affine(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
                t.weight = 1.0;
                {adds}
                print("" + flame.contractiveness());
            "#
        );
        run(&script, 1).unwrap().messages[0].clone()
    };

    let forward = build(&[
        "linear", "spherical", "swirl", "horseshoe", "polar", "handkerchief", "heart", "disc",
    ]);
    let reverse = build(&[
        "disc", "heart", "handkerchief", "polar", "horseshoe", "swirl", "spherical", "linear",
    ]);
    assert_eq!(forward, reverse, "sum order changed the result");
}

#[test]
fn decompose_schottky_reproduces_the_packed_group() {
    // The packed schottky_group variation holds four Mobius generators and
    // picks one per iteration. Decomposing emits them as four transforms;
    // the group must survive the move intact.
    let host = ScriptHost::new();
    let packed = host
        .run(
            r#"
                script("S", "generator");
                let t = flame.add_transform();
                t.add_variation("schottky_group", 1.0);
                t.weight = 1.0;
            "#,
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap()
        .config;

    let source = include_str!("../../assets/scripts/modifiers/decompose_group.rhai");
    let out = host.run(source, &packed, 1, HashMap::new()).unwrap();
    let flame = &out.config.flame;

    assert_eq!(flame.transforms.len(), 4, "one transform per generator");
    for (i, t) in flame.transforms.iter().enumerate() {
        assert!(t.variations.contains_key("mobius"), "generator {i} carries mobius");
        for name in ["re_a", "im_a", "re_b", "im_b", "re_c", "im_c", "re_d", "im_d"] {
            let key = format!("mobius.{name}");
            assert!(
                t.variation_params.contains_key(&key),
                "generator {i} is missing {key}"
            );
            assert!(
                t.variation_params[&key].is_finite(),
                "generator {i}: {key} is not finite"
            );
        }
    }

    // The word rule survives as xaos: a generator can never be followed by
    // its own inverse, which sits 2 slots away in [a, b, a', b'].
    let xaos = flame.xaos.as_ref().expect("word rule became xaos");
    for from in 0..4 {
        assert_eq!(
            xaos[from][(from + 2) % 4],
            0.0,
            "from {from}: the inverse must be unreachable"
        );
    }

    // Generators and their inverses are genuinely different maps.
    let coeff = |i: usize, n: &str| flame.transforms[i].variation_params[&format!("mobius.{n}")];
    assert_ne!(coeff(0, "re_b"), coeff(2, "re_b"), "a and a-inverse differ");

    // Run on a flame with no schottky_group: report, don't wreck it.
    let plain = host
        .run(
            r#"script("P", "generator"); let t = flame.add_transform(); t.add_variation("linear", 1.0);"#,
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap()
        .config;
    let out = host.run(source, &plain, 1, HashMap::new()).unwrap();
    assert_eq!(out.config.flame.transforms.len(), 1, "left the flame alone");
    assert!(
        out.messages.iter().any(|m| m.contains("No packed group")),
        "said why nothing happened: {:?}",
        out.messages
    );
}

#[test]
fn decompose_sphere_packing_emits_inversions() {
    // A packing is a REFLECTION group: each generator inverts in one
    // sphere. Unlike the Mobius groups there is no matrix to carry, so the
    // decomposition builds each inversion out of the flame's own affine
    // machinery: translate the centre to the origin, apply p/|p|^2 scaled
    // by r^2, translate back.
    let host = ScriptHost::new();
    let packed = host
        .run(
            r#"
                script("P", "generator");
                let t = flame.add_transform();
                t.add_variation("sphere_packing", 1.0);
                t.weight = 1.0;
            "#,
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap()
        .config;

    let source = include_str!("../../assets/scripts/modifiers/decompose_group.rhai");
    let out = host.run(source, &packed, 1, HashMap::new()).unwrap();
    let flame = &out.config.flame;

    // Default mode is Apollonian (dual), 2D: four mirror circles.
    let expected = crate::script::builtins::sphere_packing_mirrors(
        0, 1.0, 6, 1.0, 1.0, 0.0, 0.0, false,
    );
    assert_eq!(flame.transforms.len(), expected.len(), "one transform per mirror");

    for (i, (t, m)) in flame.transforms.iter().zip(expected.iter()).enumerate() {
        assert!(
            t.variations.contains_key("spherical3D_wf"),
            "mirror {i} uses the inversion kernel"
        );
        // Weight carries r^2 - that is what turns p/|p|^2 into an
        // inversion of the right radius.
        let w = t.variations["spherical3D_wf"] as f64;
        assert!(
            (w - m.r * m.r).abs() < 1e-5,
            "mirror {i}: weight should be r^2 ({} vs {})",
            w,
            m.r * m.r
        );
        // Pre-affine moves the centre to the origin, post-affine back.
        assert!((-(t.e as f64) - m.x).abs() < 1e-5, "mirror {i}: pre-translate x");
        assert!((-(t.f as f64) - m.y).abs() < 1e-5, "mirror {i}: pre-translate y");
        assert!(t.post_affine_enabled, "mirror {i}: post-affine must be on");
        assert!((t.post_e as f64 - m.x).abs() < 1e-5, "mirror {i}: post-translate x");
        assert!((t.post_f as f64 - m.y).abs() < 1e-5, "mirror {i}: post-translate y");
    }

    // An inversion is its own inverse, so the word rule blocks REPEATS -
    // the diagonal, not an offset like the Mobius groups use.
    let xaos = flame.xaos.as_ref().expect("word rule became xaos");
    for i in 0..flame.transforms.len() {
        assert_eq!(xaos[i][i], 0.0, "mirror {i} must not follow itself");
    }

    assert!(
        out.messages.iter().any(|m| m.contains("sphere inversions")),
        "reported what it did: {:?}",
        out.messages
    );
}

#[test]
fn decompose_klein_group_uses_its_own_word_rule() {
    // Three packed groups, three different "don't backtrack" rules. This
    // one excludes the previous generator's INVERSE and then draws
    // uniformly from the remaining three — no doubling, and the inverse
    // sits at index ^ 1 because the order is [a, a', b, b'].
    let host = ScriptHost::new();
    let packed = host
        .run(
            r#"
                script("K", "generator");
                let t = flame.add_transform();
                t.add_variation("klein_group", 1.0);
                t.weight = 1.0;
            "#,
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap()
        .config;

    let source = include_str!("../../assets/scripts/modifiers/decompose_group.rhai");
    let out = host.run(source, &packed, 1, HashMap::new()).unwrap();
    let flame = &out.config.flame;

    assert_eq!(flame.transforms.len(), 4, "two generators and their inverses");
    for (i, t) in flame.transforms.iter().enumerate() {
        assert!(t.variations.contains_key("mobius"), "generator {i} carries mobius");
    }

    let xaos = flame.xaos.as_ref().expect("word rule became xaos");
    for from in 0..4usize {
        let forbidden = from ^ 1;
        assert_eq!(xaos[from][forbidden], 0.0, "from {from}: inverse blocked");
        // Everything else stays equally likely — the distinguishing detail.
        for to in 0..4usize {
            if to != forbidden {
                assert_eq!(
                    xaos[from][to], 1.0,
                    "from {from} -> {to}: klein_group does not double any share"
                );
            }
        }
    }

    // Cross-check against the packed variation's own construction: at the
    // default traces the generators must be unimodular and correctly paired.
    let gens = crate::script::builtins::klein_generators(0, 2.0, 0.0, 2.0, 0.0, 1.0);
    let coeff = |i: usize, n: &str| {
        flame.transforms[i].variation_params[&format!("mobius.{n}")] as f64
    };
    for (i, g) in gens.iter().enumerate() {
        let p = g.to_params();
        assert!(
            (coeff(i, "re_a") - p[0]).abs() < 1e-5,
            "generator {i} re_a: script {} vs builtin {}",
            coeff(i, "re_a"),
            p[0]
        );
    }
}

#[test]
fn decomposition_sets_preserve_z_only_where_needed() {
    // With preserve_z off (the default) the renderer flattens z every
    // iteration, and only variations flagged AlwaysZ survive it. A
    // decomposition that trades an AlwaysZ variation for one without it
    // therefore has to turn preserve_z ON, or a 3D flame collapses to the
    // flat 2D group — while still looking like a perfectly good fractal.
    //
    // It is NOT a blanket switch: klein_group is not AlwaysZ, so forcing
    // preserve_z would make the decomposition keep a z its source
    // discards, which breaks the match just as badly in the other
    // direction. Verified by render comparison at 320x320: apollonian 3D
    // went from mean 38.3/255 to 0.180/255 with the rule, and klein_group
    // went from 0.008/255 to 39.4/255 without it.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/modifiers/decompose_group.rhai");

    let case = |variation: &str| -> bool {
        let packed = host
            .run(
                &format!(
                    r#"
                        script("S", "generator");
                        let t = flame.add_transform();
                        t.add_variation("{variation}", 1.0);
                        t.weight = 1.0;
                        config.set("render_mode", "3d");
                    "#
                ),
                &FractalConfig::default(),
                1,
                HashMap::new(),
            )
            .unwrap()
            .config;
        assert!(!packed.preserve_z, "the source starts with preserve_z off");
        host.run(source, &packed, 1, HashMap::new()).unwrap().config.preserve_z
    };

    // AlwaysZ source -> non-AlwaysZ target (mobius): needs it.
    assert!(case("apollonian_gasket"), "apollonian_gasket needs preserve_z");
    assert!(case("schottky_group"), "schottky_group needs preserve_z");
    // Not AlwaysZ at all: leave it alone.
    assert!(!case("klein_group"), "klein_group must NOT get preserve_z");
    // AlwaysZ source -> AlwaysZ target (spherical3D_wf): nothing to fix.
    assert!(!case("sphere_packing"), "sphere_packing needs no preserve_z");

    // The rule is driven by the feature flags, not a hardcoded list.
    use crate::variations::definition::Feature;
    let always_z = |n: &str| {
        crate::variations::global_registry()
            .get(n)
            .is_some_and(|i| i.has_feature(Feature::AlwaysZ))
    };
    assert!(always_z("apollonian_gasket") && always_z("schottky_group"));
    assert!(!always_z("klein_group"));
    assert!(always_z("sphere_packing") && always_z("spherical3D_wf"));
    assert!(!always_z("mobius"), "the mobius target is not AlwaysZ");
}

#[test]
fn lsystem_script_reads_rules_from_parameters() {
    // Rules are typed in as text now, so a user can paste one from any
    // reference without touching the script.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem.rhai");
    let run_with = |sets: &[(&str, &str)]| {
        let params: HashMap<String, ParamValue> = sets
            .iter()
            .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
            .collect();
        host.run(source, &FractalConfig::default(), 1, params).unwrap()
    };

    // Default is Koch: four pieces at one third.
    let out = run_with(&[]);
    assert_eq!(out.config.flame.transforms.len(), 4);

    // The dragon, typed in as two rules.
    let out = run_with(&[("rule_1", "F=F+G"), ("rule_2", "G=F-G")]);
    assert_eq!(out.config.flame.transforms.len(), 2, "dragon has two pieces");

    // A malformed rule is reported, not guessed at.
    let out = run_with(&[("rule_1", "nonsense")]);
    assert!(
        out.messages.iter().any(|m| m.contains("Could not read this rule")),
        "{:?}",
        out.messages
    );
    assert!(out.config.flame.transforms.is_empty(), "and nothing is built");

    // A rule whose symbols never draw is reported too.
    let out = run_with(&[("axiom", "X"), ("rule_1", "X=X+X")]);
    assert!(
        out.messages.iter().any(|m| m.contains("at least 2 are needed")),
        "{:?}",
        out.messages
    );
}

#[test]
fn lsystem_warns_when_the_pieces_do_not_shrink() {
    // A rule whose drawn pieces are unit steps with net displacement one
    // step long: F=F+F+F- at 90 degrees walks east, north, west — a net
    // displacement of one, so nothing shrinks and neither construction
    // applies (the axiom draws, so node mode declines). Report it plainly
    // instead of emitting a flame that quietly is not a curve.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem.rhai");
    let mut params: HashMap<String, ParamValue> =
        [("rule_1", "F=F+F+F-")]
            .iter()
            .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
            .collect();
    params.insert("angle".to_string(), ParamValue::Float(90.0));

    let out = host.run(source, &FractalConfig::default(), 1, params).unwrap();
    assert!(
        out.messages.iter().any(|m| m.contains("do not shrink")),
        "expected the non-contractive warning: {:?}",
        out.messages
    );
}

#[test]
fn space_filling_rules_build_by_node_rewriting() {
    // The classic Hilbert L-system rewrites NODES, so its drawn depth-1
    // pieces are unit steps and the edge construction has nothing to
    // converge to. The node construction takes over: one map per variable
    // occurrence, each carrying the whole curve onto a sub-cell. Hilbert's
    // known IFS is four maps at scale one half, the first and last
    // mirrored (they are Y occurrences, X's mirror partner). The attractor
    // is the FILLED square — that is what space-filling means.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem.rhai");
    let mut params: HashMap<String, ParamValue> = [
        ("axiom", "X"),
        ("rule_1", "X=-YF+XFX+FY-"),
        ("rule_2", "Y=+XF-YFY-FX+"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
    .collect();
    params.insert("angle".to_string(), ParamValue::Float(90.0));

    let out = host.run(source, &FractalConfig::default(), 1, params).unwrap();
    assert!(
        out.messages.iter().any(|m| m.contains("Space-filling")),
        "should announce the node construction: {:?}",
        out.messages
    );

    let ts = &out.config.flame.transforms;
    assert_eq!(ts.len(), 4, "Hilbert is four maps");
    let det = |t: &crate::scene::transforms::Transform| {
        (t.a as f64) * (t.d as f64) - (t.b as f64) * (t.c as f64)
    };
    // Occurrence order Y, X, X, Y: the outer two are reflected.
    assert!(det(&ts[0]) < 0.0, "first map (Y) is mirrored");
    assert!(det(&ts[1]) > 0.0, "second map (X) is direct");
    assert!(det(&ts[2]) > 0.0, "third map (X) is direct");
    assert!(det(&ts[3]) < 0.0, "fourth map (Y) is mirrored");
    for (i, t) in ts.iter().enumerate() {
        let scale = ((t.a as f64).powi(2) + (t.c as f64).powi(2)).sqrt();
        assert!(
            (scale - 0.5).abs() < 0.02,
            "map {i} scale {scale} should be about one half"
        );
    }
}

#[test]
fn lsystem_rejects_a_closed_path() {
    // A path returning to its start has no unit-segment form, so there is
    // no IFS to build. Say so instead of dividing by zero.
    let err = run(
        r#"
            script("L", "generator");
            normalize_segments(turtle(lsystem("F", #{ "F": "F+F+F+F" }, 1), 90.0));
        "#,
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("returns to where it started"), "{err}");
}

#[test]
fn mirrored_pieces_get_a_reflected_transform() {
    // The Sierpinski arrowhead is built from two symbols whose rules are
    // mirror images (F -> +G-F-G+, G -> -F+G+F-). Both draw segments with
    // the SAME endpoints, so a transform derived from endpoints alone
    // loses the chirality — and the attractor comes out as a hexagonal
    // curve instead of the gasket. Pieces drawn by the mirrored symbol
    // must get a REFLECTED similarity, which flips the determinant sign.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem.rhai");
    let params: HashMap<String, ParamValue> = [
        ("rule_1", "F=+G-F-G+"),
        ("rule_2", "G=-F+G+F-"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
    .collect();
    let out = host.run(source, &FractalConfig::default(), 1, params).unwrap();
    let ts = &out.config.flame.transforms;
    assert_eq!(ts.len(), 3, "arrowhead replaces one edge with three");

    // The mirror pair is DETECTED, not configured — the user never has to
    // know the pieces drawn by G need reflecting.
    assert!(
        out.messages.iter().any(|m| m.contains("Mirror pair found")),
        "{:?}",
        out.messages
    );

    // Pieces 0 and 2 are drawn by G (the mirrored symbol), piece 1 by F.
    let det = |t: &crate::scene::transforms::Transform| {
        (t.a as f64) * (t.d as f64) - (t.b as f64) * (t.c as f64)
    };
    assert!(det(&ts[0]) < 0.0, "first piece is reflected");
    assert!(det(&ts[1]) > 0.0, "middle piece keeps its orientation");
    assert!(det(&ts[2]) < 0.0, "last piece is reflected");

    // Reflection does not change the scale: all three stay at one half.
    for (i, t) in ts.iter().enumerate() {
        let scale = ((t.a as f64).powi(2) + (t.c as f64).powi(2)).sqrt();
        assert!((scale - 0.5).abs() < 1e-6, "piece {i} scale {scale} should be 0.5");
    }
}
