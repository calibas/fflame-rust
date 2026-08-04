//! Variation reachability census — counts what real renders actually
//! feed every variation.
//!
//! Design: `docs/projects/variation-reachability-census.md`. The math
//! probe answers *what can diverge* (variation × input-class); this
//! answers *which input classes occur* in real renders, so the two
//! join into a ranked worklist instead of 400-odd equally-weighted
//! curiosities.
//!
//! # Where the instrument lives
//!
//! Entirely inside the generated `apply_variations` dispatcher — the
//! one place every variation call already flows through. No template
//! or header changes: the counters live in a tail region of the
//! existing histogram buffer (the solid depth region's trick), and the
//! classification helpers are emitted beside the dispatcher. With
//! `ShaderConstants::census` false, not one byte of the generated WGSL
//! changes — the canonical shader dumps enforce that for free.
//!
//! # What is counted, and why per-xform for inputs
//!
//! Every normal-phase variation of a transform receives the *same*
//! point, so normal-phase input classes are counted **per transform**
//! (one interesting-only atomic per iteration) and attributed to
//! variations on the CPU, which knows each transform's active set.
//! Pre/post variations are chained — each receives the previous one's
//! output — so those inputs are counted **per variation**. Outputs are
//! always per variation.
//!
//! Classification is bit-based (`bitcast`, exponent/mantissa fields):
//! bit ops are immune to fast-math, so the instrument cannot be lied
//! to by the thing it measures.
//!
//! # Constraints (v1)
//!
//! - Direct-histogram output path only (the tail rides the histogram
//!   binding; the sample-emit path binds `samples` instead) — same
//!   constraint the probe has, and census renders are small.
//! - Solid rendering excluded: solid appends its own tail (depth
//!   region, bounds, shadow maps) and composing the two layouts buys
//!   a handful of corpus flames at real complexity. The runner skips
//!   solid flames and says so.
//! - Subflame dispatchers are not instrumented.

/// One u32 counter per possible transform, before the class tables:
/// how many times `apply_variations` ran for that `xform_id`. The
/// denominator for every fraction in the report.
pub const SEL_XFORMS: usize = 128;

/// Per-component value classes. Order is load-bearing: the WGSL
/// classifier and [`component_class_name`] index by it.
///
/// TINY and LARGE are defined by *squaring*, because that is how the
/// known bugs reached singular inputs: `ho` hit `atan2(0,0)` not at
/// v == 0 but wherever `v*v` underflowed. |x| < 1e-19 squares below
/// the subnormal range; |x| > 1e16 squares past the 1e32 bad-value
/// threshold (`probe::classify::HUGE`).
pub const CLASSES: usize = 9;

/// Class indices, mirrored exactly in the emitted WGSL.
pub mod class {
    pub const POS_ZERO: u32 = 0;
    pub const NEG_ZERO: u32 = 1;
    pub const SUBNORMAL: u32 = 2;
    pub const TINY: u32 = 3;
    pub const NORMAL: u32 = 4;
    pub const LARGE: u32 = 5;
    pub const HUGE: u32 = 6;
    pub const INF: u32 = 7;
    pub const NAN: u32 = 8;
}

/// Human name for a component class index.
pub fn component_class_name(c: u32) -> &'static str {
    match c {
        0 => "+0",
        1 => "-0",
        2 => "subnormal",
        3 => "tiny",
        4 => "normal",
        5 => "large",
        6 => "huge",
        7 => "inf",
        8 => "nan",
        _ => "?",
    }
}

/// Pair classes: `class(x) * 9 + class(y)` — 81 combinations. Slots
/// 81..90 hold the z-component class for 3D points (the pair covers
/// x/y). 90..96 spare, so the stride stays a friendly number.
pub const PAIR_CLASSES: usize = 81;
pub const Z_BASE: usize = 81;
pub const STRIDE: usize = 96;

/// The one pair class that is *not* interesting: (NORMAL, NORMAL).
/// Everything else fires an atomic; this fires nothing, which is what
/// keeps the instrument cheap on the hot path.
pub const ORDINARY_PAIR: u32 = (class::NORMAL * CLASSES as u32) + class::NORMAL;

/// Table capacity in variations. Matches
/// `MAX_VARIATIONS_PER_FLAME` — asserted by a test below rather than
/// imported, so this module states its layout in plain numbers.
pub const MAX_VARS: usize = 100;

/// Word offsets of each table, relative to the census base (which is
/// `width * height * 4` — the end of the direct histogram's pixel
/// data; solid is excluded, see module docs).
pub const XIN_BASE: usize = SEL_XFORMS;
pub const VOUT_BASE: usize = XIN_BASE + SEL_XFORMS * STRIDE;
pub const VPP_BASE: usize = VOUT_BASE + MAX_VARS * STRIDE;
/// Total tail size in u32 words (~126 KB).
pub const TOTAL_WORDS: usize = VPP_BASE + MAX_VARS * STRIDE;

/// The classification and counting helpers, emitted once before the
/// dispatcher when the census flag is on. All names `census_`-prefixed
/// per the helper-collision convention.
pub fn helpers_wgsl() -> &'static str {
    r#"
// ---- variation reachability census (see src/census/mod.rs) ----
// Counters live in a tail of the histogram buffer starting at
// census_base(). Classification is bit-based so fast-math cannot
// misreport the classes it is being used to investigate.

fn census_base() -> u32 {
    return params.width * params.height * 4u;
}

// Component class of one value. Indices mirror census::class in Rust.
fn census_class(v: f32) -> u32 {
    let bits = bitcast<u32>(v);
    let abs_bits = bits & 0x7fffffffu;
    if (abs_bits == 0u) {
        return select(0u, 1u, bits != 0u);          // +0 / -0
    }
    let exp = abs_bits >> 23u;
    if (exp == 0u) { return 2u; }                    // subnormal
    if (exp == 255u) {
        return select(8u, 7u, (abs_bits & 0x007fffffu) == 0u); // nan / inf
    }
    let a = abs(v);
    if (a < 1e-19) { return 3u; }                    // tiny: a*a underflows
    if (a > 1e32)  { return 6u; }                    // huge: past bad-value
    if (a > 1e16)  { return 5u; }                    // large: a*a overflows
    return 4u;                                        // normal
}

fn census_pair(p: vec2<f32>) -> u32 {
    return census_class(p.x) * 9u + census_class(p.y);
}

// Count `cls` into the table at word offset `table` (relative to the
// census base) — unless it is the ordinary case, which is the whole
// hot path and fires nothing.
fn census_note(table: u32, cls: u32) {
    if (cls != 40u) {                                // (NORMAL, NORMAL)
        atomicAdd(&histogram[census_base() + table + cls], 1u);
    }
}

// z components use the slots after the pair table; NORMAL is skipped.
fn census_note_z(table: u32, z: f32) {
    let cz = census_class(z);
    if (cz != 4u) {
        atomicAdd(&histogram[census_base() + table + 81u + cz], 1u);
    }
}

fn census_sel(xform_id: u32) {
    atomicAdd(&histogram[census_base() + xform_id], 1u);
}

// Normal-phase input, counted per transform (see module docs).
fn census_in2(xform_id: u32, p: vec2<f32>) {
    census_note(128u + xform_id * 96u, census_pair(p));
}
fn census_in3(xform_id: u32, p: vec3<f32>) {
    let table = 128u + xform_id * 96u;
    census_note(table, census_pair(p.xy));
    census_note_z(table, p.z);
}

// Per-variation output (table VOUT) and pre/post input (table VPP).
// Bases mirror census::VOUT_BASE / VPP_BASE.
fn census_out2(local_idx: u32, v: vec2<f32>) {
    census_note(12416u + local_idx * 96u, census_pair(v));
}
fn census_out3(local_idx: u32, v: vec3<f32>) {
    let table = 12416u + local_idx * 96u;
    census_note(table, census_pair(v.xy));
    census_note_z(table, v.z);
}
fn census_pp2(local_idx: u32, v: vec2<f32>) {
    census_note(22016u + local_idx * 96u, census_pair(v));
}
fn census_pp3(local_idx: u32, v: vec3<f32>) {
    let table = 22016u + local_idx * 96u;
    census_note(table, census_pair(v.xy));
    census_note_z(table, v.z);
}
// ---- end census helpers ----
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The WGSL hardcodes the table bases and stride as literals (WGSL
    /// has no shared constants with Rust). This pins them together —
    /// change one side and this names the drift.
    #[test]
    fn wgsl_literals_match_the_rust_layout() {
        let w = helpers_wgsl();
        assert!(w.contains(&format!("census_note(128u + xform_id * {STRIDE}u")));
        assert!(
            w.contains(&format!("census_note({}u + local_idx * {STRIDE}u", VOUT_BASE)),
            "VOUT_BASE {VOUT_BASE} not found in WGSL"
        );
        assert!(
            w.contains(&format!("census_note({}u + local_idx * {STRIDE}u", VPP_BASE)),
            "VPP_BASE {VPP_BASE} not found in WGSL"
        );
        assert!(w.contains("cls != 40u"));
        assert_eq!(ORDINARY_PAIR, 40);
        assert_eq!(XIN_BASE, 128);
    }

    #[test]
    fn capacity_matches_the_engine_limit() {
        assert_eq!(
            MAX_VARS,
            crate::scene::transforms::MAX_VARIATIONS_PER_FLAME,
            "census tables sized for a different variation cap than the engine's"
        );
    }

    /// CPU reference of the WGSL classifier, used by the readback side
    /// and pinned here against hand-computed cases.
    #[test]
    fn classifier_reference_cases() {
        assert_eq!(classify(0.0), class::POS_ZERO);
        assert_eq!(classify(-0.0), class::NEG_ZERO);
        assert_eq!(classify(1e-40), class::SUBNORMAL);
        assert_eq!(classify(1e-20), class::TINY);
        assert_eq!(classify(1.0), class::NORMAL);
        assert_eq!(classify(1e17), class::LARGE);
        assert_eq!(classify(2e32), class::HUGE);
        assert_eq!(classify(f32::INFINITY), class::INF);
        assert_eq!(classify(f32::NAN), class::NAN);
        // The boundary the whole scheme leans on: the smallest normal
        // is TINY (it squares to zero), not subnormal.
        assert_eq!(classify(1.1754944e-38), class::TINY);
    }
}

/// CPU mirror of the WGSL classifier — the readback side speaks the
/// same classes. Bit checks first, magnitude buckets after, exactly
/// like the shader.
pub fn classify(v: f32) -> u32 {
    let bits = v.to_bits();
    let abs_bits = bits & 0x7fff_ffff;
    if abs_bits == 0 {
        return if bits != 0 { class::NEG_ZERO } else { class::POS_ZERO };
    }
    let exp = abs_bits >> 23;
    if exp == 0 {
        return class::SUBNORMAL;
    }
    if exp == 255 {
        return if (abs_bits & 0x007f_ffff) == 0 { class::INF } else { class::NAN };
    }
    let a = v.abs();
    if a < 1e-19 {
        class::TINY
    } else if a > 1e32 {
        class::HUGE
    } else if a > 1e16 {
        class::LARGE
    } else {
        class::NORMAL
    }
}
