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
        let mut params: HashMap<String, ParamValue> = sets
            .iter()
            .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
            .collect();
        // The script now draws the finite-depth PATH by default; these
        // cases are about the attractor construction, so ask for it.
        params
            .entry("output".to_string())
            .or_insert_with(|| ParamValue::Text("Attractor (infinite depth)".to_string()));
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

    // The script now draws the finite-depth PATH by default; this
    // is about the attractor construction, so ask for it.
    params.insert(
        "output".to_string(),
        ParamValue::Text("Attractor (infinite depth)".to_string()),
    );
    // The script now draws the finite-depth PATH by default; this
    // is about the attractor construction, so ask for it.
    params.insert(
        "output".to_string(),
        ParamValue::Text("Attractor (infinite depth)".to_string()),
    );
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

    // The script now draws the finite-depth PATH by default; this
    // is about the attractor construction, so ask for it.
    params.insert(
        "output".to_string(),
        ParamValue::Text("Attractor (infinite depth)".to_string()),
    );
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
    let mut params: HashMap<String, ParamValue> = [
        ("rule_1", "F=+G-F-G+"),
        ("rule_2", "G=-F+G+F-"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string())))
    .collect();
    // The script now draws the finite-depth PATH by default; this
    // is about the attractor construction, so ask for it.
    params.insert(
        "output".to_string(),
        ParamValue::Text("Attractor (infinite depth)".to_string()),
    );
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
    // By NAME, not by index: the option order is a presentation choice
    // and moved once already when Path became the default.
    params.insert(
        "output".to_string(),
        ParamValue::Text("Path (finite depth)".to_string()),
    );
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
    // bracket depth, weights by size. Attractor mode — tree mode has its
    // own test below.
    let source = include_str!("../../assets/scripts/generators/lsystem_plant.rhai");
    let out = run_with(source, 1, &[("mode", ParamValue::Choice(1))]).unwrap();
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
    let out = run_with(
        source,
        1,
        &[
            ("mode", ParamValue::Choice(1)),
            ("stem_weight", ParamValue::Float(0.0)),
        ],
    )
    .unwrap();
    assert_eq!(out.config.flame.transforms.len(), 4, "stem weight 0 skips stems");
}

#[test]
fn plant_script_tree_mode_builds_one_lsystem_tree_transform() {
    // The default: the finite-depth drawing lives in a single transform
    // carrying the lsystem_tree variation — branch maps for the four
    // recursion sites, stem segments for the three drawn F runs, and
    // the preset depth as a live parameter.
    let source = include_str!("../../assets/scripts/generators/lsystem_plant.rhai");
    let out = run_with(source, 1, &[]).unwrap();
    let ts = &out.config.flame.transforms;
    assert_eq!(ts.len(), 1, "tree mode is one transform");
    let t = &ts[0];
    assert!(t.variations.contains_key("lsystem_tree"));
    let get = |k: &str| *t.variation_params.get(k).unwrap_or(&0.0) as f64;
    assert_eq!(get("lsystem_tree.map_count") as i64, 4, "four recursion sites");
    assert_eq!(get("lsystem_tree.stem_count") as i64, 3, "three stem runs");
    // The default fern rule's stems all have real length.
    for j in 0..3 {
        let dx = get(&format!("lsystem_tree.s{j}_x2")) - get(&format!("lsystem_tree.s{j}_x1"));
        let dy = get(&format!("lsystem_tree.s{j}_y2")) - get(&format!("lsystem_tree.s{j}_y1"));
        assert!(
            (dx * dx + dy * dy).sqrt() > 1e-3,
            "stem {j} has length"
        );
    }
    // Branch maps are contractions: composing them must shrink, or the
    // finite drawing would blow up level over level.
    for i in 0..4 {
        let a = get(&format!("lsystem_tree.m{i}_a"));
        let c = get(&format!("lsystem_tree.m{i}_c"));
        let s = (a * a + c * c).sqrt();
        assert!(s > 0.05 && s < 0.95, "branch {i} scale sane: {s}");
    }
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

/// A flag this build doesn't know is REPORTED, naming the ones it does —
/// and reported by the collect pass, so a typo shows while editing.
///
/// This was a hard error until public script browsing made version skew
/// real: an older build must still run a newer script rather than
/// refuse it, since both flags are UI affordances and a dropped one
/// costs an affordance, never a wrong flame.
///
/// The original concern — "a silently ignored switch looks like the
/// feature is broken" — is what the warning preserves. Degrading the
/// error without carrying warnings onto `ScriptMeta` would have traded
/// that concern away, delaying the typo report until the user pressed
/// Run.
#[test]
fn unknown_script_flags_are_reported_by_the_collect_pass() {
    let host = ScriptHost::new();
    let meta = host
        .collect(
            r#"script("A", "generator", ["norgn"]);"#,
            &FractalConfig::default(),
        )
        .expect("a typo'd flag must not stop the script");
    assert_eq!(meta.warnings.len(), 1, "{:?}", meta.warnings);
    assert!(meta.warnings[0].contains("unknown script flag"), "{:?}", meta.warnings);
    assert!(
        meta.warnings[0].contains("norng"),
        "must list what it does know: {:?}",
        meta.warnings
    );
    assert!(!meta.flags.no_rng, "the typo must not set the flag it resembles");

    // A malformed flag is still an error — that is the author's mistake,
    // not version skew.
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

/// The safety property: nothing outside the user's own store can be
/// deleted.
///
/// This used to be enforced by canonicalizing a path and checking it
/// against the user folder, because `ScriptOrigin::File` covered the
/// shipped `assets/scripts/` copies too. It is now structural: `delete`
/// takes a stem and builds the key itself, so there is no path for a
/// caller to aim elsewhere, and `ScriptOrigin::User` is the only origin
/// the panel offers a Delete button for.
///
/// What is left to test is that the structure actually holds — a stem
/// that tries to climb out lands inside the store anyway.
#[test]
fn deleting_cannot_reach_outside_the_user_store() {
    use super::store;

    // A shipped starter is not in the store, so there is nothing to
    // delete under its name and the file on disk is untouched.
    let shipped = std::path::Path::new("assets/scripts/generators/basic_random.rhai");
    let existed = shipped.exists();
    assert!(store::delete("basic_random").is_err(), "nothing to delete");
    assert_eq!(shipped.exists(), existed, "the shipped script must be untouched");

    // A hostile stem is sanitized into the store rather than escaping
    // it. Write one, confirm it landed under a mangled name, remove it.
    let hostile = "../../evil";
    let stem = store::stem_for(hostile);
    assert!(!stem.contains('/') && !stem.contains('\\'), "{stem}");
    store::save(&stem, "// scratch
").expect("saves under the mangled name");
    assert!(store::load(&stem).is_some());
    store::delete(&stem).expect("and deletes again");
    assert!(store::load(&stem).is_none());
}

/// The user's store round-trips, and a saved script shows up in
/// `discover` — the property that makes a saved script usable rather
/// than merely written.
#[test]
fn a_saved_script_appears_in_the_library() {
    use super::store;
    let stem = "library-visibility-test";
    let source = "// A test script.
script(\"Visibility Test\", \"generator\");
";
    let _ = store::delete(stem);
    store::save(stem, source).expect("save");

    let entries = super::library::discover(&crate::config::FractalConfig::default());
    let found = entries
        .iter()
        .find(|e| e.id == stem)
        .expect("a saved script must be listed");
    assert_eq!(found.display_name, "Visibility Test");
    assert_eq!(found.origin, super::library::ScriptOrigin::User);
    // The picker marks the user's own with a trailing asterisk.
    assert!(found.label().ends_with(" *"), "{}", found.label());

    store::delete(stem).expect("cleanup");
    let after = super::library::discover(&crate::config::FractalConfig::default());
    assert!(!after.iter().any(|e| e.id == stem), "and it goes away again");
}

// ============================================================================
// Colour and palette building (Phase 7)
// ============================================================================

/// Scripts could previously only SELECT a palette. Building one is what
/// makes palette generation a script rather than a Rust feature.
#[test]
fn a_script_can_build_a_palette() {
    let source = r##"
        script("Pal", "generator");
        flame.add_transform();
        let base = color_hex("#ff8800");
        flame.set_palette_colors("Generated", [
            color(0.0, 0.0, 0.0),
            base,
            base.rotate_hue(180.0),
        ]);
    "##;
    let out = ScriptHost::new()
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let pal = &out.config.palette;
    assert_eq!(pal.name, "Generated");
    assert_eq!(pal.stops.len(), 3);
    // Evenly spaced, first and last pinned to the ends.
    assert!((pal.stops[0].position - 0.0).abs() < 1e-6);
    assert!((pal.stops[1].position - 0.5).abs() < 1e-6);
    assert!((pal.stops[2].position - 1.0).abs() < 1e-6);
    // #ff8800's complement is a blue — the point of having hue at all.
    let c = pal.stops[2].color;
    assert!(c[2] > c[1] && c[1] > c[0], "complement should be blue-ish: {c:?}");
}

#[test]
fn explicit_stops_are_sorted_and_validated() {
    let host = ScriptHost::new();
    let run = |body: &str| {
        let src = format!("script(\"P\", \"generator\");\nflame.add_transform();\n{body}");
        host.run(&src, &FractalConfig::default(), 1, HashMap::new())
    };

    // Listed out of order; the gradient is walked in order, so they sort.
    let out = run(r#"flame.set_palette_stops("S", [
            [0.9, color(1.0, 0.0, 0.0)],
            [0.1, color(0.0, 1.0, 0.0)],
            [0.5, color(0.0, 0.0, 1.0)],
        ]);"#)
    .unwrap();
    let pos: Vec<f32> = out.config.palette.stops.iter().map(|s| s.position).collect();
    assert_eq!(pos, vec![0.1, 0.5, 0.9]);

    // A one-colour palette is a flat fill, not a gradient; say so rather
    // than silently blanking the flame.
    let err = run(r#"flame.set_palette_colors("S", [color(1.0, 0.0, 0.0)]);"#).unwrap_err();
    assert!(err.message.contains("at least two"), "{}", err.message);

    let err = run(r#"flame.set_palette_colors("S", [1.0, 2.0]);"#).unwrap_err();
    assert!(err.message.contains("expected a color"), "{}", err.message);

    let err = run(r#"flame.set_palette_stops("S", [[0.0]]);"#).unwrap_err();
    assert!(err.message.contains("[position, color]"), "{}", err.message);
}

/// The colour parameter reaches the script as a Color, and a supplied
/// value overrides the declared default.
#[test]
fn color_params_round_trip() {
    let source = r##"
        script("P", "generator");
        let c = param_color("base", "#ff8800");
        flame.add_transform();
        flame.set_palette_colors("P", [color(0.0, 0.0, 0.0), c]);
    "##;
    let host = ScriptHost::new();
    let base = FractalConfig::default();

    let meta = host.collect(source, &base).unwrap();
    assert!(matches!(meta.params[0], ParamDecl::Color { .. }));

    let default_run = host.run(source, &base, 1, HashMap::new()).unwrap();
    let c = default_run.config.palette.stops[1].color;
    assert!((c[0] - 1.0).abs() < 1e-3 && c[2] < 0.01, "declared default: {c:?}");

    let mut supplied = HashMap::new();
    supplied.insert("base".to_string(), ParamValue::Color([0.0, 0.0, 1.0]));
    let set_run = host.run(source, &base, 1, supplied).unwrap();
    let c = set_run.config.palette.stops[1].color;
    assert!(c[2] > 0.99 && c[0] < 0.01, "supplied value should win: {c:?}");
}

/// A bad hex in the script itself is an error with the line, not a
/// silent black.
#[test]
fn a_bad_colour_literal_is_rejected() {
    let err = ScriptHost::new()
        .run(
            "script(\"P\", \"generator\");\nlet c = color_hex(\"nope\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("not a colour"), "{}", err.message);
    assert!(err.line.is_some(), "should carry a position");
}

// ============================================================================
// Script identity (Phase 7, step 2)
// ============================================================================

/// Every discovered script carries the file stem as its id, and ids are
/// unique — that is what lets one script name another, and what the
/// picker restores its selection with.
#[test]
fn discovered_scripts_have_unique_stable_ids() {
    let entries = super::library::discover(&FractalConfig::default());
    assert!(!entries.is_empty());

    let mut seen = std::collections::HashSet::new();
    for e in &entries {
        assert!(!e.id.is_empty(), "{} has no id", e.display_name);
        assert!(!e.id.contains(".rhai"), "the id is the stem: {}", e.id);
        assert!(seen.insert(e.id.clone()), "duplicate id {}", e.id);
    }

    // The shipped ids are what scripts will call each other by, so they
    // are effectively public API — pin a couple.
    assert!(super::library::find(&entries, "basic_random").is_some());
    assert!(super::library::find(&entries, "turntable").is_some());
    assert!(super::library::find(&entries, "no_such_script").is_none());
}

/// The declared NAME cannot serve as the key: nothing stops two scripts
/// sharing one, which is exactly what made the picker jump between them.
#[test]
fn ids_survive_a_duplicated_display_name() {
    let entries = super::library::discover(&FractalConfig::default());
    let a = super::library::find(&entries, "basic_random").unwrap();
    let b = super::library::find(&entries, "turntable").unwrap();

    // Give them the same declared name, as two user scripts easily could.
    let mut clash = vec![a.clone(), b.clone()];
    clash[0].display_name = "Same Name".to_string();
    clash[1].display_name = "Same Name".to_string();

    // Names no longer distinguish them; ids still do.
    assert_eq!(clash[0].display_name, clash[1].display_name);
    assert_eq!(
        clash.iter().position(|e| e.id == "turntable"),
        Some(1),
        "the id picks the right one where the name cannot"
    );
}

// ============================================================================
// One script calling another (Phase 7, step 3)
// ============================================================================

fn host_with(scripts: &[(&str, &str)]) -> ScriptHost {
    ScriptHost::new().with_scripts(
        scripts
            .iter()
            .map(|(id, src)| (id.to_string(), src.to_string()))
            .collect(),
    )
}

/// The callee works on the SAME config, which is what makes this useful
/// without a return value.
#[test]
fn a_script_can_run_another_on_the_same_flame() {
    let host = host_with(&[(
        "pal",
        "script(\"Pal\", \"modifier\");\n\
         flame.set_palette_colors(\"Sub\", [color(1.0, 0.0, 0.0), color(0.0, 0.0, 1.0)]);",
    )]);
    let out = host
        .run(
            "script(\"Main\", \"generator\");\nflame.add_transform();\nrun_script(\"pal\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap();
    assert_eq!(out.config.palette.name, "Sub");
    assert_eq!(out.config.flame.transforms.len(), 1, "the caller's work survives");
}

/// The callee continues the caller's RNG stream rather than re-using its
/// seed, so two calls give two results while the whole run still
/// reproduces from (script, seed).
#[test]
fn a_called_script_continues_the_random_stream() {
    let host = host_with(&[(
        "r",
        "script(\"R\", \"modifier\"); print(\"\" + rand(0.0, 1.0));",
    )]);
    let main = "script(\"Main\", \"generator\");\n\
                flame.add_transform();\n\
                run_script(\"r\");\n\
                run_script(\"r\");";
    let base = FractalConfig::default();

    let a = host.run(main, &base, 7, HashMap::new()).unwrap();
    assert_eq!(a.messages.len(), 2);
    assert_ne!(a.messages[0], a.messages[1], "two calls, two draws");

    let b = host.run(main, &base, 7, HashMap::new()).unwrap();
    assert_eq!(a.messages, b.messages, "same seed still reproduces");

    let c = host.run(main, &base, 8, HashMap::new()).unwrap();
    assert_ne!(a.messages, c.messages, "a different seed differs");
}

/// A caller cannot know what the script it calls declares, so the
/// DECLARATION decides the type. Naming a choice is how anyone would
/// write it; passing the index would be unreadable.
#[test]
fn callers_may_name_a_choice_rather_than_number_it() {
    let host = host_with(&[(
        "pick",
        "script(\"Pick\", \"modifier\");\n\
         let s = param_choice(\"scheme\", [\"Alpha\", \"Beta\", \"Gamma\"], 0);\n\
         print(s);",
    )]);
    let run = |arg: &str| {
        let src = format!(
            "script(\"Main\", \"generator\");\nflame.add_transform();\nrun_script(\"pick\", #{{ scheme: {arg} }});"
        );
        host.run(&src, &FractalConfig::default(), 1, HashMap::new())
    };
    assert_eq!(run("\"Gamma\"").unwrap().messages[0], "Gamma");
    assert_eq!(run("2").unwrap().messages[0], "Gamma", "index still works");
    let err = run("\"Delta\"").unwrap_err();
    assert!(err.message.contains("expects one of"), "{}", err.message);
}

/// Calling a generator from a generator is allowed, so the protection
/// has to be structural. All three guards are checked, because any one
/// alone leaves a hole.
#[test]
fn runaway_scripts_are_stopped_three_ways() {
    // 1. A cycle, however long, names the loop rather than hanging.
    let host = host_with(&[
        ("a", "script(\"A\", \"generator\"); run_script(\"b\");"),
        ("b", "script(\"B\", \"generator\"); run_script(\"a\");"),
        ("me", "script(\"Me\", \"generator\"); run_script(\"me\");"),
    ]);
    for entry in ["run_script(\"a\");", "run_script(\"me\");"] {
        let err = host
            .run(
                &format!("script(\"Main\", \"generator\");\n{entry}"),
                &FractalConfig::default(),
                1,
                HashMap::new(),
            )
            .unwrap_err();
        assert!(
            err.message.contains("call each other in a loop"),
            "{}",
            err.message
        );
    }

    // 2. A long chain that never repeats an id still stops.
    let deep: Vec<(String, String)> = (0..20)
        .map(|i| {
            (
                format!("s{i}"),
                format!("script(\"S{i}\", \"generator\"); run_script(\"s{}\");", i + 1),
            )
        })
        .collect();
    let host = ScriptHost::new().with_scripts(deep);
    let err = host
        .run(
            "script(\"Main\", \"generator\");\nrun_script(\"s0\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("nested more than"), "{}", err.message);

    // 3. The operation budget is SHARED. Rhai counts per evaluation, so
    // without this a script would buy a fresh allowance for every call
    // it made — the sandbox hole that matters, since scripts are shared.
    let host = host_with(&[(
        "burn",
        "script(\"Burn\", \"modifier\"); let x = 0; for i in 0..400000 { x += i; }",
    )]);
    let err = host
        .run(
            "script(\"Main\", \"generator\");\nfor i in 0..200 { run_script(\"burn\"); }",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    let low = err.message.to_lowercase();
    assert!(
        low.contains("budget") || low.contains("operation"),
        "{}",
        err.message
    );
}

/// An unattributed line number in a file the reader never opened is a
/// dead end, so a failure inside a called script says which one.
#[test]
fn errors_name_the_script_they_came_from() {
    let host = host_with(&[(
        "bad",
        "script(\"Bad\", \"modifier\");\nlet c = color_hex(\"nope\");",
    )]);
    let err = host
        .run(
            "script(\"Main\", \"generator\");\nrun_script(\"bad\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("bad"), "names the script: {}", err.message);
    assert!(err.message.contains("line 2"), "and the line in it: {}", err.message);
}

/// An unknown id lists what is available; no library at all says so.
#[test]
fn calling_a_missing_script_is_explained() {
    let host = host_with(&[("known", "script(\"K\", \"modifier\");")]);
    let err = host
        .run(
            "script(\"Main\", \"generator\");\nrun_script(\"typo\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("no script with id"), "{}", err.message);
    assert!(err.message.contains("known"), "lists what there is: {}", err.message);

    // A default host can always reach the EMBEDDED starters — a shipped
    // script that calls another has to work from every entry point — so
    // an empty library now takes asking for one.
    assert!(
        ScriptHost::new()
            .run(
                "script(\"Main\", \"generator\");\nrun_script(\"random_palette\");",
                &FractalConfig::default(),
                1,
                HashMap::new(),
            )
            .is_ok(),
        "the shipped scripts must be callable without a discovered library"
    );

    let err = ScriptHost::new()
        .with_scripts(Vec::new())
        .run(
            "script(\"Main\", \"generator\");\nrun_script(\"anything\");",
            &FractalConfig::default(),
            1,
            HashMap::new(),
        )
        .unwrap_err();
    assert!(err.message.contains("no script library"), "{}", err.message);
}

/// The callee's parameters belong to it, not to the caller's panel.
#[test]
fn a_callees_parameters_stay_out_of_the_callers_metadata() {
    let host = host_with(&[(
        "sub",
        "script(\"Sub\", \"modifier\"); let x = param(\"sub_only\", 1.0, 0.0, 2.0);",
    )]);
    let main = "script(\"Main\", \"generator\");\n\
                let mine = param(\"mine\", 1.0, 0.0, 2.0);\n\
                flame.add_transform();\n\
                run_script(\"sub\");";
    let out = host
        .run(main, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    let keys: Vec<&str> = out.meta.params.iter().map(|p| p.key()).collect();
    assert_eq!(keys, vec!["mine"], "the caller declares only its own");
    assert!(
        out.warnings.is_empty(),
        "and no spurious warnings: {:?}",
        out.warnings
    );
}

/// The Palette Editor offers exactly the scripts that opt in, which is
/// what buys back the generate button that moving generation into
/// scripts would otherwise have cost.
#[test]
fn palette_scripts_opt_in_by_flag() {
    let base = FractalConfig::default();
    let host = ScriptHost::new();
    let entries = super::library::discover(&base);

    let flagged: Vec<&str> = entries
        .iter()
        .filter(|e| {
            host.collect(&e.source, &base)
                .map(|m| m.flags.palette)
                .unwrap_or(false)
        })
        .map(|e| e.id.as_str())
        .collect();

    assert!(
        flagged.contains(&"random_palette"),
        "the shipped palette script must offer itself: {flagged:?}"
    );
    assert!(
        !flagged.contains(&"turntable"),
        "an unflagged script must not appear in the Palette panel: {flagged:?}"
    );
    assert!(
        flagged.len() < entries.len(),
        "the flag must actually filter, not pass everything"
    );
}

/// A palette script has to produce a usable palette from the panel's
/// route: run it, take the palette, and nothing else.
#[test]
fn the_shipped_palette_script_produces_a_palette() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "random_palette").expect("shipped");

    let host = ScriptHost::new();
    let a = host.run(&entry.source, &base, 3, HashMap::new()).unwrap();
    assert!(a.config.palette.stops.len() >= 2, "a gradient needs two stops");
    assert!(
        a.config.palette.name.starts_with("Generated"),
        "named so it is obvious where it came from: {}",
        a.config.palette.name
    );

    // Same seed reproduces, so a palette worth keeping can be got back.
    let b = host.run(&entry.source, &base, 3, HashMap::new()).unwrap();
    assert_eq!(a.config.palette.stops, b.config.palette.stops);

    // Reroll is the next seed along, and must give something else.
    let c = host.run(&entry.source, &base, 4, HashMap::new()).unwrap();
    assert_ne!(a.config.palette.stops, c.config.palette.stops);

    // The panel takes only the palette, but the script should not be
    // wandering outside its remit anyway.
    assert_eq!(
        a.config.flame.transforms.len(),
        base.flame.transforms.len(),
        "a palette script should leave the flame alone"
    );
}

// ============================================================================
// Fixed-slot palettes, shuffling and noise
// ============================================================================

/// Fixed mode is what makes slicing and per-slot noise simple: 256 even
/// slots instead of stops sitting wherever the script put them.
#[test]
fn a_script_can_work_slot_by_slot() {
    let source = "script(\"P\", \"generator\");\n\
        flame.add_transform();\n\
        flame.set_palette_colors(\"G\", [color(0.0, 0.0, 0.0), color(1.0, 1.0, 1.0)]);\n\
        flame.palette_to_fixed();\n\
        let slots = flame.palette_colors();\n\
        print(\"\" + slots.len());\n\
        flame.set_palette_fixed(\"Fixed\", slots);";
    let out = ScriptHost::new()
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();

    assert_eq!(out.messages[0], "256", "fixed mode is 256 slots");
    let pal = &out.config.palette;
    assert_eq!(pal.name, "Fixed");
    assert_eq!(pal.stops.len(), 256);
    assert!(pal.locked, "a fixed palette is locked");
    // Evenly spaced, ends pinned.
    assert!((pal.stops[0].position - 0.0).abs() < 1e-6);
    assert!((pal.stops[255].position - 1.0).abs() < 1e-6);
    assert!((pal.stops[128].position - 128.0 / 255.0).abs() < 1e-6);
}

/// Fewer colours than slots are resampled rather than rejected: a script
/// that built sixteen still means a palette.
#[test]
fn set_palette_fixed_resamples_to_the_full_length() {
    let source = "script(\"P\", \"generator\");\n\
        flame.add_transform();\n\
        let cols = [];\n\
        for i in 0..16 { cols.push(color_hsv(i.to_float() * 22.0, 1.0, 1.0)); }\n\
        flame.set_palette_fixed(\"R\", cols);";
    let out = ScriptHost::new()
        .run(source, &FractalConfig::default(), 1, HashMap::new())
        .unwrap();
    assert_eq!(out.config.palette.stops.len(), 256);
    assert!(out.config.palette.locked);
}

/// The shipped script's three stages each have to do something, and the
/// result has to stay a well-formed 256-slot palette.
#[test]
fn the_palette_script_shuffles_and_adds_noise() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "random_palette").expect("shipped");
    let host = ScriptHost::new();

    let run = |params: Vec<(&str, ParamValue)>| {
        let map: HashMap<String, ParamValue> = params
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        host.run(&entry.source, &base, 11, map).unwrap().config.palette
    };

    // Plain: a smooth 256-slot palette.
    let plain = run(vec![]);
    assert_eq!(plain.stops.len(), 256);
    assert!(plain.locked, "the script leaves the palette in fixed mode");

    // Shuffling reorders the slots without inventing or losing any.
    let shuffled = run(vec![
        ("shuffle_slices", ParamValue::Int(8)),
        ("shuffle_amount", ParamValue::Float(1.0)),
    ]);
    assert_eq!(shuffled.stops.len(), 256, "still full length");
    assert_ne!(
        shuffled.stops.iter().map(|s| s.color[0].to_bits()).collect::<Vec<_>>(),
        plain.stops.iter().map(|s| s.color[0].to_bits()).collect::<Vec<_>>(),
        "shuffling must actually move something"
    );
    let mut a: Vec<u32> = plain.stops.iter().map(|s| s.color[0].to_bits()).collect();
    let mut b: Vec<u32> = shuffled.stops.iter().map(|s| s.color[0].to_bits()).collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(a, b, "the same colours, reordered — nothing invented or lost");

    // Noise on one axis only must leave the others alone, which is the
    // whole reason for jittering in HSV rather than RGB.
    let hue_only = run(vec![("hue_noise", ParamValue::Float(60.0))]);
    let mut moved_hue = 0;
    let mut moved_val = 0;
    for (p, n) in plain.stops.iter().zip(hue_only.stops.iter()) {
        let pc = crate::script::color::ScriptColor::from_rgb(p.color);
        let nc = crate::script::color::ScriptColor::from_rgb(n.color);
        if (pc.hue() - nc.hue()).abs() > 1.0 {
            moved_hue += 1;
        }
        if (pc.value() - nc.value()).abs() > 0.02 {
            moved_val += 1;
        }
    }
    assert!(moved_hue > 100, "hue noise should scatter the hues: {moved_hue}");
    assert_eq!(moved_val, 0, "and leave value alone: {moved_val} slots moved");
}

/// The procedural palette must actually evaluate Quilez's formula, not
/// merely produce 256 of something.
#[test]
fn the_procedural_palette_follows_the_cosine_formula() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "iq_palette").expect("shipped");
    let host = ScriptHost::new();

    let pal = host
        .run(&entry.source, &base, 1, HashMap::new())
        .unwrap()
        .config
        .palette;
    assert_eq!(pal.stops.len(), 256);
    assert!(pal.locked, "sampled straight into fixed slots");

    // Default preset is the rainbow: a = b = 0.5, c = 1,
    // d = (0, 0.33, 0.67). Check a few slots against the formula.
    let f = |a: f32, b: f32, c: f32, d: f32, t: f32| {
        a + b * (std::f32::consts::TAU * (c * t + d)).cos()
    };
    for i in [0usize, 64, 137, 255] {
        let t = i as f32 / 255.0;
        let want = [
            f(0.5, 0.5, 1.0, 0.0, t),
            f(0.5, 0.5, 1.0, 0.33, t),
            f(0.5, 0.5, 1.0, 0.67, t),
        ];
        let got = pal.stops[i].color;
        for ch in 0..3 {
            // The palette clamps to 0..1, as the formula's output must be.
            let want_c = want[ch].clamp(0.0, 1.0);
            assert!(
                (got[ch] - want_c).abs() < 2e-3,
                "slot {i} channel {ch}: got {} want {want_c}",
                got[ch]
            );
        }
    }

    // Three channels a third of a cycle apart is what makes it a
    // rainbow: the hue should travel a long way, not sit still.
    let hues: Vec<f32> = pal
        .stops
        .iter()
        .map(|s| crate::script::color::ScriptColor::from_rgb(s.color).hue())
        .collect();
    let span = hues.iter().cloned().fold(0.0f32, f32::max)
        - hues.iter().cloned().fold(360.0f32, f32::min);
    assert!(span > 180.0, "the default preset should sweep hues: {span}");
}

/// Custom uses the declared parameters; a preset overrides them. Both
/// halves matter — a preset that silently used the sliders, or sliders
/// that silently did nothing, would look identical from the outside.
#[test]
fn presets_and_custom_differ_as_advertised() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "iq_palette").expect("shipped");
    let host = ScriptHost::new();

    let run = |params: Vec<(&str, ParamValue)>| {
        let map: HashMap<String, ParamValue> =
            params.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        host.run(&entry.source, &base, 1, map).unwrap().config.palette
    };

    // A preset ignores the sliders.
    let preset_default = run(vec![("preset", ParamValue::Text("Bright".into()))]);
    let preset_fiddled = run(vec![
        ("preset", ParamValue::Text("Bright".into())),
        ("freq_r", ParamValue::Float(5.0)),
    ]);
    assert_eq!(
        preset_default.stops[100].color, preset_fiddled.stops[100].color,
        "a preset must not be moved by the sliders it ignores"
    );

    // Custom obeys them.
    let custom_a = run(vec![("preset", ParamValue::Text("Custom".into()))]);
    let custom_b = run(vec![
        ("preset", ParamValue::Text("Custom".into())),
        ("freq_r", ParamValue::Float(5.0)),
    ]);
    assert_ne!(
        custom_a.stops[100].color, custom_b.stops[100].color,
        "Custom must use the sliders"
    );
}

/// Basic Random hands Random Palette its own seed, so the two agree:
/// the palette on a generated flame is the one Random Palette produces
/// on its own at the same seed. That is what makes it a starting point
/// you can go and adjust rather than a dead end.
#[test]
fn the_generator_and_the_palette_script_share_a_seed() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let host = ScriptHost::new().with_scripts(
        entries
            .iter()
            .map(|e| (e.id.clone(), e.source.clone()))
            .collect(),
    );
    let generator = super::library::find(&entries, "basic_random").expect("shipped");
    let palette_script = super::library::find(&entries, "random_palette").expect("shipped");

    // The settings Basic Random passes through.
    let mut direct: HashMap<String, ParamValue> = HashMap::new();
    direct.insert("scheme".into(), ParamValue::Text("Monochromatic".into()));
    direct.insert("base".into(), ParamValue::Text("#000000".into()));
    direct.insert("stops".into(), ParamValue::Int(5));
    direct.insert("spread".into(), ParamValue::Float(0.35));
    direct.insert("dark_end".into(), ParamValue::Bool(true));
    direct.insert("shuffle_slices".into(), ParamValue::Int(10));
    direct.insert("shuffle_amount".into(), ParamValue::Float(1.0));
    direct.insert("hue_noise".into(), ParamValue::Float(85.0));
    direct.insert("sat_noise".into(), ParamValue::Float(0.0));
    direct.insert("val_noise".into(), ParamValue::Float(0.2));

    for seed in [3u64, 12, 8842] {
        let flame = host
            .run(&generator.source, &base, seed, HashMap::new())
            .unwrap()
            .config;
        let alone = host
            .run(&palette_script.source, &base, seed, direct.clone())
            .unwrap()
            .config;
        // Compared at the precision the format actually stores. A
        // locked palette is 8-bit hex on disk, and `config.set` — which
        // Basic Random calls after building the palette — round-trips
        // the whole config through JSON, so the generated one arrives
        // already quantised while the standalone one has not been saved
        // yet. Comparing raw floats would fail on that alone; a
        // genuinely different palette differs by far more than 1/255.
        let bytes = |p: &crate::scene::palette::Palette| -> Vec<[u8; 3]> {
            p.stops
                .iter()
                .map(|s| {
                    let c = s.color;
                    [
                        (c[0] * 255.0).round() as u8,
                        (c[1] * 255.0).round() as u8,
                        (c[2] * 255.0).round() as u8,
                    ]
                })
                .collect()
        };
        assert_eq!(
            bytes(&flame.palette),
            bytes(&alone.palette),
            "seed {seed}: the flame's palette must be reproducible on its own"
        );
    }
}

/// Generated is the default, and it really does replace the palette.
#[test]
fn basic_random_generates_a_palette_by_default() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let host = ScriptHost::new().with_scripts(
        entries
            .iter()
            .map(|e| (e.id.clone(), e.source.clone()))
            .collect(),
    );
    let generator = super::library::find(&entries, "basic_random").expect("shipped");

    let out = host.run(&generator.source, &base, 4, HashMap::new()).unwrap();
    assert!(
        out.config.palette.name.starts_with("Generated"),
        "default should generate: {}",
        out.config.palette.name
    );
    assert_eq!(out.config.palette.stops.len(), 256, "and in fixed mode");

    // Keeping the current palette must still be possible.
    let mut keep: HashMap<String, ParamValue> = HashMap::new();
    keep.insert("palette".into(), ParamValue::Text("Keep current".into()));
    let kept = host.run(&generator.source, &base, 4, keep).unwrap();
    assert_eq!(
        kept.config.palette.name, base.palette.name,
        "Keep current must leave the palette alone"
    );
}

/// An explicit seed must not disturb the caller: whether it hands one
/// over or not, everything it draws afterwards is the same.
#[test]
fn an_explicit_seed_does_not_shift_the_callers_stream() {
    let host = ScriptHost::new().with_scripts(vec![(
        "sub".to_string(),
        "script(\"Sub\", \"modifier\"); let x = rand(0.0, 1.0);".to_string(),
    )]);
    let after = |call: &str| {
        let src = format!(
            "script(\"Main\", \"generator\");\nflame.add_transform();\n{call}\nprint(\"\" + rand(0.0, 1.0));"
        );
        host.run(&src, &FractalConfig::default(), 21, HashMap::new())
            .unwrap()
            .messages[0]
            .clone()
    };
    let none = after("");
    let seeded = after("run_script(\"sub\", #{}, 99);");
    let streamed = after("run_script(\"sub\");");

    assert_eq!(none, seeded, "a seeded call must not consume caller randomness");
    assert_ne!(none, streamed, "an unseeded call continues the stream, as before");
}

/// The Random preset rolls its own set, seeded like everything else, so
/// Mutate (consecutive seeds) gives a batch to choose from and a roll
/// worth keeping comes back.
#[test]
fn the_random_preset_varies_with_the_seed() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "iq_palette").expect("shipped");
    let host = ScriptHost::new();

    let roll = |seed: u64| {
        let mut p: HashMap<String, ParamValue> = HashMap::new();
        p.insert("preset".into(), ParamValue::Text("Random".into()));
        host.run(&entry.source, &base, seed, p).unwrap().config.palette
    };

    let a = roll(1);
    let b = roll(2);
    let again = roll(1);

    assert_eq!(a.stops, again.stops, "a roll must come back from its seed");
    assert_ne!(a.stops, b.stops, "consecutive seeds must differ, or Mutate is pointless");
    assert_eq!(a.stops.len(), 256);

    // Every roll has to be a usable palette: in range, and actually
    // going somewhere rather than a flat wash.
    for seed in 1..12u64 {
        let pal = roll(seed);
        for s in &pal.stops {
            for ch in s.color {
                assert!((0.0..=1.0).contains(&ch), "seed {seed}: {ch} out of range");
            }
        }
        let hues: Vec<f32> = pal
            .stops
            .iter()
            .map(|s| crate::script::color::ScriptColor::from_rgb(s.color).hue())
            .collect();
        let span = hues.iter().cloned().fold(0.0f32, f32::max)
            - hues.iter().cloned().fold(360.0f32, f32::min);
        let vals: Vec<f32> = pal
            .stops
            .iter()
            .map(|s| crate::script::color::ScriptColor::from_rgb(s.color).value())
            .collect();
        let vspan = vals.iter().cloned().fold(0.0f32, f32::max)
            - vals.iter().cloned().fold(1.0f32, f32::min);
        assert!(
            span > 30.0 || vspan > 0.25,
            "seed {seed}: neither hue ({span}) nor brightness ({vspan}) moves — a flat wash"
        );
    }
}

// ============================================================================
// L-system presets
// ============================================================================

/// Every preset in both L-system scripts must actually build something.
///
/// The list is read from the script's own declaration rather than
/// repeated here, so adding a preset adds a case automatically — a
/// hand-kept copy would go stale the first time one is added and quietly
/// stop testing it.
///
/// Four presets were dropped or replaced during development because they
/// failed exactly this way: "2D Weed" and "3D Bush" built no transforms
/// at all, while "2D Fan" and a 3D Koch rule built plenty and rendered
/// as a full-frame wash or a bare squiggle. The transform count alone
/// does not catch the last two, so this also insists the script did not
/// report a failure in its own messages.
#[test]
fn every_lsystem_preset_builds_something() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let host = ScriptHost::new();

    for id in ["lsystem", "lsystem_plant"] {
        let entry = super::library::find(&entries, id).unwrap_or_else(|| panic!("{id} shipped"));
        let meta = host.collect(&entry.source, &base).unwrap();

        let options = meta
            .params
            .iter()
            .find_map(|p| match p {
                ParamDecl::Choice { key, options, .. } if key == "preset" => Some(options.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{id} declares a preset parameter"));

        assert!(options.len() > 8, "{id}: expected a real list, got {options:?}");
        assert_eq!(options[0], "Custom", "{id}: Custom must be the default");
        assert!(
            options.iter().any(|o| o.starts_with("2D")),
            "{id}: expected a 2D section"
        );
        assert!(
            options.iter().any(|o| o.starts_with("3D")),
            "{id}: expected a 3D section"
        );

        for option in &options {
            let mut params: HashMap<String, ParamValue> = HashMap::new();
            params.insert("preset".into(), ParamValue::Text(option.clone()));
            let out = host
                .run(&entry.source, &base, 1, params)
                .unwrap_or_else(|e| panic!("{id} / {option}: {}", e.message));

            let flame = &out.config.flame;
            assert!(
                !flame.transforms.is_empty(),
                "{id} / {option}: built no transforms"
            );

            // The scripts say so in plain words when a rule defeats them,
            // which a transform count does not reveal.
            for m in &out.messages {
                let low = m.to_lowercase();
                assert!(
                    !low.contains("could not build") && !low.contains("nothing to"),
                    "{id} / {option}: {m}"
                );
            }
        }
    }
}

/// Custom must leave the fields alone — it is the default, so a preset
/// leaking into it would silently override whatever the user typed.
#[test]
fn the_custom_preset_changes_nothing() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let host = ScriptHost::new();

    for id in ["lsystem", "lsystem_plant"] {
        let entry = super::library::find(&entries, id).unwrap();

        let mut custom: HashMap<String, ParamValue> = HashMap::new();
        custom.insert("preset".into(), ParamValue::Text("Custom".into()));
        let with_custom = host.run(&entry.source, &base, 1, custom).unwrap();
        let untouched = host.run(&entry.source, &base, 1, HashMap::new()).unwrap();

        assert_eq!(
            serde_json::to_value(&with_custom.config).unwrap(),
            serde_json::to_value(&untouched.config).unwrap(),
            "{id}: Custom must behave exactly as the default"
        );
    }
}

/// The Curve script defaults to drawing the finite-depth PATH, which is
/// the picture people have in mind and the only mode that works for a 3D
/// rule whose infinite-depth limit never settles.
#[test]
fn the_curve_script_draws_a_path_by_default() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "lsystem").unwrap();
    let host = ScriptHost::new();

    let out = host.run(&entry.source, &base, 1, HashMap::new()).unwrap();
    assert_eq!(
        out.config.flame.transforms.len(),
        1,
        "path mode carries the whole curve on one transform"
    );
    assert!(
        out.config.flame.transforms[0]
            .variations
            .contains_key("lsystem_path"),
        "expected the path variation, got {:?}",
        out.config.flame.transforms[0].variations
    );

    // A preset may lower the depth: Peano lays down nine segments per
    // level, so the default of five would be 59049 of them and fill the
    // square solid rather than draw the curve.
    let mut deep: HashMap<String, ParamValue> = HashMap::new();
    deep.insert("preset".into(), ParamValue::Text("2D · Peano curve".into()));
    let out = host.run(&entry.source, &base, 1, deep).unwrap();
    let iters = out.config.flame.transforms[0]
        .variation_params
        .get("lsystem_path.iterations")
        .copied()
        .expect("path depth");
    assert_eq!(iters, 3.0, "the preset should ask for a shallower depth");
}

/// The standard 3D Hilbert L-system rule is only self-similar two
/// levels at a time — its shape alternates between two poses — so no
/// single set of maps can draw it. The script must say exactly that and
/// name the script that CAN draw the cube-filling curve, rather than
/// produce the disconnected tangle the old extraction drew.
#[test]
fn a_two_periodic_3d_rule_is_refused_with_a_pointer() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "lsystem").unwrap();

    let mut params: HashMap<String, ParamValue> = HashMap::new();
    params.insert("axiom".into(), ParamValue::Text("X".into()));
    params.insert(
        "rule_1".into(),
        ParamValue::Text("X=^\\XF^\\XFX-F^//XFX&F+//XFX-F/X-/".into()),
    );
    params.insert("angle".into(), ParamValue::Float(90.0));

    let out = ScriptHost::new().run(&entry.source, &base, 1, params).unwrap();
    assert!(
        out.messages.iter().any(|m| m.contains("two levels at a time")),
        "expected the two-pose refusal: {:?}",
        out.messages
    );
    assert!(
        out.messages.iter().any(|m| m.contains("Hilbert Curve 3D")),
        "and the pointer at the script that works: {:?}",
        out.messages
    );
    assert!(
        out.config.flame.transforms.is_empty(),
        "nothing should be built from a rule that cannot converge"
    );
}

/// A REVERSE partner draws the primary's piece walked backwards, which
/// is the whole of what separates the Heighway dragon from the Levy C:
/// the two share their depth-1 drawn path exactly.
///
/// Checked against the dragon's known IFS rather than against a picture
/// — two maps at 1/sqrt(2), rotated 45 and 135, seated at (0,0) and
/// (1,0), both with POSITIVE determinant. That last point is why a
/// mirror is the wrong tool: a reflection would make one negative.
#[test]
fn the_dragon_is_built_from_a_reversed_piece() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "lsystem").unwrap();

    let mut params: HashMap<String, ParamValue> = HashMap::new();
    params.insert("preset".into(), ParamValue::Text("2D · Dragon curve".into()));
    params.insert(
        "output".into(),
        ParamValue::Text("Attractor (infinite depth)".into()),
    );
    let out = ScriptHost::new().run(&entry.source, &base, 1, params).unwrap();

    assert!(
        out.messages.iter().any(|m| m.contains("Reverse pair")),
        "the dragon needs its second piece reversed: {:?}",
        out.messages
    );

    let t = &out.config.flame.transforms;
    assert_eq!(t.len(), 2, "the dragon is two maps");
    let inv_root2 = 1.0f32 / 2.0f32.sqrt();
    for (i, tr) in t.iter().enumerate() {
        let scale = (tr.a * tr.a + tr.c * tr.c).sqrt();
        let det = tr.a * tr.d - tr.b * tr.c;
        assert!(
            (scale - inv_root2).abs() < 1e-3,
            "map {i} scale {scale}, expected {inv_root2}"
        );
        assert!(det > 0.0, "map {i} is a reflection, not a reversal: {det}");
    }
    // The second map is seated at the FAR end — the reversal made
    // visible. Forward-traversed it would sit on the apex at (0.5, 0.5),
    // which is exactly the Levy C.
    let far = &t[1];
    assert!(
        (far.e - 1.0).abs() < 1e-3 && far.f.abs() < 1e-3,
        "expected the second map at (1, 0), got ({}, {})",
        far.e,
        far.f
    );
}

/// A mirror wins where both tests match, and both DO match for a rule
/// that reads the same backwards — reversing and mirroring are then the
/// same string operation. Sierpinski and Hilbert land there and want the
/// mirror; applying the reversal as well turned the arrowhead into an
/// open arc and broke the Hilbert maze.
#[test]
fn a_mirror_partner_wins_over_a_reverse_partner() {
    let base = FractalConfig::default();
    let entries = super::library::discover(&base);
    let entry = super::library::find(&entries, "lsystem").unwrap();
    let host = ScriptHost::new();

    let run = |preset: &str| {
        let mut p: HashMap<String, ParamValue> = HashMap::new();
        p.insert("preset".into(), ParamValue::Text(preset.into()));
        host.run(&entry.source, &base, 1, p).unwrap()
    };

    for preset in ["2D · Sierpinski arrowhead", "2D · Hilbert curve"] {
        let out = run(preset);
        assert!(
            out.messages.iter().any(|m| m.contains("Mirror pair")),
            "{preset}: expected a mirror pair, got {:?}",
            out.messages
        );
        assert!(
            !out.messages.iter().any(|m| m.contains("Reverse pair")),
            "{preset}: a mirror must suppress the reversal, got {:?}",
            out.messages
        );
    }

    // And where there is no mirror, the reversal must still apply — the
    // Gosper curve was drawn wrong until it did.
    let gosper = run("2D · Gosper curve");
    assert!(
        gosper.messages.iter().any(|m| m.contains("Reverse pair")),
        "{:?}",
        gosper.messages
    );
}

/// The syntactic test, on its own terms.
#[test]
fn reverse_partners_are_read_from_the_rule_backwards() {
    use crate::script::builtins::{mirror_partner, reverse_partner};
    let rules = |pairs: &[(char, &str)]| -> Vec<(char, String)> {
        pairs.iter().map(|(c, s)| (*c, s.to_string())).collect()
    };

    // F -> "F+G": reverse the order, flip the turns, swap the symbols,
    // and you land on G's rule exactly.
    let dragon = rules(&[('F', "F+G"), ('G', "F-G")]);
    assert_eq!(reverse_partner(&dragon, 'F'), Some('G'));
    assert_eq!(mirror_partner(&dragon, 'F'), None, "not a reflection");

    // Koch has one symbol and neither partner.
    let koch = rules(&[('F', "F+F--F+F")]);
    assert_eq!(reverse_partner(&koch, 'F'), None);
}

// ---------------------------------------------------------------- limits
//
// Each of these reproduces an attack that was confirmed to abort or hang
// the process before the guard existed. A shared script is meant to be
// safe to run; these are the cases where it was not.

#[test]
fn a_xaos_row_cannot_ask_for_an_unbounded_allocation() {
    // `vec![1.0f32; count]` with an unbounded count: i64::MAX aborted
    // the process with `capacity overflow` from this single line.
    let err = run(
        r#"script("X", "generator"); exclude_xaos_row(0, 9223372036854775807);"#,
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("at most"), "{err}");

    let err = run(
        r#"script("X", "generator"); repeat_xaos_row(0, 9223372036854775807);"#,
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("at most"), "{err}");

    // A legitimate row still works.
    let out = run(
        r#"script("X", "generator"); print("" + exclude_xaos_row(1, 4).len());"#,
        1,
    )
    .unwrap();
    assert_eq!(out.messages, vec!["4"]);
}

#[test]
fn transforms_stop_at_the_renderer_limit() {
    // Unbounded, a script built 200,000 transforms — undrawable, and it
    // armed the O(n^2) table build in set_xaos.
    let err = run(
        r#"script("X", "generator"); for i in 0..200 { flame.add_transform(); }"#,
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("at most"), "{err}");

    // Up to the limit is fine.
    let out = run(
        r#"script("X", "generator");
           for i in 0..128 { flame.add_transform(); }
           print("" + flame.transform_count());"#,
        1,
    )
    .unwrap();
    assert_eq!(out.messages, vec!["128"]);

    // The budget is SHARED across the three pools, not per pool.
    //
    // MAX_TRANSFORMS bounds normals + linkeds + finals together — they
    // pack into one [0, MAX_TRANSFORMS) region — and
    // `Buffers::update_transforms` panics on the total. Checking each
    // pool separately let a script build 128 + 128 and abort at render
    // with "Flame has 256 total transform slots".
    let err = run(
        r#"script("X", "generator");
           for i in 0..128 { flame.add_transform(); }
           flame.add_final_transform();"#,
        1,
    )
    .unwrap_err();
    assert!(err.message.contains("in total"), "{err}");

    // A mixed flame that fits the shared budget is accepted.
    let out = run(
        r#"script("X", "generator");
           for i in 0..100 { flame.add_transform(); }
           for i in 0..20  { flame.add_linked_transform(); }
           for i in 0..8   { flame.add_final_transform(); }
           print("ok");"#,
        1,
    )
    .unwrap();
    assert_eq!(out.messages, vec!["ok"]);
}

#[test]
fn an_oversized_lsystem_rule_is_refused_before_the_walk() {
    // The piece walks are rule.chars() x body.chars(). A 200,000-char
    // rule of "[X" made one call run without returning (measured: still
    // going at 60s, uninterruptible — the op budget cannot see native
    // work, and the panel runs on the UI thread).
    let script = r#"
        script("X", "generator");
        let r = "[X";
        while r.len() < 200000 { r += r; }
        lsystem_plant_pieces("X", #{"X": r}, 25.0);
    "#;
    let err = run(script, 1).unwrap_err();
    assert!(err.message.contains("the limit is"), "{err}");

    // A real rule — the fern — is nowhere near the ceiling.
    let out = run(
        r#"script("X", "generator");
           let p = lsystem_plant_pieces("X", #{"X": "F-[[X]+X]+F[+FX]-X", "F": "FF"}, 22.5);
           print("" + p.branches.len());"#,
        1,
    )
    .unwrap();
    assert_eq!(out.messages, vec!["4"]);
}

#[test]
fn oversized_lsystem_input_is_refused_on_every_entry_point() {
    // The cap belongs at the boundary, not on one function: every entry
    // point taking a rule set funnels into the same check.
    let big = "F".repeat(5000);
    for call in [
        "lsystem(\"F\", #{\"F\": R}, 2)",
        "lsystem_bounds(\"F\", #{\"F\": R}, 3, 60.0)",
        "lsystem_bounds3(\"F\", #{\"F\": R}, 3, 60.0)",
        "lsystem_pieces3(\"F\", #{\"F\": R}, 60.0)",
        "lsystem_node_pieces(\"F\", #{\"F\": R}, 60.0)",
        "lsystem_graph_pieces(\"F\", #{\"F\": R}, 60.0)",
        "lsystem_curve_pieces3(\"F\", #{\"F\": R}, 60.0)",
    ] {
        let script = format!(
            "script(\"X\", \"generator\"); let R = \"{big}\"; {call};"
        );
        let err = match run(&script, 1) {
            Err(e) => e,
            Ok(_) => panic!("`{call}` accepted an oversized rule"),
        };
        assert!(
            err.message.contains("the limit is"),
            "`{call}` failed for the wrong reason: {err}"
        );
    }
}

// ============================================================================
// Markdown stripping (client-side, for script prose)
// ============================================================================

/// The case that would actually bite: this codebase's prose is full of
/// `snake_case`, and naive `_`-emphasis stripping mangles it.
///
/// Not hypothetical — every shipped script's header comment names other
/// scripts and API functions this way, so getting it wrong would corrupt
/// the descriptions of the ten scripts that ship.
#[test]
fn underscores_in_identifiers_survive() {
    use super::strip_markdown;
    for s in [
        "run_script(\"random_palette\") from basic_random",
        "lsystem_plant and lsystem_pieces3",
        "a_b_c_d_e",
        "snake_case_name at the end",
        "__leading and trailing__ around snake_case_word",
    ] {
        let out = strip_markdown(s);
        assert!(
            out.contains("_") || !s.contains("_"),
            "identifiers lost their underscores: {s:?} -> {out:?}"
        );
    }
    // Specifically: the identifier is untouched.
    assert_eq!(
        strip_markdown("call run_script from basic_random"),
        "call run_script from basic_random"
    );
    // ...while real emphasis at word boundaries still strips.
    assert_eq!(strip_markdown("this is _emphasis_ here"), "this is emphasis here");
    assert_eq!(strip_markdown("_bold_ start"), "bold start");
}

/// Inline syntax goes; the text stays.
#[test]
fn inline_syntax_is_removed() {
    use super::strip_markdown;
    assert_eq!(strip_markdown("**bold**"), "bold");
    assert_eq!(strip_markdown("*italic*"), "italic");
    assert_eq!(strip_markdown("`code_span`"), "code_span");
    assert_eq!(strip_markdown("see [the docs](http://x/y) now"), "see the docs now");
    assert_eq!(strip_markdown("![a picture](img.png)"), "a picture");
    assert_eq!(strip_markdown("**bold with *italic* inside**"), "bold with italic inside");
    assert_eq!(strip_markdown("escaped \\*not emphasis\\*"), "escaped *not emphasis*");
}

/// A doubled backslash is left alone, deviating from CommonMark on
/// purpose.
///
/// `\\` means one literal backslash in markdown. Here it is an L-system
/// turtle symbol: two shipped scripts document `& ^ \ /` as pitch and
/// roll, and applying the strict rule silently rewrites their prose.
/// Both shipped cases are covered by
/// `shipped_script_prose_is_not_corrupted`; this pins the rule itself,
/// so nobody "fixes" `md_escapable` back to CommonMark without seeing
/// why it is not.
#[test]
fn a_doubled_backslash_is_a_turtle_symbol_not_an_escape() {
    use super::strip_markdown;
    assert_eq!(strip_markdown("& ^ \\\\ / roll"), "& ^ \\\\ / roll");
    assert_eq!(strip_markdown("X=F[\\\\+X][/-X]"), "X=F[\\\\+X][/-X]");
    // The escapes that matter still work.
    assert_eq!(strip_markdown("\\*literal\\*"), "*literal*");
    assert_eq!(strip_markdown("\\`tick\\`"), "`tick`");
}

/// A code span is literal: markdown inside it is content, not syntax.
#[test]
fn code_spans_keep_their_contents_verbatim() {
    use super::strip_markdown;
    assert_eq!(strip_markdown("`a * b`"), "a * b");
    assert_eq!(strip_markdown("`**stars**`"), "**stars**");
    assert_eq!(strip_markdown("`[not a link](x)`"), "[not a link](x)");
}

/// Unpaired delimiters are text, not syntax — arithmetic and prose
/// asterisks must survive, or the stripper corrupts the thing it is
/// supposed to clean.
#[test]
fn unpaired_delimiters_pass_through() {
    use super::strip_markdown;
    assert_eq!(strip_markdown("2 * 3 * 4"), "2 * 3 * 4");
    assert_eq!(strip_markdown("a lone * asterisk"), "a lone * asterisk");
    assert_eq!(strip_markdown("unclosed `backtick"), "unclosed `backtick");
    assert_eq!(strip_markdown("[not a link"), "[not a link");
    assert_eq!(strip_markdown("[text] (spaced)"), "[text] (spaced)");
    // The picker's own user-script marker, which is prose here.
    assert_eq!(strip_markdown("Generator - Basic Random *"), "Generator - Basic Random *");
}

/// Block structure is NOT touched: the panel renders headings, indented
/// tables and list markers structurally, and a stripper that ate `# `
/// would silently disable that.
#[test]
fn block_structure_is_left_alone() {
    use super::strip_markdown;
    assert_eq!(strip_markdown("# Heading"), "# Heading");
    assert_eq!(strip_markdown("- a list item"), "- a list item");
    assert_eq!(strip_markdown("    F   draw forward"), "    F   draw forward");
    // Multi-line keeps its line breaks, so paragraphs still split.
    assert_eq!(strip_markdown("one\n\ntwo"), "one\n\ntwo");
    // ...but inline syntax inside a heading still goes.
    assert_eq!(strip_markdown("# A **strong** heading"), "# A strong heading");
}

/// Every shipped script's prose must survive the round trip unharmed
/// where it contains no markdown — the strongest available check that
/// this cannot corrupt what actually ships.
#[test]
fn shipped_script_prose_is_not_corrupted() {
    use super::strip_markdown;
    for (name, source) in super::library::EMBEDDED {
        let doc = super::parse_doc(source);
        for (what, text) in [("summary", &doc.summary), ("body", &doc.body)] {
            let stripped = strip_markdown(text);
            // No shipped script uses markdown emphasis or links today, so
            // stripping must be the identity. If one starts using it,
            // this fails and the expectation gets revisited deliberately.
            assert_eq!(
                &stripped, text,
                "{name}'s {what} changed under stripping — either it now uses \
                 markdown (fine, update this test) or the stripper is eating \
                 something it should not"
            );
        }
    }
}

// ============================================================================
// Untrusted (downloaded) scripts: cross-call restriction
// ============================================================================

/// A library with one shipped script and one user script, so "may it
/// call this" has both answers available.
fn cross_call_host() -> super::ScriptHost {
    super::ScriptHost::new().with_scripts(vec![
        // A real shipped stem, so `is_builtin_stem` says yes.
        (
            "random_palette".to_string(),
            super::library::EMBEDDED
                .iter()
                .find(|(n, _)| *n == "random_palette.rhai")
                .map(|(_, s)| (*s).to_string())
                .expect("random_palette ships"),
        ),
        // The user's own. Not a shipped stem.
        (
            "my_helper".to_string(),
            "script(\"My Helper\", \"modifier\");\n".to_string(),
        ),
    ])
}

/// The property: a downloaded script cannot reach the user's scripts.
///
/// Without this, `run_script("my_helper")` in a downloaded script binds
/// to whatever that machine happens to have under that name — a
/// different render on every machine, and a stranger's code invoking
/// the user's.
#[test]
fn a_downloaded_script_cannot_call_the_users_own() {
    let base = FractalConfig::default();
    let src = "script(\"Nosy\", \"generator\");\nrun_script(\"my_helper\");\n";

    // Trusted: the call resolves normally.
    cross_call_host()
        .run(src, &base, 1, Default::default())
        .expect("a trusted script may call a user script");

    // Untrusted entry: refused.
    let err = cross_call_host()
        .with_untrusted_entry()
        .run(src, &base, 1, Default::default())
        .expect_err("a downloaded script must not reach a user script");
    let msg = format!("{err}");
    assert!(msg.contains("only call scripts that ship"), "{msg}");
    assert!(msg.contains("my_helper"), "{msg}");
}

/// ...but shipped scripts stay callable, or the restriction would make
/// every downloaded script useless rather than safe.
#[test]
fn a_downloaded_script_may_still_call_shipped_ones() {
    let base = FractalConfig::default();
    let src = "script(\"Polite\", \"generator\");\nrun_script(\"random_palette\");\n";
    cross_call_host()
        .with_untrusted_entry()
        .run(src, &base, 1, Default::default())
        .expect("shipped stems remain callable");
}

/// The restriction follows a downloaded script into the library, not
/// just at the entry point: a *user* script may call a downloaded one,
/// and the downloaded one is restricted from there on.
#[test]
fn the_restriction_applies_to_a_downloaded_script_called_from_a_trusted_one() {
    let base = FractalConfig::default();
    let host = super::ScriptHost::new()
        .with_scripts(vec![
            (
                "downloaded".to_string(),
                "script(\"Downloaded\", \"modifier\");\nrun_script(\"my_helper\");\n".to_string(),
            ),
            (
                "my_helper".to_string(),
                "script(\"My Helper\", \"modifier\");\n".to_string(),
            ),
        ])
        .with_untrusted(vec!["downloaded".to_string()]);

    // The trusted entry script calls the downloaded one, which then
    // tries to reach the user's helper.
    let src = "script(\"Mine\", \"generator\");\nrun_script(\"downloaded\");\n";
    let err = host
        .run(src, &base, 1, Default::default())
        .expect_err("the downloaded frame must still be restricted");
    assert!(format!("{err}").contains("only call scripts that ship"), "{err}");
}

/// The rule is "any untrusted frame on the stack", not "the immediate
/// caller" — so a downloaded script cannot launder a call through a
/// trusted one.
///
/// No shipped script does this today and none can be made to without a
/// recompile, but "safe because of what the corpus happens to contain"
/// is not a property.
///
/// Tested against the rule directly rather than by driving the engine.
/// That is not a shortcut: the chain that separates the two readings —
/// `downloaded -> shipped -> user` — cannot be built from real scripts,
/// because reaching it needs a shipped script that calls a user one and
/// shipped scripts are compiled in. An engine-driven test would pass
/// under either rule while looking like it pinned the stricter one.
#[test]
fn a_downloaded_frame_anywhere_on_the_stack_restricts() {
    use super::host::cross_calls_restricted;

    let untrusted: std::collections::HashSet<String> =
        ["downloaded".to_string()].into_iter().collect();
    let stack = |ids: &[&str]| -> Vec<String> { ids.iter().map(|s| s.to_string()).collect() };

    // The case the strict rule exists for: the top frame is trusted, but
    // a downloaded one is still below it.
    assert!(cross_calls_restricted(false, &stack(&["downloaded", "shipped"]), &untrusted));
    // Outermost, innermost, alone — all the same answer.
    assert!(cross_calls_restricted(false, &stack(&["downloaded"]), &untrusted));
    assert!(cross_calls_restricted(false, &stack(&["mine", "downloaded"]), &untrusted));

    // Nothing untrusted anywhere: unrestricted.
    assert!(!cross_calls_restricted(false, &stack(&["mine", "shipped"]), &untrusted));
    assert!(!cross_calls_restricted(false, &[], &untrusted));

    // The entry script carries no id on the stack, so it needs its own
    // flag — and that is the ordinary case, not the edge one.
    assert!(cross_calls_restricted(true, &[], &untrusted));
    assert!(cross_calls_restricted(true, &stack(&["shipped"]), &untrusted));
}

/// The refusal is decided before the id is resolved.
///
/// Otherwise the error would report whether a script by that name
/// exists — telling a downloaded script what the user has installed,
/// which is exactly the kind of question it should not be able to ask.
#[test]
fn the_refusal_does_not_leak_whether_the_target_exists() {
    let base = FractalConfig::default();
    let present = "script(\"A\", \"generator\");\nrun_script(\"my_helper\");\n";
    let absent = "script(\"A\", \"generator\");\nrun_script(\"no_such_script\");\n";

    let a = format!(
        "{}",
        cross_call_host()
            .with_untrusted_entry()
            .run(present, &base, 1, Default::default())
            .expect_err("refused")
    );
    let b = format!(
        "{}",
        cross_call_host()
            .with_untrusted_entry()
            .run(absent, &base, 1, Default::default())
            .expect_err("refused")
    );

    // Same shape of message either way, and neither lists the library.
    assert!(a.contains("only call scripts that ship"), "{a}");
    assert!(b.contains("only call scripts that ship"), "{b}");
    assert!(!a.contains("available:"), "the trusted path's listing leaked: {a}");
    assert!(!b.contains("available:"), "{b}");
}

// ============================================================================
// Script flags: an open vocabulary
// ============================================================================

/// An unknown flag warns and is dropped — it does not stop the script.
///
/// This is what makes the flag vocabulary growable without a lockstep
/// release, and with public script browsing live it is what stops an
/// older build rejecting a newer script that would run correctly.
///
/// Safe precisely because both flags are UI affordances: `norng` hides
/// the seed controls, `palette` offers the script in the Palette
/// Editor. Neither touches the rendered flame, so a dropped flag costs
/// an affordance, never a wrong result. A flag that DID affect output
/// could not be treated this way.
#[test]
fn an_unknown_flag_warns_rather_than_failing_the_script() {
    let base = FractalConfig::default();
    let out = ScriptHost::new()
        .run(
            r#"script("Future", "generator", ["norng", "invented_next_year"]);
               flame.add_transform();"#,
            &base,
            1,
            Default::default(),
        )
        .expect("an unknown flag must not fail the script");

    assert_eq!(
        out.warnings.len(),
        1,
        "the unknown flag must be reported, not swallowed: {:?}",
        out.warnings
    );
    assert!(out.warnings[0].contains("invented_next_year"), "{:?}", out.warnings);
    // ...and the flag it DID understand still took effect.
    assert!(!out.config.flame.transforms.is_empty());
}

/// Flag spelling is normalised client-side, so nothing upstream needs
/// to be.
///
/// Worth pinning because the opposite belief leads somewhere bad: a
/// server that lowercases flags on the way in looks like a kindness and
/// is really rewriting user content to fix a problem that does not
/// exist.
#[test]
fn flag_names_are_case_and_space_insensitive() {
    let base = FractalConfig::default();
    for spelling in ["norng", "NoRng", "NORNG", "  norng  "] {
        let src = format!("script(\"A\", \"generator\", [\"{spelling}\"]);");
        let meta = ScriptHost::new().collect(&src, &base).expect("collects");
        assert!(meta.flags.no_rng, "`{spelling}` should set the flag");
    }
}

/// A malformed flag is still an error: a number where a string belongs
/// is the author's mistake, not version skew, and degrading it would
/// hide a typo behind a warning nobody reads.
#[test]
fn a_non_string_flag_is_still_an_error() {
    let base = FractalConfig::default();
    let err = ScriptHost::new()
        .run(r#"script("A", "generator", [42]);"#, &base, 1, Default::default())
        .expect_err("a non-string flag must fail");
    assert!(format!("{err}").contains("must be strings"), "{err}");
}

// ============================================================================
// Provenance survives adoption
// ============================================================================

/// Serialize every test that touches the script store's link file.
///
/// `set_link` is read-modify-write over one shared `_links.json`, so two
/// tests running in parallel lose each other's writes even when their
/// stems differ: both read the map, both insert, the second write wins
/// and the first entry is gone.
///
/// This is a test-harness problem rather than a product one — the app
/// only touches links from the main thread, since background results are
/// folded in by `poll_script_cloud`. But it made the tests **flaky**
/// rather than failing, which is worse: they passed for two commits by
/// luck of scheduling before one lost the race.
fn link_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A poisoned lock means another link test panicked. That failure is
    // reported on its own; this one should still run.
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Saving somebody else's script does not make it yours.
///
/// This is the hole that would otherwise open the moment browsing
/// lands: a downloaded script saved locally becomes
/// `ScriptOrigin::User`, and if trust were read from the origin, Save
/// would launder away the cross-call restriction. The user chose to
/// keep the script; they did not read it.
#[test]
fn adopting_a_downloaded_script_keeps_it_untrusted() {
    let _guard = link_test_lock();
    use super::store;
    let stem = "adopted-from-elsewhere";
    let _ = store::delete(stem);

    store::save(stem, "script(\"Theirs\", \"generator\");\n").expect("save");
    store::set_link(
        stem,
        store::ScriptLink {
            cloud_id: Some("abc".into()),
            version: Some(3),
            owner: Some("someone/theirs".into()),
            from_others: true,
        },
    )
    .expect("link");

    assert!(store::is_untrusted(stem), "provenance must survive the save");

    let entries = super::library::discover(&crate::config::FractalConfig::default());
    let e = entries.iter().find(|e| e.id == stem).expect("listed");
    assert!(e.untrusted, "the library must carry it through to the panel");
    assert_eq!(e.origin, super::library::ScriptOrigin::User, "it IS the user's copy");
    assert!(e.label().ends_with(" ↓"), "and it is marked as such: {}", e.label());

    store::delete(stem).expect("cleanup");
}

/// Deleting a script forgets its link, so a stem reused later does not
/// inherit the previous script's cloud identity — or, worse, its
/// provenance: writing your own script under a freed name must not
/// leave it marked as somebody else's.
#[test]
fn deleting_a_script_forgets_where_it_came_from() {
    let _guard = link_test_lock();
    use super::store;
    let stem = "reused-stem-test";
    let _ = store::delete(stem);

    store::save(stem, "script(\"Theirs\", \"generator\");\n").unwrap();
    store::set_link(
        stem,
        store::ScriptLink { from_others: true, cloud_id: Some("x".into()), ..Default::default() },
    )
    .unwrap();
    assert!(store::is_untrusted(stem));

    store::delete(stem).unwrap();
    assert!(store::link_of(stem).is_none(), "the link must go with the script");

    // The same name, now the user's own work.
    store::save(stem, "script(\"Mine\", \"generator\");\n").unwrap();
    assert!(!store::is_untrusted(stem), "a reused stem must not inherit provenance");
    store::delete(stem).unwrap();
}

/// An absent link means locally authored, and that is safe because
/// every path bringing in a foreign script writes one.
#[test]
fn no_link_means_local() {
    let _guard = link_test_lock();
    use super::store;
    assert!(!store::is_untrusted("a-stem-that-was-never-stored"));
}

// ============================================================================
// Adopting versus refetching
// ============================================================================

/// Resolving a conflict on your OWN script must not mark it as somebody
/// else's, and must not leave a second copy behind.
///
/// The two operations look interchangeable — both fetch a script and
/// write it locally — and reusing the adopt path for "load theirs" is
/// the natural mistake. It fails in two ways at once, both silent: a
/// duplicate under a freed stem, and your own script running under the
/// cross-call restriction from then on.
#[test]
fn refetching_your_own_script_keeps_it_yours() {
    let _guard = link_test_lock();
    use super::store;
    let stem = "refetch-keeps-ownership";
    let _ = store::delete(stem);

    store::save(stem, "script(\"Mine v1\", \"generator\");\n").unwrap();
    store::set_link(
        stem,
        store::ScriptLink {
            cloud_id: Some("id-1".into()),
            version: Some(4),
            owner: Some("me/mine".into()),
            from_others: false,
        },
    )
    .unwrap();

    // What the Refetch handler does: overwrite in place, preserving
    // whatever `from_others` already said.
    let was_theirs = store::link_of(stem).is_some_and(|l| l.from_others);
    let saved = store::save(stem, "script(\"Mine v2\", \"generator\");\n").unwrap();
    store::set_link(
        &saved,
        store::ScriptLink {
            cloud_id: Some("id-1".into()),
            version: Some(5),
            owner: Some("me/mine".into()),
            from_others: was_theirs,
        },
    )
    .unwrap();

    assert_eq!(saved, stem, "in place — not a second copy under a freed stem");
    assert!(!store::is_untrusted(stem), "your own script stays yours");
    assert_eq!(store::link_of(stem).unwrap().version, Some(5));
    assert!(store::load(stem).unwrap().contains("v2"));

    // Exactly one copy.
    let mine: Vec<_> = store::list().into_iter().filter(|(s, _)| s.starts_with(stem)).collect();
    assert_eq!(mine.len(), 1, "{mine:?}");

    store::delete(stem).unwrap();
}

/// ...while adopting somebody else's genuinely does mark it, and picks
/// a free stem rather than overwriting whatever is already there.
#[test]
fn adopting_takes_a_free_stem_and_marks_provenance() {
    let _guard = link_test_lock();
    use super::store;
    // A shipped stem is reserved, so adoption must not try to take it
    // even if the server let the name through.
    assert_eq!(store::free_stem("random_palette"), "random_palette-copy");

    let stem = store::free_stem("adopt-free-stem-test");
    let _ = store::delete(&stem);
    let saved = store::save(&stem, "script(\"Theirs\", \"generator\");\n").unwrap();
    store::set_link(
        &saved,
        store::ScriptLink { from_others: true, ..Default::default() },
    )
    .unwrap();
    assert!(store::is_untrusted(&saved));
    store::delete(&saved).unwrap();
}
