//! Float precision policy.
//!
//! Every numeric parameter in a flame is an `f32`, and `f32` is the hard
//! limit on what the renderer can represent. This module is the single
//! place that decides how such a value is written down and how a slider
//! quantizes one, so the UI, saved `.fflame` files and the cloud API all
//! agree.
//!
//! The rule is **shortest round-trip**: emit the shortest decimal string
//! that parses back to the identical `f32`. That is exactly what Rust's
//! `{}` does for `f32`, so [`fmt_f32`] is a thin wrapper over it.
//!
//! Deliberately NOT a fixed number of decimal places:
//! * `f32` carries ~7.2 significant digits, but round-tripping needs up to
//!   **9** in the worst case — truncating at 7 would silently alter values.
//! * "Decimal places" is the wrong unit anyway: `123456.78` needs 8
//!   significant digits but 2 decimal places, while `1e-7` needs 1
//!   significant digit but 7 decimal places. Any fixed cap breaks at one
//!   end of the range.
//!
//! The companion problem is that `serde_json::Value` has no `f32`, so
//! routing a config through it widens every number to `f64` and prints the
//! full expansion (`0.98` became `0.9799999594688416`). [`normalize_f32_numbers`]
//! undoes that just before serialization.

/// Shortest decimal string that round-trips to this exact `f32`.
///
/// `0.98 -> "0.98"`, `1.0/3.0 -> "0.33333334"`. Non-finite values fall back
/// to `"0"`, matching JSON's lack of NaN/Infinity.
pub fn fmt_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "0".to_string()
    }
}

/// Rewrite every `f64` in a JSON tree that is exactly an `f32` so that it
/// prints as that `f32`'s shortest form.
///
/// `serde_json::to_value` widens `f32` fields to `f64`; printing them then
/// shows the whole binary expansion. Re-parsing the shortest `f32` string
/// back into an `f64` gives a value whose own shortest-`f64` form IS that
/// string, so the emitted JSON is minimal and still reloads to the identical
/// `f32`.
///
/// Numbers that are not exactly representable as `f32` (nothing in a flame
/// config today, but integers and future `f64` fields could be) are left
/// untouched.
pub fn normalize_f32_numbers(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Number(n) => {
            let Some(x) = n.as_f64() else { return };
            // Integers keep their exact JSON representation.
            if n.is_i64() || n.is_u64() {
                return;
            }
            if x as f32 as f64 != x {
                return; // genuinely needs f64 precision — leave it alone
            }
            if let Ok(parsed) = fmt_f32(x as f32).parse::<f64>() {
                if let Some(num) = serde_json::Number::from_f64(parsed) {
                    *n = num;
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(normalize_f32_numbers),
        serde_json::Value::Object(map) => map.values_mut().for_each(normalize_f32_numbers),
        _ => {}
    }
}

/// A "nice" quantum for dragging across `span`, one of 1/2/5 x 10^k.
///
/// A slider is only a couple of hundred pixels wide, so there is no point
/// letting a drag land on arbitrary values: it makes tidy numbers hard to
/// hit and litters saved files with noise like `0.97999996`. Snapping to a
/// round decimal means a drag produces the nearest `f32` to a SHORT decimal,
/// which in turn keeps [`fmt_f32`] output short. Exact values are still
/// reachable by typing, which never snaps.
pub fn nice_step(span: f64) -> f64 {
    let span = span.abs();
    if !span.is_finite() || span <= 0.0 {
        return 0.0; // no sensible quantum — don't snap
    }
    // Aim for ~200 stops across the range (about one per pixel).
    let raw = span / 200.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Snap `value` to the nearest multiple of `step`, anchored at `origin`.
///
/// Rounding through the decimal string keeps the result the nearest `f32`
/// to a short decimal rather than a neighbouring `f32` a ULP away.
pub fn snap_to_step(value: f64, origin: f64, step: f64) -> f32 {
    if step <= 0.0 || !step.is_finite() {
        return value as f32;
    }
    let snapped = origin + ((value - origin) / step).round() * step;
    // Re-round in decimal space: `0.1 * 3.0` is not `0.3` in binary.
    let decimals = decimals_for_step(step);
    let factor = 10f64.powi(decimals as i32);
    ((snapped * factor).round() / factor) as f32
}

/// How many decimal places a step of this size implies (0.05 -> 2).
pub fn decimals_for_step(step: f64) -> usize {
    if step <= 0.0 || !step.is_finite() {
        return 6;
    }
    let d = -step.log10().floor();
    d.clamp(0.0, 9.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_round_trip_is_exact_and_minimal() {
        for v in [0.98f32, 1.0 / 3.0, 0.1, 1e-7, 123456.78, -2.5, 0.0] {
            let s = fmt_f32(v);
            assert_eq!(s.parse::<f32>().unwrap(), v, "{s} must reload identically");
        }
        // Minimal, not padded to some fixed width.
        assert_eq!(fmt_f32(0.98), "0.98");
        assert_eq!(fmt_f32(0.5), "0.5");
        // ...and honest when the value really is not the tidy decimal: this
        // is the value that was showing up as 0.9799999594688416.
        let one_ulp_below = f32::from_bits(0.98f32.to_bits() - 1);
        assert_eq!(fmt_f32(one_ulp_below), "0.97999996");
    }

    #[test]
    fn json_numbers_lose_the_f64_expansion() {
        // What `serde_json::to_value` does to an f32 today.
        let mut v = serde_json::json!({
            "drift": 0.98f32 as f64,
            "nested": [{ "x": 0.1f32 as f64 }],
            "count": 7,
        });
        assert!(serde_json::to_string(&v).unwrap().contains("0.9800000190734863"));

        normalize_f32_numbers(&mut v);
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"drift\":0.98"), "got {s}");
        assert!(s.contains("\"x\":0.1"), "got {s}");
        assert!(s.contains("\"count\":7"), "integers untouched: {s}");

        // Still reloads to the same f32.
        let back: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(back["drift"].as_f64().unwrap() as f32, 0.98f32);
    }

    #[test]
    fn nice_steps_are_round_decimals() {
        assert_eq!(nice_step(1.0), 0.005); // 0..1 slider
        assert_eq!(nice_step(0.2), 0.001); // 0.9..1.1 (Length Drift)
        assert_eq!(nice_step(8.0), 0.05); // -4..4 (rapidity)
        assert_eq!(nice_step(0.0), 0.0); // degenerate: no snapping
    }

    #[test]
    fn snapping_lands_on_tidy_decimals() {
        // A drag that would have produced 0.9799999... lands on 0.98 exactly.
        let snapped = snap_to_step(0.97999996, 0.9, 0.001);
        assert_eq!(snapped, 0.98f32);
        assert_eq!(fmt_f32(snapped), "0.98");
        // Step 0 means "don't snap" rather than divide by zero.
        assert_eq!(snap_to_step(0.123456, 0.0, 0.0), 0.123456f32);
    }

    #[test]
    fn decimals_track_the_step() {
        assert_eq!(decimals_for_step(1.0), 0);
        assert_eq!(decimals_for_step(0.05), 2);
        assert_eq!(decimals_for_step(0.001), 3);
    }
}
