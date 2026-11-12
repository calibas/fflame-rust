//! The API integration test: the client's own calls against a real `ffapi`
//! on this machine (docs/projects/api-updates.md, decision 7).
//!
//! Run by `python scripts/api_integration.py`, which starts the server on a
//! free port with a scratch-safe environment (email off) and runs these with
//! `FFLAME_API_URL` pointing at it. Ignored in the ordinary suite, and each
//! test returns at once when the variable is unset, so nothing here touches
//! the network unless asked.
//!
//! Each test signs up a fresh account, so they share a database safely.

use super::client::{api_get, api_post_unauth, build_url};
use super::types::*;
use super::*;
use crate::config::FractalConfig;
use crate::scene::transforms::RenderMode;
use serde_json::json;

/// The local server's address, or `None` to skip.
fn server() -> Option<String> {
    std::env::var("FFLAME_API_URL").ok()
}

/// A fresh account on the local server, signed in.
fn signed_in() -> ApiState {
    let nonce = format!("{}{}", std::process::id(), web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).unwrap().as_nanos());
    let req = RegisterRequest {
        display_name: format!("it{}", &nonce[nonce.len() - 10..]),
        email: format!("it-{nonce}@example.test"),
        password: "integration test password, long enough".into(),
    };
    let url = build_url(API_BASE_URL, "/api/auth/register");
    let resp: AuthResponse = pollster::block_on(api_post_unauth(&url, &req)).unwrap_or_else(|e| panic!("register: {e}"));
    let mut api = ApiState::default();
    api.set_token(&resp.token);
    api
}

fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    pollster::block_on(f)
}

/// A config's JSON as the client writes it, without the palette (stored by
/// hash and rebuilt on load, in its own representation).
fn comparable(config: &FractalConfig) -> serde_json::Value {
    let mut v = config.to_json_value().unwrap();
    v.as_object_mut().unwrap().remove("palette");
    v
}

fn flame_config() -> FractalConfig {
    let mut config = FractalConfig::default();
    let mut t = crate::scene::transforms::Transform::new();
    t.set_variation("linear", 0.75);
    t.set_variation("spherical", 0.25);
    config.flame.transforms.push(t);
    config.zoom = 1.7;
    config
}

/// A flame, an escape-time config and a simulation each save and load back
/// unchanged; a list at the client's reader shows all three, and one without
/// a reader only the flame.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn every_kind_of_config_round_trips_and_lists_by_reader() {
    let Some(base) = server() else { return };
    let mut api = signed_in();

    let flame = flame_config();
    let mut escape = FractalConfig::default();
    escape.render_mode = RenderMode::Escape;
    escape.escape.formula = "mandelbrot".into();
    escape.escape.center_re = "-0.74364388703715978043243722405478096275231550322210901".into();
    escape.escape.center_im = "0.13182590420531198080978012658614604004633877406962265".into();
    let mut sim = FractalConfig::default();
    sim.render_mode = RenderMode::Simulation;
    sim.sim.model = "gray_scott".into();
    sim.sim.seed = 7;

    let mut ids = Vec::new();
    for (name, config) in [("it flame", &flame), ("it escape", &escape), ("it simulation", &sim)] {
        let id = block(api.save_flame(config, Some(name), Some(ApiVisibility::Private), None)).unwrap_or_else(|e| panic!("save {name}: {e}"));
        let back = block(api.load_flame(&id)).unwrap_or_else(|e| panic!("load {name}: {e}"));
        // Saving online names the flame.
        let mut expected = config.clone();
        expected.flame.name = name.to_string();
        assert_eq!(comparable(&back), comparable(&expected), "{name} round trip");
        ids.push(id);
    }

    let mine: Vec<String> = block(api.list_my_flames(1, 100)).unwrap().into_iter().map(|f| f.id).collect();
    for id in &ids {
        assert!(mine.contains(id), "reader 7 lists {id}");
    }
    // Without a reader: the original vocabulary, 2D/3D at config version 3
    // and below. A current flame is version 7, so not even it is listed.
    let token = api.auth.get_token().unwrap().to_string();
    let bare: serde_json::Value = block(api_get(&format!("{base}/api/flames?per_page=100"), &token)).unwrap();
    let bare: Vec<&str> = bare.as_array().unwrap().iter().filter_map(|f| f["id"].as_str()).collect();
    for id in &ids {
        assert!(!bare.contains(&id.as_str()), "no reader: {id} is newer than the original vocabulary");
    }
}

/// A config from a newer app keeps what this build cannot read through a
/// save to the server and a load back.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn a_newer_configs_unknowns_survive_the_server() {
    if server().is_none() {
        return;
    }
    let api = signed_in();
    let mut v = flame_config().to_json_value().unwrap();
    v["version"] = json!(99);
    v["min_reader"] = json!(7);
    v["sparkle"] = json!({"on": true});
    v["tonemap_mode"] = json!("hologram");
    let newer = FractalConfig::from_json_value(v).unwrap();
    let id = block(api.save_flame(&newer, Some("it newer"), Some(ApiVisibility::Private), None)).unwrap();
    let back = block(api.load_flame(&id)).unwrap();
    let out = back.to_json_value().unwrap();
    assert_eq!(out["sparkle"], json!({"on": true}));
    assert_eq!(out["tonemap_mode"], json!("hologram"));
    assert_eq!(out["min_reader"], json!(7));
}

/// An animation with keyframes, a script track and a track no build knows,
/// and motion blur, saves and loads back with all of it.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn an_animation_keeps_its_tracks_and_motion_blur() {
    if server().is_none() {
        return;
    }
    let api = signed_in();
    let flame_id = block(api.save_flame(&flame_config(), Some("it base"), Some(ApiVisibility::Private), None)).unwrap();
    let teleport = json!({"target": "Zoom", "source": {"type": "Teleport", "to": [1, 2]}, "flame_target": "Main"});
    let mut anim = crate::animation::Animation::from_json_value(json!({
        "name": "it animation", "duration": 4.0, "loop_mode": "Loop",
        "tracks": [
            {"target": "Zoom", "source": {"type": "Keyframes", "keyframes": [
                {"time": 0.0, "value": 1.0, "easing": "Linear"}, {"time": 4.0, "value": 2.0, "easing": "Linear"}]}},
            {"target": "Zoom", "source": {"type": "Script", "source": "const TARGETS = #{ \"Zoom\": \"add\" };\nfn value(t, p, ctx) { 0.1 * t }"}},
            teleport.clone(),
        ],
    }))
    .unwrap();
    anim.motion_blur.enabled = true;
    anim.motion_blur.samples = 6;
    let id = block(api.save_animation(&anim, None, Some(&flame_id), Some(ApiVisibility::Private))).unwrap();
    let (back, base, _) = block(api.load_animation_full(&id)).unwrap();
    assert!(base.is_some(), "the flame comes with it");
    assert_eq!(back.tracks.len(), 3);
    let tracks = serde_json::to_value(&back.tracks).unwrap();
    assert_eq!(tracks[1]["source"]["type"], "Script");
    assert_eq!(tracks[2], teleport, "the unknown track, as it was");
    assert_eq!(back.motion_blur, anim.motion_blur);
}

/// The catalogs decode at the client's reader (a variation's phase `any`
/// among them), and the update check reads the server's latest release.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn catalogs_and_the_update_check() {
    if server().is_none() {
        return;
    }
    let api = ApiState::default();
    let list = block(api.list_variations()).expect("the variation catalog decodes");
    assert!(list.len() > 600, "{} variations", list.len());
    let linear = block(api.fetch_variation("linear")).expect("a variation download decodes");
    assert!(linear.phase.is_known(), "{:?}", linear.phase);
    let effects = block(api.list_effects()).expect("the effect catalog decodes");
    assert!(!effects.is_empty());
    // The harness sets the server's APP_LATEST_VERSION to 99.0.0.
    let latest = block(latest_app_version(API_BASE_URL)).expect("the server says its latest release");
    assert!(is_newer_release(&latest.version, env!("CARGO_PKG_VERSION")), "{latest:?}");
}

/// The catalog, end to end: a config the app saves with the Mandelbrot
/// twin unedited is found by the server under Mandelbrot, with its facets.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn the_server_catalogs_what_the_app_saves() {
    let Some(base) = server() else { return };
    let api = signed_in();
    let mut config = FractalConfig::default();
    config.render_mode = RenderMode::Escape;
    let src = crate::escape::twins::custom_source("mandelbrot").unwrap();
    config.escape.formula = crate::escape::custom::name_of(crate::lang::EntryKind::Formula, &src);
    config.escape.custom_formula = Some(src);
    config.escape.terrain.enabled = true;
    let id = block(api.save_flame(&config, Some("it twin"), Some(ApiVisibility::Private), None)).unwrap();
    let token = api.auth.get_token().unwrap().to_string();
    let list: serde_json::Value = block(api_get(&format!("{base}/api/flames?per_page=100&reader=7&type=mandelbrot&role=primary"), &token)).unwrap();
    let item = list.as_array().unwrap().iter().find(|f| f["id"] == id.as_str()).unwrap_or_else(|| panic!("not found by type: {list}"));
    assert_eq!((item["engine"].as_str(), item["scene"].as_str(), item["dimension"].as_i64()), (Some("escape"), Some("terrain"), Some(3)));
    assert_eq!(item["primary_type"]["display_name"], "Mandelbrot");
    // The server's summary is the app's.
    let app = crate::catalog::summary(&config);
    assert_eq!((app.engine.as_str(), app.scene.as_str()), ("escape", "terrain"));
}

/// A Rhai formula on the scripts resource (docs/projects/rhai-scripts-online.md):
/// published under its own `INFO.id` and updated by its author; another
/// account cannot take the id, finds it by kind, and adopts it into the
/// formula library as somebody else's, the id kept.
#[cfg(feature = "web-app")]
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn a_rhai_formula_is_published_under_its_id_and_adopted_by_another_account() {
    use crate::app::script_cloud::{adopt, Local};
    use crate::storage::formula_store;
    if server().is_none() {
        return;
    }
    let id = crate::scripted::mint_id();
    let name = format!("it_spiral_{}", &id[..8]);
    let file = format!("{name}.rhai");
    let source = crate::lang::rhai::with_own_id(
        &format!("const INFO = #{{ title: \"{name}\" }};\nfn start(pixel, p) {{ #{{ z: pixel }} }}\nfn step(s, p) {{ s.z = s.z * s.z; s }}\nfn escaped(s, p) {{ s.z.norm_sqr() > 4.0 }}\n"),
        &id,
    );

    let author = signed_in();
    let req = entries::create_request(&file, &source, Some(ApiVisibility::Public), None).unwrap();
    let created = block(author.create_script(&req)).unwrap_or_else(|e| panic!("publish: {e}"));
    assert_eq!(created.id, id, "stored under its own id");
    assert_eq!(created.kind.as_deref(), Some("formula"));
    let edited = source.replacen("4.0", "8.0", 1);
    let update = entries::create_request(&file, &edited, None, Some(created.version)).unwrap();
    let updated = block(author.update_script(&id, &update)).unwrap_or_else(|e| panic!("update: {e}"));
    assert!(updated.version > created.version);
    let mine = block(author.list_my_scripts(Some("formula"), 1, 100)).unwrap();
    assert!(mine.iter().any(|s| s.id == id), "listed as the author's formula");

    let other = signed_in();
    let taken = block(other.create_script(&req)).expect_err("the first to publish an id owns it");
    assert!(matches!(taken, crate::resources::FetchError::Http { status: 409, .. }), "{taken}");
    let found = block(other.search_public_scripts(&name, Some("formula"), 1, 50)).unwrap();
    assert!(found.iter().any(|s| s.id == id), "found by kind");
    let generators = block(other.search_public_scripts(&name, Some("generator"), 1, 50)).unwrap();
    assert!(!generators.iter().any(|s| s.id == id));

    let resp = block(other.load_script(&id)).unwrap();
    let Ok(Local::Entry(adopted)) = adopt(&resp) else { panic!("a formula is adopted into the formula library") };
    let link = formula_store::link_of(&adopted).expect("a link");
    let text = formula_store::load(&adopted).expect("saved");
    formula_store::delete(&adopted).expect("delete the adopted copy");
    assert!(link.from_others, "somebody else's");
    assert_eq!(link.cloud_id.as_deref(), Some(id.as_str()));
    assert_eq!(text, edited, "the server's latest");
    assert_eq!(crate::lang::rhai::info_string(&text, "id").as_deref(), Some(id.as_str()), "the copy is that script: its id kept");

    block(author.delete_script(&id)).unwrap_or_else(|e| panic!("unpublish: {e}"));
}
