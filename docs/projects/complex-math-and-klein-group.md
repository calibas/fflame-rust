# Complex math runtime + klein_group port

## Goal

Add a small WGSL complex-arithmetic module (`shaders/core/complex.wgsl`)
and a `CMat2` (2×2 matrix of complex) helper, then port `klein_group`
as the validation use case. This unlocks the chaos-game rendering of
Kleinian limit sets — the "bottle"/"necklace"/"shell" fractal patterns
made famous by *Indra's Pearls: The Vision of Felix Klein* (Mumford,
Series & Wright, 2002).

It also creates the foundation for porting other complex-math-blocked
variations (`pre_recip` and parts of a handful of others).

## Background

### Kleinians, briefly

A Kleinian group is a discrete subgroup of the Möbius transformations
on the Riemann sphere. The interesting thing is the group's **limit
set**: the closure of the orbit of any starting point under the
generators. For nice Kleinians the limit set is a fractal subset of
the sphere — a Cantor dust, a circle, or in the most beautiful cases
a recursive tangle of nested loops. *Indra's Pearls* spends most of
its pages on **chaos-game iteration** as the standard rendering
algorithm: pick one of the four group generators `{a, b, A=a⁻¹,
B=b⁻¹}` at random, apply, plot, repeat. Add `avoid_reversal` to skip
immediately-cancelling pairs (`aA, bB, Aa, Bb`) so the trajectory
keeps drifting through the limit set instead of bouncing back.

This is exactly Apophysis IFS structure — one self-iterating
variation that runs its own internal chaos game, ignoring its input
position the same way `curliecue2` and `macmillan` do.

### Why we need complex math

Möbius transformations are `f(z) = (a·z + b) / (c·z + d)` where `z, a,
b, c, d` are complex numbers. Our variation framework operates on
real `vec2`/`vec3`. So we need:

- A `Complex` representation (`vec2<f32>` with `.x = real, .y = imag`)
- Complex arithmetic: `add, sub, mul, div, conj, abs, sqrt, square`
- A 2×2 complex matrix type for the generator matrices
- The recipe-specific generator construction (Grandma, Maskit,
  Jorgensen, Riley, etc.)

### Prior art

Per the 2026-05-05 audit (see this conversation's exchange):

- **No mature drop-in WGSL complex library exists.** LYGIA, wgmath,
  Use.GPU all skip complex math entirely.
- **`arthomnix/fractal_viewer`** (MIT, production WGSL Mandelbrot)
  has `cmul, cdiv, csquare, cpow, ccpow` — useful verified seed for
  those four.
- **DonKarlssonSan's GLSL gist** has the most complete API but is
  GLSL with no license — treat as textbook reference, reimplement.
- **philogb's Indra's Pearls WebGPU demo** exists but no public
  source.

We write our own (~150 lines), seeded by the verified pieces.

## Non-goals

- **Transcendental complex functions for `klein_group`.** None of the
  6 recipes need `cexp/clog/csin/ccos`. They use only `add, sub, mul,
  div, sqrt, square`. We add transcendentals later if a future
  variation needs them.
- **f64 / double-precision complex.** WGSL has no f64 support. f32 is
  what we get. (Same trade-off the rest of the codebase makes.)
- **General `Complex` type via struct.** WGSL structs are awkward to
  pass around; we use `vec2<f32>` directly with named helper
  functions. Matches arthomnix's convention.
- **Full Indra's Pearls feature set.** klein_group already exposes 7
  recipes (Grandma, Maskit, Maskit-modified, Jorgensen, Riley,
  Riley-modified, Maskit-Leys-modified). We port all 7 of those but
  don't add new recipes beyond what cpp has.
- **Accept-points-at-infinity edge handling.** Möbius transformations
  can map finite points to infinity (when `cz + d = 0`). The cpp
  variation has no special handling — we'll do the same (let near-zero
  denominators clamp to a safe value via `select`). Limit sets that
  legitimately extend to ∞ will render with infinity off-screen, same
  as cpp.

## Audit / scope

### Complex API surface needed for klein_group

Walked the cpp recipe code (`output/jwildfire-vars/output/klein_group.cpp`,
embedded Java source). Operations required:

| Op | Signature | Notes |
|---|---|---|
| `cadd` | `(a, b) -> vec2<f32>` | trivial |
| `csub` | `(a, b) -> vec2<f32>` | trivial |
| `cmul` | `(a, b) -> vec2<f32>` | `(ar*br - ai*bi, ar*bi + ai*br)` |
| `cdiv` | `(a, b) -> vec2<f32>` | `cmul(a, conj(b)) / |b|²` |
| `cconj` | `a -> vec2<f32>` | trivial |
| `csquare` | `a -> vec2<f32>` | `cmul(a, a)` (perf vs cleanness) |
| `csqrt` | `a -> vec2<f32>` | branch-cut careful — see notes |
| `cmul_real` | `(a, s: f32) -> vec2<f32>` | `a * s` (just a scalar mul) |

8 functions. ~60 lines of WGSL.

### `CMat2` — 2×2 complex matrix

```wgsl
struct CMat2 {
    a: vec2<f32>,
    b: vec2<f32>,
    c: vec2<f32>,
    d: vec2<f32>,
}
```

| Op | Notes |
|---|---|
| `cmat2_apply(m, z)` | Möbius: `cdiv(cadd(cmul(m.a, z), m.b), cadd(cmul(m.c, z), m.d))` |
| `cmat2_inverse_sl2(m)` | For SL(2,ℂ) matrices (det = 1), inverse is `[d, -b, -c, a]`. cpp uses this exclusively. |
| `cmat2_make(a, b, c, d)` | Constructor — just assembles the struct. |

3 functions. ~30 lines.

Where to put them: `shaders/core/complex.wgsl`. Injected by the
shader builder before `utilities.wgsl` (same pattern as
`get_param`/`get_state`). Inject only when at least one active
variation declares `needs_complex: bool` — zero compile cost
otherwise. Or inject unconditionally since it's small and most flames
won't notice 90 lines. Probably the latter — simpler.

### klein_group port

Per cpp:

- 6 user params: `a_re, a_im, b_re, b_im, recipe (int 0-6),
  avoid_reversal (bool 0/1)`
- 16 init slots: 4 floats (= 2 complex) per matrix × 2 matrices
  (`mat_a`, `mat_b`). Inverses computed on-the-fly via
  `cmat2_inverse_sl2`.
- 1 state slot: `prev_matrix` index (0..3). Custom `wgsl_state_init`
  picks an initial matrix index uniformly from rng.
- needs_rng (per-call random matrix selection)
- needs_transform (read own weight to apply `z = p / weight`, output
  `mobius(z) * weight` correctly through framework's outer multiply)

Body in our model (after dropping the cpp `VVAR` factor that the
framework supplies):

```wgsl
let weight = transforms[xform_id].variations[variation_id];
let z = vec2<f32>(p.x / weight, p.y / weight);
// pick matrix from {a, A=inv(a), b, B=inv(b)} based on prev_matrix +
// avoid_reversal; update prev_matrix
let result_complex = cmat2_apply(chosen_mat, z);
return vec2<f32>(result_complex.x, result_complex.y);
```

Init computes `mat_a` and `mat_b` via the recipe-specific generator
function. The recipes are pure algebra over the 6 ops above — direct
translation from the embedded Java.

## Implementation plan

### Phase 1: Complex math module

1. Create `shaders/core/complex.wgsl` with the 8 complex ops + 3 CMat2
   ops + brief comment headers.
2. Inject in shader builder's `build_from_template`,
   `build_export`, `build_trajectory_*_tiled` — same insertion point
   as `build_state_accessors` (before `utilities.wgsl`).
3. Add a unit test: render a known flame, confirm byte-identical
   output (no active variation uses complex yet, so it should be a
   no-op).

### Phase 2: klein_group with one recipe (Grandma)

4. Port Grandma generator computation in `wgsl_init` (15-20 lines of
   complex arithmetic).
5. Port body: prev_matrix selection + Möbius application.
6. Smoke config + render. Verify the canonical Grandma limit set
   renders. (See *Indra's Pearls* fig. 8.3+ for reference imagery.)

### Phase 3: Other recipes

7. Add the remaining 6 recipes via switch on the `recipe` user
   param. Most are simpler than Grandma (Riley/Maskit are 1-2 lines).
8. Smoke configs for each (Maskit μ slice is the most visually
   distinctive).

### Phase 4: Documentation

9. Update `docs/projects/variation-port-blockers.md`: blocker #11
   (Complex math runtime) → resolved. klein_group → ported.
10. Note in the blockers doc which other variations could now be
    incrementally unblocked (`pre_recip` partially, etc.).

### Phase 5: PR

11. PR.md — single PR covering the complex runtime + klein_group port
    with all 7 recipes.

## Risks & open questions

- **`csqrt` branch cut**: complex sqrt has a branch discontinuity. The
  standard formula picks the principal branch (real part ≥ 0).
  Grandma + Jorgensen recipes use sqrt to solve a quadratic for
  `traceAB`; cpp uses the `−` branch (`trABminus`) consistently. We
  match. If the principal-branch convention disagrees with cpp at
  edge cases, the limit set may be the "other half" of the group —
  visually different but mathematically valid. Worth a smoke
  comparison if anything looks off.

- **`(cz + d) ≈ 0` denominators**: Möbius singularities. cpp doesn't
  guard. We add `select(d, 1e-30, abs(d) < 1e-30)` in `cdiv`. Some
  pixels will get extreme values; histogram clamp catches them.
  Visual difference vs cpp should be invisible.

- **Recipe-specific weirdness**: the embedded cpp comments mention
  *Indra's Pearls* page references for Grandma. Want the rendered
  output to match the book's plates. May need a smoke render against
  a known-good reference image to verify.

- **f32 precision near limit-set boundary**: limit sets are infinitely
  detailed; near the boundary, repeated Möbius applications amplify
  precision loss. cpp is f64, we're f32. Visible at deep zoom; fine
  at typical viewing scales. Same trade-off as elsewhere in the
  codebase.

- **Coordinate system**: Kleinian limit sets often have features
  near ∞ on the Riemann sphere. Default Apophysis viewport is bounded
  in [-1, 1]² ish. Need example smokes that zoom appropriately or
  accept that some flames render with infinity off-screen. v1 just
  uses the standard viewport.

- **`avoid_reversal` semantics with state re-init per dispatch**:
  cpp's `prev_matrix` lives for the flame's lifetime. Ours re-inits
  each compute dispatch (each frame, effectively). Within a single
  256-iteration thread the `avoid_reversal` works correctly; across
  threads/dispatches there's no shared state. This is the same
  trade-off other state ports already accept (curliecue2, macmillan,
  hexaplay3D). Visually fine.

## File touches (estimated)

```
docs/projects/complex-math-and-klein-group.md   (new, this file)
docs/projects/variation-port-blockers.md        (mark #11 resolved, klein_group ported)
shaders/core/complex.wgsl                       (new, ~90 LoC)
src/shader_builder_v2.rs                        (~30 LoC — inject complex.wgsl in 3 build paths)
src/variations/defs/klein_group_misc.rs         (new, ~400 LoC — 7 recipes + body + 3D wrapper)
src/variations/defs/mod.rs                      (~6 LoC entries)
tests/visual/configs/variations/<7 smoke configs>  (~110 LoC each, one per recipe — could trim to 2-3 representative ones)
```

Total: ~600 LoC + ~300 LoC of test configs.

## Future unlocks

After this lands, a separate (smaller) follow-on can add
transcendental complex functions (`cexp, clog, csin/ccos/ctan`). That
opens `pre_recip` (which uses `Complex.AsinH/AcosH/AtanH/AsecH/...`)
and a handful of other complex-using variations. Most are 1-3
variations per blocker so they're not their own project — just
opportunistic ports as the transcendental functions get added.
