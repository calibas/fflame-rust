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

## Verification

- `output/JWF-rando2.flame` renders pixel-identical to JWildfire at
  both preserve_z settings (user-verified A/B).
- Unit tests cover alias canonicalization, the linear/linear3D split,
  and the shader id-map reachability that originally dropped linear3D.
