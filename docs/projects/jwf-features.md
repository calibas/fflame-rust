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

## Related docs

- [`jwf-common-variations-port.md`](jwf-common-variations-port.md) — JWF "script vars" variation subset (188/190 implemented).
- [`variation-port-blockers.md`](variation-port-blockers.md) — Per-variation porting blockers + framework features needed to unblock them.
- [`apophysis-remaining-features.md`](apophysis-remaining-features.md) — Apophysis-baseline features (XAOS, 3D camera, etc.); same shape as this doc but for the Apo side.
