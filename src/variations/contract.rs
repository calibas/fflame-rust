//! The client↔API contract, generated from the code that defines it.
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

use super::definition::Feature;
use super::{VariationCategory, VariationPhase};

/// Where the generated contract lives, for the API repo to consume.
pub const CONTRACT_PATH: &str = "docs/generated/variation-contract.json";

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
    let registry = super::global_registry();

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

    let mut doc = serde_json::json!({
        "$comment": "GENERATED by src/variations/contract.rs — do not edit. \
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
                "wire": "plot_emits",
                "kind": "u8",
                "shipped_variations": plot_emits_users,
                "largest_shipped_value": plot_emits_max,
                "engine_clamp": 16,
                "on_the_wire_today": false,
            }
        ],
        "shipped_without_shader_3d": without_3d,
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
