// fractwf.wgsl — escape-time fractal helpers shared by the
// `fract_*_wf` variation family. Each variation in the family picks an
// iterator "kind" and lets these helpers run the standard
// AbstractFractWFFunc.transformIterate body (random-seed pick + escape
// loop + clip-retry + output mapping).
//
// Injected by the shader builder when any `fract_*_wf` variation is
// active in the flame. Dispatch into the iterator math is via the
// `kind` enum on `fractwf_next_iter` — runtime switch, but each branch
// is small and only the requested kind survives constant folding in
// the inner loop for variations that pick a fixed kind. (Mandelbrot /
// Julia families dispatch their *power* sub-iterator at the same
// site; users typically pick one power per flame.)
//
// Buddhabrot mode (buddhabrot_mode > 0) is NOT implemented in v1. It
// requires persistent per-thread state (chooseNewPoint + current
// trajectory point + iter counter) across multiple plot iterations.
// Variations honoring that path would declare `state_count: 6` and
// extend `fractwf_iterate_body` with a Buddhabrot branch. For now,
// passing buddhabrot_mode > 0 falls through to standard iterate mode
// (matches JWildfire output shape but not the rendered trajectory).

// Hard upper bounds on the escape loop. Reasonable per-call work is
// `max_iter × max_clip_iter` inner iterations, multiplied by 32K
// threads × `iterations_per_thread` (256 default) per dispatch. With
// these caps, the worst case is ~4 billion ops per dispatch, which
// keeps the dispatch under the ~2s Windows TDR budget on most GPUs.
//
// JWildfire's CPU code accepts much higher `max_iter` (the slider goes
// to 100000), but they don't have a per-dispatch budget; their cost
// just shows up as slower frames. We clamp here so a high-detail
// fractal-like Mandelbrot interior at, say, max_iter = 1000 doesn't
// kill the device — the rendered image will show iter_count saturating
// at the cap and the user can lower max_iter if they want truthful
// counts. (Future: an auto-scale would shrink iterations_per_thread
// in tandem.)
const FRACTWF_MAX_ITER_CAP: u32 = 500u;
const FRACTWF_MAX_CLIP_ITER_CAP: u32 = 4u;

// Iterator kinds — passed to `fractwf_next_iter` to pick the math.
// Julia / Mandelbrot have sub-kinds for powers 2, 3, 4, and "N" (≥5).
const FRACTWF_KIND_DRAGON: u32     = 0u;
const FRACTWF_KIND_JULIA2: u32     = 1u;
const FRACTWF_KIND_JULIA3: u32     = 2u;
const FRACTWF_KIND_JULIA4: u32     = 3u;
const FRACTWF_KIND_JULIA_N: u32    = 4u;
const FRACTWF_KIND_MAND2: u32      = 5u;
const FRACTWF_KIND_MAND3: u32      = 6u;
const FRACTWF_KIND_MAND4: u32      = 7u;
const FRACTWF_KIND_MAND_N: u32     = 8u;
const FRACTWF_KIND_METEORS: u32    = 9u;
const FRACTWF_KIND_PEARLS: u32     = 10u;
const FRACTWF_KIND_SALAMANDER: u32 = 11u;

// One step of the iterator's escape function.
//
// Coordinate roles by kind (slightly inconsistent because each
// JWildfire iterator class chose a different convention):
//   - Dragon, Julia, Pearls, Salamander: c = (cust_a, cust_b)
//     (Java `xseed`, `yseed` — fixed per call).
//   - Mandelbrot: c = (start_x, start_y) (the random seed = current
//     starting point each retry).
//   - Meteors: uses both start_x/y and the running x/y, no separate
//     seed.
//
// We pass everything every call. WGSL has no closures and we don't
// want to specialize the inner loop per kind, so the unused args are
// just ignored by each branch.
//
// `power` only matters for Julia / Mandelbrot "N" variants (≥5).
fn fractwf_next_iter(
    x: f32, y: f32, xs: f32, ys: f32,
    start_x: f32, start_y: f32,
    cust_a: f32, cust_b: f32,
    power: u32, kind: u32,
) -> vec2<f32> {
    switch (kind) {
        case 0u: {  // Dragon: (z - z² + ys) · (xseed + i·yseed) — JWF DragonIterator
            let nx = (x - xs + ys) * cust_a - (y - 2.0 * x * y) * cust_b;
            let ny = (x - xs + ys) * cust_b + (y - 2.0 * x * y) * cust_a;
            return vec2<f32>(nx, ny);
        }
        case 1u: {  // Julia z² + c
            return vec2<f32>(xs - ys + cust_a, 2.0 * x * y + cust_b);
        }
        case 2u: {  // Julia z³ + c
            return vec2<f32>(xs * x - 3.0 * x * ys + cust_a, 3.0 * xs * y - ys * y + cust_b);
        }
        case 3u: {  // Julia z⁴ + c
            return vec2<f32>(xs * xs + ys * ys - 6.0 * xs * ys + cust_a,
                             4.0 * x * y * (xs - ys) + cust_b);
        }
        case 4u: {  // Julia z^N + c (N ≥ 5) — JWF JuliaNIterator
            var nx = x * (xs * xs - 10.0 * xs * ys + 5.0 * ys * ys);
            var ny = y * (ys * ys - 10.0 * xs * ys + 5.0 * xs * xs);
            for (var k: u32 = 5u; k < power; k = k + 1u) {
                let xa = x * nx - y * ny;
                let ya = x * ny + nx * y;
                nx = xa;
                ny = ya;
            }
            return vec2<f32>(nx + cust_a, ny + cust_b);
        }
        case 5u: {  // Mandelbrot z² + c (c = start_x, start_y)
            return vec2<f32>(xs - ys + start_x, 2.0 * x * y + start_y);
        }
        case 6u: {  // Mandelbrot z³ + c
            return vec2<f32>(xs * x - 3.0 * x * ys + start_x, 3.0 * xs * y - ys * y + start_y);
        }
        case 7u: {  // Mandelbrot z⁴ + c
            return vec2<f32>(xs * xs + ys * ys - 6.0 * xs * ys + start_x,
                             4.0 * x * y * (xs - ys) + start_y);
        }
        case 8u: {  // Mandelbrot z^N + c (N ≥ 5)
            var nx = x * (xs * xs - 10.0 * xs * ys + 5.0 * ys * ys);
            var ny = y * (ys * ys - 10.0 * xs * ys + 5.0 * xs * xs);
            for (var k: u32 = 5u; k < power; k = k + 1u) {
                let xa = x * nx - y * ny;
                let ya = x * ny + nx * y;
                nx = xa;
                ny = ya;
            }
            return vec2<f32>(nx + start_x, ny + start_y);
        }
        case 9u: {  // Meteors — JWF MeteorsIterator
            let denom = xs + ys;
            let nx = (start_x * x - start_y * y) - (start_x * x + start_y * y) / denom;
            let ny = (start_x * y + start_y * x) + (start_x * y - start_y * x) / denom;
            return vec2<f32>(nx, ny);
        }
        case 10u: {  // Pearls — JWF PearlsIterator
            let denom = xs + ys;
            let nx = cust_a * x - cust_b * y - ((cust_a * x + cust_b * y) / denom);
            let ny = cust_a * y + cust_b * x + ((cust_a * y - cust_b * x) / denom);
            return vec2<f32>(nx, ny);
        }
        case 11u: {  // Salamander — JWF SalamanderIterator
            let nx = (xs - ys) * cust_a - (2.0 * x * y) * cust_b - 1.0;
            let ny = (xs - ys) * cust_b + (2.0 * x * y) * cust_a;
            return vec2<f32>(nx, ny);
        }
        default: { return vec2<f32>(x, y); }
    }
}

// Escape-time iteration count from (x0, y0). Matches the post-increment
// counter semantics of `AbstractFractWFFunc.Iterator.iterate()`: an
// immediate bailout returns 1.
fn fractwf_iterate_count(
    x0: f32, y0: f32,
    cust_a: f32, cust_b: f32,
    power: u32, max_iter: u32, kind: u32,
) -> u32 {
    var x = x0;
    var y = y0;
    var xs = x * x;
    var ys = y * y;
    var i: u32 = 0u;
    loop {
        if (i >= max_iter || xs + ys >= 4.0) { break; }
        let r = fractwf_next_iter(x, y, xs, ys, x0, y0, cust_a, cust_b, power, kind);
        x = r.x;
        y = r.y;
        xs = x * x;
        ys = y * y;
        i = i + 1u;
    }
    return i;
}

struct FractwfIterateResult {
    x0: f32,
    y0: f32,
    iter_count: f32,
    hide: f32,  // 0 = plot, 1 = skip (all retries failed the clip filter)
}

// Iterate-mode body. Picks a random seed in [xmin, xmax] × [ymin, ymax]
// (or uses the affine input when color_only > 0), runs the escape, and
// retries up to `max_clip_iter` times if the iteration count lies in
// the rejection band (`clip_iter_min` / `clip_iter_max`).
fn fractwf_iterate_body(
    affine_p: vec2<f32>,
    xmin: f32, xmax: f32, ymin: f32, ymax: f32,
    cust_a: f32, cust_b: f32,
    max_iter: u32, max_clip_iter: u32,
    clip_iter_min: i32, clip_iter_max: i32,
    color_only: u32,
    power: u32, kind: u32,
    rng: ptr<function, RngState>,
) -> FractwfIterateResult {
    var x0: f32 = 0.0;
    var y0: f32 = 0.0;
    var iter_count: u32 = 0u;
    var hidden: bool = false;

    // Clamp the user-set iteration bounds to the GPU-safe caps. See
    // FRACTWF_MAX_ITER_CAP / FRACTWF_MAX_CLIP_ITER_CAP for the budget
    // reasoning. When color_only is on the seed is fixed and every
    // retry produces the same iter_count — pointless on CPU and a TDR
    // risk on GPU, so we short-circuit retries to 1.
    let max_iter_safe = min(max_iter, FRACTWF_MAX_ITER_CAP);
    let clip_max_user = min(max(max_clip_iter, 1u), FRACTWF_MAX_CLIP_ITER_CAP);
    let clip_max = select(clip_max_user, 1u, color_only > 0u);
    for (var i: u32 = 0u; i < clip_max; i = i + 1u) {
        if (color_only > 0u) {
            x0 = affine_p.x;
            y0 = affine_p.y;
        } else {
            x0 = (xmax - xmin) * rng_nextf(rng) + xmin;
            y0 = (ymax - ymin) * rng_nextf(rng) + ymin;
        }
        iter_count = fractwf_iterate_count(x0, y0, cust_a, cust_b, power, max_iter_safe, kind);
        let i_signed = i32(iter_count);
        let too_high = (clip_iter_max < 0) && (i_signed >= i32(max_iter_safe) + clip_iter_max);
        let too_low = (clip_iter_min > 0) && (i_signed <= clip_iter_min);
        if (too_high || too_low) {
            if (i == clip_max - 1u) {
                hidden = true;
            }
            // retry next iteration
        } else {
            break;
        }
    }

    return FractwfIterateResult(x0, y0, f32(iter_count), select(0.0, 1.0, hidden));
}

// Map iterCount → Z. Matches transformIterate's `z` block including
// the optional log scale and `z_fill` jitter that lerps between this
// step's Z and the previous step's Z.
fn fractwf_compute_z(
    iter_count: f32, max_iter_f: f32,
    scalez: f32, offsetz: f32,
    z_logscale: u32, z_fill: f32,
    rng: ptr<function, RngState>,
) -> f32 {
    let ic_ratio = iter_count / max_iter_f;
    let scalez10 = scalez * 0.1;
    let log10 = 1.0 / log(10.0);
    var z: f32;
    if (z_logscale == 1u) {
        z = scalez10 * log(1.0 + ic_ratio) * log10 + offsetz;
    } else {
        z = scalez10 * ic_ratio + offsetz;
    }
    if (z_fill > 1e-6 && rng_nextf(rng) < z_fill) {
        let prev_ratio = (iter_count - 1.0) / max_iter_f;
        var prev_z: f32;
        if (z_logscale == 1u) {
            prev_z = scalez10 * log(1.0 + prev_ratio) * log10 + offsetz;
        } else {
            prev_z = scalez10 * prev_ratio + offsetz;
        }
        z = (prev_z - z) * rng_nextf(rng) + z;
    }
    return z;
}

// Apply direct-color side effect: `*vc += iter_count / max_iter`, then
// wrap once into [0, 1] (matches JWildfire's `if (>1) vc -= 1` followed
// by a clamp). Only called when direct_color != 0.
fn fractwf_apply_direct_color(
    vc: ptr<function, f32>,
    iter_count: f32, max_iter_f: f32,
) {
    var new_vc = *vc + iter_count / max_iter_f;
    if (new_vc > 1.0) { new_vc = new_vc - 1.0; }
    if (new_vc < 0.0) { new_vc = 0.0; }
    if (new_vc > 1.0) { new_vc = 1.0; }
    *vc = new_vc;
}

// Master per-variation body (2D). Each `fract_*_wf` variation's
// generated function is a thin wrapper: read its params (slot indices
// differ across the family because the custom params reserved 0–2
// slots at index 6+), pass values here.
//
// `kind` picks the iterator. For variations with a runtime power
// dispatch (Julia, Mandelbrot) the caller selects the right sub-kind
// before invoking.
fn fractwf_variation_body_2d(
    affine_p: vec2<f32>,
    max_iter: u32,
    xmin: f32, xmax: f32, ymin: f32, ymax: f32,
    cust_a: f32, cust_b: f32,
    direct_color: u32,
    clip_iter_min: i32, clip_iter_max: i32, max_clip_iter: u32,
    scale: f32, offsetx: f32, offsety: f32,
    color_only: u32,
    power: u32, kind: u32,
    rng: ptr<function, RngState>,
    vc: ptr<function, f32>,
) -> vec2<f32> {
    let r = fractwf_iterate_body(
        affine_p, xmin, xmax, ymin, ymax,
        cust_a, cust_b,
        max_iter, max_clip_iter,
        clip_iter_min, clip_iter_max,
        color_only, power, kind,
        rng,
    );
    if (r.hide > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }
    if (direct_color != 0u) {
        fractwf_apply_direct_color(vc, r.iter_count, f32(max_iter));
    }
    if (color_only > 0u) {
        return vec2<f32>(0.0, 0.0);
    }
    return vec2<f32>(scale * (r.x0 + offsetx), scale * (r.y0 + offsety));
}

// Master per-variation body (3D). Same shape as 2D plus the Z mapping.
fn fractwf_variation_body_3d(
    affine_p: vec3<f32>,
    max_iter: u32,
    xmin: f32, xmax: f32, ymin: f32, ymax: f32,
    cust_a: f32, cust_b: f32,
    direct_color: u32, scalez: f32,
    clip_iter_min: i32, clip_iter_max: i32, max_clip_iter: u32,
    scale: f32, offsetx: f32, offsety: f32, offsetz: f32,
    z_fill: f32, z_logscale: u32,
    color_only: u32,
    power: u32, kind: u32,
    rng: ptr<function, RngState>,
    vc: ptr<function, f32>,
) -> vec3<f32> {
    let r = fractwf_iterate_body(
        affine_p.xy, xmin, xmax, ymin, ymax,
        cust_a, cust_b,
        max_iter, max_clip_iter,
        clip_iter_min, clip_iter_max,
        color_only, power, kind,
        rng,
    );
    if (r.hide > 0.5) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    if (direct_color != 0u) {
        fractwf_apply_direct_color(vc, r.iter_count, f32(max_iter));
    }
    let z = scale * fractwf_compute_z(
        r.iter_count, f32(max_iter),
        scalez, offsetz, z_logscale, z_fill,
        rng,
    );
    var ox: f32 = 0.0;
    var oy: f32 = 0.0;
    if (color_only == 0u) {
        ox = scale * (r.x0 + offsetx);
        oy = scale * (r.y0 + offsety);
    }
    return vec3<f32>(ox, oy, z);
}
