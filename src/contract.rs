//! The client↔API contract, generated from the code that defines it.
//!
//! Named for the engine, not for variations: it started as a variation
//! vocabulary, then grew effect limits, and now carries the reserved
//! script stems too. The fingerprint hashes key paths rather than the
//! file name, so the rename cost consumers nothing.
//!
//! # Why this exists
//!
//! `docs/projects/VARIATIONS_WIRE_FORMAT.md` is maintained by hand, and
//! in two days of cross-repo work four of its vocabularies turned out to
//! be wrong:
//!
//! * §5 listed `parameterized` as a recognised category. There was no
//!   such arm and no such variant — 30 server rows were degrading to
//!   `Plugin` exactly like everything else. The doc did not merely fail
//!   to help; it manufactured a distinction that did not exist, and the
//!   analysis built on it was wrong in a way reading nothing would not
//!   have been.
//! * `Only3D` had no wire spelling at all — the one category the shader
//!   builder acts on (it is dropped from 2D shaders).
//! * §4 claimed a missing `shader_3d` gets a 2D-pass-through wrapper.
//!   The builder skips the variation entirely, so a silent-skip defect
//!   read as a graceful fallback.
//! * `VariationPhase::Any` — 545 of 646 variations — has no
//!   `ApiVariationPhase` counterpart, so downloaded variations lose
//!   `fx_priority` support.
//!
//! The shape they share: a hand-maintained cross-repo contract decays
//! **asymmetrically**. The parts nobody exercises rot fastest, and those
//! are exactly what someone reaches for when doing something new.
//!
//! So the vocabularies are emitted from the enums that define them.
//! [`generate`] produces the document; `contract_is_current` fails the
//! build when the committed copy drifts, the same arrangement the
//! canonical shader dumps use.
//!
//! # Regenerating
//!
//! ```text
//! UPDATE_CONTRACT=1 cargo test --lib contract_is_current
//! ```

use std::collections::BTreeMap;

use crate::script::{ScriptFlags, ScriptKind};
use crate::variations::definition::Feature;
use crate::variations::{VariationCategory, VariationPhase};

/// Where the generated contract lives, for the API repo to consume.
///
/// Named for the engine rather than for variations: it already carried
/// effect limits, and it now carries the reserved script stems too. The
/// fingerprint hashes key paths, not the file name, so this rename does
/// not move consumers' pins.
pub const CONTRACT_PATH: &str = "docs/generated/engine-contract.json";

/// Every key path in the document, sorted — the document's *shape*,
/// independent of its values.
///
/// Arrays contribute the union of their elements' paths, so a list of
/// objects is described by the keys any element carries.
fn key_paths(v: &serde_json::Value, prefix: &str, out: &mut std::collections::BTreeSet<String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                // `$comment` keys are prose; they should not make a
                // consumer's pin churn.
                if k.starts_with('$') {
                    continue;
                }
                let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                out.insert(path.clone());
                key_paths(val, &path, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                key_paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

/// A fingerprint of the contract's SHAPE, for consumers to pin.
///
/// # The problem this solves
///
/// A generated artifact consumed by checking it into another repo needs
/// a refresh discipline of its own, and "somebody remembers" is not
/// one. The API repo's conformance tests read this file; a stale copy
/// weakens them **silently** — a test that reads `plot_emits` on a copy
/// predating it simply finds nothing to check, and passes.
///
/// Pinning this value converts that into a loud failure: when the shape
/// changes, the consumer's assertion fails on the next sync with a
/// clear "expected X, got Y", and re-copying becomes a deliberate act
/// rather than a remembered one.
///
/// **Derived, not hand-maintained**, which is the point — the version
/// number I would otherwise have to remember to bump is exactly the
/// kind of thing this whole module exists because nobody remembers.
/// Adding a variation changes counts but not shape, so the fingerprint
/// is stable across corpus growth; adding a FIELD changes it.
fn shape_fingerprint(doc: &serde_json::Value) -> String {
    use sha2::{Digest, Sha256};
    let mut paths = std::collections::BTreeSet::new();
    key_paths(doc, "", &mut paths);
    let mut hasher = Sha256::new();
    for p in &paths {
        hasher.update(p.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

/// Build the contract document.
///
/// Vocabularies come from the enums; counts come from the registry, so
/// "how many variations actually use this" is measured rather than
/// remembered.
pub fn generate() -> serde_json::Value {
    let registry = crate::variations::global_registry();

    let mut category_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut phase_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut feature_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut max_slots = 0usize;
    let mut max_state = 0usize;
    let mut total = 0usize;
    // Variations whose WGSL needs a helper the builder splices by NAME
    // are not distributable — see `helper_libraries` below.
    let mut without_3d: Vec<String> = Vec::new();
    // PlotEmits carries a u8, so it cannot be a `features[]` string —
    // it needs its own wire field. Track usage and the largest payload.
    let mut plot_emits_users = 0usize;
    let mut plot_emits_max = 0u8;

    for name in registry.names() {
        let Some(info) = registry.get(name) else { continue };
        if !info.is_core {
            continue; // contract describes the shipped corpus
        }
        total += 1;
        *category_counts.entry(info.category.to_api_str()).or_default() += 1;
        *phase_counts.entry(info.phase.clone().to_api_str()).or_default() += 1;
        for f in &info.features {
            *feature_counts.entry(f.clone().to_api_str()).or_default() += 1;
            if let Feature::PlotEmits(n) = f {
                plot_emits_users += 1;
                plot_emits_max = plot_emits_max.max(*n);
            }
        }
        max_slots = max_slots.max(info.slot_count());
        max_state = max_state.max(info.state_count);
        if info.wgsl_source_3d.is_none() {
            without_3d.push(name.clone());
        }
    }

    let categories: Vec<_> = [
        VariationCategory::Basic2D,
        VariationCategory::Advanced2D,
        VariationCategory::Depth3D,
        VariationCategory::Rotation3D,
        VariationCategory::Full3D,
        VariationCategory::Only3D,
        VariationCategory::Plugin,
    ]
    .iter()
    .map(|c| {
        serde_json::json!({
            "wire": c.to_api_str(),
            "shipped_variations": category_counts.get(c.to_api_str()).copied().unwrap_or(0),
            // The one category with behaviour attached: dropped from 2D
            // shaders because it names variations with no meaningful 2D
            // reading. Every other category is UI grouping.
            "affects_compilation": matches!(c, VariationCategory::Only3D),
        })
    })
    .collect();

    let phases: Vec<_> = VariationPhase::ALL
        .iter()
        .map(|p| {
            let wire = p.clone().to_api_str();
            serde_json::json!({
                "wire": wire,
                "shipped_variations": phase_counts.get(wire).copied().unwrap_or(0),
                // `any` is missing from ApiVariationPhase today.
                "on_the_wire_today": !matches!(p, VariationPhase::Any),
            })
        })
        .collect();

    let features: Vec<_> = Feature::ALL
        .iter()
        .map(|f| {
            serde_json::json!({
                "wire": f.clone().to_api_str(),
                "shipped_variations": feature_counts.get(f.clone().to_api_str()).copied().unwrap_or(0),
                // Only three reached the wire before the features array.
                "on_the_wire_today": matches!(
                    f,
                    Feature::NeedsRng | Feature::NeedsTransform | Feature::WritesColor
                ),
            })
        })
        .collect();

    without_3d.sort();

    // The reserved stems. Read from the embedded starters rather than
    // listed here, so adding a shipped script cannot leave the API
    // accepting a name that is already taken.
    let base = crate::config::fractal_config::FractalConfig::default();
    let host = crate::script::ScriptHost::new();
    let mut builtin_scripts: Vec<serde_json::Value> = crate::script::library::EMBEDDED
        .iter()
        .map(|(file, source)| {
            let name = file.trim_end_matches(".rhai");
            // The declared kind, read the same way the panel reads it. A
            // script that fails to parse still gets a row — omitting it
            // would un-reserve its name.
            let meta = host.collect(source, &base).ok();
            serde_json::json!({
                "name": name,
                "kind": meta
                    .as_ref()
                    .and_then(|m| m.kind)
                    .map(|k| k.as_str()),
            })
        })
        .collect();
    builtin_scripts.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    let mut doc = serde_json::json!({
        "$comment": "GENERATED by src/contract.rs — do not edit. \
                     Regenerate: UPDATE_CONTRACT=1 cargo test --lib contract_is_current",
        "$shape_comment": "Fingerprint of this document's KEY STRUCTURE, not its \
                           values. Consumers that check this file into their own \
                           repo should pin it: a stale copy then fails loudly on \
                           the next sync instead of quietly under-checking. Stable \
                           across corpus growth; changes when a field is added or \
                           removed.",
        "shape": "",
        "shipped_variations": total,
        "categories": categories,
        "phases": phases,
        "features": features,
        "param_types": [
            "float", "unlimited_float", "integer", "unlimited_integer",
            "boolean", "angle", "enum"
        ],
        "limits": {
            "$comment": "Two different KINDS of number. Variation slots are \
                         packed dynamically, so the per-variation cap is a \
                         policy choice below a real engine ceiling. Effect \
                         params live in a fixed-size uniform, so that cap is \
                         physical capacity and the API must match it exactly.",
            "variation_slots_per_flame": crate::gpu::buffers::MAX_VARIATION_PARAM_SLOTS,
            "variation_slots_per_variation_policy_cap": 512,
            "largest_shipped_variation_slots": max_slots,
            "largest_shipped_state_count": max_state,
            "variations_per_flame": crate::scene::transforms::MAX_VARIATIONS_PER_FLAME,
            "transforms_per_flame": crate::gpu::buffers::MAX_TRANSFORMS,
            "effect_params": crate::renderer::effect_chain::MAX_EFFECT_PARAMS,
            "effect_slots": crate::renderer::effect_chain::MAX_EFFECT_SLOTS,
            "script_source_bytes": crate::script::store::MAX_SCRIPT_BYTES,
        },
        "helper_libraries": {
            "$comment": "Libraries the shader builder splices in. All but \
                         needs_mobius_lib are gated on hardcoded NAMES, so a \
                         server-hosted variation cannot reach them — its WGSL \
                         must be self-contained. No SQL check can verify this; \
                         it is a curation constraint.",
            "name_gated": {
                "noise.wgsl": ["dc_perlin", "crackle", "dc_crackle_wf"],
                "voronoi.wgsl": ["crackle", "dc_crackle_wf"],
                "fractwf.wgsl": ["fract_*_wf"],
                "subflame.wgsl": ["(any flame using a subflame)"],
            },
            "always_present": ["complex.wgsl"],
            "feature_gated": { "needs_mobius_lib": "SL(2,C) Mobius library" },
        },
        // Not a `features[]` entry: it carries a payload, so it needs
        // its own field the way init_param_count and state_count do.
        // Absent from the wire today.
        "payload_features": [
            {
                "$comment": "engine_clamp is verified against \
                             ShaderBuilder::plot_emit_cap, which takes the MAX \
                             across a flame's active variations and applies \
                             .min(16). It is not a shared or additive budget: \
                             two variations declaring 16 still yield 16. So a \
                             per-row CHECK is the right shape, and a value \
                             above the clamp is silently reduced rather than \
                             rejected — the catalog would advertise a number \
                             that never takes effect.",
                "wire": "plot_emits",
                "kind": "u8",
                "shipped_variations": plot_emits_users,
                "largest_shipped_value": plot_emits_max,
                "engine_clamp": 16,
                "on_the_wire_today": false,
            }
        ],
        "shipped_without_shader_3d": without_3d,
        "scripts": {
            "$comment": "Reserved stems, and the two vocabularies a script \
                         payload carries. `builtin_scripts` is the list the \
                         API rejects at CREATE time, regardless of visibility: \
                         a user script taking a shipped stem is refused by \
                         `script::library::merge_sources` on load, so storing \
                         one produces a script nobody — including its author — \
                         can ever run. Generated from the embedded starters, \
                         because a transcribed copy is exactly the kind of \
                         implicit cross-repo dependency this file exists to \
                         eliminate.",
            "builtin_scripts": builtin_scripts,
            "kinds": [ScriptKind::Generator.as_str(), ScriptKind::Modifier.as_str()],
            "flags": {
                "$comment": "The client WARNS on an unknown flag and drops it, \
                             so the vocabulary can grow without a coordinated \
                             deploy. A server-side closed-set CHECK would \
                             remove that property — worth knowing before \
                             choosing to add one.",
                "known": ScriptFlags::KNOWN,
            },
        },
    });

    // Computed last, over the finished document, so it covers every
    // field. The placeholder is excluded from its own input because
    // `shape` is a key path either way — only the VALUE differs.
    let fingerprint = shape_fingerprint(&doc);
    doc["shape"] = serde_json::Value::String(fingerprint);
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed contract must match what the code generates.
    ///
    /// This is the whole point: the vocabulary cannot drift from the
    /// enums, because drift fails here. Follows the same arrangement as
    /// the canonical shader dumps.
    #[test]
    fn contract_is_current() {
        let fresh = serde_json::to_string_pretty(&generate()).expect("serialize");
        let path = std::path::Path::new(CONTRACT_PATH);

        if std::env::var_os("UPDATE_CONTRACT").is_some() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).expect("create dir");
            }
            std::fs::write(path, fresh.as_bytes()).expect("write contract");
            return;
        }

        let stored = std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!(
                "{CONTRACT_PATH} is unreadable ({e}).\n\
                 Regenerate: UPDATE_CONTRACT=1 cargo test --lib contract_is_current"
            )
        });

        // Compare parsed, so line endings cannot cause a false failure.
        let a: serde_json::Value = serde_json::from_str(&stored).expect("stored contract parses");
        let b: serde_json::Value = serde_json::from_str(&fresh).expect("fresh contract parses");
        assert_eq!(
            a, b,
            "the generated contract no longer matches {CONTRACT_PATH}.\n\
             If the change is intended, regenerate and review the diff — the \
             API repo consumes this file:\n    \
             UPDATE_CONTRACT=1 cargo test --lib contract_is_current"
        );
    }

    /// The reserved stems the API rejects must be exactly the stems the
    /// client refuses.
    ///
    /// This is what makes "generated, not transcribed" a property rather
    /// than an intention. `contract_is_current` catches the file going
    /// stale; this catches the generator reading the wrong source — if
    /// `builtin_scripts` were ever built from a hand-written list, the
    /// API would reject a set that does not match what the client
    /// enforces, and the mismatch would show up as a script that
    /// uploads fine and then refuses to load.
    #[test]
    fn the_contract_reserves_exactly_what_the_client_refuses() {
        let doc = generate();
        let listed: Vec<&str> = doc["scripts"]["builtin_scripts"]
            .as_array()
            .expect("builtin_scripts is an array")
            .iter()
            .map(|e| e["name"].as_str().expect("each row has a name"))
            .collect();

        assert!(!listed.is_empty(), "an empty reserved list would reserve nothing");
        for name in &listed {
            assert!(
                crate::script::library::is_builtin_stem(name),
                "the contract reserves `{name}`, but the client would not refuse it"
            );
        }
        for (file, _) in crate::script::library::EMBEDDED {
            let stem = file.trim_end_matches(".rhai");
            assert!(
                listed.contains(&stem),
                "`{stem}` is reserved by the client but missing from the contract — \
                 the API would accept a name that can never load"
            );
        }

        // A kind of `null` means the script failed to parse. Its name is
        // still reserved (that is deliberate — omitting it would
        // un-reserve it), but it would also be a broken starter.
        for e in doc["scripts"]["builtin_scripts"].as_array().unwrap() {
            assert!(
                !e["kind"].is_null(),
                "shipped script `{}` declares no kind",
                e["name"]
            );
        }
    }

    /// Every feature must be in `ALL`, or the contract silently omits it.
    #[test]
    fn all_features_are_listed() {
        // `to_api_str` is exhaustive, so a new variant fails to compile
        // there; this guards the separate `ALL` list from falling behind.
        assert_eq!(
            Feature::ALL.len(),
            13,
            "Feature::ALL is out of step with the enum — add the new variant"
        );
        for f in Feature::ALL {
            assert_eq!(Feature::from_api_str(f.clone().to_api_str()), Some(f.clone()));
        }
    }

    /// The lowercase aliases added for API name compatibility must
    /// actually resolve, and must not have been silently discarded.
    ///
    /// `register_from_def` drops an alias that collides with an existing
    /// name or another alias — correct, but it means adding one is not
    /// self-verifying: the warning goes to a log nobody reads.
    #[test]
    fn lowercase_aliases_resolve_to_their_capital_d_variations() {
        let reg = crate::variations::global_registry();
        for (alias, canonical) in [
            ("blur3d", "blur3D"),
            ("julia3d", "julia3D"),
            ("curl3d", "curl3D"),
            ("post_curl3d", "post_curl3D"),
        ] {
            assert_eq!(
                reg.resolve_alias(alias), canonical,
                "`{alias}` must resolve to `{canonical}`"
            );
            assert!(reg.has(alias), "`{alias}` must be findable");
            assert!(reg.get(alias).is_some_and(|v| v.name == canonical));
        }
    }

    /// Unknown features degrade rather than reject.
    #[test]
    fn unknown_features_are_ignorable() {
        assert_eq!(Feature::from_api_str("invented_later"), None);
    }

    /// The fingerprint tracks SHAPE, not values.
    ///
    /// This is the property a consumer pins against: adding a variation
    /// moves every count in the document but must not invalidate their
    /// pin, or the signal becomes noise and gets ignored — which is how
    /// a staleness check dies.
    #[test]
    fn the_fingerprint_ignores_values() {
        let a = serde_json::json!({ "limits": { "n": 1 }, "xs": [ { "k": "a" } ] });
        let b = serde_json::json!({ "limits": { "n": 999 }, "xs": [ { "k": "zzz" } ] });
        assert_eq!(shape_fingerprint(&a), shape_fingerprint(&b));
    }

    /// ...and does change when a field appears — the case that matters,
    /// since a consumer reading a key their copy lacks under-checks
    /// silently.
    #[test]
    fn the_fingerprint_moves_when_a_field_is_added() {
        let before = serde_json::json!({ "limits": { "n": 1 } });
        let after = serde_json::json!({ "limits": { "n": 1, "plot_emits": 16 } });
        assert_ne!(shape_fingerprint(&before), shape_fingerprint(&after));
    }

    fn download(name: &str, category: &str, s2d: Option<&str>, s3d: Option<&str>)
        -> crate::api::types::VariationDownload
    {
        crate::api::types::VariationDownload {
            id: name.into(),
            name: name.into(),
            display_name: name.into(),
            description: None,
            category: category.into(),
            version: 1,
            phase: crate::api::types::ApiVariationPhase::Normal,
            needs_rng: false,
            needs_transform: false,
            writes_color: false,
            parameters: Vec::new(),
            shader_2d: s2d.map(|x| x.into()),
            shader_3d: s3d.map(|x| x.into()),
            init_param_count: 0,
            shader_init: None,
            features: Vec::new(),
            state_count: 0,
            shader_state_init: None,
            aliases: Vec::new(),
            plot_emits: 0,
            authors: Vec::new(),
            description_plain: None,
        }
    }

    /// The wire fields land before the server sends them.
    ///
    /// Every new field is `serde(default)`, so declaring them first is
    /// forward-compatible and the client is ready the day the migration
    /// lands rather than a coordination round later. These pin the three
    /// behaviours that make that safe.
    #[test]
    fn the_features_array_supersedes_the_legacy_bools() {
        let mut dl = download("f", "advanced_2d", Some("fn f(){}"), None);

        // Absent array: fall back to the bools, as older payloads need.
        dl.needs_rng = true;
        dl.writes_color = true;
        let info = crate::variations::VariationInfo::from_download(&dl);
        assert!(info.has_feature(Feature::NeedsRng));
        assert!(info.has_feature(Feature::WritesColor));

        // Present array: authoritative, and it can express what the
        // bools never could.
        dl.features = vec!["always_z".into(), "needs_accum".into()];
        let info = crate::variations::VariationInfo::from_download(&dl);
        assert!(info.has_feature(Feature::AlwaysZ), "173 variations need this");
        assert!(info.has_feature(Feature::NeedsAccum));
        assert!(
            !info.has_feature(Feature::NeedsRng),
            "the array must supersede the bools, not merge with them"
        );
    }

    /// An unknown feature is dropped with a warning, never fatal — a
    /// newer server has to be able to serve an older client, or every
    /// future capability becomes a breaking change.
    #[test]
    fn an_unknown_feature_does_not_reject_the_variation() {
        let mut dl = download("f", "advanced_2d", Some("fn f(){}"), None);
        dl.features = vec!["needs_rng".into(), "invented_next_year".into()];
        let info = crate::variations::VariationInfo::from_download(&dl);
        assert!(info.has_feature(Feature::NeedsRng));
        assert_eq!(info.features.len(), 1, "the unknown one is dropped, not kept");
    }

    /// state_count was hardcoded to 0, which silently mis-rendered any
    /// stateful server-hosted variation: slots allocated, read as zeros.
    /// plot_emits rides in its own field because it carries a payload.
    #[test]
    fn state_and_plot_emits_come_off_the_wire_now() {
        let mut dl = download("s", "advanced_2d", Some("fn f(){}"), None);
        dl.state_count = 4;
        dl.shader_state_init = Some("fn init(){}".into());
        dl.plot_emits = 3;
        let info = crate::variations::VariationInfo::from_download(&dl);
        assert_eq!(info.state_count, 4);
        assert!(info.wgsl_source_state_init.is_some());
        assert_eq!(info.plot_emit_cap(), 3);

        // Zero means "does not emit", not "emits zero".
        let mut dl2 = download("t", "advanced_2d", Some("fn f(){}"), None);
        dl2.plot_emits = 0;
        let info2 = crate::variations::VariationInfo::from_download(&dl2);
        assert_eq!(info2.plot_emit_cap(), 0);
        assert!(!info2.features.iter().any(|f| matches!(f, Feature::PlotEmits(_))));
    }

    /// `only_3d` may omit `shader_2d`; nothing else may.
    ///
    /// The engine filters `only_3d` out of the active set in 2D builds
    /// *before* any source lookup, so a 2D body would never be read —
    /// requiring a vestigial one means dead data a curator writes, a
    /// reviewer reads, and nothing can validate.
    ///
    /// For every other category a missing 2D body reaches the emit
    /// loop's `else { continue }` and becomes a silent no-op: the
    /// variation downloads, registers, and contributes nothing. That is
    /// the failure the original required-String settlement existed to
    /// prevent, and refusing here preserves it.
    #[test]
    fn only_3d_may_omit_its_2d_body_and_nothing_else_may() {
        let mut reg = crate::variations::VariationRegistry::new();

        // Legitimate: only_3d with a 3D body and no 2D body.
        reg.register_from_api(&download("emit3d", "only_3d", None, Some("fn f(){}")));
        assert!(reg.has("emit3d"), "only_3d without shader_2d must register");

        // The dangerous case: any other category missing its 2D body.
        reg.register_from_api(&download("sneaky", "advanced_2d", None, Some("fn f(){}")));
        assert!(
            !reg.has("sneaky"),
            "a non-only_3d variation with no shader_2d must be refused, not registered to silently render nothing"
        );

        // And only_3d with NO body at all is useless in every mode.
        reg.register_from_api(&download("empty", "only_3d", None, None));
        assert!(!reg.has("empty"), "only_3d with no shader_3d must be refused");

        // The ordinary shape still works.
        reg.register_from_api(&download("fine", "advanced_2d", Some("fn f(){}"), None));
        assert!(reg.has("fine"));
    }

    /// Prose must not churn the pin: a reworded `$comment` is not a
    /// shape change, and treating it as one would train consumers to
    /// re-pin without reading.
    #[test]
    fn the_fingerprint_ignores_comment_keys() {
        let a = serde_json::json!({ "$comment": "one", "k": 1 });
        let b = serde_json::json!({ "$comment": "two, at length", "k": 1 });
        assert_eq!(shape_fingerprint(&a), shape_fingerprint(&b));
    }
}
