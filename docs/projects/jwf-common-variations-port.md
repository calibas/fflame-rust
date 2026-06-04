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

- **Total**: 191
- **Already implemented**: 168 (88%)
- **Missing**: 23 — see groups below.

Re-run `python scripts/diff_jwf_list.py` after every change.

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

| Variation | cpp | Notes |
|---|---|---|
| `checkerboard_wf` | yes | Checkered grid pattern; small param set. |
| `cubic3D` | yes | 3D cubic deformation. |
| `cubicLattice_3D` | yes | 3D lattice grid. |
| `mandelbrot` | yes | Escape-time Mandelbrot inside a flame variation. Modest iter loop. |
| `plane_wf` | yes | Project to plane; small. |
| `synth` | yes | Multi-param waveform synthesizer; lots of params but math is straightforward. |
| `roundspher3D` | yes | Cousin of our `roundspher`, but 3D-aware: includes `z²` in `d`, falls back to `cos(sqrt(x²+y²))` when input Z is 0, modifies output Z. Distinct variation. |
| `scry_3D` | yes | Cousin of our `scry`, but 3D-aware: VVAR-dependent `VVAR_inv`, separate `r` and `u` factors for XY and Z, sign branching, fallback `cos(sqrt(t))` for zero Z. Distinct variation. |

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

### Group F — Source missing, deferred (3)

cpp not present in `output/jwildfire-vars/output/`. Need to track down
source (JWildfire repo, flam3, or hand-translate from JWF Java) before
we can port.

| Variation | Notes |
|---|---|
| `juliascope3Db` | "Variant b" of juliascope3D. We have `juliascope`. Check JWF source for what `b` adds. |
| `metaballs3d_wf` | Metaballs primitive. May overlap with our `pointgrid` family. |
| `pdjoscilloscope` | Oscilloscope curves. Looks decorative; low priority. |

## Order of operations

Recommended pass order, lowest-risk to highest:

1. **Group A** (alias / 3D body, 3 variations). Smallest diff per
   variation; immediate user-visible win for JWF import. Estimate:
   <1 hour total if they're true aliases.
2. **Group C** (pre_subflame_wf). Likely shares body with subflame_wf;
   small focused work.
3. **Group B** (8 standalone ports). Each is independent; can be
   batched in 2–3 sittings.
4. **Group E** (6 fractals). Design the shared escape-time helper
   first, then port the family.
5. **Group D** (3 noise/Voronoi). `crackle` first (find source), then
   `dc_crackle_wf` reuses it, then `dc_perlin` separately.
6. **Group F** (3 deferred). Source-hunting; ad hoc.

## Related docs

- [variation-bulk-port.md](variation-bulk-port.md) — long-tail port
  log; this batch is a focused subset.
- [variation-port-blockers.md](variation-port-blockers.md) — the
  ones we *can't* port because of missing framework features. Cross-
  check before starting any of the above.
- [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) — running residue
  buckets for everything else variation-related.
