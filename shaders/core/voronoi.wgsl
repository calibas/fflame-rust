// voronoi.wgsl — Voronoi helpers + cell-center sampler used by the
// `crackle` family. Injected by the shader builder when any of those
// variations is active. Depends on `simplex_noise_3d` from `noise.wgsl`
// (the shader builder injects `noise.wgsl` first).
//
// Ported from `org.jwildfire.create.tina.variation.VoronoiTools` and
// `CrackleFunc.position`. WGSL doesn't support indirect array parameters
// well, so each helper takes the fixed 9-cell grid array directly (every
// caller in this branch operates on a 3×3 grid centered on the test point).
//
// All math is f32 — JWildfire uses f64 internally but the per-cell
// distances stay small enough that single precision is fine for the
// visual outputs we care about.

// Relative distance ratio between two cell centers `p` and `q` from a
// test point `u`. 0 means `u` is at `p`; 1 means `u` is equidistant
// from `p` and `q`. Used as a Voronoi-cell occupancy test.
fn voronoi_vratio(p: vec2<f32>, q: vec2<f32>, u: vec2<f32>) -> f32 {
    let pmq = p - q;
    if (pmq.x == 0.0 && pmq.y == 0.0) {
        return 1.0;
    }
    return 2.0 * dot(u - q, pmq) / dot(pmq, pmq);
}

// Index (in `points`) of the closest of `n` 2D points to `u`. Linear scan
// — `n` is always 9 in this branch's callers, so vectorizing isn't
// worth it. WGSL won't accept a runtime-length slice so we take the
// full 9-array and let the caller cap iteration via `n`.
fn voronoi_closest(points: array<vec2<f32>, 9>, n: u32, u: vec2<f32>) -> u32 {
    var best: u32 = 0u;
    var best_d2: f32 = 1.0e30;
    let cap = min(n, 9u);
    for (var i: u32 = 0u; i < cap; i = i + 1u) {
        let d = points[i] - u;
        let d2 = dot(d, d);
        if (d2 < best_d2) {
            best_d2 = d2;
            best = i;
        }
    }
    return best;
}

// "Inside-ness" of `u` in the Voronoi cell of `points[q]`, measured against
// the other `n-1` cells. Returns 0 at the center, ~1 on the cell boundary,
// > 1 outside the cell. Caller uses this as a smooth-distance scalar for
// per-cell distortion.
fn voronoi_inside(points: array<vec2<f32>, 9>, n: u32, q: u32, u: vec2<f32>) -> f32 {
    var ratio_max: f32 = -1.0e30;
    let cap = min(n, 9u);
    let pq = points[q];
    for (var i: u32 = 0u; i < cap; i = i + 1u) {
        if (i == q) { continue; }
        let r = voronoi_vratio(points[i], pq, u);
        if (r > ratio_max) {
            ratio_max = r;
        }
    }
    return ratio_max;
}

// Cell-center for grid coords `(x, y)` at z-slice `z`, half-cell size `s`,
// distortion strength `d`. Each call invokes `simplex_noise_3d` twice
// (once per output component), using the cross-wired offset vectors from
// JWildfire's `CrackleFunc.position` (the magic numbers 2.5 / 30.2 / -12.1
// / 19.8 ensure the X and Y noise samples are uncorrelated). Output is
// the (possibly distorted) cell center in flame space.
//
// No caching — JWildfire's CPU code precomputes a 21×21 cell grid in
// `init()`, but per-thread per-variation buffer state would be ~880
// floats, exceeding any reasonable budget. The trade-off is that crackle
// runs simplex_noise_3d 18× per variation call (9 cells × 2 per cell ×
// 2 passes, with the re-center step). That's the main TDR pressure for
// the family.
// Cheap per-cell offset for `crackle_fast`: an integer hash of the
// lattice coords, value-noise interpolated along z.
//
// The insight that licenses this: crackle only ever samples its noise
// at INTEGER lattice points (scaled by 2.5, which spaces simplex
// samples far past its feature size — neighbouring cells are already
// decorrelated). The one property actually used is continuity in `z`,
// so the cell layout flows when z animates. A hash per (x, y, floor z)
// plus a smoothstep blend to the next z-slice delivers exactly that
// for ~15 ALU ops where the simplex pair costs ~200. Statistically the
// same kind of offset field; the specific cell layout differs from
// `crackle`'s, which is why this feeds a separate variation instead of
// changing the existing ones.
fn crackle_cell_hash2(x: i32, y: i32, zi: i32) -> vec2<f32> {
    // Integer mix (Wang/PCG-flavoured). bitcast, not conversion: the
    // lattice coords go negative and we want their bits, wrapped.
    var h: u32 = bitcast<u32>(x) * 0x8da6b343u
        ^ bitcast<u32>(y) * 0xd8163841u
        ^ bitcast<u32>(zi) * 0xcb1ab31fu;
    h = h ^ (h >> 13u);
    h = h * 0x9e3779b1u;
    h = h ^ (h >> 16u);
    let h2 = h * 0x85ebca6bu;
    // Two lanes → [-1, 1].
    let a = f32(h & 0xffffu) * (2.0 / 65535.0) - 1.0;
    let b = f32((h2 >> 8u) & 0xffffu) * (2.0 / 65535.0) - 1.0;
    return vec2<f32>(a, b);
}

fn crackle_fast_cell_noise(x: i32, y: i32, z: f32) -> vec2<f32> {
    let zf = floor(z);
    let zi = i32(zf);
    let t = z - zf;
    let sm = t * t * (3.0 - 2.0 * t);
    return mix(
        crackle_cell_hash2(x, y, zi),
        crackle_cell_hash2(x, y, zi + 1),
        sm,
    );
}

fn voronoi_crackle_position(x: i32, y: i32, z: f32, s: f32, d: f32) -> vec2<f32> {
    // Fast path — and the DEFAULT (crackle ships distort = 0): with no
    // distortion the noise contribution is `0.0 * noise`, exactly 0.0
    // for the finite values simplex always returns, so skipping the
    // two simplex calls is bit-identical and turns the whole family's
    // dominant cost off. The branch is uniform (parameters are the
    // same for every thread in the dispatch), so it does not diverge.
    if (d == 0.0) {
        return vec2<f32>(f32(x) * s, f32(y) * s);
    }
    let fx = f32(x) * 2.5;
    let fy = f32(y) * 2.5;
    let fz = z * 2.5;
    let nx = simplex_noise_3d(vec3<f32>(fx, fy, fz));
    let ny = simplex_noise_3d(vec3<f32>(fy + 30.2, fx - 12.1, fz + 19.8));
    return vec2<f32>((f32(x) + d * nx) * s, (f32(y) + d * ny) * s);
}

// Crackle's transform body, shared by `variation_crackle` and
// `variation_dc_crackle_wf` (and their 3D wrappers). Lives here rather
// than in each variation's WGSL because the shader builder reads only
// `wgsl_2d` or `wgsl_3d` per call site and the dedupe rule is
// byte-identical: keeping the body in one place avoids any chance of
// drift between the four call sites that need it.
//
// Returns (DXo, DYo, L) — the (warped point, cell inside-ness). DC
// variants read `L` from `.z` and turn it into a palette-position
// offset; non-DC variants discard it.
//
// Algorithm (per CrackleFunc.transform):
//   1. cellsize == 0 → return (0, 0, 0). "An infinite number of
//      invisible cells? No thanks!" (per the cpp). DC variants treat
//      `L = 0` as a sentinel; they still apply `color_offset` so the
//      user sees something.
//   2. Pick `u` as a blurred unit-disc point (matches CrackleFunc's
//      `pAmount != 0` path — variation weight 0 with color_only is
//      handled by the DC entry point, not implemented in this batch).
//   3. Locate u in the lattice: (XCv, YCv) = floor(u / s).
//   4. Sample 9 cell centers around (XCv, YCv); find the closest to u.
//   5. Re-center on the closest, sample 9 again; compute voronoi
//      inside-ness `L` for the center cell.
//   6. Output: (u - P[4]) · pow(L, power) · scale / L + P[4].
//
// 18 simplex_noise_3d calls per invocation (9 cells × 2 passes × 2
// components). With 32K threads × ~256 chaos iters that's ~80M noise
// calls per dispatch — stays within the TDR budget when noise.wgsl is
// the table-free Gustavson port.
// Position dispatch: `fast` selects the hashed cell noise over the
// simplex pair. A uniform (per-transform-constant) branch, so it costs
// nothing on the GPU, and it keeps ONE body for all crackle variants —
// two bodies would drift.
fn voronoi_cell_position(x: i32, y: i32, z: f32, s: f32, d: f32, fast: bool) -> vec2<f32> {
    if (fast) {
        if (d == 0.0) {
            return vec2<f32>(f32(x) * s, f32(y) * s);
        }
        // Same 2.5 z-scale as the simplex path, so animating `z` flows
        // at a familiar speed.
        let n = crackle_fast_cell_noise(x, y, z * 2.5);
        return vec2<f32>((f32(x) + d * n.x) * s, (f32(y) + d * n.y) * s);
    }
    return voronoi_crackle_position(x, y, z, s, d);
}

fn crackle_body(
    cellsize: f32, power: f32, distort: f32, scale: f32, z: f32,
    rng: ptr<function, RngState>, fast: bool,
) -> vec3<f32> {
    if (abs(cellsize) < 1.0e-6) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let s = cellsize * 0.5;

    // Blurred unit-disc pick.
    let blurr = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 + (rng_nextf(rng) - 0.5) * 0.25;
    let theta = 6.28318530718 * rng_nextf(rng);
    let u = vec2<f32>(blurr * sin(theta), blurr * cos(theta));

    var xcv = i32(floor(u.x / s));
    var ycv = i32(floor(u.y / s));

    // First 3×3 sample. JWildfire's loop is `for (di) for (dj)` so
    // index 4 = (di=0, dj=0) = the center cell; preserve that ordering
    // so the q→offset lookup below is direct.
    var p_arr: array<vec2<f32>, 9>;
    var i: u32 = 0u;
    for (var di: i32 = -1; di <= 1; di = di + 1) {
        for (var dj: i32 = -1; dj <= 1; dj = dj + 1) {
            p_arr[i] = voronoi_cell_position(xcv + di, ycv + dj, z, s, distort, fast);
            i = i + 1u;
        }
    }

    let q = voronoi_closest(p_arr, 9u, u);

    // Re-center on the chosen cell. q ∈ [0, 9), encoded as (di, dj)
    // in row-major order with -1 origin.
    let q_di = i32(q / 3u) - 1;
    let q_dj = i32(q % 3u) - 1;
    xcv = xcv + q_di;
    ycv = ycv + q_dj;

    // Second 3×3, REUSING the first pass. The new window is the old
    // one shifted by (q_di, q_dj) ∈ [-1,1]², so at least 4 of its 9
    // cells were already computed — and in the by-far-common case
    // (u starts inside the closest cell, so q = 4 and the shift is
    // zero) all 9 were. Positions are pure functions of the lattice
    // coords, so a reused entry is bit-identical to a recomputed one;
    // this only removes redundant simplex_noise_3d calls, which are
    // the family's entire cost.
    if (q != 4u) {
        var p2: array<vec2<f32>, 9>;
        i = 0u;
        for (var di: i32 = -1; di <= 1; di = di + 1) {
            for (var dj: i32 = -1; dj <= 1; dj = dj + 1) {
                // This cell's coords relative to the OLD window center.
                let odi = di + q_di;
                let odj = dj + q_dj;
                if (odi >= -1 && odi <= 1 && odj >= -1 && odj <= 1) {
                    p2[i] = p_arr[u32((odi + 1) * 3 + (odj + 1))];
                } else {
                    p2[i] = voronoi_cell_position(xcv + di, ycv + dj, z, s, distort, fast);
                }
                i = i + 1u;
            }
        }
        p_arr = p2;
    }

    let l = voronoi_inside(p_arr, 9u, 4u, u);
    var dxy = u - p_arr[4];

    // Warp: scale ratio `R = pow(L, power) * scale / L`.
    let safe_l = l + 1.0e-32;
    let trg_l = pow(safe_l, power) * scale;
    let r = trg_l / safe_l;
    dxy = dxy * r + p_arr[4];

    return vec3<f32>(dxy.x, dxy.y, l);
}
