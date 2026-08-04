//! Choosing which parameter values to probe.
//!
//! The base pass evaluates every variation at its default parameters,
//! which exercises exactly one path through code that often branches on
//! those parameters. A variation whose `if (fixed_dist_calc)` arm is
//! only reachable with the flag on is, at default, half untested — and
//! the untested half is the half nobody looked at.
//!
//! The sweep is **one parameter at a time**: parameter *j* moves while
//! every other stays at its default. Combinatorial coverage of ~4.6
//! parameters per variation across 646 variations is not a thing worth
//! wanting; it would be millions of dispatches to find bugs that, if
//! they exist, need two specific parameters to conspire. One at a time
//! finds the branch, which is what was asked for.
//!
//! Parameters live in a storage buffer read through `get_param`, so a
//! sweep step is a buffer write, not a shader rebuild. The sweep costs
//! dispatches; it does not cost the 14 compiles again.

use crate::variations::{ParamType, VariationInfo, VariationParameter};

/// Enums with more choices than this are truncated. No enum ships near
/// it today; the cap exists so that adding a 200-choice parameter
/// degrades the runtime instead of exploding it — and the truncation is
/// reported rather than silent.
pub const MAX_ENUM_STEPS: usize = 8;

/// Fallback bounds for the unlimited types, whose `min`/`max` are slider
/// hints rather than limits, and for parameters that declare neither.
const FALLBACK_LOW: f32 = -10.0;
const FALLBACK_HIGH: f32 = 10.0;

/// One parameter's sweep.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamPlan {
    pub param: String,
    /// Values to try, in order. Never contains duplicates.
    pub values: Vec<f32>,
    /// Choices dropped by [`MAX_ENUM_STEPS`], so the report can say so.
    pub dropped: usize,
}

/// The values to try for one parameter.
///
/// Booleans and enums matter most: those are the ones the shader
/// branches on, so a value there reaches code the default never does.
/// Numeric parameters get their extremes plus zero — extremes because
/// that is where a formula overflows or a domain check bites, and zero
/// because it is where things get divided by.
pub fn sweep_values(p: &VariationParameter) -> (Vec<f32>, usize) {
    let mut dropped = 0;
    let mut values: Vec<f32> = match &p.param_type {
        // Both arms, always — the whole point.
        ParamType::Boolean => vec![0.0, 1.0],

        ParamType::Enum { choices } => {
            let n = choices.len();
            let take = n.min(MAX_ENUM_STEPS);
            dropped = n - take;
            (0..take).map(|i| i as f32).collect()
        }

        ParamType::Integer | ParamType::UnlimitedInteger => {
            let low = p.min_value.unwrap_or(FALLBACK_LOW).round();
            let high = p.max_value.unwrap_or(FALLBACK_HIGH).round();
            with_zero_between(low, high)
        }

        ParamType::Float | ParamType::UnlimitedFloat | ParamType::Angle => {
            let low = p.min_value.unwrap_or(FALLBACK_LOW);
            let high = p.max_value.unwrap_or(FALLBACK_HIGH);
            with_zero_between(low, high)
        }
    };

    // A parameter whose bounds collapse (min == max, or min > max in a
    // malformed definition) would otherwise contribute duplicate
    // dispatches that all record the same thing.
    values.retain(|v| v.is_finite());
    dedup_preserving_order(&mut values);
    (values, dropped)
}

/// `[low, 0, high]`, with zero only when it genuinely lies between them.
///
/// Including zero unconditionally would push parameters outside their
/// declared range — a scale that must stay positive would get probed at
/// a value the app cannot produce, and any resulting NaN would be a
/// finding about a state that never occurs.
fn with_zero_between(low: f32, high: f32) -> Vec<f32> {
    if low < 0.0 && high > 0.0 {
        vec![low, 0.0, high]
    } else {
        vec![low, high]
    }
}

fn dedup_preserving_order(values: &mut Vec<f32>) {
    let mut seen: Vec<f32> = Vec::with_capacity(values.len());
    values.retain(|v| {
        if seen.iter().any(|s| s.to_bits() == v.to_bits()) {
            false
        } else {
            seen.push(*v);
            true
        }
    });
}

/// One thing to set before one dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub param: String,
    pub value: f32,
    /// Index of this value within its parameter's sweep, so a finding
    /// can name the step rather than only the parameter.
    pub index: usize,
}

/// A variation's sweep flattened into the order it will be dispatched.
///
/// Flattening is what makes the schedule cheap. Every variation in a
/// batch sits in its own transform, so their parameters are independent
/// and round *r* can set step *r* for all of them at once. The batch
/// then costs `max steps of any one variation` dispatches rather than
/// `max parameters x max values`, which matters a great deal when one
/// variation in the batch has 157 parameters and the rest have four.
pub fn steps_for(info: &VariationInfo) -> Vec<Step> {
    plan_for(info)
        .into_iter()
        .flat_map(|plan| {
            plan.values
                .into_iter()
                .enumerate()
                .map(move |(index, value)| Step {
                    param: plan.param.clone(),
                    value,
                    index,
                })
        })
        .collect()
}

/// The full sweep for one variation. Empty for the 200-odd variations
/// that take no parameters.
pub fn plan_for(info: &VariationInfo) -> Vec<ParamPlan> {
    info.parameters
        .iter()
        .map(|p| {
            let (values, dropped) = sweep_values(p);
            ParamPlan {
                param: p.name.clone(),
                values,
                dropped,
            }
        })
        .filter(|plan| !plan.values.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::batch::builtin_targets;

    fn param(name: &str, ty: ParamType, min: Option<f32>, max: Option<f32>) -> VariationParameter {
        VariationParameter {
            name: name.to_string(),
            display_name: name.to_string(),
            param_type: ty,
            default_value: 0.0,
            min_value: min,
            max_value: max,
            description: None,
        }
    }

    #[test]
    fn booleans_get_both_arms() {
        let (v, _) = sweep_values(&param("flag", ParamType::Boolean, None, None));
        assert_eq!(v, vec![0.0, 1.0], "a branch tested on one side is untested");
    }

    #[test]
    fn enums_get_every_choice() {
        let (v, dropped) = sweep_values(&param(
            "mode",
            ParamType::Enum { choices: &["a", "b", "c"] },
            None,
            None,
        ));
        assert_eq!(v, vec![0.0, 1.0, 2.0]);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn an_oversized_enum_is_truncated_and_says_so() {
        const MANY: &[&str] = &[
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11",
        ];
        let (v, dropped) = sweep_values(&param("mode", ParamType::Enum { choices: MANY }, None, None));
        assert_eq!(v.len(), MAX_ENUM_STEPS);
        assert_eq!(dropped, MANY.len() - MAX_ENUM_STEPS, "silent truncation reads as coverage");
    }

    #[test]
    fn zero_is_included_only_when_the_range_spans_it() {
        let (spanning, _) = sweep_values(&param("x", ParamType::Float, Some(-2.0), Some(3.0)));
        assert_eq!(spanning, vec![-2.0, 0.0, 3.0]);

        // A parameter that must stay positive should not be probed at
        // zero: a NaN there would describe a state the app cannot reach.
        let (positive, _) = sweep_values(&param("scale", ParamType::Float, Some(0.5), Some(4.0)));
        assert_eq!(positive, vec![0.5, 4.0]);
    }

    #[test]
    fn a_collapsed_range_does_not_produce_duplicate_steps() {
        let (v, _) = sweep_values(&param("fixed", ParamType::Float, Some(1.0), Some(1.0)));
        assert_eq!(v, vec![1.0], "duplicate values are duplicate dispatches");
    }

    #[test]
    fn unbounded_parameters_fall_back_to_a_usable_range() {
        let (v, _) = sweep_values(&param("free", ParamType::UnlimitedFloat, None, None));
        assert_eq!(v, vec![FALLBACK_LOW, 0.0, FALLBACK_HIGH]);
    }

    #[test]
    fn no_sweep_value_is_non_finite() {
        // An infinite bound would make every variation NaN at that step
        // and tell us nothing about the variation.
        let reg = crate::variations::global_registry();
        for target in builtin_targets() {
            let Some(info) = reg.get(&target.name) else { continue };
            for plan in plan_for(info) {
                for v in &plan.values {
                    assert!(
                        v.is_finite(),
                        "{}.{} sweeps to {v}",
                        target.name,
                        plan.param
                    );
                }
            }
        }
    }

    /// What the sweep costs, in the terms that decide the runtime: the
    /// dispatch count is `max parameters in a batch x max steps`, not
    /// the total number of parameters.
    #[test]
    fn report_the_sweep_cost() {
        use crate::probe::batch::plan_batches;

        let reg = crate::variations::global_registry();
        let batches = plan_batches(&builtin_targets());

        let mut total_params = 0usize;
        let mut total_steps = 0usize;
        let mut dispatches = 0usize;
        let mut widest = (String::new(), 0usize);

        let mut grid = 0usize;
        for batch in &batches {
            let mut max_params = 0usize;
            let mut max_values = 0usize;
            let mut rounds = 0usize;
            for target in &batch.targets {
                let Some(info) = reg.get(&target.name) else { continue };
                let plans = plan_for(info);
                total_params += plans.len();
                total_steps += plans.iter().map(|p| p.values.len()).sum::<usize>();
                max_params = max_params.max(plans.len());
                for p in &plans {
                    max_values = max_values.max(p.values.len());
                }
                rounds = rounds.max(steps_for(info).len());
                if plans.len() > widest.1 {
                    widest = (target.name.clone(), plans.len());
                }
            }
            dispatches += rounds;
            grid += max_params * max_values;
        }
        println!("(a param x value grid would have cost {grid} per dimension)");

        println!(
            "{total_params} parameters, {total_steps} sweep values\n\
             {} dispatches per dimension ({} total)\n\
             widest: `{}` with {} parameters",
            dispatches,
            dispatches * 2,
            widest.0,
            widest.1
        );
    }
}
