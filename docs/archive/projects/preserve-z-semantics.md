# Per-variation preserve_z semantics

**Status**: shipped. Replaces the old blanket per-iteration Z flatten.

## Problem

JWildfire's `preserve_z = false` (the default) does NOT zero Z between
chaos-game iterations. It works per variation:

- **Gated** (the standard 2D-variation pattern): `if
  (pContext.isPreserveZCoordinate()) pVarTP.z += pAmount·z` — the
  variation contributes z only when preserve_z is on.
- **Always**: true-3D variations (Linear3DFunc, Julia3DFunc, ZConeFunc,
  …) write `pVarTP.z` unconditionally. This is what lets Z compound
  across iterations — e.g. linear3D's `z += amount·z` carries the
  previous iteration's Z forward every pass.
- **Never**: the source never touches `pVarTP.z` at all.

Our old implementation approximated all of this with a single
`current.z = 0.0` at the end of every iteration (`FLATTEN_Z_PER_ITER`).
That matches JWF only when no always-z variation is active; otherwise it
destroys the compounding — observed as `output/JWF-rando2.flame`
rendering completely flat while JWF showed full 3D structure (julia3D
at weight 2.28 was the dominant z-writer there).

## Implementation

- `Feature::AlwaysZ` / `Feature::NeverZ` on `VariationDef` (absence =
  gated default). Features were chosen over a new struct field (zero
  churn for unaudited defs) and over `VariationCategory` (category is
  UI grouping, orthogonal to z behavior).
- `shader_builder_v2::build_apply_variations_3d` zeroes the z component
  of a contribution at the dispatch site:
  - gated + `preserve_z = false` → `result += w * vec3(f(p).xy, 0.0)`
  - `NeverZ` → zeroed under both settings
  - `AlwaysZ` → never zeroed
  Our 3D bodies return `p.z` passthrough, which equals JWF's gated add
  when kept and JWF's skip when zeroed; always-z bodies contain their
  real z math.
- `FLATTEN_Z_PER_ITER` is no longer emitted (template block kept,
  always false). When an xform has only gated variations, the summed z
  is 0 — identical to what the flatten produced, including the
  NaN-explosion protection it existed for (gated variations contribute
  no z at all, so z-scaling 2D bodies can't diverge).
- `linear3D` split into its own def (`Feature::AlwaysZ`, registered at
  the list end) — it was an import alias of `linear`, but JWF gives
  the two different z semantics, and one def can't carry both.

## Audit

`scripts/audit_z_write_semantics.py` classifies every local JWF source
(`output/variation-jwf-source/*.java`) by detecting `pVarTP.z` writes
and whether each sits inside an `isPreserveZCoordinate()` guard, then
adds `Feature::AlwaysZ` to the matching defs. Re-runnable; run it after
adding sources or porting variations.

Result at time of writing: **130 defs marked AlwaysZ**; the rest stay
gated.

### Review backlog

- **14 "mixed" files** (some z writes gated, some not — e.g.
  `attractor_flow`, `dc_carpet3D`, `maurer_lines`, `primitives_wf`):
  left at the gated default (conservative — same behavior as the old
  flatten). Each needs a manual read of its Java source to decide.
- **60 "never" candidates** (source never writes z): reported but NOT
  applied. Enforcing `NeverZ` changes `preserve_z = true` renders
  (our passthrough bodies currently add `w·z` that JWF wouldn't), and
  some of our 3D bodies extend 2D variations deliberately — needs
  per-def review before flipping.
- **Unported-source variations** (no local Java): gated by default. If
  a flame renders flat where JWF has depth, the culprit is most likely
  an unaudited always-z variation — check its source and add the flag.

## Bad-value recovery (companion mechanism)

JWF tolerates divergent points: `validateState()` re-randomizes any
Inf/NaN point via `preFuseIter()` (x, y ∈ [-1, 1], z = 0) and the plot
path skips NaN samples. We previously had NO recovery — a divergent
point NaN-poisoned itself permanently (`0·Inf = NaN` in the camera
transform), observed as missing attractor regions on
`output/JWF-rando25.flame` at preserve_z=true (edisc at weight 15
multiplies z by ×15 per pass; f32 overflows in ~40 iterations).

The iteration loop in `main_template.wgsl` now does (per iteration):

- **X/Y magnitude > 1e32** → full respawn + re-fuse (burn-in counter
  rearmed so the recovering point doesn't plot mid-flight). Magnitude
  checks rather than NaN compares — WGSL compilers may assume
  NaN-free math.
- **Z magnitude > 1e32 (3D only)** → SATURATE at ±1e32 instead of
  respawning. f32 hits the threshold ~7× sooner than JWF's f64
  reaches Inf; respawning that often visibly starves the attractor.
  A finite-huge z behaves exactly like JWF's Inf at every consumer
  (flat views: zero matrix coefficient × finite = 0, not NaN;
  pitched views: sample rejected at bounds). Non-finite z falls to 0.
- **While z sits on the rail** → 1/256 per-iteration respawn chance,
  emulating JWF's amortized respawn cycle (their f64 takes a couple
  hundred iterations of explosive growth to actually reach Inf).

Verified on JWF-rando25: preserve_z on/off now render the same (~1%
density asymmetry from differing respawn rates, same as JWF's own
on/off asymmetry in principle), where previously preserve_z=on lost
whole sections.

## Verification

- `output/JWF-rando2.flame` renders pixel-identical to JWildfire at
  both preserve_z settings (user-verified A/B).
- Unit tests cover alias canonicalization, the linear/linear3D split,
  and the shader id-map reachability that originally dropped linear3D.
