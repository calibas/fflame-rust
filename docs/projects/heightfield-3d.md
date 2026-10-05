# 3D height-field mode: escape-time and simulations as path-traced terrain

Plan, 2026-10-05. Not started. Asked for by the user as "one of the
larger goals", to be planned before anything is built.

Decisions a reader should argue with are marked **decision**. The open
questions are in section 10.

## 0. What is being asked for

In the user's words:
- "A 3D mode for rendering escape-time and simulation fractals, but not
  full 3D like some escape-time fractals already use."
- "Use 2D fractals like the Mandelbrot as the base, just the flat plane
  in 3D. Then we use things like the calculated relief and/or the escape
  time itself to determine the depth."
- The same for 2D simulations: "a flat plane in 3D space, but with the
  relief and/or the age value as depth."
- "Path tracing so I can get really high quality rendering, same way 3D
  escape-time already works."
- "I'm thinking we can reuse the 3D engine."

So:
- **The geometry.** A height field: the 2D picture's plane, displaced
  along z by a scalar the 2D renderer already computes.
- **The colour.** The 2D picture's own colour on that surface.
- **The light.** A path tracer.
- **Not in scope.** Full 3D fractals (Mandelbulbs) and the IFS solids
  of mode D.

## 1. What exists, and what this plan builds on

Three surveys of the code (2026-10-05) and a CPU prototype (section 2).
Paths are the files to read before building.

**The 2D sources already compute a height.**
- **Simulations.** The relief stage (`src/sim/renderer.rs`,
  `encode_relief`; `RELIEF_*` WGSL in `src/sim/assembler.rs`) writes a
  grid-sized `Rgba32Float` of (height, d/dx, d/dy) every frame a Relief
  colouring asks. The height is one channel of a layer (the pattern, or
  age), Gaussian-smoothed. It was built as "the height a 3D height-field
  mode would displace by" (mccabe-multiscale plan, section 10).
- **Escape-time.** The renderer's `height_tex`
  (`src/escape/renderer.rs`, `create_height` / `ensure_height`) is an
  `Rgba32Float` at render resolution: R the colouring's raw value, G
  the relief's height source.
  - The sources (`ShadingField`): smooth, banded, a texture layer,
    analytic (−ln DE), offset orbits, embossed.
  - They go through `HeightTransfer` curves.
  - It is allocated only while relief, auto-contrast or the overlay is
    on.
  - A distance estimate exists where the formula has a derivative (the
    `distance_estimate` colouring, `|z| ln|z| / |dz|`).
  - Interior points have coverage 0 and height 0.

**The escape renderer can render any region at any resolution**
(centre as decimal strings, `zoom_log2`, rotation, the job's size).
Thumbnails, export, video and the gallery all do it through
`render_with` (`src/renderer/render.rs`). It does not tile, and its
memory limits are in `allocation_error`: the perturbed state is about
48 B per pixel; the terminal records are 32 B per pixel and dropped
past the binding limit.

**There is a 3D escape engine already: mode D** (`ifs_flame_3d`,
[ifs-distance-rendering.md](ifs-distance-rendering.md)). It
sphere-traces affine IFS solids, and it has most of what a terrain
renderer needs besides the intersection:
- **A pinhole camera with a field of view**, on the escape config
  (decision D8 there): target as exact decimals, pitch, yaw, bank, FOV
  and a pan. It has ray generation (`SolidCamera`, `ifs_ray`), a panel
  (`show_solid_camera`), viewport orbit and pan (`panel_viewer.rs`),
  and animatable targets.
- **The Solid Lighting vocabulary** (`SolidShadingSettings`: four
  lights, ambient, diffuse, specular, shininess, AO, shadow strength),
  shared with 3D flames.
- **Traced lighting.** Soft shadows (`ifs_shadow`), occlusion
  (`ifs_ao`) and gradient normals (`ifs_normal`).
- **A geometry cache and a relight pass.** The walk stores normal,
  AO, per-light shadow terms and depth; a lighting edit is a relight
  costing about 2 ms, not a walk. There is also a recolour cache.
- **Progressive work** in bands and 250 ms chunks under the GPU
  watchdog, with temporal smoothing.

It is not a path tracer: there are no bounces, no sky and no
environment anywhere in the repo.

**The flame 3D engine** contributes something different.
- **What it has.** Its camera fields (`camera_rotation_x/y`,
  `camera_bank`, `rotation`, `camera_x/y/z` in `FractalConfig`), the
  View panel that edits them, fly mode, and animation tracks for every
  one of them. The track picker already offers them in escape and
  simulation modes, where nothing reads them.
- **Its projection** is depth scaling (`zr = 1 − persp·z`), not a
  pinhole, which is why mode D did not share it.
- **DoF and fog** exist as fields (`dof_focus_distance`,
  `dof_blur_strength`, `fog_strength`, `fog_start`), and mode D already
  applies the config's fog and background.

**The accumulator and tonemap contract** for escape and simulations:
- `rgb` is linear colour and `a` is coverage, so 0 lets the background
  through.
- It is Linear tonemap only, through `tonemap_pass_with_input` and the
  shared effects tail.
- Escape accumulates jittered renders only at export
  (`begin_accumulation` / `accumulate_sample`); simulations never do.

**Render-mode gating.** `src/ui/render_mode.rs` and
`src/ui/visibility.rs`. The precedent for a 3D *view* inside a 2D mode
is `Solid::of(config)`: "deliberately not derivable from a mode", read
from the config, threaded into the panel policy. Mode D's D1/D2 chose
that over a new `RenderMode` because a mode costs about 55 call sites.

## 2. The prototype

[proto_heightfield_pt.py](../../scripts/heightfield_prototypes/proto_heightfield_pt.py)
is a CPU path tracer in numpy, 640×400, 4 samples:
- **The terrain.** A bilinear height and albedo grid.
- **The light.** A sun with soft shadows, a gradient sky, one bounce of
  sky and terrain light.
- **The material.** A mirror for the escape interior.

It renders three subjects (`output/heightfield_proto/sheet.png`).

| subject | height | seen | primary march steps |
|---|---|---|---|
| Mandelbrot, seahorse valley | log of the smooth escape count | a forest of spikes climbing toward the set; mirror lakes in the interior | 400 (the cap) |
| Mandelbrot, same window | `H·exp(−DE / w)`, w = span/150 | plateaus with sheer filigree cliffs, valleys between the filaments | 230 |
| McCabe, 5 scales, colour memory | the field, Gaussian σ = 2 | rolling hills with nested texture; the relief at 4% of the width is too low | 225 |

What it settled:
- **The escape count is a poor height on its own.** It climbs without
  bound toward the set, and a log curve only slows it. The distance
  estimate gives the readable terrain. The plan offers both, with
  distance the default where the formula has a derivative (section 5).
- **A Lipschitz march is not enough.** A step bounded by the steepest
  slope anywhere crawls on spiky terrain: the escape-count subject hit
  the 400-step cap. The GPU intersection needs a real acceleration
  structure (section 4).
- **Four samples with one bounce already read as lit terrain.** The
  path tracer does not need many bounces to be worth having over the
  2D relief.

## 3. The shape of it

- **decision H1 — A view, not a render mode.**
  - **What it is.** A per-config "3D terrain" switch inside Escape and
    inside Simulation: `escape.terrain` and `sim.terrain`, each a
    `TerrainConfig` (section 7).
  - **Gating.** It follows the `Solid` precedent: `visibility.rs` gains
    the cases, `fly_mode_available` gains the config, and nothing else
    in the mode system changes.
  - **What a fifth mode would cost.** A config version, a wire form, an
    API enum and about 55 call sites, for a picture that is still the
    escape or simulation picture. Online sync stores it as what it is.
- **decision H2 — One renderer, two producers.**
  - **The renderer.** A new module, `src/terrain/`, with
    `TerrainRenderer`. It takes a height texture and an albedo texture
    covering a rectangle of the plane, plus a camera, lights and a
    material. It returns an `Rgba32Float` in the accumulator contract,
    which the shared tail tonemaps.
  - **The producers.** Escape and simulations each turn their own
    picture into those textures (sections 5 and 6). The terrain
    renderer knows nothing of iterations or steps.
  - **Why.** The path tracer, the acceleration structure and the camera
    are the bulk of the work, and none of it differs between the two.
- **decision H3 — Terrain-local coordinates, so no extended
  precision.**
  - **The space.** The terrain is a unit square (scaled to the tile's
    aspect) with heights in the same units, and the camera lives there.
  - **Deep zoom stays where it already works.** It is entirely the 2D
    renderer's problem: the escape footprint is an ordinary 2D render
    at the current view, perturbation and all.
  - **What that undoes.** Mode D needed exact-decimal camera positions
    because it marches the fractal itself; a terrain marches a
    texture. This is what makes H4 possible.
- **decision H4 — The flame camera fields drive the terrain camera,
  with a pinhole projection and a FOV.**
  - **Which fields.** `camera_rotation_x/y`, `camera_bank`, `rotation`
    and `camera_x/y/z` are unused in escape and simulation modes. The
    View panel already edits them, fly mode already flies them, and
    the animation track picker already offers them there.
  - **What is added.** One field, `terrain_fov`. The projection is a
    true pinhole: ray directions from `effective_camera_rows`
    (`src/renderer/shade_pass.rs`, the exact Rust mirror of the WGSL
    matrix).
  - **The alternative** is mode D's escape camera. It has the
    better-shaped UI (target, orbit, pan), but simulations have no
    camera at all, and its decimal target buys nothing here (H3).
  - **The orbit and pan interactions** mode D built are ported to the
    flame fields, about a pivot at the terrain point under the screen
    centre.
- **decision H5 — Two tiers on one intersection.**
  - **Lit.** Primary rays; the Solid Lighting lights with traced soft
    shadows and traced AO; a geometry cache so a lighting edit is a
    relight. Mode D's rig on a height field. This is the interactive
    view, and the one a running simulation can afford every step.
  - **Path traced.** Progressive Monte Carlo: sun and sky, bounces,
    soft materials, depth of field, fog, accumulating until a sample
    target. Used for stills and video, and in the viewport whenever
    nothing is changing.

  The tier is a setting, with "path traced when still" the default for
  escape and "lit while running, path traced when paused" for
  simulations (section 8).

## 4. The terrain renderer

**Inputs, per frame:**
- `height`: `R32Float`, N×M, in terrain units after the height curve
  and scale.
- `albedo`: `Rgba16Float` or `Rgba8UnormSrgb`. Linear albedo, and alpha
  as a mask: 0 is a hole, which the escape interior uses when it is not
  a lake.
- An optional `material` mask (`R8Unorm`): which cells are the "lake"
  material (escape interior, a simulation's empty cells).
- The tile's extent and aspect, and the tiling (single, or repeat for a
  periodic simulation; section 6).

**The acceleration structure: a maximum mipmap.**
- **What it is.** A pyramid of the height's per-texel maxima over 2×2
  blocks, up to one texel. Tevs, Ihrke and Seidel 2008, "Maximum
  Mipmaps for Fast, Accurate, and Scalable Dynamic Height Field
  Rendering".
- **The traversal.** A ray descends where its segment over a node dips
  below that node's maximum, and steps over the node otherwise. Expect
  O(log N) plus the cells actually grazed.
- **Rebuilding** is log2 N reduction dispatches. It is cheap enough to
  redo every frame for a running simulation, which is the "dynamic"
  case the paper is about. Building it every frame is what makes a live
  sim viewable in 3D at all.
- **Storage.** A separate texture per level, as the sim pyramid already
  does, to keep the levels off `texture_storage` mip views.
- **The leaf.** The exact intersection with the bilinear patch of four
  heights: a quadratic in t along the ray within the cell. The
  prototype's bisected march is the CPU reference it is tested against.

**decision H6 — Ray generation from the camera above, at f32**, with:
- jitter inside the pixel for antialiasing (accumulation is the AA);
- a thin lens for depth of field, from `dof_focus_distance` (focus, in
  terrain units) and `dof_blur_strength` (aperture);
- rays missing the tile: in Single, they see the sky; in Repeat, they
  wrap.

**Shading normals.**
- **From the source's own gradient where there is one.** The sim relief
  stage stores d/dx and d/dy, so its normals come from the smoothed
  height's derivative, not from differences of the texture.
- **Otherwise,** central differences of the height texture.
- **Interpolated bilinearly** across the patch, so the shading is
  smooth even where the geometry is faceted.

**The lit tier** is mode D's rig, ported:
- the four Solid lights, Blinn-Phong with the panel's ambient, diffuse,
  specular and shininess;
- soft shadows by shadow rays through the same traversal, jittered over
  the light's angular size;
- AO by a few short cosine rays;
- fog from the config's fields;
- a geometry record per pixel: hit t, normal, the albedo texel, the
  per-light shadow terms as unorm8 and AO. Lighting edits relight
  without intersecting, exactly mode D's scheme.

**decision H7 — In terrain views the lights are world-fixed.**
- **Mode D's lights are camera-relative** (azimuth 0, elevation 0 is a
  headlight). A terrain wants a sun that stays put while the camera
  orbits.
- **The convention.** Azimuth counter-clockwise from east on the plane,
  elevation above it: the 2D relief's light angle. The default sun is
  azimuth 135° at elevation 30°, the escape and sim reliefs' defaults
  (their Lambert elevation is 30). So turning a relief-lit 2D picture
  into a terrain keeps its light where it was.
- **The other lights** are the same `SolidLight` records, read in world
  space.

**The path-traced tier:**
- **The integrator.** Progressive. One sample per pixel per dispatch
  (more for export), accumulating a running mean. A deterministic RNG
  keyed by (pixel, sample index), so a render is reproducible and batch
  invariant: the sim engine's standard, kept here.
- **Light transport.**
  - Next-event estimation to each enabled light, sampled over its
    angular size, with a shadow ray.
  - The sky by cosine-weighted bounce rays; multiple importance
    sampling between sun and sky once the BRDF is more than Lambert.
  - Bounces 0 to 4 (default 2), Russian roulette from the second.
- **The sky.** An analytic gradient (zenith, horizon, ground colours,
  an intensity) plus a sun disc. An HDRI environment later (section 9,
  T5).
- **Materials.**
  - Lambert on the albedo, plus a GGX gloss layer (roughness,
    specular).
  - Emission: the albedo times a strength, for a glowing fractal.
  - The lake: a dielectric mirror with Fresnel and a tint, for the
    escape interior or a chosen mask.
- **Fog.** The config's exponential fog as a homogeneous medium in
  single scattering, lit by the sky colour: the cheap atmosphere.
  Volumetric sun shafts are later.
- **Output.**
  - rgb is radiance.
  - Alpha is 1 where a ray reached the terrain or the sky.
  - "Transparent sky" sets alpha 0 for rays that see only the sky, so
    a PNG can be composited.
- **Fireflies** are clamped per sample at a user-visible maximum
  (default 10× the sun-lit white).
- **The tonemap** is the shared Linear tonemap with exposure; a filmic
  curve is a later option, since a path tracer's highlights want one.
  The radiance is in the escape and sim contract's units, so the
  existing exposure and gamma controls mean what they mean.

**Interactivity.**
- **Any edit resets the accumulation:** camera, terrain, light or
  material.
- **While the camera moves,** the path-traced tier shows its
  one-sample frames with mode D's temporal smoothing. Lighting edits
  are relights in the lit tier.
- **When still,** samples accumulate up to the target (default 256 in
  the viewport). The menu-bar progress bar shows the samples.

**Cost, estimated, not measured** (T1 measures it):
- **A sample.** A primary ray of about 40 traversal steps, a shadow ray
  of about 30 and two bounces of about 40 each: on the order of 150
  texture reads per sample.
- **At 1080p** that is about 300 M reads, roughly 5–15 ms a sample on
  this machine's GPU. So 256 samples is 2–4 s, and the lit tier at 1
  sample is 3–6 ms.
- **The maximum mipmap is the lever.** The prototype's Lipschitz march
  at 225–400 steps would be 5–10× worse.

## 5. The escape-time producer

**The footprint.**
- **What it is.** A square, by default, at the current 2D view: centre
  and rotation the view's, side = the view's span × an extent (default
  1, up to 8).
- **How it is rendered.** By the escape renderer, as an ordinary 2D
  render at N×N (default 2048, up to the device's texture limit),
  writing the colour image and the height texture.
- **When it re-renders.** Only when the 2D picture changes
  (`escape_dirty`), not when the camera moves.
- **What the terrain path forces.** `ensure_height` regardless of
  whether relief is on, and a supersample of 1 (the terrain's own
  accumulation antialiases the view).

**The height**, `TerrainConfig.source` for escape:
- **Relief source.** The relief's own G channel, through the relief's
  `HeightTransfer` and scale. The 2D relief's lit surface becomes the
  3D surface: the most direct reading of "use the calculated relief".
- **Escape count.** The smooth count with a curve (log by default).
  Spiky near the set (section 2), offered because it was asked for.
- **Distance** (**decision H8**, the default where the formula has a
  derivative): `H·exp(−DE / w)`.
  - w is a width as a fraction of the footprint (default 1/150, the
    prototype's).
  - The set is the plateau, and every filament carries smooth flanks.
  - It needs DE per pixel. The renderer has `dz` in its terminal
    records (dropped past the binding limit) and the
    `distance_estimate` colouring computes DE. The footprint pass has
    the iterate pass write DE into the height texture's B channel. B
    carries the relief variants' stored slope and the overlay's count
    today; the footprint render owns it while it runs.
  - Formulas without a derivative fall back to the escape count, and
    the panel says so.
- **The interior:** a lake (a flat plane at the top, or at a level, in
  the lake material), a hole (albedo alpha 0; rays pass through to the
  ground plane or the sky), or a plateau at the maximum height.

**The colour** is the 2D picture's linear rgb, written to the albedo
at the footprint's resolution. So every colouring, palette, texture
layer and overlay comes along unchanged.

**Large footprints.**
- **The memory.** Past the escape renderer's single-render limits (the
  48 B per pixel perturbed state, the 32 B per pixel records against
  the binding limit), the footprint is rendered in tiles. Each tile is
  an escape render at a shifted centre, copied into the terrain
  textures.
- **This is the first tiled escape render.** It is straightforward
  because a tile is only a centre: decimal strings plus an offset,
  exactly as the perturbed path's `ref_offset` already is. The 2D
  export can use it afterward.
- **The gate:** a tiled footprint is byte-identical to the single
  render where both fit. The exception is the perturbed reference
  position, which moves the rounding; that case is gated by tolerance,
  as solid renders are.

**Deep zoom is free** (H3): the footprint is a 2D render at the view.
The one constraint is its side. At extents above 1 the farthest pixels
sit `extent·√2/2` spans from the reference: inside the 8192 px
relocation cap and the BLA bound at the default sizes, and checked by
`allocation_error` at others.

## 6. The simulation producer

- **The height** is the relief stage's texture: height and gradient on
  the grid, any channel, smoothed by the softness.
  - **decision H9:** the terrain reuses the stage, not the Relief
    colouring. `TerrainConfig.source` names the layer, channel and
    softness itself, and the renderer runs the relief stage for it
    whether or not a Relief colouring is in the stack.
  - **The height scale** is in field units per terrain unit. The
    prototype's McCabe at 4% of the width read as low; the default
    should be about 10%.
- **The colour** is the colour stack evaluated per cell: a second
  colour pass whose output is the grid, at Nearest, written to the
  albedo.
  - **What that keeps.** Every colouring, the stack's blends, the
    matte (alpha 0 makes a hole) and Scale Memory.
  - **What it drops.** The resolve's interpolation. The terrain's
    bilinear albedo is the interpolation now.
  - **A Relief colouring in the stack** would light the albedo twice,
    so the terrain pass skips colourings that declare `NeedsRelief`.
- **Live.**
  - **Every simulation step changes the terrain.** The relief stage,
    the albedo pass and the maximum mipmap rebuild each frame the
    simulation advanced. At a 1080p grid that is about 2 ms (relief),
    0.6 ms (colour) and 1 ms (the pyramid, estimated).
  - **So a running simulation shows the lit tier**, and the path-traced
    tier accumulates when it pauses or reaches its step cap. A still or
    a video frame is the path-traced render of the state at its step,
    which is what a simulation still already means.
- **Tiling** (**decision H10**): a periodic simulation's terrain can
  repeat to the horizon. The traversal wraps its texel coordinates,
  and the maximum mipmap wraps with it. Single is the default; Repeat
  is offered only on the periodic boundary.

## 7. Config, UI, export, animation

**`TerrainConfig`**, one struct, in `escape` and in `sim`, all fields
skip-if-default:
- `enabled`
- `source`: per producer (sections 5 and 6)
- `height_scale`
- `extent` (escape) / `tiling` (simulation)
- `interior` (escape)
- `tier`: Lit / Path traced / Auto
- `samples`: viewport target, export samples
- `bounces`
- `sky`: zenith, horizon and ground colour; intensity; sun disc on/off;
  transparent
- `material`: roughness, specular, emission
- `lake`: tint, roughness

Shared with the rest of the config:
- the camera (H4: the flame camera fields plus `terrain_fov`);
- the lights and shading (`SolidShadingSettings`, world-fixed per H7);
- fog and DoF (the flame fields).

**UI.**
- **The switch.** A "3D terrain" checkbox at the top of the Escape and
  Simulation panels.
- **A Terrain section** under it for `TerrainConfig`.
- **On, `visibility.rs` shows:**
  - the View panel's camera section, plus the FOV;
  - fly mode;
  - the Solid Lighting panel;
  - the View panel's depth effects (DoF, fog).
- **The View panel's camera code** checks `matches!(render_mode,
  ThreeD)` inline today (`src/ui/view.rs`). Those checks become
  visibility controls, which CLAUDE.md asks for anyway.
- **The viewport.**
  - Drag orbits about the pivot.
  - Shift-drag pans and the wheel dollies (mode D's gestures).
  - F2 flies.
- **2D navigation is off.** It would re-render the footprint.

**`render_with`.**
- **Order.** `render_escape` / `render_sim` produce the 2D textures
  (footprint or grid), then call `TerrainRenderer` for `samples`
  samples, chunked under the 250 ms watchdog budget. The shared tail
  follows unchanged.
- **Who gets it.** CLI, thumbnails, the gallery and video inherit it,
  as they did escape and simulations.
- **One exception.** The in-browser PNG export's hand-rolled path
  (`app/mod.rs`, about lines 2197–2480) needs the same call added. The
  survey found it does not go through `render_with`.

**Export.**
- **Direct, at the requested size.** A path tracer's pixels are
  independent, so tiling the OUTPUT (row strips, each its own
  accumulation) is easy, and it lifts the size limit for very large
  stills (T5).
- **Antialiasing is the samples:** no supersample.

**Animation.**
- **The camera tracks already exist:** the View category, now
  meaningful.
- **Terrain parameters** get `ConfigPath` arms like the relief's.
- **Video** renders `samples` per frame. A camera path (a tour) is the
  hyperbolic roadmap's "pathing and navigation in the animation
  system" (`docs/experimental/hyperbolic-camera-extras.md`), which this
  mode would be its first consumer of. It is not part of this plan.

## 8. Defaults that decide how it feels

- **Tier: Auto.**
  - Lit while anything changes: a camera drag, a running simulation, a
    slider.
  - Path traced, accumulating, when nothing does.
  - Export and video are always path traced at `samples` (default
    256).
- **The sun:** azimuth 135°, elevation 30°, 3.0 × white; sky
  intensity 1. The ratio is the prototype's, which reads as a clear
  day.
- **Bounces:** 2.
- **The camera.** On enabling: above the tile's south edge, looking
  north-down at 35°, the whole tile in view at FOV 45. The prototype's
  framing.
- **Height scale:** 10% of the tile width (simulation), 6% (escape,
  distance).

## 9. Phases and gates

Each phase ships with:
- every existing visual baseline byte-identical (terrain off changes
  nothing);
- `release.py check`;
- WGSL lints (no self-compares or self-divisions; `ff_atan2` where a
  zero pair is reachable);
- the browser storage-buffer limit test
  (`a_flame_renders_within_a_browsers_storage_limit`'s pattern).

**T1 — The terrain renderer, on synthetic terrain.**
- **Built.**
  - `src/terrain/`: the maximum mipmap build, the traversal with the
    bilinear-patch leaf, pinhole ray generation from the flame camera
    fields, primary hits, one sun with a shadow ray, Lambert.
  - A Rust CPU reference intersector, the prototype's bisected march.
- **Gates.**
  - Hit distance and normal against the CPU reference on analytic
    terrains (a sinusoid, a cone, a step, a spike field like the escape
    count's) to 1e-4 of the tile.
  - A flat plane under a sun matches `albedo · E · cos θ`.
  - Traversal steps per ray are recorded at 1080p on all four.
- **Measured.** The cost estimate of section 4, replaced by numbers.

**T2 — Simulations, lit tier.**
- **Built.**
  - `sim.terrain` and its panel section.
  - The grid-resolution colour pass.
  - The relief stage run for the terrain.
  - The camera on the flame fields with `terrain_fov`; the visibility
    cases; fly mode.
  - The lit tier with the geometry cache and relight.
  - `render_with`, export, video.
- **Gates.**
  - A running simulation in 3D at 1080p stays inside the interactive
    budget.
  - A lighting edit is a relight: it hits the cache, as mode D's gate
    asserts.
  - Terrain-off configs are byte-identical.
  - A `sim-terrain-*` visual baseline.

**T3 — The path-traced tier.**
- **Built.** Progressive accumulation; the sky; NEE with soft shadows;
  bounces with Russian roulette; Lambert plus GGX; emission; the lake;
  fog as single scattering; DoF; Auto; samples in the progress bar.
- **Gates.**
  - **The white furnace:** albedo 1, a flat infinite terrain (Repeat),
    a uniform sky L, no sun. Every bounce count converges to L, to 1%
    at 4096 samples.
  - Sun-only Lambert matches the T1 analytic case.
  - Batch invariance: the same samples in any number of dispatches,
    bit for bit.
  - Determinism: same config, same image.
  - A path-traced visual baseline at a fixed sample count, compared by
    tolerance, since a Monte Carlo image at 64 samples is not
    bit-stable across drivers.

**T4 — Escape-time.**
- **Built.**
  - `escape.terrain`: the footprint render, tiled.
  - The three sources, with the distance source writing DE.
  - The interior options; re-rendering on `escape_dirty` only.
- **Gates.**
  - The tiled footprint equals the single render where both fit
    (byte-identical on the direct path, by tolerance on the perturbed).
  - A deep-zoom terrain at zoom 2^60 renders the same structure as the
    2D view (its footprint IS the 2D view).
  - A `escape-terrain-*` baseline.

**T5 — Reach and polish**, each its own decision when it comes up:
- an HDRI environment;
- a denoiser: à-trous guided by albedo and normal, which
  `shaders/atrous.wgsl` already implements for flame normals;
- a far field: a second, coarser footprint around the first, for
  horizon views of escape;
- output tiling for very large stills;
- a filmic tonemap curve;
- volumetric sun shafts;
- presets.

## 10. Open questions for the user

1. **The camera (H4).**
   - **Recommended:** the flame camera fields (View panel, fly mode and
     tracks all work) plus a FOV.
   - **The alternative:** mode D's escape camera, which has a target
     and orbit built in but no simulation counterpart.
2. **What lies beyond the tile.** The sky (Single), the tile repeated
   (periodic simulations only), or a ground plane at the lowest height?
3. **The escape default height:** distance (smooth plateaus and cliffs,
   recommended) or the escape count (spiky, closer to the words "the
   escape time itself")?
4. **The interior.** Lake (mirror), hole, or plateau as the default?
5. **Scope of the first usable version.** T1–T2 gives a lit, live 3D
   simulation; T1–T3 adds path tracing. Should escape (T4) come before
   path tracing?
6. **The lights (H7).** World-fixed in terrain views, matching the 2D
   relief's light angle (recommended). Or camera-relative, as mode D
   and 3D flames have them?
