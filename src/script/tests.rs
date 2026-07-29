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

#[test]
fn path_mode_bakes_the_maps_into_one_variation() {
    // Path mode draws the FINITE-depth curve — the iconic Hilbert maze —
    // which the attractor transforms cannot show (their limit is the
    // filled square). One transform carries the lsystem_path variation
    // with the maps as parameters; no geometry is stored, so Iterations
    // stays a live parameter.
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
    params.insert("output".to_string(), ParamValue::Choice(1));
    params.insert("path_iterations".to_string(), ParamValue::Int(6));

    let out = host.run(source, &FractalConfig::default(), 1, params).unwrap();
    assert!(
        out.messages.iter().any(|m| m.contains("Path mode")),
        "{:?}",
        out.messages
    );

    let ts = &out.config.flame.transforms;
    assert_eq!(ts.len(), 1, "one transform carries the whole path");
    let t = &ts[0];
    assert!(t.variations.contains_key("lsystem_path"));
    let p = |k: &str| t.variation_params[&format!("lsystem_path.{k}")] as f64;
    assert_eq!(p("map_count") as i64, 4, "Hilbert is four maps");
    assert_eq!(p("iterations") as i64, 6, "depth passed through");

    // Space-filling curves draw the vertex chain through cell CENTRES —
    // their cell spans lie on the boundary lattice and overlap each other
    // (doubled lines and little boxes, reported from an in-app render;
    // exact maps changed nothing because the defect was structural). The
    // anchor is the attractor's bounding-box centre.
    assert_eq!(p("anchored"), 1.0, "node curves use the centre chain");
    assert!((p("anchor_x") - 0.5).abs() < 1e-6, "anchor at bbox centre x");
    assert!((p("anchor_y") + 0.5).abs() < 1e-2, "anchor at bbox centre y");

    // The maps are the same similarities the attractor transforms would
    // get: scale 1/2, with the mirrored Y maps carrying negative
    // determinant.
    for k in 0..4 {
        let (a, b) = (p(&format!("m{k}_a")), p(&format!("m{k}_b")));
        let (c, d) = (p(&format!("m{k}_c")), p(&format!("m{k}_d")));
        let scale = (a * a + c * c).sqrt();
        assert!((scale - 0.5).abs() < 1e-3, "map {k} scale {scale}");
        let det = a * d - b * c;
        let mirrored = k == 0 || k == 3; // occurrence order Y, X, X, Y
        assert_eq!(det < 0.0, mirrored, "map {k} determinant sign");
    }
}

#[test]
fn plant_script_grows_a_fern_from_pasted_rules() {
    // The Barnsley construction end to end: branch maps at the recursion
    // sites, squashed stem maps along the drawn segments, colour by
    // bracket depth, weights by size.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem_plant.rhai");
    let out = host
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let ts = &out.config.flame.transforms;
    assert_eq!(ts.len(), 7, "four branch sites plus three stems");

    // The stem maps are the SQUASHED ones: near-degenerate determinant
    // relative to their scale (thickness 0.06), while branch maps are
    // honest similarities. That asymmetry is the fern trick.
    let mut squashed = 0;
    for t in ts {
        let det = (t.a as f64) * (t.d as f64) - (t.b as f64) * (t.c as f64);
        let scale2 = (t.a as f64).powi(2) + (t.c as f64).powi(2);
        if det.abs() < 0.2 * scale2 {
            squashed += 1;
        }
        assert!(t.weight > 0.0, "every piece carries weight");
    }
    assert_eq!(squashed, 3, "exactly the three stems are squashed");

    // Colour by depth: the trunk-level pieces sit at the palette start,
    // the deepest twigs at its end.
    let colors: Vec<f32> = ts.iter().map(|t| t.color).collect();
    assert!(colors.iter().any(|c| *c == 1.0), "deepest level reaches 1.0: {colors:?}");
    assert!(colors.iter().any(|c| *c == 0.0), "trunk level at 0.0: {colors:?}");

    // Stems off means branch maps only.
    let out = host
        .run(
            source,
            &FractalConfig::default(),
            1,
            [("stem_weight".to_string(), ParamValue::Float(0.0))].into_iter().collect(),
        )
        .unwrap();
    assert_eq!(out.config.flame.transforms.len(), 4, "stem weight 0 skips stems");
}

#[test]
fn graph_directed_systems_build_with_xaos_and_opacity() {
    // The construction previously refused as "graph-directed": a flame
    // CAN express it — one transform per occurrence, xaos allowing a map
    // only when it consumes the type the previous one produced, opacity
    // hiding every curve but the axiom's. The hidden types still drive
    // the dynamics; they are invisible scaffolding.
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/lsystem.rhai");
    let mut params: HashMap<String, ParamValue> = [
        ("axiom", "X"),
        ("rule_1", "X=F+YFZF"),
        ("rule_2", "Y=FX-F"),
        ("rule_3", "Z=F-XF"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
    .collect();
    params.insert("angle".to_string(), ParamValue::Float(60.0));

    let out = host.run(source, &FractalConfig::default(), 1, params).unwrap();
    assert!(
        out.messages.iter().any(|m| m.contains("Graph-directed")),
        "{:?}",
        out.messages
    );

    let flame = &out.config.flame;
    // Occurrences: Y and Z in X's rule, X in Y's, X in Z's.
    assert_eq!(flame.transforms.len(), 4, "one transform per occurrence");
    for t in &flame.transforms {
        assert!(t.variations.contains_key("matrix3D"), "graph pieces are matrix3D");
    }

    // Only the axiom's curve is visible: the two maps producing X.
    let vis: Vec<bool> = flame.transforms.iter().map(|t| t.opacity > 0.5).collect();
    assert_eq!(vis, vec![true, true, false, false], "only X-producing maps plot");

    // Xaos gates on type: occ(next) == owner(prev). Piece order is
    // [(Y<-X), (Z<-X), (X<-Y), (X<-Z)] as (occ, owner).
    let xaos = flame.xaos.as_ref().expect("word structure as xaos");
    let expect = [
        [0.0, 0.0, 1.0, 1.0], // after an X-producer: only X-consumers
        [0.0, 0.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 0.0], // after the Y-producer: only the Y-consumer
        [0.0, 1.0, 0.0, 0.0], // after the Z-producer: only the Z-consumer
    ];
    for f in 0..4 {
        for t in 0..4 {
            assert_eq!(
                xaos[f][t], expect[f][t],
                "xaos[{f}][{t}] must gate by type"
            );
        }
    }
}

#[test]
fn hilbert3d_script_builds_the_path() {
    let host = ScriptHost::new();
    let source = include_str!("../../assets/scripts/generators/hilbert3d.rhai");
    let out = host
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let flame = &out.config.flame;
    assert_eq!(flame.transforms.len(), 1, "one transform carries the path");
    let t = &flame.transforms[0];
    assert!(t.variations.contains_key("lsystem_path_3D"));
    let p = |k: &str| t.variation_params[&format!("lsystem_path_3D.{k}")] as f64;
    assert_eq!(p("map_count") as i64, 8, "eight octant maps");
    // The maps in the flame are the constructed ones: scale 1/2 columns.
    for k in 0..8 {
        let (a, c, e) = (p(&format!("m{k}_xx")), p(&format!("m{k}_yx")), p(&format!("m{k}_zx")));
        let n = (a * a + c * c + e * e).sqrt();
        assert!((n - 0.5).abs() < 1e-6, "map {k} column scale {n}");
    }
    assert_eq!(out.config.render_mode, crate::scene::transforms::RenderMode::ThreeD);
    assert!(out.config.preserve_z, "z must survive between iterations");

    // Centre chain, not cell spans — the 3D port of the 2D lesson. Spans
    // lie on the cell-edge lattice and overlap, showing up in-app as
    // three-way joins and phantom dead ends under zoom; the anchor is
    // the cube's centre, whose image in every cell is that cell's centre.
    assert_eq!(p("anchored"), 1.0, "space-filling path uses the centre chain");
    assert!((p("anchor_x") - 0.5).abs() < 1e-6);
    assert!((p("anchor_y") - 0.5).abs() < 1e-6);
    assert!((p("anchor_z") - 0.5).abs() < 1e-6);

    // The curve is built in the unit cube, so its centre is (.5,.5,.5);
    // offsetting by minus that puts the object on the origin, which is
    // what camera rotation and zoom orbit around. The view must then NOT
    // pan as well, or the curve just moves back off centre.
    for axis in ["offset_x", "offset_y", "offset_z"] {
        assert!((p(axis) + 0.5).abs() < 1e-6, "{axis} centres the cube");
    }
    assert_eq!(out.config.pan_x, 0.0, "centred geometry needs no pan");
    assert_eq!(out.config.pan_y, 0.0);
}

#[test]
fn path_thickness_defaults_off_and_is_appended() {
    // Thickness must default to 0 so existing flames render exactly as
    // before, and its parameters must sit AFTER the map bank and the
    // anchor block — the slot indices the shaders read are positional,
    // so inserting rather than appending would silently reinterpret
    // every saved map.
    let reg = crate::variations::global_registry();
    for (name, before) in [("lsystem_path", "anchor_y"), ("lsystem_path_3D", "anchor_z")] {
        let info = reg.get(name).unwrap_or_else(|| panic!("{name} registered"));
        let names: Vec<String> = info.parameters.iter().map(|p| p.name.to_string()).collect();
        let anchor_at = names.iter().position(|n| n == before).unwrap();
        let thick_at = names.iter().position(|n| n == "thickness").unwrap();
        let soft_at = names.iter().position(|n| n == "soft").unwrap();
        assert!(thick_at > anchor_at, "{name}: thickness appended after the anchor block");
        assert_eq!(soft_at, thick_at + 1, "{name}: soft follows thickness");

        let thick = &info.parameters[thick_at];
        assert_eq!(thick.default_value, 0.0, "{name}: thickness defaults to off");

        // Offsets are appended after soft, for the same positional reason,
        // and default to 0 so an untouched curve stays where it was.
        let off_at = names.iter().position(|n| n == "offset_x").unwrap();
        assert_eq!(off_at, soft_at + 1, "{name}: offset_x follows soft");
        assert_eq!(names[off_at + 1], "offset_y", "{name}: offset_y follows offset_x");
        for axis in ["offset_x", "offset_y"] {
            let p = info.parameters.iter().find(|p| p.name == axis).unwrap();
            assert_eq!(p.default_value, 0.0, "{name}: {axis} defaults to no move");
        }
        if name == "lsystem_path_3D" {
            assert_eq!(names[off_at + 2], "offset_z", "3D also offsets in z");
            let p = info.parameters.iter().find(|p| p.name == "offset_z").unwrap();
            assert_eq!(p.default_value, 0.0);
        }
    }
}

// ============================================================================
// Phase 6 — script-defined animation
// ============================================================================

/// The animation a script builds must be one the APP can load, not just
/// one that round-trips through our own builder. So this drives the
/// real `AnimationController` and checks the values it evaluates.
#[test]
fn a_script_can_define_an_animation_the_app_can_play() {
    let source = r#"
        script("Spin", "generator");
        let t = flame.add_transform();
        t.add_variation("julian", 1.0);
        t.set_variation_param("julian.power", 2.0);
        t.weight = 1.0;

        anim.name = "Spin";
        anim.duration = 8;
        anim.key("rotation", 0.0, 0.0);
        anim.key("rotation", 8.0, 360.0);
        t.key("weight", 0.0, 0.2);
        t.key("weight", 8.0, 1.0);
        t.key("julian.power", 0.0, 2.0);
        t.key("julian.power", 8.0, 6.0);
    "#;

    let host = ScriptHost::new();
    let out = host
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let animation = out.animation.expect("the script defined an animation");

    assert_eq!(animation.name, "Spin");
    assert_eq!(animation.duration, 8.0);
    assert!(animation.has_base_config(), "the .anim must stand alone");

    // Targets are ConfigPath keys — the spelling the loader parses.
    let targets: Vec<&str> = animation.tracks.iter().map(|t| t.target.as_str()).collect();
    assert!(targets.contains(&"Rotation"), "got {targets:?}");
    assert!(targets.contains(&"Transform.0.Weight"), "got {targets:?}");
    assert!(
        targets.contains(&"Transform.0.VariationParam.julian.power"),
        "got {targets:?}"
    );
    for target in &targets {
        assert!(
            crate::config::ConfigPath::from_string_key(target).is_some(),
            "the app's loader must parse `{target}`"
        );
    }

    // And the app's own controller must evaluate them.
    let mut controller = crate::animation::AnimationController::new();
    controller.load(animation);
    let at = |time: f64| -> HashMap<String, f64> {
        controller
            .evaluate_at_time(time)
            .into_iter()
            .filter_map(|(_, target, value)| value.as_f64().map(|v| (target, v)))
            .collect()
    };

    let start = at(0.0);
    let mid = at(4.0);
    let end = at(8.0);
    assert_eq!(start.get("Rotation"), Some(&0.0));
    assert_eq!(end.get("Rotation"), Some(&360.0));
    assert!((mid["Rotation"] - 180.0).abs() < 1e-6, "linear halfway");
    assert!((mid["Transform.0.Weight"] - 0.6).abs() < 1e-6);
    assert!((mid["Transform.0.VariationParam.julian.power"] - 4.0).abs() < 1e-6);
}

/// Animation is opt-in: a script produces one exactly when it asks for
/// one. Checked against the source rather than a hand-kept list, so
/// adding a script can't quietly make this vacuous.
#[test]
fn scripts_produce_an_animation_only_when_they_ask_for_one() {
    let host = ScriptHost::new();
    let mut animated = 0;
    for (name, source) in super::library::EMBEDDED {
        let out = host
            .run(source, &FractalConfig::default(), 1, HashMap::new())
            .unwrap_or_else(|e| panic!("{name}: {}", e.message));
        let asked = source.contains("anim.");
        assert_eq!(
            out.animation.is_some(),
            asked,
            "{name}: animation present = {}, but the script {} mention `anim`",
            out.animation.is_some(),
            if asked { "does" } else { "does not" }
        );
        animated += asked as usize;
    }
    assert!(animated > 0, "no shipped script exercises animation");
}

/// Keyframe times may be written in any order; the player walks them in
/// time order.
#[test]
fn keyframes_are_sorted_by_time() {
    let source = r#"
        script("Backwards", "generator");
        flame.add_transform();
        anim.key("zoom", 6.0, 3.0);
        anim.key("zoom", 0.0, 1.0);
        anim.key("zoom", 3.0, 2.0);
    "#;
    let out = ScriptHost::new()
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let animation = out.animation.unwrap();
    // Duration defaults to the last keyframe when unset.
    assert_eq!(animation.duration, 6.0);

    let crate::animation::TrackSource::Keyframes { keyframes } = &animation.tracks[0].source else {
        panic!("expected a keyframe track");
    };
    let times: Vec<f64> = keyframes.iter().map(|k| k.time).collect();
    assert_eq!(times, vec![0.0, 3.0, 6.0]);
}

/// A misspelled target is an error, not a silently dropped track — a
/// track that quietly does nothing is invisible in a rendered animation.
#[test]
fn unknown_animation_targets_are_rejected() {
    let host = ScriptHost::new();
    let cases = [
        (r#"anim.key("zooom", 0.0, 1.0);"#, "not an animatable setting"),
        (
            r#"let t = flame.add_transform(); t.key("wieght", 0.0, 1.0);"#,
            "not an animatable transform value",
        ),
        (
            r#"anim.key("zoom", 0.0, 1.0, "ease_in_sideways");"#,
            "is not an easing",
        ),
        (
            r#"anim.interpolation("zoom", "wobbly");"#,
            "is not an interpolation",
        ),
    ];
    for (body, expected) in cases {
        let source = format!("script(\"T\", \"generator\");\n{body}");
        let err = host
            .run(&source, &FractalConfig::default(), 1, HashMap::new())
            .expect_err("should have been rejected");
        assert!(
            err.message.contains(expected),
            "expected {expected:?} in {:?}",
            err.message
        );
    }
}

/// Both spellings work: the script's own (`camera_rotation_x`, as
/// `config.set` takes it) and the ConfigPath key (`CameraRotationX`).
#[test]
fn animation_targets_accept_either_spelling() {
    let host = ScriptHost::new();
    let run = |body: &str| {
        let source = format!("script(\"T\", \"generator\");\nflame.add_transform();\n{body}");
        host.run(&source, &FractalConfig::default(), 1, HashMap::new())
            .unwrap()
            .animation
            .unwrap()
            .tracks[0]
            .target
            .clone()
    };
    assert_eq!(run(r#"anim.key("camera_rotation_x", 0.0, 10.0);"#), "CameraRotationX");
    assert_eq!(run(r#"anim.key("CameraRotationX", 0.0, 10.0);"#), "CameraRotationX");
    assert_eq!(run(r#"anim.key("zoom", 0.0, 2.0);"#), "Zoom");
}

/// Weight and colour belong to normal transforms only — linked and final
/// ones always run. Say so rather than emitting a dead target.
#[test]
fn weight_is_rejected_on_pools_that_have_none() {
    let source = r#"
        script("T", "generator");
        let f = flame.add_final_transform();
        f.key("weight", 0.0, 1.0);
    "#;
    let err = ScriptHost::new()
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .expect_err("final transforms carry no weight");
    assert!(err.message.contains("only exists on normal transforms"), "{}", err.message);
}

// ============================================================================
// Script flags
// ============================================================================

/// `script(name, kind)` and `script(name, kind, [...])` are two arities
/// of one function, so the flag list stays optional and every script
/// written before flags existed is untouched.
#[test]
fn script_flags_are_optional() {
    let host = ScriptHost::new();
    let meta = |src: &str| {
        host.collect(src, &FractalConfig::default())
            .unwrap_or_else(|e| panic!("{}", e.message))
    };

    let plain = meta(r#"script("A", "generator");"#);
    assert!(!plain.flags.no_rng, "no flags means no switches set");

    let flagged = meta(r#"script("A", "generator", ["norng"]);"#);
    assert!(flagged.flags.no_rng);
    assert_eq!(flagged.name, "A");
    assert_eq!(flagged.kind, Some(ScriptKind::Generator));

    // An empty list is the same as omitting it.
    assert!(!meta(r#"script("A", "modifier", []);"#).flags.no_rng);
}

/// A flag this build doesn't know is an error naming the ones it does.
/// A silently ignored switch looks like the feature is broken.
#[test]
fn unknown_script_flags_are_rejected() {
    let host = ScriptHost::new();
    let err = host
        .collect(
            r#"script("A", "generator", ["norgn"]);"#,
            &FractalConfig::default(),
        )
        .expect_err("a typo'd flag must not pass silently");
    assert!(err.message.contains("unknown script flag"), "{}", err.message);
    assert!(err.message.contains("norng"), "must list what it does know: {}", err.message);

    let err = host
        .collect(r#"script("A", "generator", [1]);"#, &FractalConfig::default())
        .expect_err("flags must be strings");
    assert!(err.message.contains("must be strings"), "{}", err.message);
}

/// `norng` is a claim about behaviour, not just a UI hint: a script that
/// declares it must genuinely produce the same flame for any seed. This
/// checks the claim against the shipped scripts rather than trusting it.
#[test]
fn scripts_declaring_norng_really_ignore_the_seed() {
    let host = ScriptHost::new();
    let base = FractalConfig::default();
    let mut checked = 0;
    for (name, source) in super::library::EMBEDDED {
        let meta = host
            .collect(source, &base)
            .unwrap_or_else(|e| panic!("{name}: {}", e.message));
        if !meta.flags.no_rng {
            continue;
        }
        let run = |seed: u64| {
            serde_json::to_value(
                &host
                    .run(source, &base, seed, HashMap::new())
                    .unwrap_or_else(|e| panic!("{name}: {}", e.message))
                    .config,
            )
            .unwrap()
        };
        assert_eq!(
            run(1),
            run(9_999),
            "{name} declares `norng` but its output changes with the seed"
        );
        checked += 1;
    }
    assert!(checked >= 3, "expected several shipped scripts to declare norng");
}

// ============================================================================
// Script documentation
// ============================================================================

#[test]
fn the_header_comment_becomes_the_description() {
    let doc = super::parse_doc(
        "// Turntable\n\
         //\n\
         // Adds a looping rotation to the flame you have open.\n\
         //\n\
         // # Notes\n\
         // A full turn ends where it started.\n\
         \n\
         script(\"Turntable\", \"modifier\");\n\
         // this comment is inside the script, not its description\n",
    );
    // The opening line is taken as a TITLE, not as the summary: it just
    // repeats the script's name, which the picker already shows. The
    // summary is the first real prose after it.
    assert_eq!(doc.title, "Turntable");
    assert_eq!(doc.summary, "Adds a looping rotation to the flame you have open.");
    assert_eq!(doc.body, "# Notes\nA full turn ends where it started.");
    assert!(
        !doc.body.contains("inside the script"),
        "the header stops at the first line of code"
    );
}

#[test]
fn a_multi_line_summary_joins_into_one_paragraph() {
    let doc = super::parse_doc("// One idea spread\n// over two lines.\n//\n// Then more.\n");
    assert_eq!(doc.summary, "One idea spread over two lines.");
    assert_eq!(doc.body, "Then more.");
}

#[test]
fn scripts_without_a_header_have_no_description() {
    assert!(super::parse_doc("script(\"A\", \"generator\");").is_empty());
    assert!(super::parse_doc("").is_empty());
}

/// A `///` block pasted from Rust reads the same as `//`, since the
/// convention is borrowed from the variation definitions.
#[test]
fn rust_style_doc_comments_are_accepted() {
    let doc = super::parse_doc("/// Summary line.\n///\n/// # Authors\n/// - somebody\n");
    assert_eq!(doc.summary, "Summary line.");
    assert_eq!(doc.body, "# Authors\n- somebody");
}

/// The point of this feature: the shipped scripts already carry their
/// documentation, so it should appear without editing any of them.
///
/// Asserting merely that a summary EXISTS is not enough — the first
/// version of this passed while every summary was just the script's
/// title repeated from the picker, which is no description at all.
#[test]
fn every_shipped_script_already_documents_itself() {
    for (name, source) in super::library::EMBEDDED {
        let doc = super::parse_doc(source);
        assert!(
            !doc.summary.is_empty(),
            "{name} has no leading comment block to describe it"
        );
        assert!(
            doc.summary != doc.title,
            "{name}: the summary is just the title again"
        );
        assert!(
            doc.summary.len() > 25 && doc.summary.contains(' '),
            "{name}: summary is too short to say anything: {:?}",
            doc.summary
        );
    }
}

#[test]
#[ignore = "inspection aid: cargo test -- --ignored --nocapture show_shipped_docs"]
fn show_shipped_docs() {
    for (name, source) in super::library::EMBEDDED {
        let doc = super::parse_doc(source);
        println!("\n=== {name}");
        println!("SUMMARY: {}", doc.summary);
        println!("TITLE:   {}", doc.title);
        for line in doc.body.lines() {
            let tag = if line.is_empty() { "     " }
                else if super::doc_line_is_heading(line) { "HEAD " }
                else if line.starts_with(char::is_whitespace) { "mono " }
                else { "text " };
            println!("  {tag}{line}");
        }
    }
}

// ============================================================================
// Deleting user scripts
// ============================================================================

/// The safety property: only files in the user folder may be deleted.
///
/// `discover` hands `ScriptOrigin::File` to the shipped
/// `assets/scripts/` files as well as to the user's copies, so origin
/// alone does not say who owns a script. Without this guard a Delete
/// button would remove the starters that ship with the app.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn only_user_scripts_can_be_deleted() {
    use super::library::{delete_user_script, is_user_script, user_script_dir};

    // A shipped script is never deletable, whatever the button thinks.
    let shipped = std::path::Path::new("assets/scripts/generators/basic_random.rhai");
    if shipped.exists() {
        assert!(!is_user_script(shipped), "a shipped starter must not look like a user script");
        let err = delete_user_script(shipped).expect_err("refused");
        assert!(err.contains("not in your scripts folder"), "{err}");
        assert!(shipped.exists(), "the shipped script must still be there");
    }

    // Neither is anything else on disk.
    let outside = std::env::temp_dir().join("fflame_not_a_user_script.rhai");
    std::fs::write(&outside, "// scratch\n").unwrap();
    assert!(!is_user_script(&outside));
    assert!(delete_user_script(&outside).is_err());
    assert!(outside.exists(), "refusing to delete must not delete");
    let _ = std::fs::remove_file(&outside);

    // A real user script is deletable, so the guard isn't just "no".
    if let Some(dir) = user_script_dir() {
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join("__delete_me_test.rhai");
            if std::fs::write(&path, "// temp\nscript(\"T\", \"generator\");\n").is_ok() {
                assert!(is_user_script(&path), "a file in the user folder is the user's");
                assert!(delete_user_script(&path).is_ok());
                assert!(!path.exists(), "it should actually be gone");
            }
        }
    }
}
