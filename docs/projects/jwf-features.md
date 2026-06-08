# JWildfire-specific Features (non-variation)

Companion to [`jwf-common-variations-port.md`](jwf-common-variations-port.md) (variations) and [`variation-port-blockers.md`](variation-port-blockers.md) (blocked variations). This doc tracks JWildfire features that are **not variations** — XML attributes on `<flame>` / `<xform>`, per-transform settings, plot-time mechanisms, etc. — that affect how a JWF flame renders but aren't yet wired up in our import or pipeline.

Same shape as [`apophysis-remaining-features.md`](apophysis-remaining-features.md) but for JWildfire's extensions to the Apophysis baseline.

Each feature listed below has a status, what it does in JWildfire, what it would take to implement, and how its absence currently manifests when importing a JWF flame.

## Deferred (not urgent)

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

### `zxCoefs` (and likely `yzCoefs`) — per-transform 3D-plane affines

**Status**: Not parsed on import. Causes Z to collapse to zero on 3D flames where xforms depend on the Z affine to generate depth.

#### The 12-coefficient picture

A general 3D affine `(x, y, z) → (x', y', z')` is a 3×4 matrix with 12 coefficients (using the convention "first letter = which output channel, second letter = which input channel", and "`O`" = the constant offset column):

```
[XX  YX  ZX  XO] [x]   [x']
[XY  YY  ZY  YO] [y] = [y']
[XZ  YZ  ZZ  ZO] [z]   [z']
                 [1]
```

**Apophysis** stores only the XY plane (6 coefs: `XX, YX, XO, XY, YY, YO`) plus a single Z offset (our `Transform.g` field = `ZO`). That's 7 of 12 — leaving `XZ, YZ, ZX, ZY, ZZ` as the five Apo doesn't have. In Apo's effective transform `ZX=ZY=ZZ=0`, so input Z is *ignored* and output Z is *just the constant `ZO`*. Z is effectively a per-transform constant.

XML serialization: Apo's `coefs="a c b d e f"` stores `XX, XY, YX, YY, XO, YO` in that order. (Yes, the column-major order is unusual — `a` and `c` together are the X-output row's X- and Y-input coefficients in the matrix above, etc.) Apo's Z offset `ZO` rides on a separate attribute.

#### What JWildfire adds via `zxCoefs`

`zxCoefs="a c b d e f"` is six floats in the *same XML layout* as `coefs` but for the **ZX plane** — a 2D affine acting on `(z, x)`:

```
[ZZ  XZ  ZO_zx] [z]   [z']
[ZX  XX  XO_zx] [x] = [x']
                [1]
```

That supplies **`ZZ, XZ, ZX`** and re-supplies **`XX, XO, ZO`** which the standard `coefs` and Apo's `g` already provide. So `zxCoefs` adds the three truly new ZX-channel coefficients to the 12-coefficient model, plus duplicates of three Apo already supplies.

The XML coefficient indexing (verified against [`output/XForm.java`](../../output/XForm.java)) is `(row, col)` where `row` indexes the *input* axis and `col` indexes the *output* axis — so `zxCoefs` position `00`/`01`/`10`/`11`/`20`/`21` maps to `ZZ, XZ, ZX, XX, ZO, XO`.

`yzCoefs` is **confirmed to exist** (also from `XForm.java`) — symmetric layout supplying `ZZ, YZ, ZY, YY, ZO, YO` for the YZ plane. Combined with the XY `coefs` and `zxCoefs`, that gives all 12 coefficients of the full 3D affine — with multiple overlaps (`XX, XO` from `coefs` and `zxCoefs`; `YY, YO` from `coefs` and `yzCoefs`; `ZZ`, `ZO` from `zxCoefs` and `yzCoefs`; `XZ` only from `zxCoefs`; `YZ` only from `yzCoefs`; `ZX` only from `zxCoefs`; `ZY` only from `yzCoefs`).

**Post-affine siblings also exist** in JWildfire's `XForm.java`: `xyPost`, `yzPost`, `zxPost`. Same coefficient layout as their pre-affine counterparts, applied after the variations run.

**JWildfire's dispatch** (from `XForm.createTransformations` in `XForm.java`):

- `TransformationAffineNoneStep` — fires when no affines (none of XY/YZ/ZX) are set
- `TransformationAffineFlatStep` — fires when only XY is set (Apo case, the cheap path)
- `TransformationAffineFullStep` — fires when *any* of YZ or ZX is set (or both)

That means our existing "standard Apo affine" math path is the right code path for any flame that *doesn't* use `yzCoefs` or `zxCoefs` — and it stays that path for those flames after we add the 3D affine support. The Full path is opt-in.

How `TransformationAffineFullStep` actually composes the three 2D affines into a 3D transform is still the open question — that class isn't included in our `XForm.java` snapshot (it's a separate file in JWildfire's tree). See "Open questions" below.

#### Why we don't have it

JWildfire-specific extension. Our import stops after reading the 2D `coefs` and Apo's `g`. Our `Transform` struct treats Z as a per-transform constant (Apo semantics). Without `zxCoefs` / `yzCoefs`, the missing 5 coefficients are all implicitly 0, so input Z is ignored by the affine and only variations can drive Z dynamics.

#### Impact when a flame uses non-identity `zxCoefs`

Z output collapses to whatever the chaos game's previous-iteration Z was, with no in-affine generation. If any xform in the active set has a flatten-equivalent in post-phase (very common in JWildfire 3D flames), once that xform fires the Z is zero forever afterward — there's no way to regenerate it without the ZX affine. Visible symptom: at high pitch / side view, the 3D fractal appears as a flat line in our render where JWildfire shows depth.

#### Open questions

- **Coefficient overlap resolution**: standard `coefs`, `zxCoefs`, and `yzCoefs` collectively define 18 floats covering 12 unique target coefficients with 6 floats of overlap (`XX, XO, YY, YO, ZZ, ZO`). JWildfire's `TransformationAffineFullStep` math determines how this is resolved — three plausible interpretations: (a) compose them as three sequential 2D affines in their respective planes, (b) matrix-multiply them into a single composite 3D affine, or (c) treat standard `coefs` as authoritative for `XX/XY/YX/YY/XO/YO` and let `zxCoefs`/`yzCoefs` fill only the new coefficients (overrides silently ignored on duplicates). The class is referenced in our `output/XForm.java` snapshot but its body lives in a separate file we don't have. Cheap test once we touch this: build a known-rotation flame (e.g., 45° pure ZX rotation, no XY component) and check which composition rule matches JWildfire's render bit-for-bit. Or hunt for `TransformationAffineFullStep.java` in JWildfire's source tree directly.

- ~~**Post-affine equivalent**~~: ✓ confirmed — `xyPost`, `yzPost`, `zxPost` all exist, same coefficient layout, applied after variations.

- ~~**Does `yzCoefs` exist**~~: ✓ confirmed in `XForm.java`.

#### What it would take

1. Add 3D affine fields to `Transform`. Mirroring JWildfire's storage (separate `xy_coefs`, `yz_coefs`, `zx_coefs` plus their `*_post` siblings, each a `[f32; 6]`, plus `is_has_*` flags so we can drive the same none/flat/full dispatch JWildfire does) keeps round-trip natural and matches the source-of-truth representation. Default flags all false, which gives us Apo semantics for free.
2. Parse `xyCoefs`/`yzCoefs`/`zxCoefs` (and the `*Post` siblings) in `parse_xform_element`. Set the corresponding `is_has_*` flag when an attribute is present.
3. Extend the GPU `Transform` struct + bytemuck layout. The "flat" case can keep using our existing 2D-only affine math (one fewer bind layout to upload); only the "full" case needs the 12-coefficient path.
4. Resolve the overlap question (above) and apply the composed 3D affine in `apply_affine` for the full path. Cheap test against a known-rotation flame before locking it in.
5. Round-trip on export — only write `yzCoefs`/`zxCoefs` (and posts) when the corresponding `is_has_*` flag is set, matching JWildfire's conditional XML output we read in `output/XForm.java`.
6. UI: optional "3D affine" matrix in the transform panel for power users. Most users won't touch it; default Apo-semantics identity preserves existing flame appearance.

**Discovered in**: `output/JWF-rando22.flame` ("Brokat3D" random preset). At 90° pitch, JWildfire shows a tall 3D structure with two cone-like spires; our app shows a flat horizontal line. The flame uses `zxCoefs` on xforms #0 (curl) and #1 (julia3Dz + flatten). User confirmed by resetting xform #0's affine to default in JWildfire — the flame also goes flat in JWF, narrowing the Z-generating role to that xform's full affine including the ZX channel.

### `size` — saved canvas extent (not just image dimensions)

**Status**: Parsed but discarded on import; written as a fixed `size="1920 1080"` on export. Causes a visible zoom mismatch on JWF/Apo flames saved at non-default dimensions.

**JWF/Apo XML attribute** (on the `<flame>` element): `size="W H"`. Looks superficially like "image dimensions for the saved PNG" but is actually a coordinate-system concept that participates in the rendering math alongside `scale` and `cam_zoom`.

**What it does in JWildfire / Apophysis**: Defines the fractal canvas the saved flame anchors to. `scale` is "pixels per fractal unit *at the saved canvas dimensions*" — so for a given `scale`, a smaller saved `size` shows a smaller portion of the fractal (zoomed in), and a larger saved `size` shows more (zoomed out). User-confirmed example: reducing `size` from `1920×1080` to `192×108` (×0.1 each axis, same aspect) renders 10× zoomed in in JWildfire. The saved fractal coordinate triple is `size + scale + cam_zoom`; any of the three can be changed to adjust the visible extent.

**What our app does instead**: Our `zoom` is a pure viewport multiplier. Our renderer's effective pixels-per-unit is `zoom × min(viewport) × 0.25` — no dependency on any saved canvas. Our import formula `zoom = (scale / 200) × cam_zoom` implicitly assumes the saved canvas is close to `1920×1080`. Flames saved at other dimensions import at the wrong visual zoom because the `size`-derived term is missing.

**Impact when a flame uses a non-1920×1080 size**: The flame renders at the wrong magnification compared to JWildfire. Easy to spot on JWF "random" presets, which often save at small preview sizes (e.g. `638×359`, `192×108`). Apo-exported flames typically save at their default `1500×1000` or close, which lands near enough to our implicit reference that the divergence isn't usually visible. Aspect ratio doesn't matter — the magnification factor is tied to one of the dimensions (width or height — single-dim, not area / min / max). Concrete confirmation needed via a non-uniform-stretch experiment before nailing the exact formula; see "Open questions" below.

**The conceptual mismatch**:

| | JWF / Apo | Ours |
|---|---|---|
| `size` | Saved canvas dimensions; defines the fractal coordinate grid `scale` anchors to | Parsed and discarded |
| `scale` | Pixels per fractal unit *at the saved canvas dimensions* | Folded into `zoom = scale/200 × cam_zoom` |
| `cam_zoom` | Linear multiplier on the rendering | Folded into our zoom |
| Visible fractal extent | `size / (scale × cam_zoom)` | `4 / zoom` (viewport-independent — depends only on our zoom) |

**Three options considered** (B selected as the path forward):

- **A. Compensate at import only**. Multiply `zoom` by `1920 / saved_width` (or whichever single-dim formula the experiment confirms). On export, always write `size="1920 1080"`. One-line import fix, *but lossy round-trip* — exporting our flame back into JWildfire produces a different `size + scale + cam_zoom` triple than the source, so re-importing it in JWF renders at a different magnification than the original.
- **B. Add `image_size` metadata to FractalConfig** (selected). Track the saved size as a first-class config field. Import reads `size` and uses it for the zoom compensation; export writes it back. Our `zoom` stays a viewport multiplier (no renderer/UI refactor), and round-trip with JWF/Apo is exact. Field defaults to `(1920, 1080)` so existing `.fflame` JSON files without it deserialize identically to current behavior.
- **C. Replace `zoom` with `fractal_extent`** (the JWF/Apo model). Refactor our primary view representation to store visible fractal area directly. Conceptually aligned with JWF/Apo but a major refactor touching the renderer, every zoom UI control, the config schema, undo history, and animation interpolation. Not justified by this issue alone.

**Plan for B (when picked up)**:

1. Add `pub image_size: (u32, u32)` to [`FractalConfig`](../../src/config/fractal_config.rs). Default `(1920, 1080)` via `#[serde(default = "...")]` so existing JSON deserializes unchanged.
2. `flame_xml.rs` import: read `size`, store on config, compute `zoom = (scale / 200) × cam_zoom × (1920 / size.0)` (or the corrected formula).
3. `flame_xml.rs` export: write `size="{w} {h}"` from `config.image_size`; back-compute `scale = config.zoom × 200 / cam_zoom × (image_size.0 / 1920)` so the triple round-trips.
4. **Viewport aspect ratio**: when importing a flame, optionally adjust our viewport's aspect ratio to match `image_size`. This avoids letterboxing on flames saved at unusual aspect ratios. (Could be a per-import opt-in.)
5. **PNG export default**: use `image_size` as the default resolution in the PNG export dialog. Users overriding it doesn't change `image_size`; it's just a smarter default.
6. UI consideration: probably expose `image_size` in an "Source / Export" advanced panel so power users who care about JWF round-trip have a knob; everyone else ignores it.

**Open questions before implementation**:

- **Which dimension drives the formula?** `width`, `height`, `min(saved)`, `max(saved)`, or something else? The pair (`JWF1a.flame` at 638×359 vs `JWF1b.flame` at 1920×1080) doesn't distinguish these because the dimensions scale uniformly. Need a non-uniform stretch test — e.g., save the same flame at `size="638 718"` (height doubled) vs `size="1276 359"` (width doubled) and measure the zoom adjustment needed in our app for each. Three data points pin down the formula.
- **Where did the `200` divisor come from?** Empirically tuned during the original import code by eyeballing the visual match against JWF for typical flames. Possibly aligned with our default size of `1920×1080` via something like `size.x / ~10`, but not verified to derive from any Apophysis or JWildfire constant. With size-aware compensation in place, the `200` might be re-derivable from `image_size` directly (e.g., `image_size.x / 9.6` or similar), eliminating the magic number — or it might stay an empirical tuning knob. Worth revisiting once the `size` formula above is nailed down.

**Discovered in**: `output/JWF-rando1.flame` and a side-by-side recreation comparison via `output/JWF1a.flame` / `output/JWF1b.flame` (identical fractal, only difference is `size="638 359"` vs `size="1920 1080"`).

## Related docs

- [`jwf-common-variations-port.md`](jwf-common-variations-port.md) — JWF "script vars" variation subset (188/190 implemented).
- [`variation-port-blockers.md`](variation-port-blockers.md) — Per-variation porting blockers + framework features needed to unblock them.
- [`apophysis-remaining-features.md`](apophysis-remaining-features.md) — Apophysis-baseline features (XAOS, 3D camera, etc.); same shape as this doc but for the Apo side.
