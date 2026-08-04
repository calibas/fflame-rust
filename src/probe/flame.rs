//! Turning a [`Batch`] into a flame the real shader builder accepts.
//!
//! The probe does not implement variation dispatch. It builds a flame
//! whose transform *i* holds exactly variation *i*, hands it to the
//! ordinary [`ShaderBuilder`], and calls the generated
//! `apply_variations(xform_i, i, …)`. Whatever the renderer does — phase
//! ordering, weight folding, the `NeedsAccum` and `WritesColor`
//! plumbing, helper-library splicing — the probe does too, because it is
//! literally the same code.
//!
//! The alternative, a harness that calls each variation function
//! directly, would mean reimplementing the ~300 lines of signature and
//! call-site generation in `build_apply_variations_2d`. Two copies of
//! that drift, and a probe that drifts tests something the renderer does
//! not do.

use super::batch::Batch;
use crate::scene::transforms::{Flame, Transform};

/// The affine every probe transform carries.
///
/// 123 variations declare `NeedsTransform` and read these coefficients
/// directly, so the values are part of the experiment and have to be
/// fixed — but not *identity*. Identity leaves `b` and `d` at zero, and
/// a variation that divides by one of them would produce the same
/// division-by-zero for every input, turning a whole row of the report
/// into uniform `n` glyphs that hide whatever else the variation does.
///
/// So: non-degenerate (determinant ~0.975), asymmetric, and nothing
/// zero. Changing these values invalidates every existing report, which
/// is what the schema number is for.
const AFFINE: [f32; 7] = [
    1.0,   // a
    0.25,  // b
    0.1,   // c
    -0.3,  // d
    0.9,   // e
    -0.2,  // f
    0.15,  // g — the Z offset, so 3D probes are not all coplanar
];

/// The variation that makes the normal phase a pass-through. See
/// [`super::batch::Target::needs_carrier`].
pub const CARRIER: &str = "linear";

/// Build the flame for one batch: one transform per target.
pub fn build_probe_flame(batch: &Batch) -> Flame {
    let mut flame = Flame::new();
    flame.name = "variation probe".to_string();
    flame.transforms.clear();

    for target in &batch.targets {
        let mut xf = Transform::new();
        xf.a = AFFINE[0];
        xf.b = AFFINE[1];
        xf.c = AFFINE[2];
        xf.d = AFFINE[3];
        xf.e = AFFINE[4];
        xf.f = AFFINE[5];
        xf.g = AFFINE[6];
        xf.weight = 1.0;

        // The carrier goes in first so the target's own dispatch order
        // is unaffected by whether it needed one — `variation_order` is
        // insertion order, and it fixes the emission order the local
        // index map is built around.
        if target.needs_carrier() {
            xf.set_variation(CARRIER, 1.0);
        }
        // `set_variation` rather than a raw map insert: it records
        // `variation_order`. A no-op when the target *is* the carrier.
        xf.set_variation(&target.name, 1.0);

        flame.transforms.push(xf);
    }

    flame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::batch::{builtin_targets, plan_batches, Target};
    use crate::variations::VariationPhase;

    fn target(name: &str, phase: VariationPhase) -> Target {
        Target {
            name: name.to_string(),
            slots: 0,
            needs_init: false,
            phase,
        }
    }

    fn batch(targets: Vec<Target>) -> Batch {
        Batch { targets, slots: 0 }
    }

    #[test]
    fn transform_i_holds_target_i() {
        let b = batch(vec![
            target("spherical", VariationPhase::Normal),
            target("sinusoidal", VariationPhase::Normal),
        ]);
        let flame = build_probe_flame(&b);

        assert_eq!(flame.transforms.len(), 2);
        assert!(flame.transforms[0].variations.contains_key("spherical"));
        assert!(flame.transforms[1].variations.contains_key("sinusoidal"));
        // The positional correspondence is the whole readback contract —
        // if it slips, every result is attributed to the wrong variation.
        assert!(!flame.transforms[0].variations.contains_key("sinusoidal"));
    }

    #[test]
    fn a_normal_variation_gets_no_carrier() {
        // The carrier would add `p` to the sum and mask a target that
        // wrongly returns zero — the atan2 signal.
        let flame = build_probe_flame(&batch(vec![target("spherical", VariationPhase::Normal)]));
        assert_eq!(flame.transforms[0].variations.len(), 1);
    }

    #[test]
    fn pre_and_post_variations_get_the_carrier() {
        for phase in [VariationPhase::Pre, VariationPhase::Post] {
            let flame = build_probe_flame(&batch(vec![target("whatever", phase.clone())]));
            let vars = &flame.transforms[0].variations;
            assert!(
                vars.contains_key(CARRIER),
                "{phase:?} needs a carrier or it evaluates to zero"
            );
            assert_eq!(vars.len(), 2);
        }
    }

    #[test]
    fn a_pre_phase_carrier_does_not_duplicate_when_the_target_is_linear() {
        // `linear` is Normal so this cannot happen today, but the
        // no-op is what makes `set_variation` safe to call twice.
        let mut t = target(CARRIER, VariationPhase::Pre);
        t.name = CARRIER.to_string();
        let flame = build_probe_flame(&batch(vec![t]));
        assert_eq!(flame.transforms[0].variations.len(), 1);
        assert_eq!(flame.transforms[0].variation_order.len(), 1);
    }

    #[test]
    fn the_affine_is_non_degenerate() {
        let flame = build_probe_flame(&batch(vec![target("linear", VariationPhase::Normal)]));
        let t = &flame.transforms[0];
        let det = t.a * t.e - t.b * t.d;
        assert!(det.abs() > 0.1, "a near-singular affine would collapse inputs");
        for (name, v) in [("a", t.a), ("b", t.b), ("c", t.c), ("d", t.d), ("e", t.e), ("f", t.f)] {
            assert!(v != 0.0, "{name} is zero — variations dividing by it would all NaN");
        }
    }

    #[test]
    fn every_real_batch_builds_a_flame_within_the_transform_cap() {
        // MAX_TRANSFORMS is 128; the batches are capped at 99 targets,
        // so this holds — but it is the kind of thing that breaks
        // silently when a cap moves.
        for b in plan_batches(&builtin_targets()) {
            let flame = build_probe_flame(&b);
            assert_eq!(flame.transforms.len(), b.targets.len());
            assert!(flame.transforms.len() <= 128);
        }
    }

    #[test]
    fn a_full_batch_stays_within_the_variations_per_flame_cap() {
        // Distinct names plus the carrier must not exceed 100.
        for b in plan_batches(&builtin_targets()) {
            let flame = build_probe_flame(&b);
            let distinct: std::collections::HashSet<&String> = flame
                .transforms
                .iter()
                .flat_map(|t| t.variations.keys())
                .collect();
            assert!(
                distinct.len() <= 100,
                "{} distinct variations exceeds MAX_VARIATIONS_PER_FLAME",
                distinct.len()
            );
        }
    }
}
