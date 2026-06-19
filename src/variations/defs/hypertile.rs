//! Hypertile family — Zueuk's hyperbolic-tiling variations
//!
//! Six variations from JWildfire/Chaotica that map the input plane onto a
//! Schläfli-symbol {p,q} hyperbolic tiling via Möbius transformations.
//! All six share the same "tile radius" formula
//!     r = 1 / sqrt((1 + cos(qa)) / (cos(pa) + cos(qa)))
//! (with `pa = 2π/p`, `qa = 2π/q`); the difference between members is which
//! tile they target (deterministic via `n`, or random per iteration) and
//! whether they extend the Möbius warp into the third dimension.
//!
//! Sources:
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile1.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile2.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile3D.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile3D1.cpp
//!   - https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/blob/master/output/hypertile3D2.cpp
//!
//! Notes on faithfulness:
//!   - Upstream multiplies its accumulator `vr` (or `d`) by `VVAR` (the
//!     variation weight) inside the body. Our shader applies weight outside
//!     the function, so we factor `VVAR` out: `vr = 1 / denom` here, then
//!     `result += weight * variation(...)` in the caller. Algebraically
//!     identical to the upstream output.
//!   - hypertile.cpp force-enables `pContext.isPreserveZCoordinate()` via
//!     `if (true /* ... */)`, so the parent passes Z through. hypertile1 and
//!     hypertile2 don't touch Z at all in upstream; we follow the codebase
//!     convention for 2D variations (return `p.z` in the wgsl_3d wrapper).
//!   - The `r` formulas in `hypertile` (`r_inner = (1-cos(pa))/(...) + 1`),
//!     `hypertile1/2` (`r2 = 1 - (cos(pa)-1)/(...)`), and the 3D members
//!     (`r_alt = -(cos(pa)-1)/(...)` then `1/sqrt(1+r_alt)`) are
//!     algebraically equal — all yield the same `r`. The fallback branch
//!     differs slightly (each variation tests its own intermediate); we
//!     preserve each variation's exact condition.
//!   - **Discrete tile-angle sampling** for the random-tile variants
//!     (`hypertile1`, `hypertile2`, `hypertile3D1`, `hypertile3D2`).
//!     JWildfire's CPU body uses
//!         `double rpa = pContext.random(Integer.MAX_VALUE) * pa;`
//!     where `random(int n)` returns an *integer* in `[0, n)` cast to
//!     double. With `pa = 2π/p`, the resulting `rpa` is `k · (2π/p)` for
//!     some integer `k`. Modulo 2π (which is all sin/cos care about),
//!     this collapses to `(k mod p) · (2π/p)` — i.e. one of the `p`
//!     discrete tile-center angles, picked uniformly. That is what
//!     gives the {p, q} tiling its p-fold rotational symmetry.
//!
//!     Our previous WGSL `rng_nextf(rng) * pa` produced a *continuous*
//!     angle in `[0, 2π/p)`, which sampled inside one tile's slice only
//!     and dropped the symmetry — the visible output was a single tile
//!     repeated, not the full hyperbolic ring. The fix:
//!         `floor(rng_nextf(rng) * p) * pa`
//!     picks an integer `k ∈ [0, p)` and multiplies by `pa`, exactly
//!     matching the JWF CPU output distribution. `p` is the user param
//!     in init slot 0 of all four variations. (JWildfire's own GPU port
//!     hardcodes `(int)(RANDFLOAT()*10) * pa` regardless of `p` — that
//!     appears to be a JWF GPU-port bug; we match the CPU behavior.)

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// hypertile: deterministic {p,q} tile selector (n picks which tile)
//   User: p, q, n
//   Init: re, im  (tile-center offset in complex coords)
//   Body: Möbius shift onto tile (re, -im), divide by |denom|², output.
//   Z is preserved (pass-through in our 3D wrapper).
// =============================================================================
/// Maps the plane onto a {p, q} hyperbolic tiling via Möbius
/// transformation. `n` picks which tile of the tiling is targeted
/// (deterministic).
///
/// # Authors
/// - Zueuk
pub static HYPERTILE: VariationDef = VariationDef {
    name: "hypertile",
    aliases: &[],
    display_name: "Hypertile",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
        param!("n", "N", int, 1.0, 0.0, 50.0, "Index of which tile to target — deterministic tile selector."),
    ],
    // 2 derived values at slots 3..5:
    //   3: re   r * cos(n * pa)
    //   4: im   r * sin(n * pa)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_hypertile(user: array<f32, 3>) -> array<f32, 2> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let n = user[2];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r_inner = (1.0 - cos(pa)) / (cos(pa) + cos(qa)) + 1.0;
    var r: f32;
    if (r_inner > 0.0) {
        r = 1.0 / sqrt(r_inner);
    } else {
        r = 1.0;
    }
    let a = n * pa;
    var out: array<f32, 2>;
    out[0] = r * cos(a);
    out[1] = r * sin(a);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let re = get_param(xform_id, variation_id, 3u);
    let im = get_param(xform_id, variation_id, 4u);

    let a = p.x + re;
    let b = p.y - im;
    let c = re * p.x - im * p.y + 1.0;
    let d = re * p.y + im * p.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    return vec2<f32>(
        inv * (a * c + b * d),
        inv * (b * c - a * d),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let re = get_param(xform_id, variation_id, 3u);
    let im = get_param(xform_id, variation_id, 4u);

    let a = p.x + re;
    let b = p.y - im;
    let c = re * p.x - im * p.y + 1.0;
    let d = re * p.y + im * p.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    return vec3<f32>(
        inv * (a * c + b * d),
        inv * (b * c - a * d),
        p.z,
    );
}
"#,
};

// =============================================================================
// hypertile1: random tile selection per iteration; "rotate first" form
//   User: p, q
//   Init: pa (= 2π/p), r
//   Body: pick angle uniformly in [0, pa], apply Möbius shift onto that
//   tile center, divide by |denom|², output.
// =============================================================================
/// Maps the plane onto a {p, q} hyperbolic tiling — random tile per
/// iteration. The rotation picking the tile is applied first.
///
/// # Authors
/// - Zueuk
pub static HYPERTILE1: VariationDef = VariationDef {
    name: "hypertile1",
    aliases: &[],
    display_name: "Hypertile 1",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
    ],
    // 2 derived values at slots 2..4:
    //   2: pa   2π / p
    //   3: r    tile radius
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_hypertile1(user: array<f32, 2>) -> array<f32, 2> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r2 = 1.0 - (cos(pa) - 1.0) / (cos(pa) + cos(qa));
    var r: f32;
    if (r2 > 0.0) {
        r = 1.0 / sqrt(r2);
    } else {
        r = 1.0;
    }
    var out: array<f32, 2>;
    out[0] = pa;
    out[1] = r;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile1(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let re = r * cosa;
    let im = r * sina;

    let a = p.x + re;
    let b = p.y - im;
    let c = re * p.x - im * p.y + 1.0;
    let d = re * p.y + im * p.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    return vec2<f32>(
        inv * (a * c + b * d),
        inv * (b * c - a * d),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile1(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let re = r * cosa;
    let im = r * sina;

    let a = p.x + re;
    let b = p.y - im;
    let c = re * p.x - im * p.y + 1.0;
    let d = re * p.y + im * p.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    return vec3<f32>(
        inv * (a * c + b * d),
        inv * (b * c - a * d),
        p.z,
    );
}
"#,
};

// =============================================================================
// hypertile2: random tile selection per iteration; "shift first, rotate last"
//   User: p, q
//   Init: pa, r
//   Body: shift onto tile center (r, 0) in real coords, then apply random
//   rotation by angle in [0, pa]. Equivalent to hypertile1 up to which
//   step the rotation is fused with — but produces a visually distinct
//   per-frame distribution because the rotation is applied to the
//   already-Möbius-warped point rather than the tile center.
// =============================================================================
/// Variant of Hypertile1 that applies the rotation last instead of first.
/// Equivalent math up to ordering, but the per-iteration distribution looks
/// visually distinct.
///
/// # Authors
/// - Zueuk
pub static HYPERTILE2: VariationDef = VariationDef {
    name: "hypertile2",
    aliases: &[],
    display_name: "Hypertile 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
    ],
    // 2 derived values at slots 2..4: same as hypertile1.
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_hypertile2(user: array<f32, 2>) -> array<f32, 2> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r2 = 1.0 - (cos(pa) - 1.0) / (cos(pa) + cos(qa));
    var r: f32;
    if (r2 > 0.0) {
        r = 1.0 / sqrt(r2);
    } else {
        r = 1.0;
    }
    var out: array<f32, 2>;
    out[0] = pa;
    out[1] = r;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);

    let a = p.x + r;
    let b = p.y;
    let c = r * p.x + 1.0;
    let d = r * p.y;
    let mx = a * c + b * d;
    let my = b * c - a * d;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);

    return vec2<f32>(
        inv * (mx * cosa + my * sina),
        inv * (my * cosa - mx * sina),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);

    let a = p.x + r;
    let b = p.y;
    let c = r * p.x + 1.0;
    let d = r * p.y;
    let mx = a * c + b * d;
    let my = b * c - a * d;
    let denom = c * c + d * d;
    let inv = 1.0 / max(denom, 1e-30);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);

    return vec3<f32>(
        inv * (mx * cosa + my * sina),
        inv * (my * cosa - mx * sina),
        p.z,
    );
}
"#,
};

// =============================================================================
// hypertile3d: deterministic 3D tile selector (n picks tile)
//   User: p, q, n
//   Init: cx, cy, c2, c2x, c2y, s2x, s2y, s2z
//   Body: 3D Möbius reflection through a sphere whose center sits at
//   (cx, cy, 0) on the unit-disc boundary; warps the full 3D point.
// =============================================================================
/// 3D version of Hypertile — Möbius reflection through a sphere on the
/// unit-disc boundary. `n` picks which tile (deterministic).
///
/// # Authors
/// - Zueuk
pub static HYPERTILE3D: VariationDef = VariationDef {
    name: "hypertile3D",
    aliases: &[],
    display_name: "Hypertile 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
        param!("n", "N", int, 0.0, 0.0, 50.0, "Index of which tile to target — deterministic tile selector."),
    ],
    // 8 derived values at slots 3..11:
    //   3: cx     r * cos(n*pa)
    //   4: cy     r * sin(n*pa)
    //   5: c2     cx² + cy²
    //   6: c2x    2*cx
    //   7: c2y    2*cy
    //   8: s2x    1 + cx² - cy²
    //   9: s2y    1 + cy² - cx²
    //  10: s2z    1 - cx² - cy²
    init_param_count: 8,
    wgsl_init: Some(r#"
fn init_hypertile3D(user: array<f32, 3>) -> array<f32, 8> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let n = user[2];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r_alt = -(cos(pa) - 1.0) / (cos(pa) + cos(qa));
    var r: f32;
    if (r_alt > 0.0) {
        r = 1.0 / sqrt(1.0 + r_alt);
    } else {
        r = 1.0;
    }
    let na = n * pa;
    let cx = r * cos(na);
    let cy = r * sin(na);
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    var out: array<f32, 8>;
    out[0] = cx;
    out[1] = cy;
    out[2] = cx2 + cy2;
    out[3] = 2.0 * cx;
    out[4] = 2.0 * cy;
    out[5] = 1.0 + cx2 - cy2;
    out[6] = 1.0 + cy2 - cx2;
    out[7] = 1.0 - cx2 - cy2;
    return out;
}
"#),
    // 2D form: use only the in-plane part, drop z² from r2.
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let c2 = get_param(xform_id, variation_id, 5u);
    let c2x = get_param(xform_id, variation_id, 6u);
    let c2y = get_param(xform_id, variation_id, 7u);
    let s2x = get_param(xform_id, variation_id, 8u);
    let s2y = get_param(xform_id, variation_id, 9u);

    let r2 = p.x * p.x + p.y * p.y;
    let x2cx = c2x * p.x;
    let y2cy = c2y * p.y;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(denom, 1e-30);

    return vec2<f32>(
        d * (p.x * s2x - cx * (y2cy - r2 - 1.0)),
        d * (p.y * s2y + cy * (-x2cx - r2 - 1.0)),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 3u);
    let cy = get_param(xform_id, variation_id, 4u);
    let c2 = get_param(xform_id, variation_id, 5u);
    let c2x = get_param(xform_id, variation_id, 6u);
    let c2y = get_param(xform_id, variation_id, 7u);
    let s2x = get_param(xform_id, variation_id, 8u);
    let s2y = get_param(xform_id, variation_id, 9u);
    let s2z = get_param(xform_id, variation_id, 10u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let x2cx = c2x * p.x;
    let y2cy = c2y * p.y;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(denom, 1e-30);

    return vec3<f32>(
        d * (p.x * s2x - cx * (y2cy - r2 - 1.0)),
        d * (p.y * s2y + cy * (-x2cx - r2 - 1.0)),
        d * (p.z * s2z),
    );
}
"#,
};

// =============================================================================
// hypertile3d1: random tile per iteration, full 3D Möbius warp
//   User: p, q
//   Init: pa, r, c2 (= r²), s2z (= 1 - r²)
//   Body: pick angle uniformly in [0, pa], compute (cx, cy) = (r·cos, r·sin)
//   per iteration, then same 3D warp as hypertile3d.
// =============================================================================
/// 3D version of Hypertile1 — random 3D tile per iteration.
///
/// # Authors
/// - Zueuk
pub static HYPERTILE3D1: VariationDef = VariationDef {
    name: "hypertile3D1",
    aliases: &[],
    display_name: "Hypertile 3D 1",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
    ],
    // 4 derived values at slots 2..6:
    //   2: pa
    //   3: r
    //   4: c2    r²
    //   5: s2z   1 - r²
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_hypertile3D1(user: array<f32, 2>) -> array<f32, 4> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r_alt = -(cos(pa) - 1.0) / (cos(pa) + cos(qa));
    var r: f32;
    if (r_alt > 0.0) {
        r = 1.0 / sqrt(1.0 + r_alt);
    } else {
        r = 1.0;
    }
    let c2 = r * r;
    var out: array<f32, 4>;
    out[0] = pa;
    out[1] = r;
    out[2] = c2;
    out[3] = 1.0 - c2;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile3D1(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let cx = r * cosa;
    let cy = r * sina;
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    let s2x = 1.0 + cx2 - cy2;
    let s2y = 1.0 + cy2 - cx2;

    let r2 = p.x * p.x + p.y * p.y;
    let x2cx = 2.0 * cx * p.x;
    let y2cy = 2.0 * cy * p.y;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(denom, 1e-30);

    return vec2<f32>(
        d * (p.x * s2x - cx * (y2cy - r2 - 1.0)),
        d * (p.y * s2y + cy * (-x2cx - r2 - 1.0)),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile3D1(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let r = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);
    let s2z = get_param(xform_id, variation_id, 5u);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let cx = r * cosa;
    let cy = r * sina;
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    let s2x = 1.0 + cx2 - cy2;
    let s2y = 1.0 + cy2 - cx2;

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let x2cx = 2.0 * cx * p.x;
    let y2cy = 2.0 * cy * p.y;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(denom, 1e-30);

    return vec3<f32>(
        d * (p.x * s2x - cx * (y2cy - r2 - 1.0)),
        d * (p.y * s2y + cy * (-x2cx - r2 - 1.0)),
        d * (p.z * s2z),
    );
}
"#,
};

// =============================================================================
// hypertile3d2: tile-on-real-axis warp + per-iter random rotation
//   User: p, q
//   Init: pa, cx (= r), c2, c2x, s2x, s2y, s2z
//   Body: 3D Möbius warp through a sphere centered at (r, 0, 0), then
//   rotate the (x, y) result by a random angle in [0, pa].
// =============================================================================
/// 3D version of Hypertile2 — tile centered on the real axis, with per-
/// iteration random XY rotation applied after the Möbius warp.
///
/// # Authors
/// - Zueuk
pub static HYPERTILE3D2: VariationDef = VariationDef {
    name: "hypertile3D2",
    aliases: &[],
    display_name: "Hypertile 3D 2",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[
        param!("p", "P", int, 3.0, 3.0, 50.0, "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.)."),
        param!("q", "Q", int, 7.0, 3.0, 50.0, "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`."),
    ],
    // 7 derived values at slots 2..9:
    //   2: pa
    //   3: cx    r
    //   4: c2    cx² (= r²)
    //   5: c2x   2 * cx
    //   6: s2x   1 + cx²
    //   7: s2y   1 - cx²    (s2y == s2z in upstream; we keep both for clarity)
    //   8: s2z   1 - cx²
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_hypertile3D2(user: array<f32, 2>) -> array<f32, 7> {
    let pi = 3.14159265358979;
    let p = user[0];
    let q = user[1];
    let pa = 2.0 * pi / p;
    let qa = 2.0 * pi / q;
    let r_alt = -(cos(pa) - 1.0) / (cos(pa) + cos(qa));
    var r: f32;
    if (r_alt > 0.0) {
        r = 1.0 / sqrt(1.0 + r_alt);
    } else {
        r = 1.0;
    }
    let cx = r;
    let cx2 = cx * cx;
    var out: array<f32, 7>;
    out[0] = pa;
    out[1] = cx;
    out[2] = cx2;
    out[3] = 2.0 * cx;
    out[4] = 1.0 + cx2;
    out[5] = 1.0 - cx2;
    out[6] = 1.0 - cx2;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_hypertile3D2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let cx = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);
    let c2x = get_param(xform_id, variation_id, 5u);
    let s2x = get_param(xform_id, variation_id, 6u);
    let s2y = get_param(xform_id, variation_id, 7u);

    let r2 = p.x * p.x + p.y * p.y;
    let x2cx = c2x * p.x;
    let x = p.x * s2x - cx * (-r2 - 1.0);
    let y = p.y * s2y;
    let denom = c2 * r2 + x2cx + 1.0;
    let inv = 1.0 / max(denom, 1e-30);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);

    return vec2<f32>(
        inv * (x * cosa + y * sina),
        inv * (y * cosa - x * sina),
    );
}
"#,
    wgsl_3d: r#"
fn variation_hypertile3D2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let pa = get_param(xform_id, variation_id, 2u);
    let cx = get_param(xform_id, variation_id, 3u);
    let c2 = get_param(xform_id, variation_id, 4u);
    let c2x = get_param(xform_id, variation_id, 5u);
    let s2x = get_param(xform_id, variation_id, 6u);
    let s2y = get_param(xform_id, variation_id, 7u);
    let s2z = get_param(xform_id, variation_id, 8u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;
    let x2cx = c2x * p.x;
    let x = p.x * s2x - cx * (-r2 - 1.0);
    let y = p.y * s2y;
    let denom = c2 * r2 + x2cx + 1.0;
    let inv = 1.0 / max(denom, 1e-30);

    // Discrete tile-angle sampling — see module-level "Notes on
    // faithfulness" for why the previous `rng_nextf * pa` dropped the
    // p-fold symmetry. `p` is the user param in init slot 0.
    let ang = floor(rng_nextf(rng) * get_param(xform_id, variation_id, 0u)) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);

    return vec3<f32>(
        inv * (x * cosa + y * sina),
        inv * (y * cosa - x * sina),
        inv * (p.z * s2z),
    );
}
"#,
};
