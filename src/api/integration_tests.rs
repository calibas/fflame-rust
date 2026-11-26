//! The API integration test: the client's own calls against a real `ffapi`
//! on this machine (docs/archive/projects/api-updates.md, decision 7).
//!
//! Run by `python scripts/api_integration.py`, which starts the server on a
//! free port with a scratch-safe environment (email off) and runs these with
//! `FAE_API_URL` pointing at it. Ignored in the ordinary suite, and each
//! test returns at once when the variable is unset, so nothing here touches
//! the network unless asked.
//!
//! Each test signs up a fresh account, so they share a database safely.

use super::client::{api_get, api_post, api_post_unauth, build_url};
use super::types::*;
use super::*;
use crate::config::FractalConfig;
use crate::scene::transforms::RenderMode;
use serde_json::json;

/// The local server's address, or `None` to skip.
fn server() -> Option<String> {
    crate::env_vars::var("API_URL")
}

/// A fresh account on the local server, signed in.
fn signed_in() -> ApiState {
    let nonce = format!("{}{}", std::process::id(), web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).unwrap().as_nanos());
    let req = RegisterRequest {
        display_name: format!("it{}", &nonce[nonce.len() - 10..]),
        email: format!("it-{nonce}@example.test"),
        password: "integration test password, long enough".into(),
    };
    let url = build_url(API_BASE_URL, "/api/v1/auth/register");
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
    let bare: serde_json::Value = block(api_get(&format!("{base}/api/v1/users/me/fractals?per_page=100"), &token)).unwrap();
    let bare: Vec<&str> = bare["items"].as_array().unwrap().iter().filter_map(|f| f["id"].as_str()).collect();
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
    let list: serde_json::Value = block(api_get(&format!("{base}/api/v1/users/me/fractals?per_page=100&reader=7&type=mandelbrot&role=primary"), &token)).unwrap();
    let item = list["items"].as_array().unwrap().iter().find(|f| f["id"] == id.as_str()).unwrap_or_else(|| panic!("not found by type: {list}"));
    assert_eq!((item["engine"].as_str(), item["scene"].as_str(), item["dimension"].as_i64()), (Some("escape"), Some("terrain"), Some(3)));
    assert_eq!(item["primary_type"]["display_name"], "Mandelbrot");
    // The server's summary is the app's.
    let app = crate::catalog::summary(&config);
    assert_eq!((app.engine.as_str(), app.scene.as_str()), ("escape", "terrain"));
}

/// A Rhai formula on the scripts resource (docs/archive/projects/rhai-scripts-online.md):
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
    let mine = block(author.browse_scripts(Scope::Mine, &Browse { kind: Some("formula".into()), per_page: 100, ..Default::default() })).unwrap().items.into_inner();
    assert!(mine.iter().any(|s| s.id == id), "listed as the author's formula");

    let other = signed_in();
    let taken = block(other.create_script(&req)).expect_err("the first to publish an id owns it");
    assert!(matches!(taken, crate::resources::FetchError::Http { status: 409, .. }), "{taken}");
    let search = |kind: &str| Browse { q: name.clone(), kind: Some(kind.into()), per_page: 50, ..Default::default() };
    let found = block(other.browse_scripts(Scope::Public, &search("formula"))).unwrap().items.into_inner();
    assert!(found.iter().any(|s| s.id == id), "found by kind");
    let generators = block(other.browse_scripts(Scope::Public, &search("generator"))).unwrap().items.into_inner();
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

/// The caller's palettes are items (the catalog API's step 6): listed with
/// their colours, renamed by id, deleted by id. A stale script write is the
/// contract's 409, recovered as a conflict naming the current revision.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn palettes_are_items_and_a_stale_write_is_a_conflict() {
    if server().is_none() {
        return;
    }
    let api = signed_in();
    let token = api.auth.get_token().unwrap().to_string();
    let made: serde_json::Value = block(api_post(
        &build_url(API_BASE_URL, "/api/v1/palettes"),
        &json!({"name": "it palette", "color_data": [1, 2, 3, 4]}),
        &token,
    ))
    .unwrap_or_else(|e| panic!("create a palette: {e}"));
    let id = made["id"].as_str().unwrap().to_string();

    let my_palettes = || block(api.browse_palettes(Scope::Mine, &Browse { per_page: 100, ..Default::default() })).unwrap().items.into_inner();
    let mine = my_palettes();
    let listed = mine.iter().find(|p| p.id == id).expect("listed");
    assert_eq!(listed.name, "it palette");
    assert_eq!(listed.content.hash.as_deref(), Some(listed.content_hash.as_str()));
    let patch = crate::api::types::ItemPatch { name: Some("renamed".into()), visibility: None };
    block(api.patch_item("palettes", &id, &patch)).unwrap();
    assert_eq!(block(api.get_palette(&id)).unwrap().name, "renamed");
    block(api.delete_palette(&id)).unwrap();
    assert!(my_palettes().iter().all(|p| p.id != id));

    let source = "// It conflict\nscript(\"It conflict\", \"generator\", []);\n";
    let name = format!("it_conflict_{}", &crate::scripted::mint_id()[..8]);
    let create = crate::api::sync::script_to_create_request(&name, source, None, None).unwrap();
    let created = block(api.create_script(&create)).unwrap_or_else(|e| panic!("create: {e}"));
    let update = crate::api::sync::script_to_create_request(&name, source, None, Some(created.version)).unwrap();
    let updated = block(api.update_script(&created.id, &update)).unwrap_or_else(|e| panic!("update: {e}"));
    let stale = block(api.update_script(&created.id, &update)).expect_err("the revision it sent is stale now");
    let conflict = ScriptConflict::from_error(&stale, &created.id).unwrap_or_else(|| panic!("a conflict: {stale}"));
    assert_eq!((conflict.id.as_str(), conflict.current_version), (created.id.as_str(), updated.version));
    block(api.delete_script(&created.id)).unwrap();
}

/// The Catalog's Online place (docs/archive/projects/catalog.md, *Phase 3*): a
/// guest finds a public fractal by search, with its owner and thumbnail,
/// downloads the thumbnail and loads the fractal without signing in; the
/// owner pages through their own by cursor; a public palette is browsed
/// with its colours; the built-in scripts browse by kind.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn the_online_place_browses_every_resource() {
    if server().is_none() {
        return;
    }
    let author = signed_in();
    let tag = &crate::scripted::mint_id()[..8];
    let mut jpeg = Vec::new();
    image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 10]))
        .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .unwrap();
    let names = [format!("it browse {tag} one"), format!("it browse {tag} two")];
    let mut ids = Vec::new();
    for name in &names {
        let id = block(author.save_flame(&flame_config(), Some(name), Some(ApiVisibility::Public), Some(&jpeg))).unwrap_or_else(|e| panic!("save: {e}"));
        ids.push(id);
    }

    let guest = ApiState::default();
    let found = block(guest.browse_flames(Scope::Public, &Browse { q: format!("it browse {tag}"), ..Default::default() })).unwrap_or_else(|e| panic!("browse: {e}"));
    let items = found.items.into_inner();
    assert_eq!(items.len(), 2, "both, found by the search: {items:?}");
    let one = items.iter().find(|f| f.id == ids[0]).unwrap();
    assert!(one.owner.as_ref().is_some_and(|o| !o.display_name.is_empty()), "with its owner");
    let thumb = one.thumbnail_url.clone().expect("a thumbnail");
    let bytes = block(guest.fetch_thumbnail(&thumb)).unwrap_or_else(|e| panic!("thumbnail: {e}"));
    assert!(image::load_from_memory(&bytes).is_ok(), "a picture");
    let loaded = block(guest.load_flame_with_visibility(&ids[0])).unwrap_or_else(|e| panic!("a guest loads a public fractal: {e}"));
    assert_eq!(loaded.is_public, Some(true));

    let first = block(author.browse_flames(Scope::Mine, &Browse { per_page: 1, sort: Some("name".into()), ..Default::default() })).unwrap();
    let cursor = first.next.clone().expect("a second page");
    let second = block(author.browse_flames(Scope::Mine, &Browse { per_page: 1, sort: Some("name".into()), cursor: Some(cursor), ..Default::default() })).unwrap();
    let paged: Vec<String> = first.items.into_inner().into_iter().chain(second.items.into_inner()).map(|f| f.id).collect();
    assert_eq!(paged.len(), 2);
    assert_ne!(paged[0], paged[1], "the cursor moves on");
    assert!(block(guest.browse_flames(Scope::Mine, &Browse::default())).is_err(), "your own is the account's");

    let token = author.auth.get_token().unwrap().to_string();
    let palette_name = format!("it browse palette {tag}");
    let colors: Vec<u8> = (0..768).map(|i| (i % 256) as u8).collect();
    let _: serde_json::Value = block(api_post(
        &build_url(API_BASE_URL, "/api/v1/palettes"),
        &json!({"name": palette_name, "visibility": "public", "color_data": colors}),
        &token,
    ))
    .unwrap_or_else(|e| panic!("create a palette: {e}"));
    let palettes = block(guest.browse_palettes(Scope::Public, &Browse { q: palette_name.clone(), ..Default::default() })).unwrap().items.into_inner();
    let p = palettes.iter().find(|p| p.name == palette_name).expect("the public palette");
    assert!(p.owner.is_some());
    assert_eq!(crate::api::sync::palette_from_item(p).stops.len(), 256, "with its colours");

    let generators = block(guest.browse_scripts(Scope::Public, &Browse { kind: Some("generator".into()), per_page: 5, ..Default::default() })).unwrap();
    let generators = generators.items.into_inner();
    assert!(!generators.is_empty() && generators.iter().all(|s| s.kind.as_deref() == Some("generator")), "{generators:?}");

    for id in &ids {
        block(author.delete_flame(id)).unwrap();
    }
}

/// Your own resources (docs/archive/projects/catalog.md, *Phase 4*): a palette of
/// yours published as an item, then renamed and made public; a fractal of
/// yours renamed and made public -- each as the Catalog's info pane asks.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn your_own_items_publish_rename_and_change_visibility() {
    if server().is_none() {
        return;
    }
    let author = signed_in();
    let tag = &crate::scripted::mint_id()[..8];
    let mut palette = crate::scene::palette::Palette::fire();
    palette.name = format!("it own palette {tag}");
    let made = block(author.create_palette(&palette, ApiVisibility::Private)).unwrap_or_else(|e| panic!("publish a palette: {e}"));
    assert_eq!(made.name, palette.name);
    let guest = ApiState::default();
    let hidden = block(guest.browse_palettes(Scope::Public, &Browse { q: palette.name.clone(), ..Default::default() })).unwrap().items.into_inner();
    assert!(hidden.iter().all(|p| p.id != made.id), "private: not listed to others");

    let renamed = format!("it renamed palette {tag}");
    block(author.patch_item("palettes", &made.id, &types::ItemPatch { name: Some(renamed.clone()), visibility: Some(ApiVisibility::Public) }))
        .unwrap_or_else(|e| panic!("rename a palette: {e}"));
    let shown = block(guest.browse_palettes(Scope::Public, &Browse { q: renamed.clone(), ..Default::default() })).unwrap().items.into_inner();
    assert!(shown.iter().any(|p| p.id == made.id && p.name == renamed), "public under its new name");

    let id = block(author.save_flame(&flame_config(), Some(&format!("it own flame {tag}")), Some(ApiVisibility::Private), None)).unwrap();
    let new_name = format!("it renamed flame {tag}");
    block(author.patch_item("fractals", &id, &types::ItemPatch { name: Some(new_name.clone()), visibility: Some(ApiVisibility::Public) }))
        .unwrap_or_else(|e| panic!("rename a fractal: {e}"));
    let found = block(guest.browse_flames(Scope::Public, &Browse { q: new_name.clone(), ..Default::default() })).unwrap().items.into_inner();
    assert!(found.iter().any(|f| f.id == id), "public under its new name");

    block(author.delete_flame(&id)).unwrap();
    block(author.delete_palette(&made.id)).unwrap();
}

/// Curated browsing (docs/projects/curated-content.md, step 4): the
/// curated lists hold curated items only; a canonical name resolves (a
/// built-in variation's too); and what *Open by ID or link* reads --
/// a palette by id, whether an item exists, a public animation for a
/// guest -- answers.
#[test]
#[ignore = "needs a local ffapi: python scripts/api_integration.py"]
fn curated_browsing_and_opening_by_id() {
    if server().is_none() {
        return;
    }
    let guest = ApiState::default();
    let curated = block(guest.browse_flames(Scope::Curated, &Browse { per_page: 20, ..Default::default() })).expect("the curated fractals");
    assert!(curated.items.into_inner().iter().all(|f| f.curated.is_some()), "a curated list holds curated items only");
    let animations = block(guest.browse_animations(Scope::Curated, &Browse { per_page: 5, ..Default::default() })).expect("the curated animations");
    assert!(animations.items.into_inner().iter().all(|a| a.curated.is_some()));
    let linear = block(guest.resolve_curated("variation", "linear")).expect("a built-in variation resolves");
    assert_eq!((linear.resource.as_str(), linear.href.as_str()), ("variation", "/api/v1/variations/linear"));
    assert!(block(guest.resolve_curated("fractal", "no-such-name-anywhere-x1")).is_err());

    let author = signed_in();
    let mut palette = crate::scene::palette::Palette::default();
    palette.name = "it open".into();
    let made = block(author.create_palette(&palette, ApiVisibility::Unlisted)).expect("a palette");
    let got = block(guest.get_palette(&made.id)).expect("an unlisted palette opens by id");
    assert_eq!(got.content_hash, made.content_hash);
    let flame_id = block(author.save_flame(&flame_config(), Some("it open"), Some(ApiVisibility::Public), None)).unwrap();
    block(guest.exists("fractals", &flame_id)).expect("a public fractal exists");
    assert!(block(guest.exists("palettes", &flame_id)).is_err(), "not a palette");
    let animation = crate::animation::Animation::new("it open".into(), 5.0);
    let anim_id = block(author.save_animation(&animation, Some("it open"), Some(&flame_id), Some(ApiVisibility::Public))).expect("an animation");
    block(guest.load_animation_full(&anim_id)).expect("a guest loads a public animation");
    let mine = block(author.browse_animations(Scope::Mine, &Browse::default())).expect("your own animations");
    assert!(mine.items.into_inner().iter().any(|a| a.id == anim_id));
}
