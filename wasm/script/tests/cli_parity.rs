//! The module's core guarantee: `run_impl`'s `config_json` is
//! byte-identical to what the desktop `generate` CLI writes for the
//! same script + seed + params. The fixtures were produced by the CLI
//! (see the commands in each test); if either side drifts, these fail.
//!
//! This compiles the same code natively — the wasm build is the same
//! crate, and desktop/WASM script determinism is already covered by
//! the main crate's platform-determinism tests. The browser end is
//! exercised again by the gallery page against these same seeds.

fn config_of(envelope: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(envelope).expect("envelope parses");
    v["config_json"].as_str().expect("has config_json").to_string()
}

#[test]
fn generator_matches_the_cli_byte_for_byte() {
    // fractal_flame_wgpu generate -s assets/scripts/generators/basic_random.rhai \
    //   --seed 7 -o basic_random_seed7.fflame
    let fixture = include_str!("fixtures/basic_random_seed7.fflame");
    let source = include_str!("../../../assets/scripts/generators/basic_random.rhai");
    let out = fflame_script::run_impl(source, 7, "{}", None).expect("runs");
    assert_eq!(config_of(&out), fixture, "generator output drifted from the CLI");
}

#[test]
fn modifier_on_a_base_matches_the_cli() {
    // fractal_flame_wgpu generate -s assets/scripts/modifiers/jitter.rhai \
    //   --seed 7 -b basic_random_seed7.fflame -o basic_random_seed7_jitter.fflame
    let base = include_str!("fixtures/basic_random_seed7.fflame");
    let fixture = include_str!("fixtures/basic_random_seed7_jitter.fflame");
    let source = include_str!("../../../assets/scripts/modifiers/jitter.rhai");
    let out = fflame_script::run_impl(source, 7, "{}", Some(base)).expect("runs");
    assert_eq!(config_of(&out), fixture, "modifier output drifted from the CLI");
}

#[test]
fn choice_params_resolve_like_the_cli_set_flag() {
    // fractal_flame_wgpu generate -s assets/scripts/generators/lsystem.rhai \
    //   --seed 3 --set "preset=2D · Koch snowflake" -o lsystem_koch_seed3.fflame
    let fixture = include_str!("fixtures/lsystem_koch_seed3.fflame");
    let source = include_str!("../../../assets/scripts/generators/lsystem.rhai");
    let params = r#"{"preset": "2D · Koch snowflake"}"#;
    let out = fflame_script::run_impl(source, 3, params, None).expect("runs");
    assert_eq!(config_of(&out), fixture, "choice-param output drifted from the CLI");
}

#[test]
fn unknown_params_fail_loudly() {
    let source = include_str!("../../../assets/scripts/modifiers/jitter.rhai");
    let err = fflame_script::run_impl(source, 1, r#"{"nope": 1.0}"#, None).unwrap_err();
    assert!(err.contains("no parameter `nope`"), "got: {err}");
}

#[test]
fn the_library_lists_the_embedded_scripts_with_params() {
    let json = fflame_script::list_scripts_impl().expect("lists");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parses");
    let arr = v.as_array().expect("array");
    assert!(arr.len() >= 10, "all shipped scripts present: {}", arr.len());
    let basic = arr
        .iter()
        .find(|e| e["id"] == "basic_random")
        .expect("basic_random listed");
    assert_eq!(basic["kind"], "generator");
    assert!(!basic["summary"].as_str().unwrap().is_empty(), "doc summary present");
    assert!(!basic["params"].as_array().unwrap().is_empty(), "params declared");

    // And the source getter round-trips an id from the listing.
    let src = fflame_script::script_source_impl("jitter").expect("source by id");
    assert!(src.contains("script("));

    // Flags surface: a gallery must know turntable ignores the seed
    // (a norng hallway would show one image over and over) and that
    // the palette scripts belong in a palette picker.
    let turntable = arr.iter().find(|e| e["id"] == "turntable").expect("listed");
    assert_eq!(turntable["flags"]["norng"], true);
    let iq = arr.iter().find(|e| e["id"] == "iq_palette").expect("listed");
    assert_eq!(iq["flags"]["palette"], true);
}

#[test]
fn a_script_defined_animation_rides_in_the_envelope() {
    // The Animation wing's door: turntable defines an animation, and
    // the envelope must carry it — the same .anim JSON the CLI writes.
    let source = include_str!("../../../assets/scripts/modifiers/turntable.rhai");
    let out = fflame_script::run_impl(source, 1, "{}", None).expect("runs");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let anim = v["animation_json"].as_str().expect("animation present");
    let parsed: serde_json::Value = serde_json::from_str(anim).expect("animation is JSON");
    assert!(
        !parsed["tracks"].as_array().unwrap().is_empty(),
        "turntable defines at least one track"
    );

    // And a script with no animation says null, not a missing key.
    let jitter = include_str!("../../../assets/scripts/modifiers/jitter.rhai");
    let out = fflame_script::run_impl(jitter, 1, "{}", None).expect("runs");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["animation_json"].is_null());
}
