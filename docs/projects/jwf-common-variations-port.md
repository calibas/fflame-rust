# JWildfire common-variations port

Targeted port pass for variations that appear in JWildfire's stock
"script vars" set but aren't in our registry. Tracked separately from
the broader long-tail in
[`variation-bulk-port.md`](variation-bulk-port.md) because this batch
is bounded (~24 names) and visible to users — a JWF flame using any of
these silently drops the variation on import.

Source list: [`output/jwildfire-script-vars.txt`](../../output/jwildfire-script-vars.txt)
(191 entries). Diff against our registry: `python scripts/diff_jwf_list.py`.

## Status

- **Total**: 190
- **Already implemented**: 184 (97%)
- **Missing**: 6 — see groups below. All six are now legitimately
  blocked (framework features or missing source), not just unported.

Re-run `python scripts/diff_jwf_list.py` after every change.

Batch 1 (`jwf-variations-batch1`, merged in PR #92) landed:
`checkerboard_wf`, `cubic3D`, `cubicLattice_3D`, `plane_wf` (math
modes only), `roundspher3D`, `scry_3D`, `synth` (with the
per-flame WGSL specialization framework), `juliascope3Db`,
`pre_subflame_wf`; and removed `pdjoscilloscope` from the source
list (turned out to be a typo of `pdj` + `oscilloscope` smashed
together, not a real JWF variation).

Batch 2 (`jwf-variations-batch2`) landed the fract_*_wf family
(`fract_dragon_wf`, `fract_julia_wf`, `fract_mandelbrot_wf`,
`fract_meteors_wf`, `fract_pearls_wf`, `fract_salamander_wf`) plus
the standalone `mandelbrot`. All seven share the same escape-time
infrastructure in [`shaders/core/fractwf.wgsl`](../../shaders/core/fractwf.wgsl).

## Triage

Grouped by expected effort and reuse opportunity. Each row:
- `cpp`: whether `output/jwildfire-vars/output/<name>.cpp` exists. If
  not, we'll need to find source elsewhere (jwildfire master, flam3,
  or hand-port from the JWF Java).
- `notes`: what's blocking or what shape the port likely takes.

### Group A — True aliases (LANDED)

JWildfire-side renames of variations we already ship with identical
math. Wired up as `aliases: &[...]` on the existing `VariationDef`.

| JWF name | Mapped to | Status |
|---|---|---|
| `cylinder_apo` | `cylinder` | ✅ aliased (`cylinder` family — Scott Draves) |

After investigating `roundspher3D` and `scry_3D` cpp sources, both
turned out to be genuinely different variations (3D-aware formulas
with their own Z math), not just 3D-body additions to the 2D
versions we have. They moved to Group B.

### Group B — Standalone ports, straightforward (8)

Simple geometry / math variations with available cpp. Each is
basically a translate-cpp-to-WGSL job; no exotic features needed.

| Variation | cpp | Status | Notes |
|---|---|---|---|
| `checkerboard_wf` | yes | ✅ LANDED | Checkered grid pattern; small param set. |
| `cubic3D` | yes | ✅ LANDED | 3D cubic deformation. |
| `cubicLattice_3D` | yes | ✅ LANDED | 3D lattice grid. |
| `mandelbrot` | yes | ✅ LANDED | Random-walk Buddhabrot — distinct from `fract_mandelbrot_wf` despite the shared escape map. Persistent 3-slot state carries `(x0, y0, z0)` between chaos-game iterations; outer 10-retry loop picks fresh-or-walk seeds and filters by the JWF `invert`/`skin` acceptance band. `iter` GPU-clamped to ≤ 250. |
| `plane_wf` | yes | ✅ LANDED | Math modes only (the texture/image-sample modes still need framework work). |
| `synth` | yes | ✅ LANDED | 35 params, 26 modes. Has its own mode-specialization framework — see "Specialization framework" below. |
| `roundspher3D` | yes | ✅ LANDED | Cousin of our `roundspher`, but 3D-aware: includes `z²` in `d`, falls back to `cos(sqrt(x²+y²))` when input Z is 0, modifies output Z. Distinct variation. |
| `scry_3D` | yes | ✅ LANDED | Cousin of our `scry`, but 3D-aware: VVAR-dependent `VVAR_inv`, separate `r` and `u` factors for XY and Z, sign branching, fallback `cos(sqrt(t))` for zero Z. Distinct variation. |

### Group B-blocked — re-classified as architecturally blocked

These looked simple from line count but turned out to need
framework features we don't have. Cross-referenced into
[variation-port-blockers.md](variation-port-blockers.md).

| Variation | Blocker |
|---|---|
| `post_colormap_wf` | Texture / image sampling (extends `AbstractColorMapWFFunc`). |
| `szubieta` | Custom-primitive plotting framework (extends `DrawFunc`, builds `primitives` of `Ngon`s in init, samples via `plotPolygon`). Already listed in variation-port-blockers.md as a "Custom primitive". |

### Group C — Pre/post phase relatives of variations we have (1, LANDED)

Same body as a normal-phase variation we already ship, but
registered as a different phase.

| Variation | Related | cpp | Status | Notes |
|---|---|---|---|---|
| `pre_subflame_wf` | `subflame_wf` (we have) | yes | ✅ LANDED | Pre-phase variant — same nested chaos-game machinery as `subflame_wf`, but the output replaces the affine point (raw `q` assignment, no scale/angle/offset) instead of being summed. Shader-builder `has_subflame` check extended to fire on either name; both excluded from the parallel `apply_subflame_variations` dispatcher to preserve the v1 no-nested-subflames recursion break. |

### Group D — Voronoi / noise family (3)

Need either a cell-based Voronoi sampler or a Perlin-noise sampler.
Once one lands, the others are easier. Both involve hashing + nearest-
neighbor logic; non-trivial but well-trodden territory.

| Variation | cpp | Notes |
|---|---|---|
| `crackle` | NO | Need source from jwildfire master or hand-port. |
| `dc_crackle_wf` | yes | DC version — writes color from the cell index. Needs `crackle` first. |
| `dc_perlin` | yes | Perlin noise + DC color writing. |

### Group E — Escape-time fractal family (6, all LANDED)

Each embeds a small iteration of a Julia/Mandelbrot-style escape map
inside the variation. Same skeleton (loop, escape radius, accept/clip
band), different `z = f(z, c)` formulas. All six landed in batch 2
on top of a shared escape-time helper module
[`shaders/core/fractwf.wgsl`](../../shaders/core/fractwf.wgsl) — the
shader builder injects it whenever any `fract_*_wf` is active.

| Variation | Status | Notes |
|---|---|---|
| `fract_dragon_wf` | ✅ LANDED | Dragon-curve iterator, `(xseed, yseed)` complex multiplier. 18 common + 2 custom params. |
| `fract_julia_wf` | ✅ LANDED | `z ← z^power + c`, fixed `c = (xseed, yseed)`. Power dispatched at runtime: 2/3/4 closed-form, ≥5 loop. **GPU-clamped to power ∈ [2, 8]**. |
| `fract_mandelbrot_wf` | ✅ LANDED | Same dispatch, but `c` is the random seed point. **GPU-clamped to power ∈ [2, 8]**. |
| `fract_meteors_wf` | ✅ LANDED | No custom params — iterator uses the random seed as the complex constant. |
| `fract_pearls_wf` | ✅ LANDED | Inverse-radial iterator with `(xseed, yseed)`. |
| `fract_salamander_wf` | ✅ LANDED | Quadratic with `-1` shift, `(xseed, yseed)` multiplier. |

All six share these GPU TDR clamps (visible in the param tooltips):
`max_iter ≤ 500`, `max_clip_iter ≤ 4` (forced to 1 when `color_only`
is on), plus the Julia/Mandelbrot power clamp above. The buffer
carries the user's actual values so `.flame` XML round-trips
losslessly; the inner loop just doesn't honor anything past the cap.

Buddhabrot mode (`buddhabrot_mode > 0`) is NOT implemented in v1 —
the param is accepted for round-trip but always falls through to the
iterate path. Adding it requires extending each variation with a
6-slot state machine (chooseNewPoint + trajectory carry). Deferred.

### Group F — Source missing, deferred (was 3, now 1)

cpp not present in `output/jwildfire-vars/output/`. Need to track down
source (JWildfire repo, flam3, or hand-translate from JWF Java) before
we can port.

| Variation | Status | Notes |
|---|---|---|
| `juliascope3Db` | ✅ LANDED | Ported from `output/variation-jwf-source/JuliaScope3DBFunc.java` (Java source appeared upstream). 11 params, 0 init slots; kaleidoscope dispatch with even/odd flip, type switch for full-3D vs 2D length, mode switch on the Z multiplier. |
| `metaballs3d_wf` | ⏳ pending | Metaballs primitive. May overlap with our `pointgrid` family. |
| `pdjoscilloscope` | ❌ removed | Not a real variation — typo for `pdj` + `oscilloscope` smashed together. Both already implemented. Removed from `output/jwildfire-script-vars.txt`. |

## Specialization framework

Added while porting `synth` (commits `5806d9a`, `6aabaa5`). Variations
with a runtime dispatch (synth's 26-mode switch) used to pay the cost
of compiling every case body every time the shader rebuilt — ~5s on
Vulkan for synth, since each case calls helpers that themselves inline
nested switches. The driver's SPIR-V → native pass grinds through all
branches even when only one is reachable at runtime for a given flame.

The framework lets a variation re-emit its WGSL per flame based on
runtime values that are constant for that flame:

1. Variation exports `specialize_wgsl_2d(flame: &Flame) -> String` and
   `specialize_wgsl_3d(flame: &Flame) -> String` (see
   `src/variations/defs/synth.rs`).
2. `shader_builder_v2::generate_variation_code` dispatches by name in
   its `variation_specialized_source` helper. Returning `None` ⇒ use
   the static `wgsl_2d` / `wgsl_3d` as before.
3. `ShaderCache::specialization_key: Vec<(String, String)>` captures
   what each specializer reads. The cache's `ensure_current_full`
   early-exit checks this alongside `variations_changed` etc., so a
   mid-flame param change that affects the WGSL forces a rebuild.

Measured wins on synth (Vulkan, interactive):

| scenario | before | after |
|---|---|---|
| Add synth (cold) | ~5.2s | 113ms |
| Change `synth.mode` (cold) | ~5.2s | 53ms |
| Change `synth.mode` (driver-cached) | ~5.2s | 3ms |

Group E ended up not needing the framework: the escape-time iteration
is `max_iter`-bounded with a small switch on iterator kind, and naga
compiles the whole shared helper module fine even with all 12
sub-kinds emitted. The performance lever there was different —
runtime GPU clamps on `max_iter` / `max_clip_iter` / `power` to keep
the worst-case dispatch under TDR. A future Julia/Mandelbrot
power-specialization (emit only the active power's branch) could
build on top of the existing framework, but no flame we've measured
needs it.

## Order of operations

This doc started life triaging 23 variations into 6 groups. As of
batches 1 + 2 (17 ports landed) the work-list is much shorter:

- **Group D** (3 noise/Voronoi). `crackle` first (still no source
  — not in `output/jwildfire-vars/output/`), then `dc_crackle_wf`
  reuses it, then `dc_perlin` separately. Both crackle variants
  share the cell-noise infra; perlin is a separate noise function.
- **Group F residue** (`metaballs3d_wf`). Source-hunting; ad hoc.
- **Group B-blocked** (`post_colormap_wf`, `szubieta`,
  `fract_formula_julia_wf`, `fract_formula_mand_wf`). All still
  waiting on framework features (texture sampling, custom-primitive
  plotting, runtime expression interpreter) — see
  [variation-port-blockers.md](variation-port-blockers.md).

## Related docs

- [variation-bulk-port.md](variation-bulk-port.md) — long-tail port
  log; this batch is a focused subset.
- [variation-port-blockers.md](variation-port-blockers.md) — the
  ones we *can't* port because of missing framework features. Cross-
  check before starting any of the above.
- [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) — running residue
  buckets for everything else variation-related.
