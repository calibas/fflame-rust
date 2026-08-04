//! Packing every variation into as few flames as possible.
//!
//! The probe evaluates a variation by giving it its own transform in a
//! flame and calling the real generated `apply_variations`. Two caps
//! bound how many fit in one flame, and each flame is a shader compile,
//! so the packing decides the runtime.

/// A variation the probe intends to evaluate.
///
/// Owned rather than borrowed from the registry: `global_registry()`
/// hands out a read guard, and holding one across the whole probe run
/// would block any registry write for its duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    /// Packed parameter slots — user params plus init-derived.
    pub slots: usize,
    /// Whether the variation declares any init-derived slots. Those
    /// need the init dispatch to run, or the shader reads zeros.
    pub needs_init: bool,
    /// `Pre` and `Post` variations need a companion — see
    /// [`needs_carrier`].
    pub phase: crate::variations::VariationPhase,
}

impl Target {
    /// Whether this variation needs a `linear` alongside it in its
    /// transform to produce anything observable.
    ///
    /// The generated dispatcher runs three phases: `Pre` variations
    /// modify the input point, `Normal` variations accumulate into a
    /// weighted sum, and `Post` variations transform that sum. A
    /// transform holding *only* a pre variation therefore returns the
    /// empty normal-phase sum — zero — no matter what the pre variation
    /// computed, and a post-only transform returns `post(0)`. Either
    /// way the probe would record a column of identical glyphs for 45
    /// variations and call it a pass.
    ///
    /// Adding `linear` at weight 1.0 makes the normal phase a
    /// pass-through, so the transform evaluates to `pre(p)` or
    /// `post(p)` as intended. It is deliberately *not* added to normal
    /// variations: there the sum would become `p + target(p)`, and the
    /// offset would mask exactly the signal that matters most — a
    /// target returning zero where it should not, which is the shape of
    /// the `atan2(0,0)` bug.
    pub fn needs_carrier(&self) -> bool {
        use crate::variations::VariationPhase;
        matches!(self.phase, VariationPhase::Pre | VariationPhase::Post)
    }
}

/// Ceiling on variations the probe puts in one flame.
///
/// One below `MAX_VARIATIONS_PER_FLAME` because `linear` may need to
/// occupy a slot as the carrier above. It takes only one for the whole
/// flame however many transforms use it — the local index map is keyed
/// by name — so reserving a single slot unconditionally is both
/// sufficient and simpler than making the budget depend on the mix.
pub const MAX_PER_FLAME: usize = 99;

/// Ceiling on packed parameter slots in one flame
/// (`MAX_VARIATION_PARAM_SLOTS`).
pub const MAX_SLOTS: usize = 1600;

/// One flame's worth of variations: one transform each.
#[derive(Debug, Clone)]
pub struct Batch {
    /// The variations, in transform order. Transform *i* holds
    /// `targets[i]`, so the probe reads variation *i*'s output from
    /// `apply_variations(xform_i, i, …)`.
    pub targets: Vec<Target>,
    /// Packed slots this batch consumes, for the report.
    pub slots: usize,
}

impl Batch {
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.targets.iter().map(|t| t.name.as_str())
    }

    /// Whether any target here needs the `linear` carrier, and so
    /// whether the flame gains a variation beyond the targets.
    pub fn needs_carrier(&self) -> bool {
        self.targets.iter().any(|t| t.needs_carrier())
    }
}

/// Group targets into flames that respect both caps.
///
/// Deliberately **first-fit in registration order** rather than
/// best-fit: the packing is part of what the report describes, and an
/// order-independent packing means a variation stays in the same batch
/// as the registry grows, so a report diff shows the variation that
/// changed rather than a reshuffle. Registration order is already the
/// project's stable ID order (`defs/mod.rs` is append-only), so a new
/// variation lands at the end and disturbs nothing before it.
///
/// The count cap is not the only binding one: the widest single
/// variation takes 259 slots, so a batch of expensive variations fills
/// the slot budget after six.
pub fn plan_batches(targets: &[Target]) -> Vec<Batch> {
    let mut batches = Vec::new();
    let mut group: Vec<Target> = Vec::new();
    let mut slots = 0usize;

    for target in targets {
        // A single variation wider than the whole budget cannot be
        // probed at all. None exists today (worst is 259 of 1600) and
        // the shader builder would reject it too, but say so rather
        // than emitting a batch that fails to compile later.
        if target.slots > MAX_SLOTS {
            log::error!(
                "variation `{}` needs {} slots, over the {MAX_SLOTS} budget \
                 for a whole flame — it cannot be probed",
                target.name,
                target.slots
            );
            continue;
        }

        let full = group.len() + 1 > MAX_PER_FLAME || slots + target.slots > MAX_SLOTS;
        if !group.is_empty() && full {
            batches.push(Batch {
                targets: std::mem::take(&mut group),
                slots,
            });
            slots = 0;
        }

        group.push(target.clone());
        slots += target.slots;
    }

    if !group.is_empty() {
        batches.push(Batch { targets: group, slots });
    }
    batches
}

/// Every **shipped** variation, in registration order.
///
/// Downloads and local plugins are excluded on purpose. The report is
/// committed and compared across machines, so it has to describe the
/// same set everywhere; whatever the running user happens to have
/// installed would make two reports incomparable for a reason that has
/// nothing to do with the shader math.
pub fn builtin_targets() -> Vec<Target> {
    let reg = crate::variations::global_registry();
    reg.names()
        .iter()
        .filter_map(|n| reg.get(n))
        .filter(|info| info.provenance.is_builtin())
        .map(|info| Target {
            name: info.name.clone(),
            slots: info.slot_count(),
            needs_init: info.init_param_count > 0,
            phase: info.phase.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(name: &str, slots: usize) -> Target {
        Target {
            name: name.to_string(),
            slots,
            needs_init: false,
            phase: crate::variations::VariationPhase::Normal,
        }
    }

    #[test]
    fn every_target_lands_in_exactly_one_batch() {
        let targets = builtin_targets();
        let batches = plan_batches(&targets);

        let packed: Vec<&str> = batches
            .iter()
            .flat_map(|b| b.names())
            .collect();
        let expected: Vec<&str> = targets.iter().map(|t| t.name.as_str()).collect();

        // Equality covers loss, duplication *and* reordering — order
        // preservation is what keeps a batch stable as the registry
        // grows.
        assert_eq!(packed, expected);
    }

    #[test]
    fn no_batch_exceeds_either_cap() {
        for batch in plan_batches(&builtin_targets()) {
            assert!(
                batch.targets.len() <= MAX_PER_FLAME,
                "batch of {} exceeds the {MAX_PER_FLAME} variation cap",
                batch.targets.len()
            );
            assert!(
                batch.slots <= MAX_SLOTS,
                "batch of {} slots exceeds the {MAX_SLOTS} budget",
                batch.slots
            );
        }
    }

    #[test]
    fn the_slot_cap_can_bind_before_the_count_cap() {
        // Six at 259 fit (1554); a seventh would not. Without the slot
        // check the count cap alone would happily pack 100 of them.
        let wide: Vec<Target> = (0..7).map(|i| t(&format!("w{i}"), 259)).collect();
        let batches = plan_batches(&wide);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].targets.len(), 6);
        assert_eq!(batches[1].targets.len(), 1);
    }

    #[test]
    fn a_variation_wider_than_the_whole_budget_is_dropped_not_batched() {
        let batches = plan_batches(&[t("ok", 10), t("impossible", MAX_SLOTS + 1), t("fine", 10)]);
        let packed: Vec<&str> = batches.iter().flat_map(|b| b.names()).collect();
        assert_eq!(packed, vec!["ok", "fine"]);
    }

    /// Not an assertion about a number so much as a record of the cost:
    /// each batch is a real shader compile, so this is what a full probe
    /// run pays.
    #[test]
    fn report_the_packing_cost() {
        let targets = builtin_targets();
        let batches = plan_batches(&targets);
        let slots: usize = batches.iter().map(|b| b.slots).sum();
        let widest = targets.iter().max_by_key(|t| t.slots).unwrap();
        let with_init = targets.iter().filter(|t| t.needs_init).count();

        println!(
            "{} shipped variations -> {} batches ({slots} slots total)\n\
             widest: `{}` at {} slots; {with_init} need the init dispatch",
            targets.len(),
            batches.len(),
            widest.name,
            widest.slots,
        );
        for (i, b) in batches.iter().enumerate() {
            println!("  batch {i:>2}: {:>3} variations, {:>4} slots", b.targets.len(), b.slots);
        }
    }
}
