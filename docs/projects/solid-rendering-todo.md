# Solid Rendering & Lighting — deferred / future work

Items deliberately skipped on the `solid-rendering` branch (shipped
2026-07-16, see [solid-rendering.md](solid-rendering.md)) that are worth
revisiting later. Roughly ordered by expected user value.

## Features

### Surface textures & materials
- **Reflection / environment maps** — JWildfire solid materials carry
  `refl_map_filename` + `refl_mappping` (BLINN_NEWELL etc.) and a
  reflection intensity. Ours writes JWF defaults on XML export and
  ignores them on import. A screen-space environment lookup in the shade
  pass (world normal → equirect sample) would cover the common case.
- **Bump mapping** — perturb the shade pass's screen-space normals with
  a high-frequency height source before lighting. Candidate sources:
  a noise function in the shade pass (cheap, no assets), the palette
  color's luminance (ties bumps to flame structure), or an image asset.
  Needs a strength + scale dial. Our normals come from the smoothed
  depth field, so the perturbation must happen AFTER the à-trous chain
  or it gets smoothed away.
- **Multiple materials** — JWildfire supports per-xform `material`
  indices into a material array; we have one global material. Would need
  a per-sample material channel through the histogram (space is tight —
  the RGBD layout has no spare channel; could ride the palette index).

### Lighting
- **Positional point lights** (queue item 5, skipped by choice) —
  current lights are directional (azimuth/elevation, infinitely far).
  Point lights inside the flame need per-light cube or paraboloid
  shadow projections instead of one ortho map — a bigger follow-on.
- **Per-light shadow strength** — JWF stores `shadow_intensity` per
  light; ours is one global `shadow_strength` (XML import takes the max,
  export writes the global to every light). Per-light needs a strength
  in the shade pass's light loop — cheap once wanted.
- **JWF AO parameter mapping** — `sld_render_ao_search_radius` /
  `_falloff` / sample counts describe JWF's AO sampler; semantics
  unverified, so import keeps our SSAO radius default. If someone
  produces matched A/B renders, a conversion could be calibrated.

### DoF
- **Tiled-export support** — post-process DoF is skipped (with a
  warning) on the strip-tiled export path: a gather can't cross strip
  boundaries without apron rows. Needs the shade/DoF strips to carry
  an apron of `MAX_RADIUS` rows, or a full-image DoF pass after strip
  assembly.
- **Reset-free DoF sliders in solid mode** — focus/strength are
  classified IterationReset (required for the 2D at-splat path), but the
  post-process pass doesn't need a reset. A conditional UpdateType
  (ShadingOnly when solid) would make focus tuning on a finished render
  instant.
- **CoC cap under supersampling** — `MAX_RADIUS = 32` render pixels;
  at 2× AA that is 16 final pixels, so very strong blur saturates
  earlier than at 1×. Scale the cap by the supersample factor.

### Supersampling / AA
- **WASM support** — the 2× AA checkbox is desktop-only; the WASM
  export path (`export_headless_wasm`) never sees the flag. Memory is
  the concern (4× pixels inside a browser GPU budget).
- **4× option** — the machinery generalizes (render scale N, N²-sample
  quads for the firefly clamp); only worth it with a use case, since
  cost is N²·iterations.

### Shadow maps
- **Resolution setting** — fixed at 1024² ×4 lights (16 MB tail). JWF
  exposes `shadowmap_size` up to 4096. Ours is baked into buffer
  layout (`SHADOW_MAP_RES`); making it a setting means histogram-tail
  resize on change (same machinery as the depth-region toggle).
- **CPU-histogram export path** — shadow maps are GPU-only; the CPU
  fallback (very large exports) renders lit but unshadowed, with a
  warning. Would need a CPU splat of the maps or a hybrid pass.

## Performance

Good enough to merge; known headroom, largest first:

- **DoF gather cost** — the dense 2-D disk gather is O(CoC²) per pixel.
  Half-resolution gather during interactive accumulation (full-res once
  settled / on export) would cut the interactive cost ~4× with minimal
  visible difference while iterating.
- **Shade chain at idle-ish states** — normals + 3× à-trous + shade run
  whenever dirty; with `normal_smoothing = 0` the à-trous dispatches are
  no-ops that still launch. Gate the dispatches on their settings.
- **Interactive lighting preview** — the whole deferred chain runs per
  accumulation batch. A half-res shade during active iteration (like
  the DoF idea) is the generic lever if lighting-on FPS needs help.
- **Shadow-splat atomic pressure** — every plotted sample does up to 4
  `atomicMax` shadow-map writes. Could subsample during early
  accumulation (maps converge fast) and go full-rate later.
- **Shade-chain bind-group caching** (from the pre-merge review) — the
  normals + à-trous bind groups bind only stable resources but are
  recreated every interactive shade frame; cache them alongside
  `normal_texs` and invalidate on resize. (The shade and DoF bind
  groups genuinely must rebuild — their inputs ping-pong.)
- **Hoist the shadow-map basis** — `shadow_map_factor` rebuilds the
  light-space ortho basis (2× cross + normalize) per pixel per light;
  it depends only on the light direction, so it could ride ShadeParams.
- **2× AA cost** — inherently ~4× (iterations scale by design). A
  cheaper "edges only" mode (render 1×, supersample only high-gradient
  tiles) is possible but architecturally invasive; not recommended
  unless demanded.

## Cross-references
- Orbital camera / tour mode: tracked in CLAUDE.md (Optional/Future).
- EXR/HDR export: CLAUDE.md medium-priority; interacts with solid
  (shade output is HDR pre-tonemap — a natural EXR tap point).
