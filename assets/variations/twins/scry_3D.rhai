/// 3D-aware companion to [`SCRY`] (Larry Berlin, Sep 2009). Distinct
/// variation, not an alias: the inversion radius is weight-dependent
/// (`r = 1 / (sqrt(t) × (t + 1/w))`), signed by the variation amount;
/// a second factor `u` handles the Z component over a different
/// denominator (`s = sqrt(t) + z²`). Z falls back to
/// `cos(sqrt(x²+y²))` whenever the input or accumulator Z is zero,
/// so the variation lands on a cylinder for the first iteration
/// rather than collapsing flat. For |weight| ≤ 1 the cpp's
/// `smooth = 1 − w` blend collapses to pure additive on Z; for
/// |weight| > 1 the prior accumulator is wiped and replaced by
/// `w × (accumZ + Z·u)` — both branches are handled exactly.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - Larry Berlin

// Scry 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Scry 3D", name: "rhai_scry_3d", category: "full_3d" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let vvar_inv = 1.0 / (w + (if w < 0.0 { -1.0e-12 } else { 1.0e-12 }));
    let abs_inv = abs(vvar_inv);
    let sign_factor = (if vvar_inv < 0.0 { -1.0 } else { 1.0 });
    let t = z.re * z.re + z.im * z.im;
    let safe_f = max(sqrt(t), 1.0e-30);
    let r = sign_factor / (safe_f * (t + abs_inv));
    complex(z.re * r, z.im * r)
}

fn variation3(v, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let vvar_inv = 1.0 / (w + (if w < 0.0 { -1.0e-12 } else { 1.0e-12 }));
    let t = v.x * v.x + v.y * v.y;
    let f = sqrt(t);
    let kikr = cos(f);
    let footzee = (if v.z == 0.0 { kikr } else { v.z });
    let foopzee = (if ctx.accum.z == 0.0 { kikr } else { ctx.accum.z });
    let s = f + footzee * footzee;
    let abs_inv = abs(vvar_inv);
    let sign_factor = (if vvar_inv < 0.0 { -1.0 } else { 1.0 });
    let safe_f = max(f, 1.0e-30);
    let safe_s = max(s, 1.0e-30);
    let r = sign_factor / (safe_f * (t + abs_inv));
    let u = sign_factor / (sqrt(safe_s) * (s + abs_inv));
    let smoothed = (if abs(w) <= 1.0 { 1.0 - w } else { 0.0 });
    let return_z = ctx.accum.z * (smoothed - 1.0) * inv_w + (foopzee + footzee * u);
    vec3(v.x * r, v.y * r, return_z)
}
