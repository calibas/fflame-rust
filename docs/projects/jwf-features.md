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

## Deferred (not urgent)

### Camera rotation — Apophysis/JWildfire `bank` + matrix port

**Status**: Three of four Apo/JWF camera-rotation angles are wired up; the fourth (`bank`) is missing. The matrix function we use is also a slight approximation of JWildfire's — the same shape at small angles, diverges at extremes. Apophysis interop is *mostly* correct; full parity is a small follow-up.

**The 4-angle model**. Apophysis and JWildfire both treat the camera as a 4-Euler-angle system, though only 3 of the angles are strictly necessary to span SO(3). The redundancy is historical / animation-friendly. Mapping to axes (verified by experiment on a flat 2D image):

| Apo/JWF angle | Rotates around | Visual effect (camera default) |
|---|---|---|
| Pitch | X | Tilts camera elevation up/down |
| Yaw | Z (world-up) | Pans heading left/right |
| Roll | Z (look-axis when pitch=yaw=0) | Twists view around look direction |
| Bank | Y | Tilts camera, creating a perspective skew |

**XML attribute quirks**. JWildfire's serialization names don't match the parameter names:

| JWF internal field | XML attribute | XML unit | Internal unit |
|---|---|---|---|
| `pitch` | `cam_pitch` | radians | degrees |
| `yaw` | `cam_yaw` | radians | degrees |
| `bank` | **`cam_roll`** ← rename quirk | radians | degrees |
| `roll` | **`rotate`** | **degrees** | degrees |

Two specific traps in here:

1. The XML attribute named `cam_roll` actually carries the `bank` parameter. JWF's internal `roll` field is serialized as `rotate` instead.
2. All four `cam_*` attributes are radians, but `rotate` is degrees. Verified by reading JWildfire's `XMLFlameWriter`: it does `pFlame.getCamPitch() * Math.PI / 180.0` (internal degrees → XML radians) for the `cam_*` set, and writes `rotate` directly without conversion.

**What we have**:

- `camera_rotation_x` ← XML `cam_pitch` (radians) ✓
- `camera_rotation_y` ← XML `cam_yaw` (radians) ✓
- `rotation` ← XML `rotate` (degrees, converted to radians on import) ✓ — semantically the same as JWildfire's `roll`
- Camera matrix function `build_camera_matrix(pitch, yaw)` — only handles 2 angles

**What's missing**:

1. **Bank field.** Need a `camera_bank: f32` (radians, default 0) on `FractalConfig`. Round-trips via XML `cam_roll` (per JWF's rename quirk). None of our JWF sample files have a non-zero `cam_roll` so the practical impact today is small, but full Apo round-trip needs it.

2. **Matrix function port.** Replace `build_camera_matrix(pitch, yaw)` with the 4-angle function transcribed verbatim from JWildfire's `FlameRendererView.createProjectionMatrix(yaw, pitch, bank, roll)` (see `output/FlameRendererView.java`). Our current shader builds an approximation that's *the transpose* of JWildfire's matrix and is missing one term. At small angles the divergence is invisible (~`sin(pitch)·sin(yaw)` for the missing x-z coupling); at high pitch+yaw it's substantial. Pure pitch and pure yaw render the same as JWildfire (user-confirmed by 90° experiment); combined pitch+yaw diverge mildly.

3. **`camera_transform` 9-term fix.** The shader's `camera_transform` function drops the `m[2][0]·z_translated` term entirely because for the current matrix's specific shape that element is always zero. After the matrix port, that term will be non-zero. The function needs to use all 9 matrix elements.

4. **Roll composition order.** Our existing `rotation` is applied as a 2D rotation *after* projection. JWildfire applies its `roll` (= our `rotation`) *inside* the 3D matrix. For pure roll with no other camera rotation, these are equivalent (rotating around the look axis = rotating the projected image). With pitch and/or yaw active, the order matters: JWildfire's order produces a different final orientation than ours. In 3D mode, the `rotation` field should be routed into the new matrix function as the `roll` argument instead of post-projection. In 2D mode, keep it as 2D post-projection (no camera matrix there).

5. **UI**. Add a "Bank" slider next to existing Pitch/Yaw in 3D mode. The existing "Rotation" slider stays — it's still the Z-axis / look-axis twist, just routed into the matrix now in 3D mode.

**Behavior change for existing flames**. After the matrix port:
- 2D mode: unchanged
- 3D mode flames with `rotation = 0`: unchanged
- 3D mode flames with non-zero `rotation` AND non-zero pitch or yaw: rendering shifts slightly to match JWildfire / Apophysis. One-time correction toward parity. Vast majority of our existing flames have `rotation = 0` so this is rarely visible.

**Experimental verification** (user-tested on a flat 2D image, both apps):

| Test | Our app | JWF |
|---|---|---|
| Pitch=0, Yaw=90 | rotates CCW | rotates CCW ✓ |
| Pitch=0, Rotation=90 | rotates CW | rotates CW ✓ |
| Pitch=0, Bank=90 | (no bank field) | line along Y ✓ matches matrix |
| Pitch=90, Yaw=90 | line along Z | line along Z ✓ |
| Pitch=90, Rotation=90 | rotates CW | rotates CW ✓ |
| Pitch=90, Bank=90 | (no bank field) | rotates CW ✓ matches matrix |

Confirms (a) the existing 3-angle behavior matches JWildfire at extreme pitches, (b) bank is the Y-axis rotation we'd need to add for full parity, (c) bank at pitch=90 acts as a clockwise screen twist because the Y axis is now aligned with the look direction.

**Bigger picture — free 3D camera movement**. This work is stage 1 of a longer goal: full FPS-style movement around the fractal (WASD + mouse-look + Q/E for up/down). The pieces stage 2 needs:

- Camera *position* fields (`camera_x`, `camera_y` alongside the existing `camera_z`), or a JWildfire-style position triple (`cam_pos_x/y/z`) plus orbital focus point (`cam_xfocus/y/zfocus`). JWildfire's `.flame` XML already stores these — they appear in every random preset (e.g., `cam_pos_x="0.0" cam_pos_y="0.0" cam_pos_z="0.0"`).
- Camera transform that translates by the position before applying the rotation matrix.
- Input handlers that convert WASD/QE deltas to world-space using the current rotation matrix (so "forward" means "where the camera is currently looking").

The rotation work in stage 1 makes stage 2 cheap: the camera-local basis vectors are derived from the rotation matrix, which is built once per frame regardless.

**Implementation order**. All of bank field, matrix port, `camera_transform` fix, roll re-routing, and Bank UI slider should ship together. They're a single coherent change — partial versions either (a) leave the matrix bug unfixed or (b) let users author flames they can't round-trip through JWildfire. Total scope is about 1 new field + 1 new XML attribute + 1 shader function rewrite + 1 UI slider + a couple of tests.

**Discovered**: User-reported during the size-attribute / image_size work cycle while exploring camera options. Cross-referenced against `output/FlameRendererView.java` (import side, lines ~91) and `output/FlameRendererView.java`-or-similar XML writer (export side — `getCamPitch() * Math.PI / 180.0` confirms degrees-internal / radians-on-disk).

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
