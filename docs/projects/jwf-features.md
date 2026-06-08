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

**JWildfire XML attribute** (on each `<xform>` / `<finalxform>`): `zxCoefs="a c b d e f"` — six floats, same layout as the standard `coefs` attribute. Defines a 2D affine in the ZX plane. We don't have JWildfire's `XForm.java` source locally to confirm the application order, but the empirical behavior matches the hypothesis that `zxCoefs` is applied as part of the affine transform alongside the standard XY `coefs`, producing a 3D affine of the form:

```
x' = a·x + b·y + zxa·z + e
y' = c·x + d·y + …
z' = zxc·x + zxd·z + zxf
```

(With `coefs="a c b d e f"` and `zxCoefs="zxa zxc zxb zxd zxe zxf"` mixing into the standard rotation/translation. Exact coefficient layout needs verification once we touch the importer — likely a one-line test against a known-rotation flame.)

A `yzCoefs` sibling for the YZ plane would round out the 3D affine; I haven't seen one in our local flame samples, but it's a reasonable guess that JWildfire supports it for symmetry. Worth confirming when we look at the JWF source.

**Why we don't have it**: JWildfire-specific extension to Apophysis's 2D affine. Apophysis flames have only the XY `coefs` and `post` attributes; JWildfire adds a ZX channel to give xforms a way to generate Z from X without needing a Z-producing variation. Our app stops reading after the 2D `coefs` / `post`, treating Z as either preserved (when `preserve_z="1"`) or reset per iteration (when off).

**Impact when a flame uses non-identity `zxCoefs`**: Z output collapses to whatever the chaos game's previous-iteration Z was, with no in-affine generation. If any xform in the active set has a flatten-equivalent in post-phase (very common in JWildfire 3D flames), once that xform fires the Z is zero forever afterward — there's no way to regenerate it without the ZX affine. Visible symptom: at high pitch / side view, the 3D fractal appears as a flat line in our render where JWildfire shows depth.

**What it would take**:

1. Add `zx_*` fields (six floats) to the `Transform` struct, default to identity (`1 0 0 1 0 0`).
2. Parse `zxCoefs` in `parse_xform_element` (and probably the post equivalent if it exists in JWildfire, e.g. `zxPost`).
3. Extend the GPU `Transform` struct + bytemuck layout.
4. Apply the ZX affine in `apply_affine` in the shader — multiply the input point through a 3×3 (or 4×4) matrix combining the XY and ZX coefs.
5. Round-trip on export.
6. UI: a "ZX affine" matrix in the transform panel for power users. Most users won't touch it; default identity preserves existing flame appearance.
7. (If `yzCoefs` exists) same for the YZ plane — `yz_*` fields + parse + apply + export.

The complication: combining XY `coefs`, ZX `zxCoefs`, and possibly YZ `yzCoefs` cleanly into a single 3D affine matrix. JWildfire likely applies them in a documented order (probably standard XY first, then ZX, then YZ — or matrix-composes them). Need to verify against the source before implementing.

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
