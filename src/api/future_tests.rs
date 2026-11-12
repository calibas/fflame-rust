//! Payloads from the future: what a newer server or a newer app may send
//! an older client, each read as specified and written back unchanged
//! (docs/projects/api-updates.md, step 1). Every value here is one no
//! build knows -- `hologram`, `sideways`, `Teleport` -- so these keep
//! testing what an older client does with a newer one's data after the
//! real new values arrive.

use super::types::*;
use serde_json::json;

fn list_item(id: &str, render_mode: &str) -> serde_json::Value {
    json!({
        "id": id, "name": format!("flame {id}"), "render_mode": render_mode,
        "transform_count": 3, "variation_names": ["linear"], "has_3d": false,
        "created_at": "2026-11-11T00:00:00Z", "updated_at": "2026-11-11T00:00:00Z",
    })
}

/// A render mode this build does not know reads as `Unknown`, keeps its
/// spelling, and is written back as read.
#[test]
fn an_unknown_render_mode_reads_and_round_trips() {
    let m: ApiRenderMode = serde_json::from_value(json!("hologram")).unwrap();
    assert_eq!(m, ApiRenderMode::Unknown("hologram".into()));
    assert!(!m.is_known());
    assert_eq!(m.render_mode(), None);
    assert_eq!(serde_json::to_value(&m).unwrap(), json!("hologram"));
    // The known ones keep their spellings both ways.
    for (mode, wire) in [(ApiRenderMode::TwoD, "2d"), (ApiRenderMode::ThreeD, "3d"), (ApiRenderMode::Escape, "escape"), (ApiRenderMode::Simulation, "simulation")] {
        assert_eq!(serde_json::to_value(&mode).unwrap(), json!(wire));
        assert_eq!(serde_json::from_value::<ApiRenderMode>(json!(wire)).unwrap(), mode);
    }
}

/// A list page with a flame of a newer kind keeps it (marked unknown),
/// and an item that cannot be read at all is skipped; the rest of the
/// page survives. Before, either failed the whole page.
#[test]
fn a_list_page_survives_newer_and_broken_items() {
    let mut broken = list_item("b", "2d");
    broken.as_object_mut().unwrap().remove("name");
    let page = json!([list_item("a", "2d"), list_item("h", "hologram"), broken, list_item("c", "escape")]);
    let items = serde_json::from_value::<Lenient<FlameListItem>>(page).unwrap().into_inner();
    let ids: Vec<&str> = items.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["a", "h", "c"]);
    assert_eq!(items[1].render_mode, ApiRenderMode::Unknown("hologram".into()));
    // Not a list at all is still an error.
    assert!(serde_json::from_value::<Lenient<FlameListItem>>(json!({"error": "x"})).is_err());
}

/// The phases: `any`, which the server stores (migration 071) and which
/// failed every variation download before, is known; a newer phase reads
/// as `Unknown` and lands in the normal bucket.
#[test]
fn variation_phases_any_and_unknown() {
    let any: ApiVariationPhase = serde_json::from_value(json!("any")).unwrap();
    assert_eq!(any, ApiVariationPhase::Any);
    let newer: ApiVariationPhase = serde_json::from_value(json!("sideways")).unwrap();
    assert_eq!(newer, ApiVariationPhase::Unknown("sideways".into()));
    assert_eq!(serde_json::to_value(&newer).unwrap(), json!("sideways"));
}

/// A parameter type this build does not know reads as `Unknown`, holding
/// what was read (string or object), written back the same.
#[test]
fn an_unknown_param_type_reads_and_round_trips() {
    for raw in [json!("vector3"), json!({"vector": {"size": 3}})] {
        let t: ApiParamType = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(t, ApiParamType::Unknown(raw.clone()));
        assert_eq!(serde_json::to_value(&t).unwrap(), raw);
    }
    let e: ApiParamType = serde_json::from_value(json!({"enum": {"choices": ["a", "b"]}})).unwrap();
    assert_eq!(e, ApiParamType::Enum { choices: vec!["a".into(), "b".into()] });
    assert_eq!(serde_json::to_value(&e).unwrap(), json!({"enum": {"choices": ["a", "b"]}}));
    assert_eq!(serde_json::from_value::<ApiParamType>(json!("angle")).unwrap(), ApiParamType::Angle);
}

/// Loop modes and visibilities: a newer value reads, round-trips, and a
/// loop mode plays as a loop.
#[test]
fn unknown_loop_modes_and_visibilities() {
    let l: ApiLoopMode = serde_json::from_value(json!("bounce_twice")).unwrap();
    assert_eq!(crate::animation::LoopMode::from(l.clone()), crate::animation::LoopMode::Loop);
    assert_eq!(serde_json::to_value(&l).unwrap(), json!("bounce_twice"));
    let v: ApiVisibility = serde_json::from_value(json!("followers")).unwrap();
    assert_eq!(serde_json::to_value(&v).unwrap(), json!("followers"));
    assert_eq!(serde_json::from_value::<ApiLoopMode>(json!("ping_pong")).unwrap(), ApiLoopMode::PingPong);
}

/// A track of a newer kind is kept as it was read, plays nothing, and is
/// written back byte for byte; the animation's other tracks load and
/// play. Before, the one track emptied the list, and a save erased them
/// all on the server.
#[test]
fn a_newer_track_is_kept_and_the_rest_play() {
    use crate::animation::{Animation, TrackSource};
    let teleport = json!({"target": "Zoom", "source": {"type": "Teleport", "to": [1, 2]}, "flame_target": "Main"});
    let keyframes = json!({"target": "Zoom", "source": {"type": "Keyframes", "keyframes": [
        {"time": 0.0, "value": 1.0, "easing": "Linear"}, {"time": 1.0, "value": 3.0, "easing": "Linear"}]}});
    let tracks: Vec<crate::animation::Track> = serde_json::from_value(json!([keyframes, teleport.clone()])).unwrap();
    assert_eq!(tracks.len(), 2);
    assert!(matches!(tracks[0].source, TrackSource::Keyframes { .. }));
    assert!(matches!(&tracks[1].source, TrackSource::Unknown(raw) if *raw == teleport));
    assert_eq!(tracks[1].target, "Zoom");
    // Written back exactly as read.
    assert_eq!(serde_json::to_value(&tracks[1]).unwrap(), teleport);

    // Through the API's animation response, as a pre-#158 client met a
    // script track: both tracks arrive, and the keyframes play.
    let resp: AnimationResponse = serde_json::from_value(json!({
        "id": "x", "user_id": "u", "name": "a", "duration": 1.0, "loop_mode": "loop", "visibility": "private",
        "tracks": serde_json::to_value(&tracks).unwrap(),
        "created_at": "", "updated_at": "",
    }))
    .unwrap_or_else(|e| panic!("the response shape changed: {e}"));
    let anim: Animation = super::sync::animation_response_to_animation(&resp);
    assert_eq!(anim.tracks.len(), 2);
    let mut ctl = crate::animation::AnimationController::new();
    ctl.load(anim);
    let mut base = crate::config::FractalConfig::default();
    base.zoom = 1.0;
    let values = ctl.evaluate_at_time(0.5, &base);
    assert!(values.iter().any(|(_, path, v)| path == "Zoom" && v.as_f64().is_some_and(|z| (z - 2.0).abs() < 1e-9)), "{values:?}");
}
