# su_mobius table generators

Source of truth for the shared SL(2,ℂ) Möbius-group machinery. Two
generators, both run from the repo root:

- **`gen_wgsl.py`** → `shaders/core/su_mobius.wgsl` (committed). The
  baked generator tables (`SU_MOBIUS_BASE`), the `su_group_range`
  baked-group index map for `su_mobius`, and the SuMat / conjugator /
  Poincaré-H3 helpers used by every `Feature::NeedsMobiusLib`
  variation (`su_mobius`, `su_custom`, `fuchsian_triangle`,
  `apollonian_gasket`, `hecke_group`, `lorentz_mobius`,
  `schottky_group`).
- **`gen_init.py`** → `su_custom_init.wgsl` (generated locally, not
  committed). The live reduction-tensor init pass; paste into
  `src/variations/defs/su_custom.rs`'s `WGSL_INIT` if the closed
  forms change.

Data files:

- `su4_baked.json` — the baked SU(4) reduction (2×2 complex matrices
  as `[[re,im]×4]` rows).
- `custom_forms.json` — the SU(2)/SU(3)/SU(4) reduced-matrix entries
  `w[i] = t·s[i]·tᵀ` as WGSL expressions in the Reduce sliders
  a/b/c/d, derived symbolically from the Lie-algebra generator sets.

Table discipline: **append-only**. `SU_MOBIUS_BASE` offsets are baked
into `su_group_range` and must never move for existing groups — add
new groups at the end (`allm = ... + newgroup`) and give them a new
`su_group_range` case. Both outputs are verified byte-identical to the
committed state as of the split commit; keep it that way by
re-running after any edit.

Provenance: SU(2) 6-group and SU(3) reduced are Roger Bagula's
matrices (Programing4 notebook series, July 2026); SU(5)/SO(5) are our
reductions of the generalized Gell-Mann set via
`t = [[1,1,1,−1,−1],[0,1,−1,i,−i]]` with trace-plugging (traceless
results get +2 → parabolic, the circle-packing condition); SU(4) is
our baked reduction. See the `su_mobius` / `su_custom` module docs.
