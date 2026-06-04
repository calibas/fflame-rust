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
- **Already implemented**: 176 (93%)
- **Missing**: 14 — see groups below.

Re-run `python scripts/diff_jwf_list.py` after every change.

Batch 1 (`jwf-variations-batch1`) landed: `checkerboard_wf`, `cubic3D`,
`cubicLattice_3D`, `plane_wf` (math modes only), `roundspher3D`,
`scry_3D`, `synth`, `juliascope3Db`; and removed `pdjoscilloscope`
from the source list (turned out to be a typo of `pdj` + `oscilloscope`
smashed together, not a real JWF variation).

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
| `mandelbrot` | yes | ⏳ pending | Escape-time Mandelbrot inside a flame variation. Modest iter loop. Closely related to Group E `fract_mandelbrot_wf` — port together. |
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

### Group C — Pre/post phase relatives of variations we have (1)

Same body as a normal-phase variation we already ship, but
registered as a different phase. Should be quick once we identify
the existing variation's body to reuse.

| Variation | Related | cpp | Notes |
|---|---|---|---|
| `pre_subflame_wf` | `subflame_wf` (we have) | yes | Pre-phase variant — applies the subflame iteration as a pre-step instead of normal. May share most of the body. |

### Group D — Voronoi / noise family (3)

Need either a cell-based Voronoi sampler or a Perlin-noise sampler.
Once one lands, the others are easier. Both involve hashing + nearest-
neighbor logic; non-trivial but well-trodden territory.

| Variation | cpp | Notes |
|---|---|---|
| `crackle` | NO | Need source from jwildfire master or hand-port. |
| `dc_crackle_wf` | yes | DC version — writes color from the cell index. Needs `crackle` first. |
| `dc_perlin` | yes | Perlin noise + DC color writing. |

### Group E — Escape-time fractal family (6)

Each embeds a small iteration of a Julia/Mandelbrot-style escape map
inside the variation. Same skeleton (loop, escape radius, smoothing),
different `z = f(z, c)` formulas. Worth designing a shared helper or
macro before porting all six.

| Variation | cpp | Notes |
|---|---|---|
| `fract_dragon_wf` | yes | Dragon curve embed. |
| `fract_julia_wf` | yes | Julia set. |
| `fract_mandelbrot_wf` | yes | Mandelbrot set. Closely related to standalone `mandelbrot` above. |
| `fract_meteors_wf` | yes | Meteors variant. |
| `fract_pearls_wf` | yes | Pearls variant. |
| `fract_salamander_wf` | yes | Salamander variant. |

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

When porting Group E (escape-time `fract_*_wf`), evaluate whether
their iteration loops or per-flame type selectors benefit from the
same approach.

## Order of operations

Recommended next pass, lowest-risk to highest. Groups A/B/F are mostly
landed (synth specialization done in batch 1 above); what's left:

1. **Group C** (`pre_subflame_wf`). Likely shares body with `subflame_wf`;
   small focused work.
2. **Group B residue** (`mandelbrot`). Port alongside Group E
   `fract_mandelbrot_wf` — same escape-time skeleton.
3. **Group E** (6 fractals). Design the shared escape-time helper
   first, then port the family.
4. **Group D** (3 noise/Voronoi). `crackle` first (find source — not
   in `output/jwildfire-vars/output/`), then `dc_crackle_wf` reuses it,
   then `dc_perlin` separately.
5. **Group F residue** (`metaballs3d_wf`). Source-hunting; ad hoc.
6. **Group B-blocked** (`post_colormap_wf`, `szubieta`). Still waiting
   on framework features — see
   [variation-port-blockers.md](variation-port-blockers.md).

## Related docs

- [variation-bulk-port.md](variation-bulk-port.md) — long-tail port
  log; this batch is a focused subset.
- [variation-port-blockers.md](variation-port-blockers.md) — the
  ones we *can't* port because of missing framework features. Cross-
  check before starting any of the above.
- [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) — running residue
  buckets for everything else variation-related.
