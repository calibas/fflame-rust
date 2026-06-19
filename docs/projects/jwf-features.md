# JWildfire-specific Features (non-variation)

Companion to [`jwf-common-variations-port.md`](jwf-common-variations-port.md) (variations) and [`variation-port-blockers.md`](variation-port-blockers.md) (blocked variations). This doc tracks JWildfire features that are **not variations** — XML attributes on `<flame>` / `<xform>`, per-transform settings, plot-time mechanisms, etc. — that affect how a JWF flame renders but aren't yet wired up in our import or pipeline.

Same shape as [`apophysis-remaining-features.md`](apophysis-remaining-features.md) but for JWildfire's extensions to the Apophysis baseline.

Each feature listed below has a status, what it does in JWildfire, what it would take to implement, and how its absence currently manifests when importing a JWF flame.

## Shipped

### `zxCoefs` / `yzCoefs` — per-transform 3D-plane affines

**Shipped in [PR #97](https://github.com/calibas/fflame-rust/pull/97)** (commit range `176a0f8..843c1e4` on `main`). Closes what was previously the largest gap in JWF 3D-flame interop.

**What it does**: JWildfire extends Apophysis's XY affine (`coefs`) with per-xform 2D affines on the YZ and ZX planes (`yzCoefs`, `zxCoefs`), plus their post-affine siblings (`yzPost`, `zxPost`). Together they cover all 12 coefficients of a 3D affine, applied via `TransformationAffineFullStep`'s composition: sequential 2×2 linear application in fixed XY → YZ → ZX order, then **all six raw offsets summed at the end without rotation by subsequent matrices**. The offset-decoupling is JWildfire-specific — not "true" affine composition where offsets would propagate through later rotations. We mirror their math exactly.

**Composition rule** (transcribed from [`TransformationAffineFullStep.java`](../../output/TransformationAffineFullStep.java)):

```java
if (xform.hasXYCoeffs) { x = xy00·sx + xy10·sy; y = xy01·sx + xy11·sy; z = sz; }
if (xform.hasYZCoeffs) { ny = yz00·y + yz10·z; nz = yz01·y + yz11·z; y = ny; z = nz; }
if (xform.hasZXCoeffs) { nx = zx00·x + zx10·z; nz = zx01·x + zx11·z; x = nx; z = nz; }
pAffineT.x = x + xy20 + zx20;
pAffineT.y = y + xy21 + yz20;
pAffineT.z = z + yz21 + zx21;
```

**Coefficient indexing** (`xyCoeffIJ` where `I` = input axis, `J` = output axis, `2` = constant). Six floats per attribute in XML position order `00 01 10 11 20 21`. Mapping into the 12-coef 3D affine model:

| Attribute | XML positions yield |
|---|---|
| `coefs` (Apo XY) | XX, XY, YX, YY, XO, YO |
| `zxCoefs` | XX, XZ, ZX, ZZ, XO, ZO |
| `yzCoefs` | YY, YZ, ZY, ZZ, YO, ZO |

Standard `coefs` is required; the JWF planes are conditionally present (JWildfire only writes them when non-identity, and we do too).

**Implementation pointers**:

- Storage: four `[f32; 6]` arrays on `Transform` — `yz_coefs`, `zx_coefs`, `yz_post_coefs`, `zx_post_coefs`. No `has_*` flags on the struct; identity comparison via `is_*_identity()` helpers stands in for them (and matches JWildfire's "write only when non-identity" XML output naturally). See [`src/scene/transforms.rs`](../../src/scene/transforms.rs).
- GPU: `GpuTransform.plane_flags: u32` packs the four "non-identity" bits, computed host-side on upload. The WGSL `apply_affine` / `apply_post_affine` in [`shaders/core/affine_3d.wgsl`](../../shaders/core/affine_3d.wgsl) have two paths — "Flat" (flags == 0, byte-identical to pre-extension Apo math) and "Full" (any flag set, transcribes `TransformationAffineFullStep`).
- UI: four collapsible sections per transform (`render_jwf_plane_sections` in [`src/ui/transforms.rs`](../../src/ui/transforms.rs)). Works for all three transform pools (Normal / Linked / Final). Post sections hide when the XY post-affine is disabled.
- XML round-trip: `parse_xform_element` + `write_xform` in [`src/flame_xml.rs`](../../src/flame_xml.rs); coverage in `test_jwf_plane_affines_roundtrip`.

**One documented caveat**: in Full mode the Apo `g` (Z offset) is silently dropped — JWildfire has no equivalent field, so we follow their semantics. A flame mixing non-zero `g` with active plane affines won't see `g`. Acceptable because flames that opt into the JWF plane affines are explicitly the JWF-extension path; Apo flames stay on the flat path where `g` keeps working.

**Originally discovered in**: `output/JWF-rando22.flame` ("Brokat3D" random preset). At 90° pitch, JWildfire showed a tall 3D structure with two cone-like spires; our app showed a flat horizontal line because we silently dropped `zxCoefs`. Post-fix the structure matches.

**Still deferred** (split out of this entry — not regressions, just scope cuts):

- **Plane selector in the triangle editor.** Currently the new plane affines are edited via the numeric DragValue inputs in the four collapsible sections. JWildfire lets users switch the triangle editor between editing the XY/YZ/ZX plane. Worth doing once we have user feedback on the numeric-only UX. Track here.
- **True 12-coefficient 3D affine mode.** Discussed in [`../experimental/PROPOSAL-true-3d-affine.md`](../experimental/PROPOSAL-true-3d-affine.md). Not the same as JWildfire's three-plane decomposition — would have offsets-rotate-with-matrix semantics where JWF has offsets-summed-raw. JWF's approach has the same *linear* expressive power (three plane rotations compose to any SO(3) rotation), so the practical visual difference is small. Deferred indefinitely pending a concrete use case that the JWF path can't express.

### `symmetry` XML attribute — `color_speed` name correction

**Shipped in [PR #97](https://github.com/calibas/fflame-rust/pull/97)** (commit `843c1e4`). One-line fix tagged onto the 3D affine PR.

Apophysis and JWildfire both store per-transform color speed under the XML attribute name `symmetry` despite their internal field being `colorSpeed`. Our importer already accepted both spellings; the exporter was emitting `color_speed=`, which Apo and JWF then showed as an unknown attribute. Now we write `symmetry=` like everyone else. Coverage in `test_color_speed_exports_as_symmetry`.

### `size` — saved canvas dimensions (partial implementation)

**Partially shipped** in commit `032755f`. Data plumbing + Export PNG pre-fill are done; the visual zoom mismatch on non-1920×1080 flames is **not** fixed yet — see "Still deferred" below.

**What ships in this round**:

- `FractalConfig::image_size: (u32, u32)` field with default `(1920, 1080)` ([`src/config/fractal_config.rs`](../../src/config/fractal_config.rs)). Skip-serialized when at default so existing `.fflame` JSON files stay unchanged.
- XML import: `size` lands on `image_size` instead of being discarded.
- XML export: `image_size` replaces the hardcoded `(1920, 1080)` write — flames now round-trip their authored dimensions through JWF / Apo.
- Export PNG dialog pre-fill: `load_config_with_undo` pushes `image_size` into `SystemExportWidth` / `SystemExportHeight`, so opening a portrait flame no longer leaves the previous flame's pref in the Custom Export Size inputs.
- Coverage in `test_image_size_roundtrip`.

**Still deferred** (the visual issue the entry was originally written about):

- **Zoom magnification mismatch.** Our `zoom` is a pure viewport multiplier with no canvas-extent term. JWildfire / Apophysis define their visible fractal area as `size / (scale × cam_zoom)`. A JWF flame saved at `size="638 359"` renders at ~3× the magnification ours does because our import formula `zoom = (scale / 200) × cam_zoom` implicitly assumes a 1920-wide canvas. The shipped field lets us *track* the saved size; the compensation math hasn't been wired into the import/export zoom formulas yet. Visible on JWF "random" presets that save at small preview sizes (`192×108`, `638×359`); Apo flames usually save at `1500×1000` which lands close enough to our reference that the divergence isn't usually noticeable.
- **Which dimension drives the zoom formula** — width, height, `min(saved)`, or `max(saved)`. Needs a non-uniform stretch test (same flame saved at `size="638 718"` vs `size="1276 359"`) to pin down. Three data points distinguish the candidates; we have one pair from uniform-scale captures that doesn't.
- **The `200` divisor** in our import formula was empirically tuned by eyeballing — possibly a `size.x / ~10` ratio at the 1920 default, but not verified against any Apo or JWF constant. May become re-derivable from `image_size` once the formula above is nailed down.
- **Viewport aspect ratio**: when importing a flame, optionally adjust our viewport's aspect ratio to match `image_size` so flames saved at non-standard ratios don't letterbox.
- **UI for editing**: a "Source / Canvas" advanced panel would let power users who care about JWF round-trip adjust `image_size` directly. Today the only way to change it is to re-import a JWF flame with the new value.

The deferred items are loosely coupled — the zoom math is the heaviest single piece and the most user-visible improvement. Pre-fill alone is a real UX win on its own merit, hence the partial-ship.

**Originally discovered in**: `output/JWF-rando1.flame` and a side-by-side recreation comparison via `output/JWF1a.flame` / `output/JWF1b.flame` (identical fractal, only difference is `size="638 359"` vs `size="1920 1080"`).

### Camera rotation — Apophysis/JWildfire 4-angle camera matrix

**Shipped on the `camera-bank-and-matrix-port` branch** (PR pending). Closes the rotation half of the Apo/JWF camera-parity work.

**What it does** (the 4-angle model):

| Apo/JWF angle | Rotates around | Visual effect (camera default) |
|---|---|---|
| Pitch | X | Tilts camera elevation up/down |
| Yaw | Z (world-up) | Pans heading left/right |
| Roll | Z (look-axis when pitch=yaw=0) | Twists view around look direction |
| Bank | Y | Tilts camera, creating a perspective skew |

**XML attribute rename quirks** (JWildfire's serialization names don't match parameter names — discovered from `output/FlameRendererView.java`):

| JWF internal field | XML attribute | XML unit |
|---|---|---|
| `pitch` | `cam_pitch` | radians |
| `yaw` | `cam_yaw` | radians |
| `bank` | **`cam_roll`** ← rename quirk | radians |
| `roll` | **`rotate`** | **degrees** |

**Pieces shipped**:

- Bank field on `FractalConfig` + JSON serde + XML round-trip via `cam_roll`
- 4-angle `build_camera_matrix(yaw, pitch, bank, roll)` ported from JWildfire's `createProjectionMatrix` in `output/FlameRendererView.java`
- `camera_transform` uses all 9 matrix elements (was dropping one term in the 2-angle approximation)
- Empirical convention mapping at the call site: yaw↔roll slot swap + per-axis sign tuning so each slider direction matches JWildfire and our pre-branch app. The matrix function stays a verbatim JWildfire transcription; the call site documents the convention diff for future debugging.
- Bank UI slider in the View panel + ConfigPath wiring + animation target

**Caveats**:

- Pure JWF/Apo camera math reproduction would land all four slider directions naturally. We needed empirical sign tuning to get it right — JWildfire almost certainly applies their matrix with a different convention internally (M^T·v or different basis vectors). The tuning is documented at `project_3d_to_2d_apophysis` in `shaders/core/utilities.wgsl`.
- The 2D `rotation` field doubles as the 3D `roll` angle in our model (mirroring JWildfire's `rotate` ↔ `roll` mapping). 2D mode still applies `rotation` as a post-projection screen rotation since there's no camera matrix there.

**Stage 2 — free camera movement**. With rotation done, the natural next project is FPS-style free-fly: WASD + mouse-look + Q/E for up/down. See [`free-camera-movement.md`](free-camera-movement.md) for the full plan.

## Deferred (not urgent)

### `<var>_fx_priority` — per-variation priority / phase ordering

**Status**: Not implemented. Silently ignored on import (we don't parse
`*_fx_priority` at all — confirmed by grep over `src/flame_xml.rs`).

**JWF XML attribute** (per variation, e.g. `combimirror_fx_priority="-1"`):
an integer that controls the order variations are applied *within* a
single transform. JWildfire sorts a transform's variations by priority
into phases:

- **priority < 0 ("pre")** — applied first, sequentially: each pre
  variation transforms the working point and its output feeds the next
  step (a chain, not a sum).
- **priority 0 ("normal")** — the standard flam3 behavior: each is
  evaluated on the affine point and the results are *summed*.
- **priority > 0 ("post")** — applied last, sequentially, on the summed
  normal result.

We model phase **per variation *definition*** (`VariationPhase::Pre /
Normal / Post`) but not **per *instance***. So a flame that puts a
normally-`Normal` variation at priority −1 (or a normally-`Pre`
variation at 0) is mis-ordered on import.

**How it surfaced** (`output/JWF-rando7.flame`, 2026-06-18): the flame
has `combimirror_fx_priority="-1"` — combimirror runs as a *pre* chain
step in JWF, but we summed it as a normal variation. combimirror is
replace-style with a tiny weight (0.018), so running it in the wrong
phase changed the spatial composition, which in palette mode shows up
as a **color** difference (points land in different palette regions —
combimirror's own `*colorshift` params are all 0 here, so this is *not*
a direct-color bug; the variation's color write was verified faithful
to `CombimirrorFunc.java` and is a no-op at shift 0). Re-exporting
through our app drops `fx_priority`, so JWF then reproduces our
normal-phase interpretation and the two match — which is exactly why
`JWF-rando7-reexported2.flame` matches but the original doesn't.

Note `pre_blur_fx_priority="-1"` in the same flame is *not* a problem:
`pre_blur`'s definition phase is already `Pre`, so it coincidentally
lands in the right phase.

**Verified JWF dispatch model** (from `XForm.createTransformations` +
the `*VariationTransformationStep` classes, fetched 2026-06-18):

Every variation step calls `variation.transform(ctx, xform, inPt, outPt)`;
the variation reads `inPt` and accumulates/replaces into `outPt`. By
convention normal-style variations write `outPt`; some pre-style ones
(e.g. `pre_blur`) write `inPt` directly. The phase is controlled by
*which points are passed*:

| step | call | meaning |
|---|---|---|
| `VariationTransformationStep` (normal) | `transform(pAffineT, pVarT)` | read affine, **sum** into accumulator |
| `PreVariationTransformationStep` | `transform(pAffineT, pVarT)`; `pAffineT.invalidate()` | natural-pre vars write `pAffineT` → perturb the affine point; pre vars **chain** |
| `PostVariationTransformationStep` | `transform(pAffineT, pVarT)` | runs after normal, on `pVarT` |
| `EnforcedPreVariationTransformationStep` | `tmp = pAffineT; transform(tmp, pAffineT)` | the var's output lands in `pAffineT` → a normal var's write acts *pre* |
| `EnforcedVariationTransformationStep` (normal) | `tmp = pVarT; transform(tmp, pVarT)` | input is the accumulator snapshot |
| `EnforcedPostVariationTransformationStep` | `tmp = pVarT; transform(tmp, pVarT)` | same, after normal |

Bucketing is by instance-priority **sign**: `< 0` pre, `== 0` normal,
`> 0` post. The special values `== 2` (→ pre) and `== -2` (→ post) are
the "inv" prepost family (`pre_*`/`post_*` inverse pairs) — out of scope
here. Within a bucket: **pre/post chain** (each reads the working point
as updated by the previous step), **normal sums** (all read the same
affine input). That matches our existing model (`temp = f(temp)` for
pre/post, `result += w·f(temp)` for normal).

The `Enforced*` step (used when the instance bucket differs from the
variation's *own* default priority) is just an argument remap, and it
maps cleanly onto our dispatch — no per-variation body changes needed.
**Confirmed**: forcing combimirror (normal-default) to pre via
`temp = w · combimirror_body(temp)` (the existing idisc body, dispatch
supplies `w`, the idisc divide cancels) makes `output/JWF-rando7.flame`
match JWF (user-verified A/B, 2026-06-18).

**Key insight — the accumulate emission is uniform.** Reusing a variation's
existing (normal) body in the pre bucket needs *no per-variation work* for
the common case, because JWF's `EnforcedPre` for an **accumulate** variation
is `pAffineT += w·f(affineCopy)` — it *adds* the weighted contribution to the
working point, which is exactly what `pre_blur` does. So for any accumulate
variation the pre emission is the single shared form `temp = temp + w·body(temp)`.
Verified: `blur → pre` = `temp + w·(random disc)` (a blur perturbation =
pre_blur); `linear → pre` JWF gives `(1+w)·affine`, ours `temp + w·linear(temp)`
= `(1+w)·temp`. So the *hundreds* of accumulate variations are movable for
free; only the replace-style minority needs a different emission. (Confirmed
the inverse too: you can't fold both into one emission — making the
accumulate form correct for combimirror would break its normal-phase result.)

**Design — two orthogonal axes, both reusing existing concepts (no lock flag):**

1. **`VariationPhase::Any`** — *where it can run*. Opt-in movability. A
   variation tagged `Any` honors `fx_priority`; `Pre`/`Normal`/`Post`
   stay **locked** (priority ignored). "Locked" is just "not `Any`" — no
   separate flag. An `Any` variation defaults to the normal bucket when
   `fx_priority` is absent.
2. **`Feature::Replace`** (new) — *how it combines its output*: the
   variation assigns the point (`pVarTP.x = …`) instead of accumulating
   (`+=`). Intrinsic and phase-independent; lives as a feature, not in
   the phase enum. Only affects the pre/post emission (assign vs add);
   `Normal` is unchanged. Sourced from the JWF `=` vs `+=` classification
   (≈622 accumulate / 133 replace / 19 mixed across local sources;
   confirm against our port's idisc usage).

Dispatch, reading the two axes independently:

| bucket | accumulate (default) | `Feature::Replace` |
|---|---|---|
| normal | `result += w·body(temp)` | `result += w·body(temp)` (idisc → replace-when-sole; unchanged from today) |
| pre | `temp = temp + w·body(temp)` | `temp = w·body(temp)` |
| post | `result = result + w·body(result)` | `result = w·body(result)` |

Normal is identical for both axes, so nothing about current renders
changes — the combine-mode only kicks in when a variation is actually
*moved*. **Multiple variations at the same non-default priority** falls
out for free: emit the bucket's vars in order, each updating `temp` (pre)
or `result` (post) — JWF re-snapshots each Enforced step, i.e. chains,
which is what sequential emission does. **Pre-default var pushed to
normal** (`EnforcedNormal`) is the degenerate case — JWF passes the
accumulator snapshot as the *input*, and pre-style bodies write their
input arg, so e.g. `pre_blur` in normal is a **no-op** in JWF; we get the
same by leaving naturally-`Pre`/`Post` variations locked (not `Any`).

**Normal-phase replace (implemented).** `Feature::Replace` is honored in
the **normal** phase too, not just when a variation is moved to pre/post.
`resolve_phase_buckets` emits the normal bucket as *accumulate variations
first* (`result += w·body`) *then replace variations last* (`result =
w·body`), so a replace variation overwrites the running sum — matching
JWF, where `pVarTP.x =` clobbers prior `+=` contributions. A **sole**
replace variation is unchanged (`result += w·body` from `result = 0`
equals `result = w·body`), so single-variation flames render identically;
only multi-variation normal xforms change.

Caveat — *order*: every normal variation reads the fixed affine input, so
accumulate variations commute (order-independent). A replace, though,
discards everything emitted before it, so JWF's result is *(last replace)
+ (accumulates after it)*. Our `Transform.variations` is an unordered
map, so we can't reconstruct JWF's intra-xform order; we apply replaces
last. This is exact for the common "one replace, added last" case (e.g.
shredlin in JWF-rando14, where it clobbers `poincare3D` — confirmed
matching JWF) but won't match a flame that deliberately places an
accumulate *after* a replace. (`NeedsAccum` variations, which read the
running sum, are excluded from `Any` and are the only order-sensitive
accumulates.)

**Storage model (confirmed):** a third name-keyed map on `Transform`,
parallel to `variations`/`variation_params`:
```rust
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub variation_priorities: HashMap<String, i32>,   // canonical name -> JWF priority
```
- **Sparse override:** an entry exists only when the instance priority
  *differs* from the variation def's default-phase priority
  (`Pre`→−1, `Normal`→0, `Post`→1). Plain normal vars store nothing;
  most transforms keep an empty map.
- **`i32`** holds the raw JWF priority (preserves the `±2` "inv" specials);
  dispatch buckets by sign at build time.
- Keyed by **canonical** name (same alias handling as `variations`);
  one priority per variation name per transform (matches the existing
  one-weight-per-name model — same pre-existing dup-name limitation).
- `serde(default, skip_serializing_if = empty)` ⇒ old `.fflame` files
  load unchanged; new files only write the field when overrides exist.

**Implementation steps:**
1. Add `variation_priorities` to `Transform` (field above) + the
   constructors/`Default`/custom deserializer; `flame_xml`
   parse/round-trip `<var>_fx_priority` (store only when ≠ def default).
2. Add `VariationPhase::Any` and `Feature::Replace`.
3. Shader builder: for `Any` variations, bucket by instance priority sign
   and emit per the table; everything else unchanged.
4. Migrate (`scripts/migrate_fx_priority_phases.py`, dry-run by default,
   `--apply` to write; re-runnable). Two independent passes over the def
   files:
   - **Pass 1 (`Any`)** — set `VariationPhase::Any` on every `Normal`
     variation that is **mechanically safe to move**:
     `¬NeedsAccum ∧ state_count == 0`. `NeedsAccum` *must* be excluded —
     its function signature carries an `accum` arg the pre emission
     doesn't pass (a moved NeedsAccum var would fail to compile); stateful
     vars are excluded for safety. Everything excluded stays `Normal`
     (locked) and lands in the printed **review queue**. (The earlier
     "must have local JWF source" gate is dropped — source availability
     governs the *Replace* classification below, not movability; a clean
     accumulate `Normal` var moves faithfully whether or not we have its
     Java. Best-effort + A/B against JWF as users actually move them; a
     no-op until a flame sets `fx_priority`.)
   - **Pass 2 (`Replace`)** — classify each variation's local JWF source
     by `pVarTP.{x,y,z}` write op: all-`=` ⇒ replace ⇒ add
     `Feature::Replace`; all-`+=` ⇒ accumulate (nothing); mixed/no-source
     ⇒ review queue (left accumulate). `Replace` is inert in the normal
     phase, so a misclassification only affects a variation once it's
     actually moved (then it's A/B-visible). combimirror is already done
     by hand as the reference.
5. Tests: rando7 (combimirror → pre), a synthetic accumulate→pre
   (blur ≈ pre_blur), and a 2-var-same-priority chain.

**Migration applied** (`scripts/migrate_fx_priority_phases.py --apply`):
- **509 variations → `Any`** (movable). combimirror is hand-done
  (reference); the other 508 by Pass 1.
- **17 variations → `Feature::Replace`**: anamorphcyl, combimirror, crop,
  crop3D, ennepers, hyperbolicellipse, hypershift2, iconattractor_js,
  mobius_dragon_3D, rays1/2/3, ripple, shredlin, sintrange, squirrel,
  svensson_js, tile_reverse.
- **Pass 1 review queue (stay `Normal`, locked — not movable):**
  - `NeedsAccum` (11): crown_js, cubic3D, cubicLattice_3D, farblur,
    hexaplay3D, hexnix3D, lorenz_js, macmillan, octapol, roundspher3D,
    scry_3D.
  - stateful `state_count > 0` (4): curliecue2, klein_group, mandelbrot,
    subflame_wf.
- **Pass 2 review queue (stay accumulate when moved):** mixed JWF source
  (circlecrop, julia3D, julia3Dz, rhodonea, spherecrop, synth,
  wallpaper_js — note julian/juliascope correctly classify as accumulate:
  `pVarTP.x = pVarTP.x + …`); no local JWF source (CircleTrans1, arcsech,
  bwraps, crackle, exp2, parplot2d_wf, polarplot2d_wf, polarplot3d_wf,
  yplot2d_wf, yplot3d_wf). Revisit these if a user reports a moved-phase
  mismatch vs JWF.
- The bulk `Any` flip is a verified **codegen no-op** for existing
  renders (an `Any` var with no `fx_priority` override resolves to the
  normal bucket with byte-identical emission; covered by a test).

### Per-transform `color_type` — color-flow mode selector

**Status**: Not implemented. Silently ignored on import. We always use Apophysis-standard `DIFFUSION` semantics for every transform regardless of what the source XML says.

**JWF XML attribute** (on each `<xform>` and `<finalxform>`): `color_type="..."`. Six known values:

- **`DIFFUSION`** — Apophysis-standard color flow: lerp the running color toward the transform's `color` value via `color_speed`. This is what we already do for every transform; matching this case is "free."
- **`TARGET`** — Lerp toward a specific target color (semantics need JWF source confirmation; likely uses `color` as the target with different blending math).
- **`TARGETG`** — `TARGET` variant, possibly gradient-aware. Needs source.
- **`DISTANCE`** — Color derived from a distance metric (possibly distance from the affine input to the variation output, or distance from the origin). Needs source.
- **`CYCLIC`** — Cyclic palette traversal, the color register increments rather than lerps. Needs source.
- **`NONE`** — Skip the color update step entirely; running color is unchanged after the transform. Cheapest to implement (just gate the existing color-flow block on `color_type != NONE` per transform).

**Why we don't have it**: JWildfire-specific extension to Apophysis's color flow. We support only `DIFFUSION`.

**Impact when a JWF flame uses it**: Narrower than it first appears, because our final-chain plot-time filter already discards any color writes from final transforms (see [`shaders/core/main_template.wgsl`](../../shaders/core/main_template.wgsl) ≈line 250 — `var final_vc: f32 = color_index; // discarded after the call`, and there's no `color_speed` lerp on the final chain either). That gives us `color_type="NONE"` semantics on **every** final transform by default, which happens to match what `output/JWF-rando1.flame` requests on its two finals.

The actual gaps are:

- A non-`NONE` `color_type` on a `<finalxform>` (e.g., `DIFFUSION` — JWF applies color flow on the final, we never do).
- A `NONE` `color_type` on a *normal* `<xform>` (JWF skips color flow on it, we always apply Step 1 / Step 3 of the DC blend).
- Non-`DIFFUSION` modes on either (`TARGET`, `TARGETG`, `DISTANCE`, `CYCLIC` — different per-iter color math we don't implement at all).

**What it would take**: A `ColorType` enum on the GPU `Transform` struct (or just a `u32` mode tag), then per-mode branches in the color-flow block of `main_template.wgsl` — for normal transforms only, since the final-chain path is already color-noop. `NONE`-on-normal is the simplest start: gate the existing color block with `if (xform.color_type != NONE)`. The other four modes need JWF source investigation before we can implement them faithfully.

### Per-transform `mod_*` color modulators

**Status**: Not implemented. Silently ignored on import.

**JWF XML attributes** (on each `<xform>` and `<finalxform>`), each as a pair:

- `mod_gamma` + `mod_gamma_speed`
- `mod_contrast` + `mod_contrast_speed`
- `mod_saturation` + `mod_saturation_speed`
- `mod_hue` + `mod_hue_speed`

Defaults are all `0.0` — they're opt-in modulators.

**What they do in JWildfire**: Each transform can apply its own gamma / contrast / saturation / hue shift to the per-iteration color contribution before histogram accumulation. The `*_speed` companion modulates *how fast* the shift transitions in (likely the same shape as Apophysis's `color_speed` — a lerp rate against the running mod state). Effect: certain transforms become local "warm" / "cool" / "high-contrast" zones inside the flame's color flow, regardless of palette.

**Why we don't have it**: JWildfire-specific. Our pipeline applies gamma / contrast / saturation only at tonemap time (global, per-frame), not per-iteration / per-transform.

**Impact when a JWF flame uses it**: Subtle on flames where the source author kept these at `0.0` (the default for all 5 xforms in `output/JWF-rando1.flame`). On flames that use them, certain regions of the rendered image will have flatter / less-shifted color than the JWildfire reference.

**What it would take**: 8 floats per transform (4 mod values + 4 speeds), plumbed through GPU `Transform`, then a per-iteration color post-processing step before the histogram write. HSV → RGB conversion shaders we already have (used in some `glsl_*` variations). The modulation math itself is straightforward; the architectural piece is moving gamma/contrast/etc. from tonemap-time to per-iteration. Real work.

### `material` / `material_speed` — JWF solid-render material

**Status**: Not implemented. Silently ignored on import.

**JWF XML attributes**: `material="..."` + `material_speed="..."` on each `<xform>` and `<finalxform>`. Default `0.0`.

**What it does in JWildfire**: Picks a material from JWF's solid-rendering Tina mode (the raytraced 3D mode that's separate from the flame renderer). When JWF renders the flame as a 2D fractal image, `material` is typically inert. When JWF renders the same scene as a 3D solid through Tina, the material's reflectivity / refractivity / texture come into play.

**Why we don't have it**: We don't have a Tina-equivalent solid-render mode and probably never will — it's a different rendering paradigm entirely, not a flame extension.

**Impact when a JWF flame uses it**: Zero on flame rendering (matches JWF's 2D-flame mode). Would matter only if we ever added a solid-render pipeline (no plans).

### `wfield_*` — per-transform weighted noise field

**Status**: Not implemented. Silently ignored on import.

**JWF XML attributes** (on `<xform>` elements):
- `wfield_type` — `OFF` / `CELLULAR_NOISE` / others. Picks the noise source.
- `wfield_input` — `AFFINE` / others. Where the noise is sampled.
- `wfield_color_intensity`, `wfield_var_amount_intensity`, `wfield_var_param1_intensity` (and `param2`, `param3`), `wfield_jitter_intensity` — modulation strength per channel.
- `wfield_noise_seed`, `wfield_noise_frequency` — noise generator config.
- `wfield_cell_noise_return_type` (e.g. `DISTANCE2`), `wfield_cell_noise_dist_function` (e.g. `EUCLIDIAN`) — cellular-noise-specific knobs.

**What it does in JWildfire**: A noise field (cellular, perlin, etc.) is sampled at the transform's input coordinates and used to **modulate** that transform's variation amounts, variation parameter values, color contribution, and/or position jitter. Each `_intensity` knob controls how strongly the field affects that channel. Result: each chaos-game iteration that picks this transform gets slightly different variation behavior depending on where it hits in noise-space, producing organic textures and gradients that vary smoothly across the flame's shape.

**Why we don't have it**: JWildfire-specific. Hasn't been seen in Apophysis, Chaotica, or any other flame renderer the user is aware of. The mechanism touches the variation dispatch directly (modulating amounts and params per iteration), so it can't be modeled by a variation — it'd need a framework-level hook on the variation call site.

**Impact when a JWF flame uses it**: The variations themselves still run, but the per-iteration noise modulation that would shape them is absent. Rendering will differ from JWildfire — typically less textured / more uniform than the source.

**What it would take**: Per-transform noise-field metadata on the GPU `Transform`, a sample step at the variation dispatch site that reads the field at the affine-input coordinates, and per-channel modulation hooks for variation amount, variation params, color, and position. Probably also new shader helpers for cellular and perlin noise (we already have some via `crackle` and `dc_perlin` — see [`shaders/core/noise.wgsl`](../../shaders/core/noise.wgsl) and [`shaders/core/voronoi.wgsl`](../../shaders/core/voronoi.wgsl)). The XML importer side is straightforward (collect the attributes onto the `Transform`); the runtime side is the bulk of the work.

**Discovered in**: `output/JWF-rando1.flame` ("Orchids" random preset) uses `wfield_type="CELLULAR_NOISE"` on xform #3.

## Related docs

- [`jwf-common-variations-port.md`](jwf-common-variations-port.md) — JWF "script vars" variation subset (188/190 implemented).
- [`variation-port-blockers.md`](variation-port-blockers.md) — Per-variation porting blockers + framework features needed to unblock them.
- [`apophysis-remaining-features.md`](apophysis-remaining-features.md) — Apophysis-baseline features (XAOS, 3D camera, etc.); same shape as this doc but for the Apo side.
