# Variation Bulk Port from JWildfire / Chaotica

## Goal

Port the bulk of standard fractal-flame variations from upstream JWildfire/Chaotica into our `src/variations/defs/` registry, then upload them to the API as the variation cache's primary content. Target: from ~80 today to ~500 commonly-used variations.

Source repo: https://github.com/mwegner/chaotica-apophysis-plugins-from-jwildfire/tree/master/output (C++ ports of JWildfire variations).

## Inventory (2026-04-26)

| | Count |
|---|---|
| Our current variations (`ALL_VARIATIONS` in `src/variations/defs/mod.rs`) | **84** *(at start; **326** as of 2026-05-01 — see [Porting progress](#porting-progress-2026-04-27))* |
| Upstream `.cpp` files | **636** |
| Already implemented (exact name match) | 78 |
| Ours-only (name doesn't appear upstream) | 6 |
| Upstream-only (potentially to port) | **558** *(initially; **~316 remaining** after the porting since)* |

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

1. ✅ **This doc + lists** — done
2. ✅ **Bulk-fetch the 415 .cpp files** for offline analysis — done (subagent run, 2026-04-26). The same source is now mirrored locally under `output/jwildfire-vars/` (gitignored) for offline reading via the file tools — preferred over web-fetching during ports.
3. ✅ **Per-file classification** — pure / parameterized / RNG / uses-internal-weight / broken — done (see [Classification](#classification-auto-generated-2026-04-26) below)
4. 🚧 **Port in batches** — 38 batches landed (207 variations); see [Porting progress](#porting-progress-2026-04-27) below
5. ⏳ **Database import scripts** — generate from the ported `VariationDef`s for upload to the variations API
6. ⏳ **Test pass** — flame referencing each new variation renders correctly via local registry, then via API fetch with cache cleared

A prerequisite that surfaced during step 4 and got its own commit: the GPU
buffer was sized `[f32; 100]` and indexed by global registry index, so the
registry was capped at 100 even though the shader only emits code for the
few active variations. The fix was per-flame **local index assignment** for
both the shader emitter and the buffer populator, sharing
`compute_local_index_map` in `src/scene/transforms.rs`. Buffer cap is now a
"max 100 active per flame" constraint, not a registry-size constraint.

## Porting progress (2026-04-27)

| Batch | File | Family | Count | Notes |
|---|---|---|---:|---|
| 1 | `hyperbolic.rs` | Inverse hyperbolic (acoth, acosh, etc.) | 7 | First batch. RNG miss in classifier surfaced (acosh/acosech use GOODRAND_01). |
| 2 | `trig.rs` | Direct trig + hyperbolic (sin, cos, …, sinh, cosh, …) | 12 | Upstream's `sech` is mislabeled csch — preserved. |
| 3 | `quaternion.rs` | zephyrtronium quaternion (sinq, cosq, …) | 12 | All 3D-aware. 2D form is the natural collapse at `z=0`. |
| 4 | `sqrt_hyperbolic.rs` | sqrt-prefixed inverse hyperbolic | 6 | All RNG (classifier missed); `sqrt_asech` is upstream-bugged → calls AcosH not AsecH. Preserved. |
| 5 | `trig_bs.rs` | Brad Stefanov parameterized trig (sin2_bs …) | 13 | First **param** batch — validates the param flow. `sech2_bs` uses the *correct* sech denom (cos+cosh), unlike `trig.rs:sech`. |
| 6 | `exp_log.rs` | exp + log_db + log_tile2 + tile_log | 4 | First **param + RNG** combo. `log_db` upstream has `atan2(x,y)` (swapped) — preserved. (Originally included `log_apo`; dropped before merge — functionally identical to existing `log` from the base 84.) |
| 7 | `shapes.rs` | Misc trig + standalone shapes | 12 | First batch with `Float` and `Angle` ParamTypes. `secant2` uses internal weight — see watchlist. |
| 8 | `shapes2.rs` | Standalone shapes continued | 12 | `ennepers` upstream uses `=` not `+=` — fixed (see decisions). |
| 9 | `numbered.rs` | Numbered/3D variants of existing | 12 | First ports with init-step precompute inlined (juliaq/julia3dq/juliac). |
| 10 | `heavy_init.rs` | cpow2, cpow3, disc2 (heavy init) | 3 | Larger init-step precomputes inlined into the per-iteration body. |
| 11 | `init_ports.rs` | `target`, `yin_yang` | 2 | First ports using the new `wgsl_init` dispatch step (per-flame precompute on the GPU). Cleared the porter-omitted-init watchlist for these two. |
| 12 | `affine_ports.rs` | `popcorn` (+ `waves` migration) | 1 | First port using the new `needs_transform` flag. `waves` was simultaneously migrated from a hard-coded placeholder to the actual Scott Draves formula. |
| 13 | `dc.rs` | `dc_linear`, `dc_bubble` | 2 | First ports using `writes_color: true` (direct-color register `vc` plus the Apophysis 3-step color flow). Required broadening `needs_affine` → `needs_transform` so DC bodies can read the per-iteration weight. |
| 14 | `hypertile.rs` | Zueuk hyperbolic tilings | 6 | `hypertile`/`1`/`2`/`3d`/`3d1`/`3d2`. All use `wgsl_init` for the {p,q}-tile-radius precompute. The 3 sub-families' `r` formulas are algebraically equivalent — preserved each variation's exact fallback condition for parity. |
| 15 | `classic_2d.rs` | Popular standalone 2D | 6 | `fan` (needs_transform for affine coeffs), `fisheye` (preserves cpp X/Y swap — corrected version is our `eyefish`), `gridout`, `circular`, `panorama1`, `panorama2`. |
| 16 | `mobius_extended.rs` | Möbius extensions | 2 | `mobiusN` (N-power Möbius, 10 user params + |power|<1 clamp inlined) and `mobiq` (quaternion Möbius, full 3D, exactly 16 user params — the buffer-slot maximum, no init room). `prepost_mobius` skipped (priority-2 pre+post pattern). |
| 17 | `circle_blur.rs` | Circle/blur distortions | 4 | `circleblur`, `circlesplit`, `flipcircle` (uses needs_transform to read VVAR for its radius comparison), `blur_linear` (init for sin/cos of angle). |
| 18 | `numbered_extras.rs` | Numbered cont'd | 3 | `bipolar2`, `blob3d`, `circular2`. |
| 19 | `glynn.rs` | Glynnia + GlynnSim set | 5 | `glynnia` (factored the `_vvar2 = VVAR · sqrt(2)/2` internal weight cleanly), `glynnia3`, `glynnSim1`/`2`/`3`. The GlynnSim ports follow the Java semantics rather than the cpp (the cpp `circle()` helper takes `Point` by value — porter bug; we inline the helper, matching the JWildfire engine's output). |
| 20 | `wedge_extended.rs` | Wedge family extensions | 2 | `wedge_julia`, `wedge_sph`. wedge_julia upstream computes `ca = cos(sa)` (cos-of-sin-of-a, not cos(a)) — preserved in both Java and cpp. |
| 21 | `shapes3.rs` | Big standalone shapes | 3 | `super_shape`, `henon` (cpp PluginVarCalc was unported_stub — translated from the Java comment), `apollony` (Apollonian-gasket IFS via 3-way random branch). |
| 22 | `radial_extras.rs` | Radial weight-independent | 2 | `onion`, `target_sp`. Both have X/Y output lines lacking the usual `VVAR *` multiplier — body uses `needs_transform` + divide-out so outer × w restores the cpp output. New pattern; described in [Notable decisions](#notable-decisions-during-porting). |
| 23 | `internal_weight.rs` | Watchlist via needs_transform | 4 | `loonie3`, `loonie_3d`, `sigmoid`, `blocky`. Two patterns: threshold-only weight (factors cleanly) and non-linear weight (divide-out). `blocky`'s upstream `sqrt_safe(vp, x)` macro-expansion bug bypassed — we follow Java intent. |
| 24 | `pre_post_bridges.rs` | Pre/post phase bridges | 3 | `pre_curl`, `post_juliaq`, `post_julia3dq`. Pre/post phases have no outer multiplier, so body reads `w` via `needs_transform` and applies VVAR directly. Cpp's `GOODRAND_0X(INT_MAX) * inv_power_2pi` replaced with `floor(rand · power) * inv_power_2pi` (semantically equivalent uniform N-th-root branch without distribution bias). |
| 25 | `truchet.rs` | Truchet kickoff | 1 | `truchet_fill`. Internal weight via `scale = 1/VVAR` plus weight-independent FPx/FPy lines — divide-out pattern. Other Truchet members deferred (mostly `unportable_dc` or `unported_stub`). |
| 26 | `blur_extras.rs` | More blur primitives | 3 | `sineblur`, `starblur`, `r_circleblur`. All factor cleanly through outer multiplier. `farblur`/`exblur`/`nblur` deferred — persistent `_r[4]` state buffer. |
| 27 | `boarders.rs` | Boarders / border-tile | 4 | `boarders`, `boarders2`, `pre_boarders2`, `splitbrdr`. `pre_boarders2` is pre-phase with `needs_transform` to apply VVAR inside (no outer multiplier). `splitbrdr` mostly factors but ends with two extra non-VVAR lines — divide-out. |
| 28 | `standalone_exotics.rs` | Misc exotics | 3 | `kaleidoscope` (preserves cpp's `cos(45.0)` = 45 RADIANS quirk), `taurus` (Z-only divide-out), `hole2` (cpp was unported_stub; 10 radial-shape selector translated from Java). |
| 29 | `parametric_curves.rs` | Parametric curves + crop | 4 | `spirograph`, `lissajous`, `vogel` (φ-spiral), `crop3d`. crop3d uses standard outer-multiplier convention (cpp's `FPx = VVAR · x` matches our `+= VVAR · x` when crop3d is the only normal variation in its transform — typical use). |
| 30 | `stub_recoveries.rs` | unported_stub recoveries | 6 | `bsplit` (cpp `APO_VARIABLES` empty — recovered 2 user params from Java), `cylinder2`, `eclipse`, `lozi`, `pulse`, `hypershift`. Each cpp PluginVarCalc was empty; ported from the embedded Java comment block. |
| 31 | `maurer_hyper.rs` | Maurer rose + hypercrop | 2 | `maurer_rose` (Gregg Helt 2016, 11 user params, RNG-branched line/point/curve sampling), `hypercrop` (n-gon corner-cropping warp; another unported_stub recovery). |
| 32 | `misc_2d.rs` | Small 2D primitives | 8 | `split`, `squirrel`, `stripes`, `shift` (init: cos/sin of angle), `pressure_wave` (porter bug — cpp prepare hardcoded init values to 1.0; recovered the actual `pwx = freq·2π, ipwx = 1/pwx` from Java), `sphericaln`, `spligon`, `tile_hlp` (needs_transform for offset). |
| 33 | `misc_extras.rs` | Larger / 3D primitives | 6 | `ho` (3D hyperbolic-octahedron — uses sign-preserving `pow` to handle negative bases that WGSL `pow` rejects), `chunk` (quadratic-conic mask, divide-out), `ptransform`, `rational3` (degree-3 complex-rational), `tile_reverse`, `ortho` (4-branch Möbius warp). |
| 34 | `bipolar_series.rs` | B-/E-series + barycentroid | 10 | Faber's bSeries (`bcollide`, `bmod`, `bswirl`) and eSeries (`ecollide`, `emod`, `eswirl`, `escale`, `epush`, `erotate`) using bipolar/elliptic coordinate transforms; plus `barycentroid` (Xyrus02). All clean factor through outer multiplier. Each E-series body inlines the same elliptic preamble — we trust GPU compiler CSE when multiple are active. |
| 35 | `singleton_misc.rs` | Standalone misc | 8 | `corners` (recovered 7 omitted params from Java; cpp exposed only 2 of 9), `modulus`, `octagon` (3D), `circus`, `circlize` (divide-out for the `+ hole` term — cpp's "tsk tsk... not scaled by vvar"), `circlize2` (clean), `atan` (3-mode selector), `murl` (cpp put per-iter intermediates in the persistent struct unnecessarily — treated as locals). |
| 36 | `misc_extras2.rs` | More misc | 6 | `collideoscope` (Faber's radial branch-and-mod), `bent2` (per-quadrant scale), `mcarpet` (bubble + twist + tilt; FracFx), `lineart3d` (3D sign-preserving power), `oscilloscope` (Y-flip in damped cosine band), `fibonacci2` (Larry Berlin — Binet-formula complex Fibonacci with constants inlined). Skipped `cubicLattice_3D` (mid-iteration accumulator read). |
| 37 | `misc_extras3.rs` | Even more misc | 4 | `oscilloscope2` (DarkBeam tweak — both X and Y flipped inside band), `lineart` (2D version of lineart3d — porter naming inconsistency `lineart` vs `linearT`), `phoenix_julia` (TyrantWave; X/Y distortion + N-th-root random branch), `pow_block` (cothe / DarkBeam — generalized N-th root). Skipped `arctruchet` (malloc'd persistent state), `circleTrans1` (do-while + hash). |
| 38 | `stub_recoveries2.rs` | More unported_stub recoveries | 4 | `disc3` (8 user knobs over base disc), `projective` (eralex61 — linear-fractional projective), `tqmirror` (Anderson — quadrant fold-mirror; needs_transform for VVAR-as-threshold), `intersection` (Stefanov — random row/col tile blur, needs_transform for divide-out). Skipped `mobius_strip` (10 params + multi-mode init; focused single-port batch better), `pre_recip` (Java Complex class extensively used). |
| 39 | `lazy_family.rs` | Lazy family (Faber / FarDareisMai) | 2 | `lazyjess` (4 user, 4 init slots: vertex, sin_vertex, pie_slice, corner_rot — n=2 special case + n>2 inscribed N-gon test), `lazytravis` (3 user, 2 init slots: 4·spin_in/4·spin_out — square fold-mirror with quadrant routing on perimeter parameterization). Both `needs_transform` for VVAR-as-threshold. |
| 40 | `misc_extras4.rs` | More misc | 8 | `anamorphcyl` (Sosa), `svf` (gossamer light, 3D), `shredlin` (Zy0rg), `shredrad` (Zy0rg — porter bug: cpp PluginVarPrepare empty; `_alpha = 2π/n` recovered from Java setParameter), `xheart` (xyrus02), `stwin` (Apo pack — needs_transform divide-out), `whorl` (Apo pack — needs_transform for VVAR threshold), `devil_warp` (dark-beam — needs_transform divide-out). |
| 41 | `watchlist_misc.rs` | Internal-weight watchlist + misc | 8 | `trade` (Faber — clean two-disc swap), `voron` (eralex61 — Voronoi-cell snap with bit-mixed i32 hash; relies on signed-int wrap parity between cpp and WGSL), `squircular` (Möbius — VVAR² in body radius; needs_transform divide-out), `flux` (meckie — additive `xpw=x+VVAR` shift; needs_transform), `rays` (Z+ — RNG cubic in VVAR; needs_transform), `rays1` (Raykoid666 — additive VVAR inside `1/tan + VVAR·(2/π)²`; needs_transform), `loonie2` (dark-beam — N-sided loonie, sqrvvar=w² threshold, 6 init slots, runtime sides loop; needs_transform), `fourth` (guagapunyaimel — 4-quadrant compound: spherical/loonie/susan/linear; needs_transform divide-out). |
| 42 | `classic_blades_misc.rs` | Classic blades + small classics | 9 | `arch`, `blade`, `blade3D`, `twintrian` (Z+ Jan 2007 family — RNG with VVAR inside angle; needs_transform divide-out; `blade3D` is Full3D with explicit z output); `bi_linear` (clean swap); `squarize` (Faber angle-pack); `squish` (Faber, 1 user param + 1 init slot, RNG); `twoface` (Apo classic — internal w; needs_transform); `unpolar` (Apo classic — `w/(2π)` factor; needs_transform). `twintrian` uses `log/ln(10)` for log10 since WGSL has no log10. |
| 43 | `apo_misc.rs` | Apophysis miscellany 5 | 8 | `xerf` (zephyrtronium / dark-beam — 3D piecewise erf/inverse; ships A&S 7.1.26 erf approximation, max error ~1.5e-7); `inverted_julia` (Whittaker Courtney 2018 — 9 user params recovered from Java setParameter; cpp APO_VARIABLES only declared 2); `idisc` (Faber — needs_transform divide-out); `conic` (cyberxaos — needs_transform divide-out, RNG); `power` (Apo classic — preserves cpp's cosA-instead-of-sinA exponent quirk + xy-output swap, rotating Java result 90°); `roundspher` (Raykoid666 — body has w² factor, needs_transform divide-out); `checks` (Apo classic — 4 user + 1 init, RNG); `cone` (Brad Stefanov — 9 params recovered from Java unported_stub, 3D, RNG). |
| **Total new** | | | **242** | |
| Registry size | | | **326** | (84 base + 242 ported) |

Branches and commits:
- Batches 1–10 on `variation-bulk-port-batch1`. Commits in order:
  `a7ecc30`, `8698337`, `0ef937e` (refactor), `dd1239c`, `ae80fbc`, `3fc37a8`,
  `d11533a`, `a7664e8`, `28e571a`, `46b8c7e`, `d132ce3` (doc update),
  `a61cc70`, `3756ce1` (doc update), `4c739ae`.
- Batches 11–12 on `variation-init-dispatch` (introduces `wgsl_init` and the
  `needs_affine` flag, later renamed to `needs_transform`). Merge: PR #59.
- Batch 13 on `transform-and-dc` (introduces `writes_color`, broadens
  `needs_affine` → `needs_transform`, adds `Transform.direct_color`). Merge:
  PR #60.
- Batches 14–19 on `variation-port-hypertile`. Commits in order:
  `3b9f648` (hypertile), `5c882ce` (classic 2D), `a4f56e8` (Möbius),
  `1a7974d` (circle/blur), `5aee059` (numbered extras), `9f4b603` (Glynn).
- Batches 20–29 on `variation-bulk-port-2`. Commits in order:
  `a29145d` (wedge), `10e5e05` (shapes3), `fc0601d` (radial extras),
  `6094b56` (internal-weight watchlist), `f6460d1` (pre/post bridges),
  `5a463de` (truchet kickoff), `5af2340` (blur extras), `6203121`
  (boarders), `d26ec36` (standalone exotics), `ca293ad` (parametric
  curves + crop3D).
- Batches 30–35 also on `variation-bulk-port-2`. Commits in order:
  `2fbb1cf` (stub recoveries), `efaa12f` (maurer_rose + hypercrop),
  `ca5f946` (misc 2d), `85066ce` (misc extras), `b628bec` (B/E-series),
  `6eb8ca5` (singleton misc).
- Batches 36–38 also on `variation-bulk-port-2`. Commits in order:
  `996b5f8` (misc extras 2), `2389c2c` (misc extras 3), `2ee631b`
  (stub recoveries 2).
- Batches 39–43 also on `variation-bulk-port-2`. Commits in order:
  `c61e734` (lazy family), `8dff306` (misc extras 4), `c3ce9b0`
  (watchlist + misc), `079cecb` (classic blades + misc), `9767976`
  (apo misc 5).

### Notable decisions during porting

These are places where I diverged from a literal C++→WGSL port and why:

- **`ennepers` (batch 8) — fixed upstream typo.** Upstream writes
  `FPx = pAmount * (x - x³/3) + x·y²` (assignment, not `+=`, and only the
  first term scaled by weight). Treating both as porter typos and
  accumulating both terms with the outer weight produces the more sensible
  Enneper-surface mapping `(x(1 − x²/3 + y²), y(1 − y²/3 + x²))`.
- **`sech` (batch 2) — preserved upstream bug.** Upstream's "sech" formula
  divides by `(e^z − e^-z)` instead of `(e^z + e^-z)`, so it actually
  computes csch(z·π/4). Kept the bug so JWildfire flames render the same.
- **`sqrt_asech` (batch 4) — preserved upstream bug.** Upstream calls
  `complexAcosH(sqrt(z))` instead of an asech formula — copy-paste from
  `sqrt_acosh`. Kept for parity.
- **`atan2(x, y)` swap bug (recurring).** A systematic C++ porter mistake:
  the C++ ports translate Java's `getPrecalcAtanYX()` (which is
  `atan2(y, x)`) as `atan2(FTx, FTy)` (swapped — `atan2(x, y)`). Affects
  `log_db` (batch 6), `cpow2`/`cpow3`/`disc2` (batch 10), and almost
  certainly more we haven't ported yet. We preserve the C++ behavior so
  flames built against the C++ ports render the same. Watch for this in
  every future port that uses atan2.
- **`secant2` (batch 7) — internal weight diverges from upstream.** Upstream
  computes the radius for `cos(r)` with `r = pAmount · sqrt(x²+y²)`, so
  the non-linear `cos` part scales with weight. Our outer-multiplier
  convention can't capture that. We compute with unweighted `r` — at the
  conventional `weight = 1` results match upstream, drift at other weights.
  Added to internal-weight watchlist.
- **`acosh`, `acosech`, `sqrt_*`** — classifier missed `GOODRAND_01`. Marked
  `needs_rng: true`. Same heuristic gap reported in classification notes.
- **Init-step inlining (`log_apo`, `log_db`, ...)** — upstream computes
  `_denom = 0.5 / log(base)` etc. in `init()`. We have no init hook, so we
  recompute per-iteration. Negligible perf cost, slight code duplication.
  *Resolved post-batch-10:* batches 11+ have the `wgsl_init` GPU dispatch,
  so newer ports do the precompute once per flame setup instead of
  inlining. Ports done before that (batch 1–10) stay as-is.
- **`fisheye` (batch 15) — preserved upstream X/Y swap.** Upstream's
  `fisheye.cpp` uses `FTy / r` for the X output and `FTx / r` for Y —
  swapped from the Java intent. The corrected version is `eyefish`,
  already in our base 84. Both ship; flames pick whichever matches their
  upstream lineage.
- **`fan` / `panorama1`/`panorama2` (batch 15) — preserved cpp atan2 form.**
  Upstream's atan2 args are non-standard (`atan2(x, y)` rather than the
  usual `atan2(y, x)`). Same systematic porter swap as `log_db` etc.,
  preserved for parity.
- **`mobiusN` (batch 16) — `|power| < 1` clamp inlined.** Upstream's
  `init()` clamps `power = 1.0` when `|power| < 1`. We don't get a chance
  to mutate user params from `wgsl_init`, so the clamp lives in the body
  (one extra branch per iteration; negligible).
- **`mobiq` (batch 16) — exactly fills the param-slot budget.** 16 user
  params × 1 quaternion-Möbius variation = the full 16-slot per-variation
  buffer space. No room for derived-init values, so the body inlines the
  full quaternion arithmetic.
- **`prepost_mobius` (batch 16) — skipped, architectural.** Upstream is a
  JWildfire priority-2 variation: it runs the **inverse** Möbius BEFORE
  the affine *and* the forward Möbius AFTER the variation accumulator,
  with assignment (`FPx = ...`) rather than `+=`. Our pre/normal/post
  phase model has separate slots for pre and post, and the normal phase
  accumulates. See [Architectural blockers](#architectural-blockers-deferred).
- **`flipcircle` (batch 17) — internal weight via `needs_transform`.**
  Upstream's `r² > VVAR²` comparison treats VVAR as the radius of the
  flip threshold, so the geometry varies with weight. Rather than
  hard-coding weight=1, the body reads
  `transforms[xform_id].variations[variation_id]` directly; full fidelity
  at any weight.
- **`glynnSim1`/`2`/`3` (batch 19) — followed Java, not the cpp port.**
  The cpp ports declare `void circle(Variation* vp, Point p)` with
  `Point p` as a value parameter, so writes inside the helper don't
  mutate the caller's `_toolPoint` — the cpp body reads stale state. The
  Java original passes `Point` by reference (object types in Java) and
  writes propagate. We inline the helper directly into the body so writes
  are local variables (the natural WGSL idiom), matching the JWildfire
  engine. Flames built against the buggy C++ port will differ.
- **`glynnia` / `glynnia3` (batch 19) — internal weight factored out.**
  Both are flagged on the internal-weight watchlist (`_vvar2 = VVAR ·
  sqrt(2)/2`, plus `r = VVAR / dx` in two branches). Algebraic check
  showed the upstream output is `weight · (sqrt(2)/2 · ...)` and
  `weight · (1/dx · ...)` — the weight factors out cleanly through our
  outer-multiplier. Ports do **not** need `needs_transform`; they ship
  with full fidelity at any weight. Adjusts the watchlist below.
- **`onion` / `target_sp` (batch 22) — divide-out pattern for
  weight-independent X/Y.** Both upstream variations write `FPx +=
  stuff` *without* multiplying `stuff` by VVAR — making the X/Y output
  weight-independent in the cpp semantics (only Z preserve, where
  present, scales with weight). Our pipeline always multiplies the
  variation's return by the weight in the outer dispatcher, so we read
  the weight via `needs_transform` and divide it out (`return output ·
  inv_w`) — outer × w then restores the cpp result. New pattern;
  reused by `truchet_fill` (batch 25), `blocky` Z-divide variants in
  batch 23, `splitbrdr` (batch 27), `kaleidoscope`/`taurus`/`crop3d`
  (batches 28–29) for the same shape of mismatch.
- **`sigmoid` (batch 23) — sign-pass for absolute weight.** Upstream
  uses `vv = |VVAR|` then `FPx += vv · stuff`. We emit `sign(w) · stuff`
  so outer × w = `|w| · stuff`. Edge: `select(sign(w), 1.0, w == 0)`
  to keep weight=0 well-defined.
- **`blocky` (batch 23) — fixed upstream `sqrt_safe` macro bug.**
  Upstream's `sqrt_safe(vp, x)` helper takes `double x` as its function
  argument but reads `VAR(x)` inside (via the macro that expands to the
  variation's `x` *user parameter*) — a porter bug that ignores the
  function's actual argument and returns `sqrt(user_x)` regardless of
  what the caller passed. We follow the obvious Java intent
  (`sqrt(max(1 − a², 0))`) rather than reproducing the bug — preserving
  the cpp behavior would make `blocky`'s output depend on a user
  parameter that has no business affecting that codepath.
- **`flipcircle` (batch 17) — `needs_transform` for threshold weight.**
  Upstream's `r² > VVAR²` uses VVAR as a comparison threshold (geometry
  varies with weight). Body reads `w = transforms[xform_id]
  .variations[variation_id]` directly. Threshold-only weight; output
  factors cleanly. (Same pattern reused by `loonie3`/`loonie_3d` in
  batch 23.)
- **`pre_curl` / `post_juliaq` / `post_julia3dq` (batch 24) — direct
  VVAR application in pre/post phases.** Pre and post phases replace
  `temp` (no outer multiplier in our dispatcher), so the body just
  reads `w` via `needs_transform` and uses it directly inline.
- **`pre_boarders2` (batch 27) — same pattern as batch 24.** Pre-phase
  variant of `boarders2`; cpp uses `FTx = VVAR · stuff` (assignment).
  Body reads weight and applies VVAR directly.
- **N-th-root branch sampling (batch 24).** Cpp posts (`post_juliaq`,
  `post_julia3dq`) use `GOODRAND_0X(INT_MAX) · inv_power_2pi` for the
  N-th-root branch index. We replace with `floor(rand · power) ·
  inv_power_2pi` — semantically equivalent uniform branch selection
  without the cpp's distribution bias when `power` doesn't divide
  `2^31` evenly.
- **`kaleidoscope` (batch 28) — preserved upstream `cos(45.0)` quirk.**
  Both Java and cpp use `cos(45.0)` and `sin(45.0)` — that's 45
  RADIANS (≈ 0.5253 / 0.8509), not degrees. Looks like the original
  author intended degrees but `Math.cos`/`cos` take radians. Long-lived
  quirk; preserved.
- **`crop3d` (batch 29) — accept the assignment-vs-accumulator
  caveat.** Cpp uses `FPx = VVAR · x` (assignment). Returning `x`
  unscaled and letting the outer multiplier reapply VVAR matches cpp at
  any weight when crop3d is the only normal variation in its transform
  (typical use); diverges when mixed with other normal variations. Same
  caveat applies to the `circlecrop` family — but `crop3d` is small
  enough that the divergence is documented inline and the variation
  ships.
- **`hole2` (batch 28) and `henon` (batch 21) — translated from Java
  comment block.** Both are `unported_stub` in upstream cpp (empty
  `PluginVarCalc`). The Java implementations live in the embedded
  comment block at the bottom of each file; we ported directly from
  there. `hole2` cascades through 10 radial-formula cases via a
  `shape` int.
- **Stub-bucket bulk recoveries (batch 30).** Six more
  `unported_stub` cpp ports translated from their Java comment blocks
  in one batch: `bsplit` (also a porter-omitted param case — cpp's
  `APO_VARIABLES` was empty, recovered 2 user params from Java),
  `cylinder2`, `eclipse`, `lozi`, `pulse`, `hypershift`. The Java
  comment block is reliable as-is for these — the cpp porter just
  forgot to translate the body.
- **`pressure_wave` (batch 32) — porter bug fix.** cpp's
  `PluginVarPrepare` hardcodes `_pwx = _pwy = _ipwx = _ipwy = 1.0`
  unconditionally — porter forgot to translate the actual derivation
  in Java's `setParameter` (`pwx = freq · 2π; ipwx = 1/pwx`, with
  freq=0 fallback). Without the fix the variation behaves like
  identity-plus-sin regardless of frequency. Recovered from the Java
  comment block.
- **`blocky` (batch 23) — `sqrt_safe` macro bug fix.** cpp's
  `sqrt_safe(vp, x)` helper takes a `double x` argument but uses
  `VAR(x)` inside (which the macro expands to the variation's `x`
  user parameter, not the function argument). The cpp code thus
  ignores the function argument and returns `sqrt(user_x)` — a clear
  porter bug. We follow Java's obvious intent (`sqrt(max(1 − a², 0))`)
  rather than reproducing the bug.
- **`corners` (batch 35) — porter-omitted params recovery.** cpp's
  `APO_VARIABLES` exposes only `xwidth` and `ywidth`; the other 7
  params (`multx, multy, xpower, ypower, xypower, logmode, log_base`)
  live in the Java comment block. Recovered all 9.
- **`ho` (batch 33) — sign-preserving `pow` for negative bases.**
  WGSL's `pow(x, p)` returns NaN for negative `x` (unlike many cpp
  `pow` implementations which can be permissive in some configs). We
  use `pow(|x|, p) · sign(x)` to keep the variation's output continuous
  through `x = 0` while matching upstream visual.
- **N-th-root branch sampling (batches 16, 24).** Multiple variations
  use cpp's `GOODRAND_0X(INT_MAX) · 2π/power` to pick a random N-th-root
  branch. We replace with `floor(rand · power) · 2π/power` —
  semantically equivalent uniform branch selection without the cpp
  approach's distribution bias when `power` doesn't divide `2^31`
  evenly. Used in `mobiusN`, `post_juliaq`, `post_julia3dq`,
  `sphericaln`, `cpow2`, `cpow3`.
- **`murl` (batch 35) — cpp's spurious `Variables` struct fields.**
  cpp put `_c, _p2, _vp, _a, _sina, _cosa, _r, _re, _im, _rl` in the
  per-thread `Variables` struct, but they're all computed inside
  `PluginVarCalc` from per-iteration values — they're not "init"
  values. Treated as local variables (no init slots needed).

### Common patterns shaken out (cumulative through batch 35)

By the end of batch 35 we've seen the following concrete patterns
recur enough to be canonical references for future ports:

1. **Outer multiplier factors cleanly** (the easy case). Just port
   `f(p)` without the `VVAR *` and let the outer dispatcher reapply.
2. **Threshold-only weight** (`flipcircle`, `loonie3`/`_3d`,
   `chunk`'s comparison): use `needs_transform: true` to read the
   weight, use it as a comparison threshold, return the unscaled
   output.
3. **Multiplicative VVAR factors out** (`glynnia`, `glynnia3`):
   identify the algebraic VVAR factor by hand and drop it from the
   body; outer multiplier reapplies.
4. **`vv = |VVAR|` sign-pass** (`sigmoid`): emit `sign(w) · output`
   so outer × w = `|w| · output`.
5. **Divide-out** (`onion`, `target_sp`, `truchet_fill`, `splitbrdr`,
   `kaleidoscope`, `taurus` Z-only, `crop3d` (caveat), `corners`,
   `circlize`, `tile_hlp`, `chunk`'s output, `blocky`, `hypershift`,
   `eclipse`, `splitbrdr`, etc.): `needs_transform: true`, body
   computes the cpp output using the read weight, divides by `w`
   in the return so outer × w restores the cpp result.
6. **Direct VVAR application in pre/post phases** (`pre_curl`,
   `pre_boarders2`, `post_juliaq`, `post_julia3dq`): pre/post phases
   have no outer multiplier in our dispatcher, so just read `w` via
   `needs_transform` and apply it inline.
7. **Recover from Java comment block** for `unported_stub` cpp ports
   (`henon`, `hole2`, `bsplit`, `cylinder2`, `eclipse`, `lozi`,
   `pulse`, `hypershift`, `hypercrop`).
8. **Recover from Java for porter-omitted params** (`target`,
   `yin_yang`, `bsplit`, `pressure_wave`, `corners`, etc.): cpp had
   an empty or shrunken `APO_VARIABLES`; Java's `setParameter` lists
   the actual full param schema.

### Newly-found classifier misses (during porting)

Patterns the auto-classifier didn't flag but porting surfaced:

- **`GOODRAND_01` not always recognized** — the heuristic matched
  `GOODRAND_0[1X]?` but missed it in some files where the call appears in
  unusual positions. Affected: `acosh`, `acosech`, `sqrt_acoth`,
  `sqrt_acosh`, `sqrt_acosech`, `sqrt_asech`, `sqrt_asinh`, `sqrt_atanh`,
  `chrysanthemum`. All are RNG-dependent; corrected when porting.
- **Internal weight not always flagged** — `secant2` (computes
  `r = VVAR · sqrt(...)` then uses `cos(r)`), `rays` (`tanr = VVAR · …`),
  `rays1` (`u = … + VVAR · (2/π)²`), `flux` (`xpw = FTx + VVAR`). All
  add to the watchlist below.
- **Init-time precomputed fields not always flagged** — `target` reads
  `VAR(_t_size_2)` which doesn't appear in `APO_VARIABLES`. `yin_yang`
  reads `cosa/sina/cosb/sinb` similarly. Both need Java-source recovery.
- **Affine-coefficient dependency not flagged** — `popcorn` uses
  `XFORM_COEFF_20` / `XFORM_COEFF_21` (the affine `c` and `f` coefficients,
  i.e. translation parts). Variations in our system only see the
  post-affine point; the affine matrix isn't exposed to variation
  functions. New watchlist below.

## Open questions

- **Internal weight convention** — ~~for variations that use `weight` inside their formula (not just as outer multiplier), do we want to extend `VariationDef` to optionally pass weight into the function, or accept the per-variation rewrite cost?~~ **RESOLVED 2026-04-29 / 2026-05-01.** `needs_transform: true` lets the body read `transforms[xform_id].variations[variation_id]` directly, no `VariationDef` extension needed. Two patterns in use:
  - *Multiplicative VVAR* — factor through the outer multiplier (cleanest; `glynnia`/`glynnia3` use this pattern).
  - *VVAR as magnitude/threshold* — use `needs_transform` (`flipcircle` uses this pattern).
  Watchlist entries below should be reattacked with these patterns in mind.
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
| ~~`blocky`~~ | `param` | 196 | 3 | no | **PORTED batch 23** — non-linear weight (`v · r` ≈ w²); body reads `w` via `needs_transform` and divides output by `w` so outer × w restores the cpp result. Also fixes the upstream `sqrt_safe` macro bug. |
| `cubic3d` | `param` | 180 | 2 | yes | VVAR used internally |
| `elliptic2` | `param_rng` | 241 | 11 | no | VVAR used internally |
| `extrude` | `param_rng` | 142 | 1 | yes | VVAR used internally |
| `flux` | `param` | 150 | 1 | no | added during porting (batch 8 skip): `xpw = FTx + VVAR` shifts position by weight |
| `fourth` | `param` | 262 | 5 | no | VVAR used internally |
| ~~`glynnia`~~ | `rng` | 198 | 0 | no | **PORTED batch 19** — internal `_vvar2 = VVAR·sqrt(2)/2` and `r = VVAR/dx` factor out cleanly through the outer multiplier. No `needs_transform` required. |
| ~~`glynnia3`~~ | `param_rng` | 246 | 4 | no | **PORTED batch 19** — same factoring as `glynnia`. |
| `hexnix3d` | `param` | 247 | 4 | yes | VVAR used internally |
| `idisc` | `pure` | 141 | 0 | no | VVAR used internally; 2 porter-omitted params (recover from Java) |
| `loonie2` | `param` | 253 | 3 | no | VVAR used internally |
| ~~`loonie3`~~ | `pure` | 145 | 0 | no | **PORTED batch 23** — threshold-only weight pattern (`r² < w²` comparison; output factors cleanly). |
| ~~`loonie_3d`~~ | `pure` | 80 | 0 | yes | **PORTED batch 23** — threshold-only weight, same pattern as loonie3. |
| `loq` | `param` | 148 | 1 | yes | VVAR used internally |
| `npolar` | `param_rng` | 193 | 2 | no | VVAR used internally |
| `popcorn2_3d` | `param` | 115 | 4 | yes | VVAR used internally **and** assigns FPz instead of accumulating — see [Architectural blockers](#architectural-blockers-deferred). |
| `prepost_affine` | `param` | 314 | 9 | yes | VVAR used internally |
| `prepost_mobius` | `param` | 250 | 8 | no | VVAR used internally; also blocked by priority-2 pre+post pattern — see [Architectural blockers](#architectural-blockers-deferred). |
| `rays` | `rng` | 119 | 0 | no | added during porting (batch 8 skip): `ang = VVAR·rng·π`, `r = VVAR/(x²+y²)`, `tanr = VVAR·tan(ang)·r` — three internal uses |
| `rays1` | `pure` | 119 | 0 | no | added during porting (batch 8 skip): `u = 1/tan(sqrt(t)) + VVAR·(2/π)²` — additive |
| `scry2` | `param` | 242 | 3 | no | VVAR used internally |
| `scry_3d` | `pure` | 92 | 0 | yes | VVAR used internally |
| `secant2` | `pure` | 131 | 0 | no | added during porting (ported with caveat in batch 7): `r = VVAR·sqrt(...)` then `cos(r)` — non-linear weight scaling |
| ~~`sigmoid`~~ | `param` | 200 | 2 | no | **PORTED batch 23** — `vv = |VVAR|` pattern; body emits `sign(w) · output` so outer × w = `|w| · output`. |
| `spliptic_bs` | `param_rng` | 188 | 2 | no | VVAR used internally |
| `squircular` | `pure` | 118 | 0 | no | VVAR used internally |
| `truchet_fill` | `param` | 282 | 3 | no | VVAR used internally |
| `waffle` | `param_rng` | 219 | 4 | no | VVAR used internally |

**During the port:** `secant2` was shipped at weight=1 fidelity (deviates at
other weights, see decisions above). `rays`, `rays1`, `flux`, `loonie_3d`
were skipped from their respective batches — revisit when we make the
design call on internal-weight handling. **Update 2026-05-01:** `needs_transform`
unblocks reading the per-variation weight from the body, so the (b) option is
now available without a `VariationDef` change. `flipcircle` (batch 17) was
ported using exactly this pattern — read `transforms[xform_id].variations[variation_id]`
inside the body. The same approach should work for any of the watchlist
entries that use VVAR as a magnitude / threshold rather than as an additive
shift. Items where VVAR factors *cleanly* (algebraically multiplicative)
should prefer factoring through the outer multiplier — see `glynnia` /
`glynnia3` ports for the pattern.

### Architectural blockers (deferred)

Variations that don't fit our shader / phase model. Each blocker is a class
of incompatibility, not a per-variation issue; resolving the class unlocks
all members at once. Tracked here so the next implementer doesn't re-discover
the issue on each port attempt.

#### Assignment-vs-accumulator (FPx = …, not FPx +=)

Our normal-phase pipeline accumulates: each variation contributes
`weight · f(p)` and the contributions are summed. A subset of upstream
variations *replace* `FPx`/`FPy`/`FPz` with an absolute value (and some of
them also add a constant offset that doesn't factor through the outer
multiplier). The replacement semantics implicitly require the variation to
be the only normal-phase contributor in its transform; mixing with other
variations changes the output.

We could port these with a `needs_transform: true` body that reads its own
weight and rescales the output to match the cpp's `FPx = weight · stuff +
offset`, but the "no other normal variations allowed" caveat would still
apply (since we have no way to clear prior contributions from inside one
variation).

| name | bucket | LOC | notes |
|---|---|---:|---|
| `circlecrop` | `param_rng` | 237 | `FPx = rad_v · …` plus `+ x0` shift after. |
| `pre_circlecrop` | `param_rng` | 232 | Same body as `circlecrop`, pre-phase. |
| `post_circlecrop` | `param_rng` | 232 | Same body as `circlecrop`, post-phase. |
| `post_rblur` | `param_rng` | 168 | `FPx = VVAR · (FPx + …)` post-phase blur with absolute assignment. |
| `popcorn2_3d` | `param` | 115 | XY uses `+=` (compatible), but `FPz = …` assigns. |
| `prepost_mobius` | `param` | 250 | See "priority-2 pre+post" below. |

#### Priority-2 pre+post (single variation runs both before and after the affine)

JWildfire's "priority 2" variations run both the inverse op on `FTx`/`FTy`
**before** the affine *and* the forward op on `FPx`/`FPy` **after** it.
Our pre/normal/post phase model has separate slots for pre and post; a
single variation can't bind both. Porting these needs either a
`PrePost` phase with two WGSL bodies on one `VariationDef`, or splitting
the upstream variation into two independent `VariationDef`s and
documenting that they should be wired together by convention.

| name | bucket | LOC | notes |
|---|---|---:|---|
| `prepost_mobius` | `param` | 250 | Inverse Möbius pre-affine + forward Möbius post-affine, with assignment. |
| `prepost_affine` | `param` | 314 | (Internal-weight too.) |
| `prepost_circlize` | `param` | 234 | |

#### Indefinite-loop iterators (do-while sampling)

A few "circle*" generator variations sample candidate points in a
`do-while` loop until a discrete-noise condition is satisfied. On the
GPU, an unbounded loop can spin if the density param is small, and the
discrete noise function used (`DiscretNoise2 = bit-mixed integer hash`)
needs careful WGSL adaptation (the C `(int)` truncation differs from
WGSL `i32`).

We could port these with (a) a hard cap on loop iterations + a sentinel
fallback, accepting visual divergence in low-density configs, or (b) a
direct sampling formula equivalent to "expected value" of the do-while.

| name | bucket | LOC | notes |
|---|---|---:|---|
| `circleRand` | `param_rng` | 95 | Hash-keyed cell sampling with rejection. |
| `circleLinear` | `param_rng` | 124 | Same hash; cell-mode geometry. |
| `circleTrans1` | `param_rng` | 128 | Same hash; helper outputs `(Ux, Vy)`. |

#### Subflame state (recursion via JWildfire's flame engine)

Upstream `*subfl*` variations call back into the host flame's IFS engine
to render a nested flame as a sub-step. WGSL doesn't support recursive
function calls and our shader has no buffer for "another flame's
transforms"; both would need substantial pipeline changes. Listed in
the upstream `unportable_subflame` bucket already.

#### 16-slot per-variation budget overflow

Our variation parameter buffer gives each variation 16 contiguous
slots (user params + init-derived values combined). A small set of
upstream variations declare more than 16 user params; even with zero
init slots they don't fit.

The fix would be to widen the per-variation slot count (with a
corresponding cut to the per-flame variation cap, currently ~50) or
to introduce a parallel "extended params" buffer for the few
variations that need it. Until then these are blocked by budget, not
by math:

| name | bucket | LOC | notes |
|---|---|---:|---|
| `synth` | `param` | 1149 | 35 user params |
| `maurer_lines` | `param_rng` | 4677 | 36 user params |
| `quaternion` | `unported_stub` | 966 | 92 user params (extreme) |
| `complex` | `unported_stub` | 707 | 64 user params |
| `vibration2` | `unported_stub` | 338 | 26 user params |
| `inversion` | `param_rng` | 1110 | 25 user params |
| `jubiq` | `param_rng` | 401 | 24 user params |
| `truchet_ae` | `unportable_dc` | 881 | 22 user params (also DC-blocked) |

`mobiq` ships at exactly 16 user params (no init room) — the budget
limit, not over it. Anything bigger needs the budget extension.

#### Persistent per-thread variation state

A small but recurring pattern: variations that maintain state between
iterations within a single thread. Java keeps this state on the
`VariationFunc` instance; cpp ports keep it on the `Variation*`
struct. Either way, neither approach maps to our model — we have no
per-variation per-thread storage between iterations.

The two sub-cases:
- **Circular history buffer** of recent random draws (`farblur`,
  `exblur`, `nblur`): `_r[4]` array advanced one slot per iteration,
  used as a low-pass-filter weight on the current draw.
- **Autonomous trajectory walk** (`curliecue2`): `(x0, y0, theta,
  phi)` updated each iteration *ignoring* the input point; the
  variation effectively walks its own path through space, ignoring
  the IFS attractor entirely.

Solving the first sub-case alone would unlock all three blur variants
(small, popular). The second is more architecturally fundamental.

| name | bucket | LOC | notes |
|---|---|---:|---|
| `farblur` | `param_rng` | 213 | `_r[4]` ring buffer; also reads mid-iteration FPx accumulator (see below) |
| `exblur` | `param_rng` | 228 | `_r[4]` ring buffer (same pattern) |
| `nblur` | `param` | 430 | larger, but same architectural blocker |
| `curliecue2` | `rng` | 166 | autonomous trajectory; ignores input point |
| `arctruchet` | `param_rng` | 367 | malloc'd `_tiltArray` of per-thread persistent state plus `PluginVarTerminate` to free it |
| `hexnix3D` | `param` | 247 | `rswtch` / `fcycle` / `bcycle` cycle counters that persist across iterations |
| `hexaplay3D` | `param` | 177 | same persistent cycle counters |

#### Mid-iteration accumulator reads

Most upstream variations only read the input point (`FTx`/`FTy`/`FTz`)
plus the variation's own state. A handful instead read the running
post-variations accumulator (`FPx`/`FPy`/`FPz`) before this variation
adds to it — i.e., they "see" prior variations' contributions in the
same iteration. Our normal-phase calling convention exposes only the
input point, not the running accumulator.

| name | bucket | LOC | notes |
|---|---|---:|---|
| `farblur` | `param_rng` | 213 | reads FPx, FPy, FPz mid-iteration |
| `post_depth` | `param` | 144 | post-phase, reads BOTH pre-affine `FTx` and post-variations `FPx` (already mentioned in batch-24 deferral) |
| `cubicLattice_3D` | `param` | 157 | reads BOTH pre-affine `FTx`/`FTy`/`FTz` and post-variations `FPx`/`FPy`/`FPz` mid-iteration |

#### Z-coordinate clamping / non-linear-weight FPz assignment

A few variations assign `FPz` (rather than `+=`) to a clamped value
or to a non-linear function of weight. Our outer-multiplier
convention can't reproduce a saturating Z without doing the
saturation inside *with the weight already known* — feasible via
`needs_transform`, but the divide-out pattern fights with the
clamping (the clamp is in absolute units, not weight-scaled units).

| name | bucket | LOC | notes |
|---|---|---:|---|
| `flower_db` | `param` | 194 | `FPz = -stem_length` clamp + non-linear weight scaling on Z |
| `popcorn2_3d` | `param` | 115 | listed under assignment-vs-accumulator; same Z-assignment issue |

### Affine-coefficient access watchlist — RESOLVED 2026-04-29

These variations read fields of the XForm's affine matrix directly
(`pXForm.getXYCoeff20()` / `XFORM_COEFF_20` etc.) rather than only the
transformed point.

**Status:** unblocked by the `needs_affine: bool` flag on `VariationDef`,
which threads `xform_id` into the function signature for opted-in
variations so the body can read `transforms[xform_id].a/b/c/d/e/f`.
See `variation-init-dispatch.md` § Affine-access addendum.

| name | bucket | LOC | params | 3d | status |
|---|---|---:|---:|:---:|---|
| `popcorn` | `pure` | 126 | 0 | no | ported on the variation-init-dispatch branch (commit `dd410f9`) |

In the same change, the existing `waves` (a "made up" placeholder that
hardcoded `b/c/e/f = 0.5`) was migrated to its actual Scott Draves
formula reading the affine.

### Porter-omitted params / init-precomputed-fields watchlist

Two related conditions, both flagged here:

1. **Porter-omitted params**: the C++ file declares zero `VAR_REAL`s but
   `PluginVarPrepare` initializes private `_xxx` fields to literal constants
   and `PluginVarCalc` reads them. The porter forgot to expose them as user
   parameters — the original Java has the correct param schema in the
   comment block. Recover from there.

2. **Init-precomputed fields with declared params** (added during porting):
   the file *does* declare params but `PluginVarPrepare` precomputes
   `cos(angle)`, `1/(2·log(base))`, etc. into private fields the body uses.
   We don't have an init hook, so we either (a) inline the precompute
   per-iteration (cheap), or (b) extend `VariationDef` with an init step.
   Several already ported (e.g. `log_apo`, `log_db`) inline successfully;
   listed below are ones not yet ported because the precompute uses fields
   not exposed as user params at all (`_t_size_2`, `cosa/sina/cosb/sinb`)
   and the formulas need Java recovery.

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
| ~~`target`~~ | `param` | 193 | 1 init | no | **PORTED batch 11** via the new `wgsl_init` GPU dispatch (`_t_size_2 = size/2`). |
| `wdisc` | `pure` | 137 | 2 | no | 2 porter-omitted params (recover from Java) |
| ~~`yin_yang`~~ | `param_rng` | 102 | 4 init | no | **PORTED batch 11** via `wgsl_init` (precomputes `sin/cos(π·ang1)`, `sin/cos(π·ang2)`). |

### Anomalies

None — all 415 candidate names matched a `.cpp` file in the upstream `output/` directory (case-insensitive).

### Notes & caveats

- **Filename case mismatch**: many upstream files use mixed case (`affine3D.cpp`, `julia3Dq.cpp`, `popcorn2_3D.cpp`) while our candidate list is lowercase. Matched case-insensitively.
- **`unportable_dc` is soft**: 16 entries write to `TC` (transform color), but for several (`truchet`, `mandala`, `mandala2`, `triantruchet`) the geometric component is a normal IFS and could be ported by skipping the color line. Worth a manual pass.
- **`unported_stub` (35)** is the largest non-portable bucket but it's not a hard wall: the C++ ports just left these blank. The full Java implementation lives in the comment block at the bottom of each file, so they're translatable by hand. Includes some interesting standalone shapes: `complex` (64 params!), `ducks`, `eclipse`, `glynnsshape`, `glynnspiro`, `glynnlissa`, `recurrenceplot`. `complex`, `ducks`, and the `glynn*` ones likely use Janino runtime-compiled Java in the original — flag during port.
- **`prepost_*` variations** (3 entries: `prepost_affine`, `prepost_circlize`, `prepost_mobius`) MUTATE the input `FTx`/`FTy`/`FTz` and then write `FPx`/`FPy`/`FPz`. They're effectively two-stage (pre then post) collapsed into one variation. Need careful porting; they don't fit the single-`Phase` model cleanly.
- **Existing-name overlap**: `cpow`, `julia3d`, `post_curl`, etc. don't appear in the candidate list because they're already implemented. `cpow2`/`cpow3` (parameterized variants) are in the list as `param_rng`.