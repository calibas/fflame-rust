# Variation Bulk Port from JWildfire / Chaotica

## Goal

Port the bulk of standard fractal-flame variations from upstream JWildfire/Chaotica into our `src/variations/defs/` registry, then upload them to the API as the variation cache's primary content. Target: from ~80 today to ~500 commonly-used variations.

Source repo: https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/tree/master/output (C++ ports of JWildfire variations).

## Inventory (2026-04-26)

| | Count |
|---|---|
| Our current variations (`ALL_VARIATIONS` in `src/variations/defs/mod.rs`) | **84** |
| Upstream `.cpp` files | **636** |
| Already implemented (exact name match) | 78 |
| Ours-only (name doesn't appear upstream) | 6 |
| Upstream-only (potentially to port) | **558** |

Lists are checked into this directory:
- [variation-upstream-only.txt](variation-upstream-only.txt) — all 558 upstream-only names
- [variation-port-candidates.txt](variation-port-candidates.txt) — 415 names after dropping skip categories

## Ours-only — minor name divergences

Six names in our registry don't appear verbatim upstream. All look like version drift:

| Ours | Upstream equivalent |
|---|---|
| `bwraps`, `pre_bwraps`, `post_bwraps` | `bwraps7` / `pre_bwraps2` / `post_bwraps2` |
| `pre_disc` | `pre_disc3d` |
| `pre_falloff2` | `pre_falloff3` |
| `pre_sinusoidal` | `pre_sinusoidal3d` |

Likely functionally equivalent. Worth confirming when we port the upstream "newer" versions, but no rename action needed now.

## Upstream-only categorization

### Skip (won't fit current shader/system) — 143 entries

| Pattern | Count | Reason |
|---|---|---|
| `dc_*` direct color | 48 | Need direct-color (DC) support; our pipeline has no per-iteration color override slot |
| `glsl_*` embedded fragment shader | 19 | Each ships a unique GLSL shader; not standard variation functions |
| `*_js` JavaScript-defined | 25 | L-systems / strange attractors driven by scripted iterations, not pure functions |
| `*_wf` JWildfire-specific | 54 | Many need image input (`colormap`, `displacemap`, `bumpmap`, `text`, `svg`), subflames, or external state. Subset may be salvageable on inspection. |
| `fract_*` escape-time fractals | 8 | Mandelbrot/Julia escape iterations, need a bounded inner loop |

### Port candidates — 415 entries

Rough sub-buckets, just by name pattern (real classification comes after `.cpp` analysis):

- **14 `_bs` (Bessel-style trig variants)** — `sin2_bs`, `cos2_bs`, `sinh2_bs`, etc. Small and uniform, good first batch.
- **Trig / hyperbolic primitives** — `sin`, `cos`, `tan`, `cot`, `sec`, `csc`, `sinh`, `cosh`, `tanh`, `coth`, `sech`, `csch`, plus `*q` quaternion variants, `arc*` inverse variants, `sqrt_*` family. Another large coherent batch.
- **Numbered variants of things we have** — `bipolar2`, `blob3d`, `bubble2`, `cpow2`, `cpow3`, `disc2/3/3d`, `elliptic2`, `julia3dq`, `juliaq`, `julian2`, `loonie2/3`, `popcorn/2/2_3d`, `rays1/2/3`, `splits3d`, `square/3d`, `truchet/2/_ae/_fill`, etc.
- **Big standalone shapes** — `apollony`, `cell`, `flower`, `henon`, `kaleidoscope`, `mandelbrot`, `mobius_strip`, `pyramid`, `taprats`, `super_shape`, `whitney_umbrella`, etc.
- **Pre/post variants of base ones** — many `pre_*` and `post_*` of variations we already have or are about to port.

## System-fit notes

What our `VariationDef` system supports today, from `src/variations/defs/advanced.rs` precedent:

- **Pure functions of `p` returning a vector** — straightforward port
- **RNG access** via `needs_rng: true` — the function gets a `rng: ptr<function, RngState>` parameter
- **Parameters** via `parameters: &[VariationParamDef]` — Float, Integer, Angle types with min/max
- **Per-variation WGSL** for both `wgsl_2d` and `wgsl_3d`
- **Phase**: `Pre`, `Normal`, `Post` (and the shader builder routes them correctly)
- **Weight is applied outside the function** — `result += weight * fn(p)`. Ports that scale internally need either a weight parameter or a rewrite. (See conversation 2026-04-26 for the elliptic-style discussion.)

What we **don't** support:

- Direct color manipulation (`dc_*`)
- Image inputs (colormap / displacemap / bumpmap / SVG / text)
- Subflames / nested IFS calls (`subflame_wf`)
- Embedded GLSL fragment shaders (`glsl_*`)
- Scripted iteration with arbitrary state (`*_js` L-systems)

## Plan

1. **This doc + lists** — done
2. **Bulk-fetch the 415 .cpp files** for offline analysis
3. **Per-file classification** — pure / parameterized / RNG / uses-internal-weight / broken
4. **Port in batches** — start with the trig/hyperbolic family (uniform, low risk), move to numbered variants, then bigger shapes
5. **Database import scripts** — generate from the ported `VariationDef`s for upload to the variations API
6. **Test pass** — flame referencing each new variation renders correctly via local registry, then via API fetch with cache cleared

## Open questions

- **Internal weight convention** — for variations that use `weight` inside their formula (not just as outer multiplier), do we want to extend `VariationDef` to optionally pass weight into the function, or accept the per-variation rewrite cost? Decision deferred to first-encounter during porting.
- **`_wf` salvage pass** — some `_wf` variations are pure functions despite the suffix; worth a quick scan before discarding all 54.
- **Renames for our 6 ours-only entries** — keep current names (preset compatibility) and port upstream's `_3d` / `2` / `7` variants under their upstream names (so a flame from JWildfire loads correctly).

## Classification (auto-generated 2026-04-26)

Per-file analysis of all 415 candidate `.cpp` files at https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/tree/master/output. Heuristics extracted from each file's `PluginVarCalc` body (excluding the embedded Java comment block).

### Heuristics used

- **params**: count of `VAR_REAL` / `VAR_INTEGER` / `VAR_BOOLEAN` declarations in the `APO_VARIABLES(...)` block.
- **RNG**: matches `GOODRAND_0[1X]?(...)`, `randNum`, `pContext->random`, or `rand|frand|sfrand(...)` in `PluginVarCalc` or `PluginVarPrepare`.
- **3D-aware**: uses `FTz` more than once, OR computes `FPz` with anything other than the standard `FPz += VVAR * FTz` Z-preserve line.
- **internal-weight (`VVAR used internally`)**: `VVAR` appears inside a math fn arg (`sqrt`, `sin`, `pow`, etc.), in a comparison/ternary, or as a divisor — not just as the outer `result += VVAR * (...)` multiplier. Flagged as a separate watchlist below.
- **prepare-time precompute**: `PluginVarPrepare` writes private fields like `_sinX`, `_cosY`, etc. We don't have a prepare hook; either (a) recompute per-iteration (perf cost) or (b) extend `VariationDef` with an init step.
- **direct color (DC)**: `TC = ...` write in body. Our pipeline has no per-iteration color slot.
- **subflame**: literal `subflame` token in C++ body.
- **image input**: `transformImage` / `imageGet` / `colormap_pixel` / `displacemap` / `bumpmap` / `getRGBColor` etc. in body.
- **Java residue (`unportable_other`)**: body references `new Complex(...)` / `new Vec(...)` / compares to `null`. The C++ porter left Java syntax in place; the original uses Janino runtime compilation or Java wrapper classes that don't have C++ counterparts in the port.
- **unported_stub**: `PluginVarCalc` body is just `return TRUE;`. The C++ porter left it blank; the formula is in the Java comment block.
- **porter-omitted params**: zero `VAR_REAL` declarations but `PluginVarPrepare` initializes 2+ private `_xxx` fields to literal constants and `PluginVarCalc` reads them. The C++ porter forgot to expose them — recover from Java source. Flagged in the watchlist below.

### Summary

| Bucket | Count | Description |
|---|---:|---|
| `pure` | 104 | Pure function of `p`, no params, no RNG. Drop-in port. |
| `param` | 152 | Parameterized but otherwise pure. Declare `parameters: &[...]`, port body verbatim. |
| `rng` | 30 | No params, but uses RNG. Set `needs_rng: true`. |
| `param_rng` | 70 | Has both parameters and RNG. |
| `unportable_dc` | 16 | Writes transform color (`TC = ...`). Geometry may still be portable if we drop the color line. |
| `unportable_image` | 1 | Reads image / colormap / texture data we don't have a binding for. |
| `unportable_subflame` | 2 | Uses subflame state (nested IFS). |
| `unportable_other` | 5 | C++ body still references Java types like `new Complex(...)` or compares to `null`. Formula uses opaque Java wrapper classes (often Janino-compiled at runtime in JWildfire); not a clean port without manually expanding the underlying math. |
| `unported_stub` | 35 | C++ port has empty `PluginVarCalc` body. Formula lives in the embedded Java comment block — manual translation required. |
| **Total** | **415** | |

Cross-cutting flag: **23** entries use `VVAR` (weight) inside the formula and need design discussion before porting. See [Internal-weight watchlist](#internal-weight-watchlist) below.

Cross-cutting flag: **104** portable entries are meaningfully 3D (write `FPz` non-trivially or read `FTz` more than once).

### Per-bucket breakdown

#### `pure` (104)

Pure function of `p`, no params, no RNG. Drop-in port.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `acoth` | 126 | 0 | no |  |
| `arcsech` | 65 | 0 | no |  |
| `arcsech2` | 70 | 0 | no |  |
| `arcsinh` | 63 | 0 | no |  |
| `arctanh` | 63 | 0 | no |  |
| `atan2_spirals` | 254 | 0 | no | 11 porter-omitted params (recover from Java) |
| `bi_linear` | 107 | 0 | no |  |
| `bsplit` | 149 | 0 | no | 2 porter-omitted params (recover from Java) |
| `butterfly` | 121 | 0 | no |  |
| `butterfly3d` | 117 | 0 | yes |  |
| `cos` | 125 | 0 | no |  |
| `cosh` | 125 | 0 | no |  |
| `coshq` | 126 | 0 | yes |  |
| `cosine` | 117 | 0 | no |  |
| `cosq` | 126 | 0 | yes |  |
| `cot` | 129 | 0 | no |  |
| `coth` | 57 | 0 | no |  |
| `cothq` | 142 | 0 | yes |  |
| `cotq` | 142 | 0 | yes |  |
| `csc` | 133 | 0 | no |  |
| `csc_squared` | 195 | 0 | no | 7 porter-omitted params (recover from Java) |
| `csch` | 133 | 0 | no |  |
| `cschq` | 128 | 0 | yes |  |
| `cscq` | 128 | 0 | yes |  |
| `cylinder_apo` | 106 | 0 | yes |  |
| `deltaa` | 127 | 0 | no | 2 porter-omitted params (recover from Java) |
| `edisc` | 149 | 0 | no |  |
| `ennepers` | 111 | 0 | no |  |
| `erf` | 116 | 0 | no |  |
| `erf3d` | 111 | 0 | yes |  |
| `estiq` | 122 | 0 | yes |  |
| `exp` | 121 | 0 | no |  |
| `exp2` | 59 | 0 | no |  |
| `exponential` | 117 | 0 | no |  |
| `fan` | 129 | 0 | no |  |
| `fdisc` | 217 | 0 | no | 3 porter-omitted params (recover from Java) |
| `fisheye` | 113 | 0 | no |  |
| `flipcircle` | 117 | 0 | no |  |
| `flipy` | 117 | 0 | no |  |
| `foci_3d` | 135 | 0 | yes |  |
| `gamma` | 117 | 0 | no |  |
| `gridout` | 185 | 0 | no |  |
| `holesq` | 149 | 0 | no |  |
| `idisc` | 141 | 0 | no | VVAR used internally; 2 porter-omitted params (recover from Java) |
| `inflatez_1` | 63 | 0 | yes |  |
| `inflatez_2` | 65 | 0 | yes |  |
| `inflatez_3` | 63 | 0 | yes |  |
| `inflatez_4` | 75 | 0 | yes |  |
| `inflatez_5` | 71 | 0 | yes |  |
| `inflatez_6` | 64 | 0 | yes |  |
| `invpolar` | 109 | 0 | no |  |
| `jac_cn` | 141 | 0 | no |  |
| `jac_dn` | 141 | 0 | no |  |
| `jac_sn` | 141 | 0 | no |  |
| `linear3d` | 103 | 0 | no |  |
| `loonie3` | 145 | 0 | no | VVAR used internally |
| `loonie_3d` | 80 | 0 | yes |  |
| `mask` | 179 | 0 | no | 5 porter-omitted params (recover from Java) |
| `onion2` | 391 | 0 | yes | 8 porter-omitted params (recover from Java) |
| `panorama1` | 119 | 0 | no |  |
| `panorama2` | 119 | 0 | no |  |
| `petal` | 120 | 0 | no |  |
| `popcorn` | 126 | 0 | no |  |
| `post_heat` | 278 | 0 | yes | 9 porter-omitted params (recover from Java) |
| `post_spherical` | 110 | 0 | no |  |
| `post_spin_z` | 64 | 0 | no |  |
| `power` | 113 | 0 | no |  |
| `pre_sinusoidal3d` | 116 | 0 | yes |  |
| `pre_spin_z` | 65 | 0 | no |  |
| `pyramid` | 57 | 0 | yes |  |
| `rays1` | 119 | 0 | no |  |
| `rays2` | 117 | 0 | no |  |
| `rays3` | 117 | 0 | no |  |
| `rings` | 115 | 0 | no |  |
| `rippled` | 113 | 0 | no |  |
| `roundspher` | 118 | 0 | no |  |
| `roundspher3d` | 85 | 0 | yes |  |
| `scry_3d` | 92 | 0 | yes | VVAR used internally |
| `sec` | 133 | 0 | no |  |
| `secant2` | 131 | 0 | no |  |
| `sech` | 62 | 0 | no |  |
| `sechq` | 128 | 0 | yes |  |
| `secq` | 128 | 0 | yes |  |
| `sin` | 123 | 0 | no |  |
| `sinh` | 62 | 0 | no |  |
| `sinhq` | 126 | 0 | yes |  |
| `sinq` | 126 | 0 | yes |  |
| `sinusoidal3d` | 107 | 0 | yes |  |
| `spherical3d` | 56 | 0 | yes |  |
| `spiralwing` | 123 | 0 | no |  |
| `squarize` | 155 | 0 | no |  |
| `squircular` | 118 | 0 | no | VVAR used internally |
| `tan` | 133 | 0 | no |  |
| `tancos` | 117 | 0 | no |  |
| `tangent` | 131 | 0 | no |  |
| `tangent3d` | 119 | 0 | yes |  |
| `tanh` | 62 | 0 | no |  |
| `tanhq` | 140 | 0 | yes |  |
| `tanq` | 138 | 0 | yes |  |
| `twoface` | 117 | 0 | no |  |
| `unpolar` | 128 | 0 | no |  |
| `wdisc` | 137 | 0 | no | 2 porter-omitted params (recover from Java) |
| `whitney_umbrella` | 109 | 0 | yes |  |
| `xerf` | 117 | 0 | yes |  |

#### `param` (152)

Parameterized but otherwise pure. Declare `parameters: &[...]`, port body verbatim.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `affine3d` | 110 | 15 | yes |  |
| `anamorphcyl` | 173 | 3 | no |  |
| `atan` | 194 | 2 | yes |  |
| `barycentroid` | 229 | 4 | no |  |
| `bcollide` | 200 | 2 | no |  |
| `bent2` | 152 | 2 | no |  |
| `bipolar2` | 218 | 9 | no |  |
| `blob3d` | 155 | 3 | yes |  |
| `blocky` | 196 | 3 | no | VVAR used internally |
| `bmod` | 182 | 2 | no |  |
| `bswirl` | 180 | 2 | no |  |
| `bubble2` | 157 | 3 | yes |  |
| `bubblet3d` | 532 | 6 | yes |  |
| `butterfly_fay` | 493 | 11 | no |  |
| `bwraps7` | 330 | 5 | yes |  |
| `cardioid` | 147 | 1 | no |  |
| `cell` | 200 | 1 | no |  |
| `checks` | 96 | 4 | yes |  |
| `chunk` | 239 | 7 | yes |  |
| `circlelinear` | 124 | 8 | no |  |
| `circlerand` | 95 | 5 | no |  |
| `circlesplit` | 175 | 2 | no |  |
| `circletrans1` | 128 | 5 | no |  |
| `circlize` | 190 | 1 | no |  |
| `circlize2` | 196 | 1 | no |  |
| `circus` | 165 | 1 | no |  |
| `collideoscope` | 210 | 2 | no |  |
| `corners` | 234 | 2 | no |  |
| `cos2_bs` | 169 | 4 | no |  |
| `cosh2_bs` | 168 | 4 | no |  |
| `cot2_bs` | 172 | 4 | no |  |
| `coth2_bs` | 178 | 4 | no |  |
| `crob` | 421 | 7 | no |  |
| `csc2_bs` | 176 | 4 | no |  |
| `csch2_bs` | 176 | 4 | no |  |
| `cubic3d` | 180 | 2 | yes | VVAR used internally |
| `cubiclattice_3d` | 157 | 2 | yes |  |
| `curve` | 175 | 4 | no |  |
| `devil_warp` | 182 | 6 | no |  |
| `disc2` | 203 | 2 | no |  |
| `disc3d` | 60 | 1 | yes |  |
| `ecollide` | 225 | 2 | no |  |
| `emod` | 213 | 2 | no |  |
| `emotion` | 223 | 2 | no |  |
| `ennepers2` | 153 | 3 | yes |  |
| `epush` | 213 | 3 | no |  |
| `erotate` | 191 | 1 | no |  |
| `escale` | 217 | 2 | no |  |
| `eswirl` | 205 | 2 | no |  |
| `exp2_bs` | 159 | 3 | no |  |
| `fibonacci2` | 212 | 2 | no |  |
| `flower_db` | 194 | 7 | yes |  |
| `flux` | 150 | 1 | no |  |
| `fourth` | 262 | 5 | no | VVAR used internally |
| `funnel` | 136 | 1 | no |  |
| `gdoffs` | 289 | 8 | no |  |
| `gridout2` | 228 | 4 | no |  |
| `helicoid` | 143 | 1 | yes |  |
| `helix` | 142 | 2 | yes |  |
| `hexaplay3d` | 177 | 3 | yes |  |
| `hexes` | 286 | 4 | no |  |
| `hexnix3d` | 247 | 4 | yes | VVAR used internally |
| `ho` | 191 | 3 | yes |  |
| `hyperbolicellipse` | 154 | 1 | no |  |
| `hypertile` | 202 | 3 | no |  |
| `hypertile3d` | 99 | 1 | yes |  |
| `jac_asn` | 120 | 2 | yes |  |
| `kaleidoscope` | 194 | 5 | no |  |
| `layered_spiral` | 144 | 1 | no |  |
| `lazyjess` | 318 | 4 | no |  |
| `lazytravis` | 314 | 3 | no |  |
| `lineart` | 163 | 2 | no |  |
| `lineart3d` | 165 | 3 | yes |  |
| `log_apo` | 144 | 1 | no |  |
| `loonie2` | 253 | 3 | no | VVAR used internally |
| `loq` | 148 | 1 | yes | VVAR used internally |
| `mcarpet` | 161 | 4 | no |  |
| `minkowskope` | 307 | 6 | no |  |
| `minkqm` | 252 | 6 | no |  |
| `mobiq` | 314 | 16 | yes |  |
| `modulus` | 181 | 2 | no |  |
| `murl` | 193 | 2 | no |  |
| `murl2` | 222 | 2 | no |  |
| `nblur` | 430 | 3 | no |  |
| `octagon` | 215 | 3 | yes |  |
| `onion` | 254 | 2 | yes |  |
| `ortho` | 299 | 2 | no |  |
| `oscilloscope` | 199 | 4 | no |  |
| `oscilloscope2` | 215 | 6 | no |  |
| `ovoid3d` | 147 | 3 | yes |  |
| `perspective` | 165 | 2 | no |  |
| `poincare3d` | 199 | 3 | yes |  |
| `popcorn2` | 148 | 3 | no |  |
| `popcorn2_3d` | 115 | 4 | yes | VVAR used internally |
| `post_bwraps2` | 308 | 5 | no |  |
| `post_depth` | 144 | 1 | yes |  |
| `pre_bwraps2` | 308 | 5 | no |  |
| `pre_curl` | 164 | 2 | no |  |
| `pre_dcztransl` | 194 | 5 | yes |  |
| `pre_disc3d` | 146 | 1 | yes |  |
| `prepost_affine` | 314 | 9 | yes | VVAR used internally |
| `prepost_circlize` | 234 | 3 | no |  |
| `prepost_mobius` | 250 | 8 | no | VVAR used internally |
| `pressure_wave` | 172 | 2 | no |  |
| `ptransform` | 185 | 5 | no |  |
| `rational3` | 234 | 8 | no |  |
| `ripple` | 288 | 8 | no |  |
| `rosoni` | 309 | 7 | yes |  |
| `scrambly` | 163 | 2 | no |  |
| `scry2` | 242 | 3 | no | VVAR used internally |
| `sec2_bs` | 177 | 4 | no |  |
| `sech2_bs` | 176 | 4 | no |  |
| `shift` | 164 | 3 | no |  |
| `shredlin` | 176 | 4 | no |  |
| `shredrad` | 168 | 2 | no |  |
| `sigmoid` | 200 | 2 | no | VVAR used internally |
| `sin2_bs` | 166 | 4 | no |  |
| `sinh2_bs` | 166 | 4 | no |  |
| `sintrange` | 138 | 1 | no |  |
| `sph3d` | 162 | 3 | yes |  |
| `sphere_nja` | 252 | 6 | yes |  |
| `sphericaln` | 65 | 2 | no |  |
| `spligon` | 170 | 2 | no |  |
| `split` | 161 | 2 | no |  |
| `splits3d` | 170 | 3 | yes |  |
| `squirrel` | 148 | 2 | no |  |
| `stripes` | 148 | 2 | no |  |
| `stwin` | 195 | 4 | no |  |
| `svf` | 142 | 1 | yes |  |
| `swirl3` | 146 | 1 | no |  |
| `synth` | 1149 | 35 | no |  |
| `tan2_bs` | 176 | 4 | no |  |
| `tanh2_bs` | 176 | 4 | no |  |
| `target` | 193 | 3 | no |  |
| `target_sp` | 204 | 4 | no |  |
| `taurus` | 160 | 4 | yes |  |
| `trade` | 218 | 4 | no |  |
| `truchet_fill` | 282 | 3 | no | VVAR used internally |
| `voron` | 243 | 5 | no |  |
| `w` | 386 | 14 | no |  |
| `waves2_3d` | 62 | 2 | yes |  |
| `waves2_radial` | 179 | 6 | no |  |
| `waves2b` | 447 | 10 | no |  |
| `wedge_sph` | 180 | 4 | no |  |
| `whorl` | 163 | 2 | no |  |
| `x` | 312 | 13 | no |  |
| `xheart` | 185 | 2 | no |  |
| `xtrb` | 298 | 5 | no |  |
| `y` | 312 | 13 | no |  |
| `yin_yang` | 102 | 3 | no |  |
| `z` | 312 | 13 | no |  |
| `ztwister` | 159 | 2 | no |  |

#### `rng` (30)

No params, but uses RNG. Set `needs_rng: true`.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `acosech` | 69 | 0 | no |  |
| `acosh` | 132 | 0 | no |  |
| `apollony` | 151 | 0 | no |  |
| `arch` | 128 | 0 | no | 2 porter-omitted params (recover from Java) |
| `blade` | 139 | 0 | no |  |
| `blade3d` | 135 | 0 | yes |  |
| `boarders` | 172 | 0 | no |  |
| `chrysanthemum` | 132 | 0 | no |  |
| `circleblur` | 127 | 0 | no |  |
| `curliecue2` | 166 | 0 | no | 8 porter-omitted params (recover from Java) |
| `dustpoint` | 145 | 0 | no |  |
| `glynnia` | 198 | 0 | no | VVAR used internally |
| `hypertile1` | 82 | 0 | no |  |
| `hypertile2` | 81 | 0 | no |  |
| `hypertile3d1` | 97 | 0 | yes |  |
| `hypertile3d2` | 92 | 0 | yes |  |
| `line` | 170 | 0 | yes | 2 porter-omitted params (recover from Java) |
| `pre_blur3d` | 162 | 0 | no | 5 porter-omitted params (recover from Java) |
| `rays` | 119 | 0 | no |  |
| `seashell3d` | 186 | 0 | yes | 4 porter-omitted params (recover from Java) |
| `sqrt_acosech` | 67 | 0 | no |  |
| `sqrt_acosh` | 133 | 0 | no |  |
| `sqrt_acoth` | 137 | 0 | no |  |
| `sqrt_asech` | 134 | 0 | no |  |
| `sqrt_asinh` | 135 | 0 | no |  |
| `sqrt_atanh` | 135 | 0 | no |  |
| `square` | 107 | 0 | no |  |
| `square3d` | 103 | 0 | yes |  |
| `starfractal` | 76 | 0 | no |  |
| `twintrian` | 131 | 0 | no |  |

#### `param_rng` (70)

Has both parameters and RNG.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `arctruchet` | 367 | 4 | no |  |
| `asteria` | 202 | 1 | yes |  |
| `blur_linear` | 162 | 2 | no |  |
| `boarders2` | 221 | 3 | no |  |
| `btransform` | 191 | 4 | no |  |
| `bwrands` | 560 | 12 | yes |  |
| `circlecrop` | 237 | 5 | no |  |
| `circular` | 165 | 2 | no |  |
| `circular2` | 179 | 4 | no |  |
| `conic` | 146 | 2 | no |  |
| `cpow2` | 216 | 4 | no |  |
| `cpow3` | 222 | 4 | no |  |
| `crop3d` | 254 | 8 | yes |  |
| `d_spherical` | 165 | 1 | yes |  |
| `ejulia` | 240 | 1 | no |  |
| `elliptic2` | 241 | 11 | no | VVAR used internally |
| `exblur` | 228 | 5 | yes |  |
| `extrude` | 142 | 1 | yes | VVAR used internally |
| `farblur` | 213 | 6 | yes |  |
| `flower` | 154 | 2 | no |  |
| `glynnia3` | 246 | 4 | no | VVAR used internally |
| `glynnsim1` | 265 | 6 | no |  |
| `glynnsim2` | 248 | 6 | no |  |
| `glynnsim3` | 243 | 4 | no |  |
| `hypershift2` | 187 | 2 | yes |  |
| `inversion` | 1110 | 25 | yes |  |
| `inverted_julia` | 203 | 2 | no |  |
| `jubiq` | 401 | 24 | yes |  |
| `julia3dq` | 182 | 2 | yes |  |
| `juliac` | 185 | 3 | no |  |
| `julian2` | 223 | 8 | no |  |
| `julian3dx` | 246 | 8 | yes |  |
| `juliaq` | 177 | 2 | no |  |
| `lissajous` | 187 | 7 | no |  |
| `log_db` | 201 | 2 | no |  |
| `log_tile2` | 162 | 3 | yes |  |
| `mandelbrot` | 365 | 12 | yes |  |
| `maurer_lines` | 4677 | 36 | no |  |
| `maurer_rose` | 460 | 11 | no |  |
| `mobiusn` | 265 | 10 | no |  |
| `npolar` | 193 | 2 | no | VVAR used internally |
| `parabola` | 151 | 2 | no |  |
| `phoenix_julia` | 205 | 4 | no |  |
| `pie` | 155 | 3 | no |  |
| `pie3d` | 151 | 3 | yes |  |
| `post_circlecrop` | 232 | 5 | no |  |
| `post_julia3dq` | 187 | 2 | yes |  |
| `post_juliaq` | 176 | 2 | no |  |
| `post_rblur` | 168 | 4 | no |  |
| `pow_block` | 199 | 5 | no |  |
| `pre_boarders2` | 220 | 3 | no |  |
| `pre_circlecrop` | 232 | 5 | no |  |
| `prose3d` | 476 | 16 | yes |  |
| `r_circleblur` | 213 | 5 | no |  |
| `rhodonea` | 846 | 15 | no |  |
| `sineblur` | 149 | 1 | no |  |
| `spherecrop` | 251 | 6 | yes |  |
| `spirograph` | 192 | 9 | no |  |
| `spliptic_bs` | 188 | 2 | no | VVAR used internally |
| `splitbrdr` | 225 | 4 | no |  |
| `squish` | 224 | 1 | no |  |
| `starblur` | 188 | 2 | no |  |
| `super_shape` | 202 | 6 | no |  |
| `supershape3d` | 334 | 16 | yes |  |
| `tile_hlp` | 175 | 1 | no |  |
| `tile_log` | 143 | 1 | no |  |
| `tile_reverse` | 202 | 3 | no |  |
| `vogel` | 147 | 2 | no |  |
| `waffle` | 219 | 4 | no | VVAR used internally |
| `wedge_julia` | 180 | 4 | no |  |

#### `unportable_dc` (16)

Writes transform color (`TC = ...`). Geometry may still be portable if we drop the color line.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `curl_sp` | 261 | 6 | yes | writes TC/color |
| `curliecue` | 894 | 0 | yes | writes TC/color |
| `gpattern` | 434 | 9 | no | writes TC/color |
| `macmillan` | 71 | 5 | no | writes TC/color |
| `mandala` | 367 | 0 | no | writes TC/color |
| `mandala2` | 915 | 0 | no | writes TC/color |
| `nsudoku` | 598 | 5 | no | writes TC/color |
| `post_smartcrop` | 220 | 9 | yes | writes TC/color |
| `pre_stabilize` | 102 | 4 | no | writes TC/color |
| `recurrenceplot` | 879 | 0 | no | writes TC/color |
| `sphtiling3v2` | 255 | 9 | no | writes TC/color |
| `sunflower` | 267 | 6 | no | writes TC/color |
| `szubieta` | 310 | 4 | yes | writes TC/color |
| `triantruchet` | 391 | 4 | no | writes TC/color |
| `truchet` | 432 | 7 | no | writes TC/color |
| `truchet_ae` | 881 | 22 | no | writes TC/color |

#### `unportable_image` (1)

Reads image / colormap / texture data we don't have a binding for.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `wangtiles` | 1190 | 12 | no | image/colormap input |

#### `unportable_subflame` (2)

Uses subflame state (nested IFS).

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `glynns3subfl` | 444 | 8 | yes | subflame state |
| `ringsubflame` | 397 | 8 | no | subflame state |

#### `unportable_other` (5)

C++ body still references Java types like `new Complex(...)` or compares to `null`. Formula uses opaque Java wrapper classes (often Janino-compiled at runtime in JWildfire); not a clean port without manually expanding the underlying math.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `colordomain` | 526 | 0 | no | Java residue (Complex/Janino classes left in body) |
| `custom_wf_full` | 797 | 0 | no | Java residue (Complex/Janino classes left in body) |
| `klein_group` | 577 | 6 | no | Java residue (Complex/Janino classes left in body) |
| `plusrecip` | 170 | 2 | no | Java residue (Complex/Janino classes left in body) |
| `polylogarithm` | 450 | 2 | no | Java residue (Complex/Janino classes left in body) |

#### `unported_stub` (35)

C++ port has empty `PluginVarCalc` body. Formula lives in the embedded Java comment block — manual translation required.

| name | LOC | params | 3d | notes |
|---|---:|---:|:---:|---|
| `complex` | 707 | 64 | no | empty PluginVarCalc body; port from embedded Java |
| `cone` | 166 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `cylinder2` | 104 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `disc3` | 188 | 8 | no | empty PluginVarCalc body; port from embedded Java |
| `ducks` | 1120 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `eclipse` | 152 | 1 | no | empty PluginVarCalc body; port from embedded Java |
| `falloff3` | 107 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `glynnlissa` | 301 | 11 | no | empty PluginVarCalc body; port from embedded Java |
| `glynnspiro` | 359 | 11 | no | empty PluginVarCalc body; port from embedded Java |
| `glynnsshape` | 328 | 11 | no | empty PluginVarCalc body; port from embedded Java |
| `gridout3d` | 280 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `henon` | 151 | 3 | no | empty PluginVarCalc body; port from embedded Java |
| `hole2` | 207 | 6 | no | empty PluginVarCalc body; port from embedded Java |
| `hypercrop` | 173 | 3 | no | empty PluginVarCalc body; port from embedded Java |
| `hypershift` | 144 | 2 | no | empty PluginVarCalc body; port from embedded Java |
| `intersection` | 233 | 10 | no | empty PluginVarCalc body; port from embedded Java |
| `invsquircular` | 97 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `knots3d` | 955 | 12 | no | empty PluginVarCalc body; port from embedded Java |
| `lazysensen` | 177 | 3 | no | empty PluginVarCalc body; port from embedded Java |
| `lozi` | 151 | 3 | no | empty PluginVarCalc body; port from embedded Java |
| `mobius_strip` | 329 | 3 | no | empty PluginVarCalc body; port from embedded Java |
| `post_falloff3` | 111 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `pre_falloff3` | 112 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `pre_recip` | 362 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `projective` | 190 | 9 | no | empty PluginVarCalc body; port from embedded Java |
| `pulse` | 156 | 4 | no | empty PluginVarCalc body; port from embedded Java |
| `q_ode` | 212 | 12 | no | empty PluginVarCalc body; port from embedded Java |
| `quaternion` | 966 | 92 | no | empty PluginVarCalc body; port from embedded Java |
| `spirograph3d` | 205 | 8 | no | empty PluginVarCalc body; port from embedded Java |
| `stripfit` | 147 | 1 | no | empty PluginVarCalc body; port from embedded Java |
| `sunvoroni` | 351 | 5 | no | empty PluginVarCalc body; port from embedded Java |
| `taprats` | 224 | 5 | no | empty PluginVarCalc body; port from embedded Java |
| `tqmirror` | 196 | 0 | no | empty PluginVarCalc body; port from embedded Java |
| `truchet2` | 278 | 7 | no | empty PluginVarCalc body; port from embedded Java |
| `vibration2` | 338 | 26 | no | empty PluginVarCalc body; port from embedded Java |

### Internal-weight watchlist

These variations use `VVAR` (the canonical JWildfire weight `pAmount`) *inside* their formula — not just as the outer `result += VVAR * f(p)` multiplier. Our shader applies weight outside the function, so any internal use needs a design decision before porting:

Options per case:
- (a) Hard-code weight = 1 inside the function and expose a separate parameter for what was effectively a self-scaling factor.
- (b) Extend `VariationDef` to optionally pass `weight` into the function as an argument.
- (c) Per-variation rewrite to factor weight out (sometimes algebraically possible).

| name | bucket | LOC | params | 3d | notes |
|---|---|---:|---:|:---:|---|
| `blocky` | `param` | 196 | 3 | no | VVAR used internally |
| `cubic3d` | `param` | 180 | 2 | yes | VVAR used internally |
| `elliptic2` | `param_rng` | 241 | 11 | no | VVAR used internally |
| `extrude` | `param_rng` | 142 | 1 | yes | VVAR used internally |
| `fourth` | `param` | 262 | 5 | no | VVAR used internally |
| `glynnia` | `rng` | 198 | 0 | no | VVAR used internally |
| `glynnia3` | `param_rng` | 246 | 4 | no | VVAR used internally |
| `hexnix3d` | `param` | 247 | 4 | yes | VVAR used internally |
| `idisc` | `pure` | 141 | 0 | no | VVAR used internally; 2 porter-omitted params (recover from Java) |
| `loonie2` | `param` | 253 | 3 | no | VVAR used internally |
| `loonie3` | `pure` | 145 | 0 | no | VVAR used internally |
| `loq` | `param` | 148 | 1 | yes | VVAR used internally |
| `npolar` | `param_rng` | 193 | 2 | no | VVAR used internally |
| `popcorn2_3d` | `param` | 115 | 4 | yes | VVAR used internally |
| `prepost_affine` | `param` | 314 | 9 | yes | VVAR used internally |
| `prepost_mobius` | `param` | 250 | 8 | no | VVAR used internally |
| `scry2` | `param` | 242 | 3 | no | VVAR used internally |
| `scry_3d` | `pure` | 92 | 0 | yes | VVAR used internally |
| `sigmoid` | `param` | 200 | 2 | no | VVAR used internally |
| `spliptic_bs` | `param_rng` | 188 | 2 | no | VVAR used internally |
| `squircular` | `pure` | 118 | 0 | no | VVAR used internally |
| `truchet_fill` | `param` | 282 | 3 | no | VVAR used internally |
| `waffle` | `param_rng` | 219 | 4 | no | VVAR used internally |

### Porter-omitted params watchlist

These C++ files declare zero `VAR_REAL` parameters but their `PluginVarPrepare` initializes private `_xxx` fields to literal constants and `PluginVarCalc` reads them. The C++ porter forgot to expose them as user parameters — the original Java (in the comment block at the bottom of each file) defines them properly. When porting, recover the parameter names, defaults, and ranges from the Java source.

| name | bucket | LOC | omitted-param count | 3d | notes |
|---|---|---:|---:|:---:|---|
| `arch` | `rng` | 128 | 2 | no | 2 porter-omitted params (recover from Java) |
| `atan2_spirals` | `pure` | 254 | 11 | no | 11 porter-omitted params (recover from Java) |
| `bsplit` | `pure` | 149 | 2 | no | 2 porter-omitted params (recover from Java) |
| `csc_squared` | `pure` | 195 | 7 | no | 7 porter-omitted params (recover from Java) |
| `curliecue2` | `rng` | 166 | 8 | no | 8 porter-omitted params (recover from Java) |
| `deltaa` | `pure` | 127 | 2 | no | 2 porter-omitted params (recover from Java) |
| `fdisc` | `pure` | 217 | 3 | no | 3 porter-omitted params (recover from Java) |
| `idisc` | `pure` | 141 | 2 | no | VVAR used internally; 2 porter-omitted params (recover from Java) |
| `line` | `rng` | 170 | 2 | yes | 2 porter-omitted params (recover from Java) |
| `mask` | `pure` | 179 | 5 | no | 5 porter-omitted params (recover from Java) |
| `onion2` | `pure` | 391 | 8 | yes | 8 porter-omitted params (recover from Java) |
| `post_heat` | `pure` | 278 | 9 | yes | 9 porter-omitted params (recover from Java) |
| `pre_blur3d` | `rng` | 162 | 5 | no | 5 porter-omitted params (recover from Java) |
| `seashell3d` | `rng` | 186 | 4 | yes | 4 porter-omitted params (recover from Java) |
| `wdisc` | `pure` | 137 | 2 | no | 2 porter-omitted params (recover from Java) |

### Anomalies

None — all 415 candidate names matched a `.cpp` file in the upstream `output/` directory (case-insensitive).

### Notes & caveats

- **Filename case mismatch**: many upstream files use mixed case (`affine3D.cpp`, `julia3Dq.cpp`, `popcorn2_3D.cpp`) while our candidate list is lowercase. Matched case-insensitively.
- **`unportable_dc` is soft**: 16 entries write to `TC` (transform color), but for several (`truchet`, `mandala`, `mandala2`, `triantruchet`) the geometric component is a normal IFS and could be ported by skipping the color line. Worth a manual pass.
- **`unported_stub` (35)** is the largest non-portable bucket but it's not a hard wall: the C++ ports just left these blank. The full Java implementation lives in the comment block at the bottom of each file, so they're translatable by hand. Includes some interesting standalone shapes: `complex` (64 params!), `ducks`, `eclipse`, `glynnsshape`, `glynnspiro`, `glynnlissa`, `recurrenceplot`. `complex`, `ducks`, and the `glynn*` ones likely use Janino runtime-compiled Java in the original — flag during port.
- **`prepost_*` variations** (3 entries: `prepost_affine`, `prepost_circlize`, `prepost_mobius`) MUTATE the input `FTx`/`FTy`/`FTz` and then write `FPx`/`FPy`/`FPz`. They're effectively two-stage (pre then post) collapsed into one variation. Need careful porting; they don't fit the single-`Phase` model cleanly.
- **Existing-name overlap**: `cpow`, `julia3d`, `post_curl`, etc. don't appear in the candidate list because they're already implemented. `cpow2`/`cpow3` (parameterized variants) are in the list as `param_rng`.