# 3D height-field mode: escape-time and simulations as path-traced terrain

Plan, 2026-10-05. Not started. Asked for by the user as "one of the
larger goals", to be planned before anything is built.

**Revised the same day with the user's answers (section 10).** The
camera is the escape solid camera, not the flame camera. The terrain
is built on mode D's 3D pipeline in the escape engine. So escape comes
first, and the phases are reordered.

Decisions a reader should argue with are marked **decision**.

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
environment anywhere in the repo. Mode D is a ray tracer with direct
light. Sphere tracing finds the surface, and real rays find the shadows
and the occlusion. Its antialiasing is analytic at edges (from the
sub-pixel distance), plus the escape engine's supersampling, plus
jittered accumulation at export. Path tracing adds bounces, sky light
and sampled materials to that.

**The flame 3D engine.** The first draft borrowed its camera fields;
the revision does not (H4). For the record:
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
- **decision H2 — Built on mode D's pipeline: one renderer, two
  producers.**
  - **The renderer.** A second geometry source in the escape engine's
    solid pipeline, beside mode D's distance march. It takes a height
    texture and an albedo texture covering a rectangle of the plane.
  - **What it shares with mode D:**
    - ray generation (`SolidCamera`, `ifs_ray`, `eye_rel`);
    - the lighting rig;
    - the geometry cache and relight pass;
    - the bands and chunks under the watchdog;
    - supersampling and jittered accumulation;
    - the accumulator contract into the shared tail.

    Only the intersection differs: a height-field traversal instead
    of the distance march.
  - **The producers.** Escape and simulations each turn their own
    picture into the textures (sections 5 and 6). The terrain path
    knows nothing of iterations or steps.
  - **Simulations reach it** by handing their textures to the escape
    engine's terrain path. So a simulation terrain needs
    `engine-escape`.
  - **The gallery modules skip terrains** (the user's answer,
    2026-10-05). `wasm/render`, `wasm/escape` and `wasm/sim` are built
    without the terrain path, behind a feature as the engines are.
    Their downloads do not carry it, and a terrain config renders
    there as its 2D picture, with the terrain switch ignored.
  - **Why.** The camera, the rig and the progressive machinery are
    built and tested in mode D. The path tracer (T3) then extends that
    rig, so mode D's IFS solids are path traced too.
- **decision H3 — Terrain-local coordinates, so no extended
  precision.**
  - **The space.** The terrain is a unit square (scaled to the tile's
    aspect) with heights in the same units, and the camera lives there.
  - **Deep zoom stays where it already works.** It is entirely the 2D
    renderer's problem: the escape footprint is an ordinary 2D render
    at the current view, perturbation and all.
  - **The camera keeps its exact target (H4).** The terrain path
    subtracts the footprint's centre from it in decimals, and the
    shader sees small local numbers. That is what lets the footprint
    follow the camera into the set (section 5) without the shader ever
    needing extended precision.
- **decision H4 — The escape solid camera, shared with simulations.**
  (The user's call, 2026-10-05.)
  - **Mode D's camera.** On the escape config:
    - an exact-decimal target (`cam_target_x/y/z`);
    - pitch, yaw and bank about the target;
    - a vertical FOV (`cam_fov`);
    - distance from `zoom_log2`;
    - the screen roll is `rotation`.

    It is a true pinhole, with ray generation in `SolidCamera` /
    `ifs_ray`. It has a camera panel (`show_solid_camera`), viewport
    orbit and pan (`panel_viewer.rs`) and animation targets
    (`EscapeCam*`).
  - **Escape terrain uses it as it is.** The target is a point of the
    plane at its height, and a dolly is a zoom.
  - **Simulations get the same camera.**
    - The fields move into one struct, flattened into `EscapeConfig`
      so escape files keep their keys, and added to `SimConfig`.
    - Simulations get ConfigPaths for it.
    - The camera panel and viewport gestures take the struct, not the
      escape config.
    - A simulation's target is in grid units with no deep zoom, so
      its decimal strings are ordinary numbers.
  - **Fly mode** does not drive this camera yet. Mode D's record says
    so ("not done, and known"). A phase adds it (T2), and IFS solids
    gain it too.
  - **Why this and not the flame camera fields** (the first draft's
    recommendation):
    - It is already a pinhole with a FOV, a target and orbit.
    - Its renderer already antialiases and lights.
    - Deep zoom needs the exact target as soon as the terrain follows
      the camera into the set.
    - The flame fields would have brought the View panel and fly mode
      for free. That is now a phase of work, and the cost of the
      choice.- **decision H5 — Two tiers on one intersection.**
  - **Lit.** Primary rays; the Solid Lighting lights with traced soft
    shadows and traced AO; a geometry cache so a lighting edit is a
    relight. Mode D's rig on a height field. This is the interactive
    view, and the one a running simulation can afford every step.
  - **Path traced.** Progressive Monte Carlo: sun and environment, bounces,
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

**decision H6 — Ray generation is mode D's** (`ifs_ray`, `eye_rel`):
- f32 directions from the exact target;
- jitter inside the pixel for antialiasing. Supersampling and
  accumulation are both available, as in mode D;
- a thin lens for depth of field. Two new fields on the solid camera,
  focus distance and aperture, which IFS solids gain as well;
- rays missing the tile: in Single, they see **the background colour**
  (coverage 0, so the tonemap composites the background and a PNG can
  be transparent: the user's answer). In Repeat, they wrap.

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
  - The environment by cosine-weighted bounce rays; multiple
    importance sampling between sun and environment once the BRDF is
    more than Lambert.
  - Bounces 0 to 4 (default 2), Russian roulette from the second.
- **The environment light** is the background colour times an
  intensity: uniform, so what a ray sees on missing the tile is what
  lights the terrain. A gradient sky and an HDRI are later (T5).
- **Materials.**
  - Lambert on the albedo, plus a GGX gloss layer (roughness,
    specular).
  - Emission: the albedo times a strength, for a glowing fractal.
  - The lake: a dielectric mirror with Fresnel and a tint, for the
    escape interior or a chosen mask.
- **Fog.** The config's exponential fog as a homogeneous medium in
  single scattering, lit by the environment colour: the cheap
  atmosphere. Volumetric sun shafts are later.
- **Output.**
  - rgb is radiance.
  - Alpha is the terrain's coverage. A ray that misses has alpha 0, so
    the background shows there and a PNG export is transparent there,
    as escape and simulations already are.
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
  derivative; the user's answer): `H·exp(−DE / w)`.
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
- **The interior:** a plateau at the maximum height (**the default**,
  the user's answer), a lake (a flat plane in the lake material), or a
  hole (albedo alpha 0, so rays pass through to the background).
  - **The plateau's colour** is the escape interior's own: the
    colouring's interior colour where it draws one, otherwise the
    background colour.

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

**The footprint follows the camera** (**decision H11**). With mode D's
camera a dolly is a zoom, so approaching a point of the terrain is a
deep zoom into it.
- **When it re-renders.** The footprint is re-rendered around the
  target when either is true:
  - its texels at the target grow past about 1.5 screen pixels;
  - the target nears the footprint's edge.

  The old footprint draws until the new one lands, with hysteresis so
  a dolly back and forth does not thrash.
- **Re-render during the zoom wherever it is fast enough** (the user's
  answer, 2026-10-05: "if we can re-render at <15 ms rates on average
  hardware, then I'd rather do that during a deep zoom").
  - The renderer measures its own footprint renders, a smoothed
    average as the simulation measures `ms_per_step`.
  - While that average is under 15 ms, the footprint re-renders during
    the dolly, as often as the conditions above call for.
  - Above it, the footprint re-renders on the drag's release, and the
    stretched texels show until then.
  - Decided per device by measurement, so a fast GPU gets the live
    zoom and a slow one stays smooth.
- **The levers that bring it under 15 ms**, measured at T2 before any
  is built:
  - **A motion footprint.** Half the side (a quarter of the pixels)
    while the dolly is moving, the full size on release.
  - **The recolour cache.** Mode D and the 2D escape path already skip
    the walk when only colouring changed; a dolly changes the walk, so
    this helps only the release frame.
  - **Reusing the reference orbit across re-renders** (the perturbed
    path's pan reuse, `MAX_RELOCATE_PX`), since successive footprints
    share a centre region.
- **The cost.** A re-render is one 2D escape render at N². The
  survey's figure is 2.3 ms direct and 14.2 ms perturbed at 960×720
  (Mandelbrot, 2000 iterations). That puts a 1024² motion footprint
  near the line on the perturbed path and well under it on the direct
  one. It is what lets a terrain be flown into at any depth.
- **The heights are scale-invariant.** H and w are fractions of the
  span, so each footprint's terrain looks like the last one's at its
  own scale. The handover pops slightly where the DE flanks were cut
  at the old footprint's resolution.
- **A smooth handover** (keeping the coarser footprint as a far field
  and blending) is T5, with the far field.

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
- **The camera** is the shared solid camera (H4), with its target in
  grid units.
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
- `environment`: intensity (its colour is the background's, for now)
- `material`: roughness, specular, emission
- `lake`: tint, roughness

Shared with the rest of the config:
- the camera: the solid camera struct (H4), in `escape` and in `sim`,
  plus focus and aperture;
- the lights and shading (`SolidShadingSettings`, world-fixed per H7);
- fog and the background colour.

**UI.**
- **The switch.** A "3D terrain" checkbox at the top of the Escape and
  Simulation panels.
- **A Terrain section** under it for `TerrainConfig`.
- **On, `visibility.rs` shows** (the `Solid` precedent: `Solid::of`
  learns the terrain switch):
  - the solid camera panel (`show_solid_camera`, generalized over the
    owner);
  - the Solid Lighting panel;
  - fly mode once it drives the solid camera (T2);
  - fog.
- **The viewport** has mode D's gestures:
  - drag orbits the target;
  - pan slides it;
  - the wheel dollies, which is a zoom, and the footprint follows
    (H11);
  - F2 flies once T2 lands.

**`render_with`.**
- **Order.** `render_escape` / `render_sim` produce the 2D textures
  (footprint or grid), then call `TerrainRenderer` for `samples`
  samples, chunked under the 250 ms watchdog budget. The shared tail
  follows unchanged.
- **Who gets it.** CLI, thumbnails and video inherit it, as they did
  escape and simulations. The gallery modules do not (H2).
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
- **The camera tracks already exist for escape** (`EscapeCam*`);
  simulations gain their twins with the shared struct.
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
- **The sun:** azimuth 135°, elevation 30°, 3.0 × white; environment
  intensity 1. The ratio is the prototype's, which reads as a clear
  day.
- **Bounces:** 2.
- **The camera.** On enabling:
  - the target at the footprint's centre, at its height;
  - pitch 35° above the horizon, looking north (yaw 0);
  - the whole tile in view at mode D's default FOV.

  The prototype's framing.
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

**T1 — The terrain geometry in mode D's pipeline, on synthetic
terrain.**
- **Built.**
  - The height-field geometry source: the maximum mipmap build, the
    traversal with the bilinear-patch leaf, inside the escape solid
    pipeline.
  - Rays from the solid camera; primary hits.
  - The rig's lights with traced shadows, world-fixed (H7); AO.
  - The geometry cache and relight.
  - A Rust CPU reference intersector, the prototype's bisected march.
- **Gates.**
  - Hit distance and normal against the CPU reference on analytic
    terrains (a sinusoid, a cone, a step, a spike field like the escape
    count's) to 1e-4 of the tile.
  - A flat plane under a sun matches `albedo · E · cos θ`.
  - A lighting edit is a relight: it hits the cache, as mode D's gate
    asserts.
  - Mode D's IFS baselines byte-identical.
  - Traversal steps per ray recorded at 1080p on all four terrains.
- **Measured.** The cost estimate of section 4, replaced by numbers.
- **As built** (2026-10-05). [src/escape/terrain.rs](../../src/escape/terrain.rs):
  `TerrainRenderer`, a self-contained stage of the escape engine. It
  takes a tile (heights and albedo, in cell units) and a
  `TerrainView` (a `SolidCamera`, the Solid lighting, fog, shadow,
  softness, occlusion reach). T2 wires it to the config.
  - **Three passes:**
    - The maximum mipmap: level 0 a cell's highest corner, each level
      above the highest of a 2x2 block. Real mips of one `R32Float`.
    - The walk: mode D's 16-byte geometry record per pixel.
    - The relight: mode D's rig, spliced as the same WGSL text
      (`assembler::ifs_rig_plain`), so a terrain and an IFS solid
      cannot disagree about a light.
  - **The traversal.** Skip a node the ray stays above, descend where
    it does not, ascend a level after every skip. At the leaf, the
    exact bilinear-patch intersection: a quadratic in t, rebased at the
    cell's entry.
  - **Decisions made while building:**
    - **The mipmap is allocated at power-of-two sides.** A texture's
      mips halve rounding down and the node grid halves rounding up.
      They agree only at powers of two; otherwise the top levels' last
      nodes fell outside their mip. The build's writes were dropped
      and the walk read zero there: 57 wrong pixels in 19,200 and
      distances 29 cells off on the first run.
    - **A step never goes backward and is sized to the coordinates.**
      With the eye 2,200 cells out, a point just past a node's edge
      could round back into the node it left. Its exit then lay behind
      t, and the ray crawled: rays reached the 4,096-step cap, and the
      mean primary steps read 68–208 instead of 8–21.
    - **Occlusion comes from the horizon, not rays.** In eight
      directions, the steepest rise within the reach. A height field's
      own occlusion: deterministic, so there is no noise for a lit
      frame to show. Ray occlusion is the path tracer's (T3).
    - **The penumbra is measured at the leaves.** It is how close a
      shadow ray passed the surface at the cells it descended to,
      over its distance: mode D's `k·d/t` in a height field's terms.
      The coarse nodes' maxima would understate the clearance and
      darken every penumbra, so they are not used.
    - **The tile is a slab.** A ray entering through a side under the
      surface meets a wall, shaded by its face's outward normal and
      open to the sky; the albedo runs down it like a cross-section.
      The slab's floor is 1% of the tile's longer side below its
      lowest point. A wall is recognised only ON the boundary: a point
      a rounding under a steep flank is the flank (it was misread as a
      wall on the spike field until this rule).
    - **The solid camera's yaw is the eye's azimuth about the target.**
      So −90° puts the eye south of the target, looking north; T2's
      default camera uses that.
  - **Gates, all passing:**
    - Hits against the CPU reference on the four terrains at 160×120:
      no hit/miss disagreement on any pixel. Distances to 7e-4 cells
      (the gate is 1e-4 of the tile, 0.026). Normals to 1e-3, the f16
      packing, including 100–1,200 wall hits per terrain.
    - The CPU reference itself against a dense march with bisection:
      to 1e-3 on 400 random rays.
    - The mipmap's every level equals the CPU maximum.
    - A flat plane under one sun: `albedo·E·cos θ` to 3e-8.
    - A light's colour, power or fog is a relight (no walk, the same
      picture a fresh walk makes); its direction walks.
    - The shaders pass the fast-math lints.
    - Mode D is untouched: only `ifs_rig_plain` was added beside it.
  - **Measured** at 1080p over 2049² tiles, on this machine's GTX 1660
    SUPER, batched:

    | terrain | primary steps, mean / max | all rays, steps a pixel | walk + relight | relight alone |
    |---|---|---|---|---|
    | sinusoid | 7.7 / 80 | 14.8 | 2.5 ms | 0.3 ms |
    | cone | 8.4 / 162 | 10.5 | 1.8 ms | 0.3 ms |
    | step | 7.9 / 64 | 14.6 | 1.9 ms | 0.3 ms |
    | spikes | 21.2 / 190 | 35.7 | 5.1 ms | 0.3 ms |

    The estimate was 3–6 ms for the lit tier, so it lands inside it.
    Timing one submission at a time measured latency, not work: rays
    stuck at 4,096 steps "cost" the same 0.6 ms. The measurement
    batches twenty.
  - **Seen** (`output/heightfield_t1/mandelbrot_de.png`). The
    prototype's distance terrain through the GPU renderer: plateaus,
    filigree cliffs and spirals, a cross-section along the near wall.

**T2 — Escape terrain, lit.**
- **Built.**
  - `escape.terrain` and its panel section.
  - The footprint render, tiled.
  - The three sources, with distance the default and the iterate pass
    writing DE.
  - Interior plateau, lake or hole.
  - Re-rendering on `escape_dirty`, and the footprint following the
    camera (H11): during the zoom under a measured 15 ms, on release
    above it.
  - Fly mode driving the solid camera, for IFS solids as well.
  - `render_with`, export, video.
- **Gates.**
  - The tiled footprint equals the single render where both fit
    (byte-identical on the direct path, by tolerance on the
    perturbed).
  - A deep-zoom terrain at zoom 2^60 renders the same structure as the
    2D view (its footprint IS the 2D view).
  - **The 15 ms line, measured.** Footprint re-render times at the
    motion and full sizes, direct and perturbed, on this machine (its
    GPU named in the record). That says which paths zoom live on it,
    and what "average hardware" can expect.
  - Terrain-off configs are byte-identical.
  - The gallery modules build without the terrain path, and render a
    terrain config as its 2D picture.
  - An `escape-terrain-*` visual baseline.
- **Built in four steps:** T2a the footprint into a tile, T2b the
  orchestrator, camera mapping, `render_with` and export, T2c the app
  and panel, T2d fly mode.
- **T2a as built** (2026-10-05): the footprint writes the height
  source, and the ingest makes the tile.
  - **The config.** `EscapeConfig.terrain` (`TerrainConfig`,
    skip-if-default): `enabled`, `source` (Distance, the default;
    EscapeCount; Relief), `height` (0.06 of the footprint), `de_width`
    (1/150), `resolution` (2048), `interior` (Plateau, the default;
    Hole). `EscapeConfig::wants_derivative` is the relief's or the
    terrain's, and it is what all three derivative decisions now read.
  - **The height source is written into G, not B** as section 5
    planned. G is already the relief's source channel, and
    `esc_relief_source` already chooses what goes there, so the
    terrain's sources became two more of its codes (8 distance, 9 the
    smooth count). B stays the relief's stored slope. No new pass, no
    new binding, and nothing changes while the terrain is off.
    - **Distance** is `r·ln r / |dz|` with `dz` per render pixel: the
      estimate comes out in pixels, which are the tile's cells, so
      `w` is in cells too. A formula without a derivative falls back
      to the count in the shader; T2b must also tell the ingest so
      (`derivative_gap`), or it reads a count as a distance.
    - **A pixel that did not escape** writes −1e30, but only where the
      colouring draws the interior. Otherwise the interior is the
      pixels with no coverage (alpha 0). The ingest and the gate read
      it both ways.
  - **While the terrain is on**, the renderer keeps the full height
    field and does not run the 2D relief shade in the resolve: the
    terrain is lit in 3D.
  - **The ingest** (`TerrainRenderer::set_tile_from_escape`, two
    passes): the source's range over the escaped pixels (atomic
    min/max on an order-preserving float encoding), then heights in
    [0, H] and the albedo.
    - Distance: `H·exp(−d/w)`. Count: a log curve over the range.
      Relief: linear over the range.
    - The interior: a plateau at H in the background colour, or a hole
      (the slab's floor, albedo alpha 0).
    - Rows are flipped: the picture's top is the tile's north.
  - **Not yet:** the lake interior. It needs a material, so it comes
    with T3's GGX. The supersample of 1 is forced by T2b's
    orchestrator, which owns the footprint's render, not by the
    renderer.
  - **Gates, all passing:**
    - The height source against the f64 Mandelbrot at 96²: every pixel
      that escapes within 200 iterations and lies at least 0.01 px
      outside the set agrees to 0.1% (distance worst 5.2e-4, count
      2.3e-5). 6 pixels of 9,216 escape on one side and not the
      other.
      - Closer to the set, the f32 orbit parts from the f64 one: the
        distance is off by percents (2.9x at 1e-6 px) and the count by
        up to two iterations. A distance height has saturated to the
        plateau there, so it does not show.
    - The ingest against the same footprint: heights to 9.5e-7,
      albedo to 4.9e-4 (the f16), interior pixels a plateau.
    - All 111 escape baselines byte-identical; the shader dumps and
      `release.py check` unchanged.
- **T2b as built** (2026-10-05): the terrain through `render_with`, so
  the CLI, thumbnails and video draw it.
  [src/escape/footprint.rs](../../src/escape/footprint.rs):
  `EscapeTerrain`, the footprint's `EscapeRenderer` and the
  `TerrainRenderer`, and the mapping from the config to both.
  - **The `terrain` cargo feature**, on by default. The gallery modules
    are built without it (H2), so their downloads do not carry it.
    `EscapeConfig::terrain_active` is the one question everything asks:
    switched on, for a formula that draws a plane (a solid IFS is
    already mode D's 3D), in a build with the feature. Without it a
    terrain config renders as its 2D picture, relief and all.
  - **The footprint is the view.** The config's own centre, zoom and
    rotation, square, at `resolution` (clamped to the device's texture
    side), with supersample forced to 1. The view's `rotation` turns
    the footprint, as it turns the 2D picture, instead of rolling the
    screen as it does in mode D.
  - **What re-renders it.** The footprint is keyed on its picture
    alone: the escape config with the camera and the 3D-only terrain
    fields set to their defaults; the palette and every setting that
    writes its texture, by CONTENT; and the flame, when the formula is a
    2D IFS that draws it. The flame renderer's palette generation could
    not serve: every config load bumps it, so each video frame would
    have re-rendered an unchanged footprint.
  - **The camera, decided while building** (all in the tile's cells,
    where the footprint is `n` cells across at any zoom):
    - **The target is the view's centre**, not mode D's
      `cam_target_*`, which terrains leave unused. One centre for the
      2D picture and the terrain: switching between them keeps the
      place, and the footprint is centred on the target by
      construction.
    - **The target stands at the terrain's top, H.** With any pitch
      above the horizon the eye is then above every point of the
      terrain, including while a footprint lags a dolly (T2c).
    - **`cam_yaw` 0 looks north**, up the 2D picture: the frame's yaw
      is `cam_yaw − 90°`. So a straight-down view is the 2D picture
      upright.
    - **The distance is 1.3 footprint widths** (`FRAME_DISTANCE`): the
      whole footprint in a 16:9 frame at the default field of view and
      pitch, with a tenth to spare. A narrower frame crops the near
      corners, as any fixed vertical field of view does.
    - **The pitch is the shared default, 24°**, not section 8's 35°.
      It is mode D's default and the same field. Section 8's 35° on
      enabling is the panel's to set (T2c).
  - **What mode D keeps as formula parameters**, Shadows, Shadow
    Sharpness and Occlusion Reach, are `TerrainConfig` fields:
    `shadow` 0.7 and `shadow_sharpness` 12 (mode D's defaults), and
    `occlusion` 0.006 of the footprint, about the distance flanks'
    width.
  - **Fog is measured in footprint widths**, as mode D's is in the
    attractor's units.
  - **Antialiasing is the config's supersample, as accumulation:**
    `supersample²` renders, each with its rays jittered within the
    pixel (spliced into `ifs_ray` at its lens marker, so mode D's text
    is unchanged), folded into a running mean. The mean is taken
    premultiplied and stored straight, the accumulator contract, so a
    silhouette's edge is the surface's colour at a fraction of its
    coverage.
  - **The plateau's colour** is the interior's own where the colouring
    draws one, the background's where it does not (section 5; T2a had
    only the background).
  - **`render_with`** branches to `render_escape_terrain` after the
    config load. The escape path's preparation (a 2D IFS's analysis, a
    texture's image) and its tail (density effects, tonemap, colour
    effects, readback) were moved into `prepare_escape` and
    `escape_tail`, which both paths call. The 2D path's calls are
    unchanged in order: all 111 baselines are byte-identical.
  - **`RenderEngines.terrain`** keeps the `EscapeTerrain` across a
    video's frames, so a camera move between frames is a 3D render
    alone.
  - **Memory** is checked before allocating: the footprint as an
    escape render at `n²`, and the terrain's 64 B per output pixel (the
    geometry record, the render and the accumulation's pair) against
    the storage-binding and texture limits.
  - **Gates, all passing:**
    - The camera's conventions: the target at the screen's centre,
      north up and east right at yaw 0, a quarter turn of yaw looking
      east, the eye above the terrain, the footprint's eight corners
      inside a 16:9 frame.
    - The footprint's key ignores the camera, the heights, the
      interior, the shadow and the occlusion, and not the source or the
      zoom.
    - Through `render`: sky across the top row, lit terrain at the
      centre, the same bytes twice, and not the 2D picture.
    - A caller-owned engine renders the footprint once over camera,
      height and light edits, and again for a zoom; its frames equal
      a fresh render's.
    - Video's shape (one flame renderer across frames): four frames,
      one footprint; a palette rotation, a second, and it shows.
    - **Deep zoom** (`fe-zoom-60-edge`, 2^60, the perturbed floatexp
      path): the footprint's colour equals the 2D render's exactly
      (worst difference 0), and its distance estimates are in pixels,
      median 11.9 px, 95% under 54: the sizes they have at any zoom.
    - The `escape-terrain-seahorse` baseline (112 escape baselines
      pass), and `release.py check`, including the gallery builds
      without the feature.
  - **Measured** on the GTX 1660 SUPER, CLI, including the 2048²
    footprint:
    - the seahorse valley (1,024 iterations) at 800×600 with 4 samples:
      84 ms;
    - 2^60 at 30,000 iterations, 1280×720: 1.4 s, nearly all of it the
      footprint.
  - **Seen** (`output/heightfield_t2/`): the whole set, the seahorse
    valley by distance and by count (the count's spikes, as section 2
    found), the interior as a hole, and 2^60.
- **T2c as built** (2026-10-05): the terrain in the app.
  - **The frame loop** holds an `EscapeTerrain` instead of the 2D
    `EscapeRenderer` while the config draws a terrain, and frees
    whichever is not in use: the footprint is a full escape render of
    its own. Each frame:
    - the footprint renders a chunk when its picture changed
      (progressive, reference orbits on the worker thread, as the 2D
      view's), and is ingested when it settles;
    - a height, flank or interior edit re-ingests the footprint it
      has;
    - the viewport renders one sample of the config's antialiasing
      grid (the export's own `supersample²` jitters) into the
      accumulation, which any change to the view or the tile restarts.
      So a still viewport converges to the export's picture.
  - **H11, as built.** A footprint's time is taken from its first chunk
    to the GPU's completion of its ingest (`on_submitted_work_done`),
    smoothed. During a gesture (the 250 ms interaction window) a stale
    footprint re-renders only if that time is under 15 ms
    (`LIVE_FOOTPRINT_MS`) or unmeasured; otherwise the old one is drawn
    with the camera moved over it (`terrain_camera_over`: the target
    offset by the pan since, the eye nearer by the zoom since, the
    offset subtracted in fixed point so it holds at any depth), and the
    new one renders when the gesture ends.
  - **The 15 ms line, measured** (GTX 1660 SUPER; a dolly's steps of
    zoom, each timed to the GPU's completion):

    | footprint | direct, 2^3, 2,000 iterations | perturbed, 2^60, 30,000 iterations |
    |---|---|---|
    | 512² | 2.1 ms | 77 ms |
    | 1024² | 6.1 ms | 278 ms |
    | 2048² | 21.9 ms | 910 ms |

    So on this GPU a shallow terrain follows the camera live up to
    about 1024², and a deep one on release. The motion footprint
    (half the side while moving) would put a shallow 2048² under the
    line; it is not built.
  - **The Escape panel** has a 3D Terrain section above the relief:
    the switch (disabled, with the reason, for a solid formula), the
    source (with a note where the formula has no derivative), height,
    flank width, interior, footprint size, shadows, shadow sharpness,
    occlusion reach, fog and fog start, and mode D's camera angles.
    The target row is not shown: a terrain's target is the view's
    centre.
  - **`visibility.rs`**: `Solid::of` says yes for a terrain, so the
    Solid Lighting panel is offered.
  - **Gestures in the viewport**, the orbit viewers' convention: drag
    (or Alt+drag) orbits, Shift+drag or a right-drag pans, the wheel
    moves in. The pan moves the target across the ground -- along the
    camera's right, and along its heading foreshortened by the pitch --
    and hands it to the plane's own exact-decimal pan as a drag of the
    footprint picture. The wheel is the plane's zoom without its
    cursor anchor: the point under the cursor is on the ground, at a
    depth the plane's anchor knows nothing of.
  - **Nine ConfigPaths** (`Escape.Terrain.*`). Height, flank width,
    shadows, sharpness and occlusion animate; the switch, the source,
    the interior and the footprint size do not.
  - **The in-browser export's hand-rolled path** gained the terrain:
    the footprint settled in fixed chunks without waiting, as its 2D
    export settles, then the tile and the antialiasing grid. The
    in-app viewport-size and transparent exports read the terrain's
    image where they read the 2D picture's.
  - **Not yet:** fly mode (moved to the end of the plan, the user's
    call, 2026-10-05); the motion footprint; a readout of the measured
    footprint time in the panel.
  - **Seen in the app** (the user, 2026-10-05): the picture's quality
    is the footprint's. 2048² and 4096² look good; near the camera it
    wants 8192² to 16384², and more antialiasing does not reach past
    the footprint's texels. That is the uniform footprint's limit: a
    perspective view wants texels small near the eye and large far
    away. See section 11.
- **T2d as built** (2026-10-05): the footprint sized by the view, built
  in tiles, its colour filtered (section 11's decision).
  - **The size** (`wanted_resolution`): with `resolution` 0, the new
    default, about `supersample` texels per screen pixel where the
    terrain is nearest the eye -- the fractal sampled as finely as a 2D
    render at that antialiasing. The nearest point is a grid of the
    camera's rays cast at the terrain's box. Capped at 8192 (about
    1.4 GB of tile, twice that while a build replaces one); a fixed
    `resolution` is still itself.
    - At 1080p in the default framing: 1792² without antialiasing,
      3584² at 2x.
    - The viewport follows the camera's want only when it would grow
      by a fifth or shrink by half, so an orbit does not re-render the
      footprint at every step. An export takes its own pixels' want
      exactly.
  - **The tiles** (`FootprintLayout`): a footprint past 2048 a side is
    `per_side²` escape renders, each a multiple of 256 a side, each at
    its own centre (`tile_config`: the offset kept as a power of two
    and a mantissa, added in fixed point, so it holds at any depth).
    They arrive region by region into a tile being built, and the
    drawn tile is replaced only when the last has.
    - **Against the whole render**, 2x2 at 2^2 rotated: 0.05% of the
      raw samples and 0.5% of the colours differ, scattered (17 of the
      1,249 within 2 px of a seam, where chance puts about 25). Not byte
      for byte, as section 5 hoped: each tile computes its pixels'
      coordinates from another centre, and where the last bit moves an
      escape count the colour changes.
    - **Deep tiles share the reference orbit:** a 4096² at 2^60 takes
      4.1 times the 2048².
  - **The tile keeps the RAW height source**, and the walk maps it to
    heights (`hf_f`): the distance's `H exp(-d/w)`, the count's log
    curve, the relief's linear one, with the interior's sentinels.
    Every map is monotone, so the mipmap keeps the raw extreme that maps
    highest (the least distance, the greatest count) and the walk maps
    the bound too. A tile set from heights maps by the identity.
    - Why: a tile no longer survives in the escape renderer (only its
      last region does), so a height or flank-width edit would have
      re-rendered the whole footprint. Now it is a uniform: a re-walk,
      milliseconds.
    - The count's and the relief's range is measured over every region
      into the tile's own buffer, which the walk reads; nothing is read
      back.
    - The interior's encoding (plateau or hole) is in the footprint's
      key: changing it re-renders.
  - **The colour is filtered** by each ray's share of a pixel: the
    albedo has a mip chain (2x2 box averages), and the relight samples
    it trilinearly at `log2(t · pixel / samples_per_axis / √cosθ)`
    cells. Distant ground is its texels' average, not whichever one a
    ray struck. Gate: a one-cell checkerboard seen at about eight
    cells a pixel comes out as even as a uniform grey tile (standard
    deviation 0.0000).
  - **The shadow speckles fixed.** At 8192² the plains shadowed
    themselves in speckles, with a thin dark line. A shadow ray started
    a fixed thousandth of a cell above its hit, which at coordinates
    near ten thousand is the rounding; on curving ground it began under
    the surface. It now starts `1e-3 + 1e-6 × span` above (span the
    larger of the tile's side and the eye's coordinates).
    - Confirmed by restoring the old bias alone: the speckles and the
      line came back.
    - The gate is gently curving ground (slopes under 4°, a 30° sun)
      seen from 12,000 cells: 35 self-shadowed pixels at the old bias,
      0 now. A flat plane does not show it; there the interpolated
      normal is the patch's own.
  - **Measured** (GTX 1660 SUPER), footprint renders as a dolly makes
    them:

    | footprint | direct, 2^3 | perturbed, 2^60, 30,000 iterations |
    |---|---|---|
    | 2048² | 18.6 ms | 915 ms |
    | 4096² (4 tiles) | 75.7 ms | 3.7 s |
    | 8192² (16 tiles) | 290 ms | -- |

    The seahorse at 1080p, 2x: the auto footprint (3584², 4 tiles)
    renders in 168 ms, against 773 ms at a fixed 8192², and differs
    from it by 0.57 in 255 on average. The near crops are hard to tell
    apart (`output/heightfield_clip/`).

**T3 — Path tracing, for terrains and IFS solids.**
- **Built.**
  - Progressive accumulation.
  - The environment light (the background colour).
  - NEE with soft shadows; bounces with Russian roulette.
  - Lambert plus GGX; emission; the lake.
  - Fog as single scattering; DoF.
  - The Auto tier; samples in the progress bar.

  On the shared rig, so mode D's IFS solids are path traced too.
- **Gates.**
  - **The white furnace:** albedo 1, a flat infinite terrain (Repeat),
    a uniform environment L, no sun. Every bounce count converges to L,
    to 1% at 4096 samples.
  - Sun-only Lambert matches the T1 analytic case.
  - Batch invariance: the same samples in any number of dispatches,
    bit for bit.
  - Determinism: same config, same image.
  - Path-traced visual baselines (a terrain and an IFS solid) at a
    fixed sample count, compared by tolerance, since a Monte Carlo
    image at 64 samples is not bit-stable across drivers.

- **Built in steps:** T3a the terrain path tracer's core; T3b GGX,
  emission, fog as single scattering, depth of field, the firefly
  clamp's tuning; T3c mode D's IFS solids on the same core; T3d the
  lake, the progress bar, the panel's polish.
- **T3a as built** (2026-10-06):
  [src/escape/terrain.rs](../../src/escape/terrain.rs) (`PATH_WGSL`,
  `render_path`) and [src/escape/footprint.rs](../../src/escape/footprint.rs)
  (`path_settings`, the tiers).
  - **The integrator.** A sample: a ray jittered within its pixel; at
    each surface the lights sampled over their angular size with a
    shadow ray, then a cosine-weighted bounce whose escape sees the
    environment, with Russian roulette from the second bounce. Lambert
    on the filtered albedo; a hole's floor is the sky through it.
    Coverage is the primary hit's, faded by the fog as the lit tier
    fades it.
  - **The lights are the rig's.** The rig's convention (`albedo × I ×
    diffuse × cos θ`, shadow strength mixing the visibility), so a
    sunlit surface reads alike in both tiers. A light's angular radius
    is half the inverse of the Shadow Sharpness, the lit tier's penumbra
    in an area light's terms.
  - **The environment** is the background colour brought into the
    accumulator's units (through the inverse of the Linear tonemap's
    exposure and gamma) times Sky light: the sky and an albedo-1 surface
    it lights read as the background does.
  - **Fireflies** are clamped at ten times the brightest light.
  - **Batch invariance.** Each invocation adds its pixel's samples, in
    order, to a running SUM (a storage buffer, premultiplied), and a
    resolve writes the mean into the output. A sample's random numbers
    are keyed by (pixel, sample index, seed). So the sum is the same
    bits however the samples are split into dispatches, and a seed fixed
    at 1 makes a video's noise steady from frame to frame.
  - **The tiers** (`TerrainConfig.tier`, plan section 8):
    - **Auto**, the default: the lit tier's antialiasing grid while
      anything moves; path-traced samples while nothing does, as many a
      frame as fit 12 ms at the measured cost, up to `samples` (256).
      The path tracer's picture replaces the lit one from 8 samples.
    - **Lit** and **Path traced**: one tier always.
    - Exports path trace at `samples` unless the tier is Lit, in
      batches of about a quarter second.
  - **Config:** `tier`, `samples` (256), `bounces` (2), `environment`
    (1); the panel's Rendering, Samples, Bounces and Sky light.
  - **Gates, all passing:**
    - **The white furnace:** albedo 1, a uniform environment L, no
      light. A flat plane is exactly L at 1, 2 and 4 bounces (worst
      error 0). A sinusoid's valleys at 24 bounces and 1,024 samples
      average L to four digits in every channel.
    - **Sun only:** with no environment and a near-point sun, the path
      tracer's plane equals the lit tier's `albedo E cos θ`, to 3e-8.
    - **Batch invariance:** twelve samples as 12, 3 and 1 dispatches
      (sun, sky, three bounces, a sinusoid) are bit-identical sums, and
      again on a second pass: determinism.
    - Baselines: `escape-terrain-seahorse` path traced (256 samples,
      re-baselined) and `escape-terrain-seahorse-lit`, the lit tier.
      113 escape baselines and `release.py check` pass.
  - **Measured:** the seahorse at 1080p, 256 samples, 2 bounces: 6.4 s
    with its sections, about 23 ms a sample on the GTX 1660 SUPER. The
    viewport gathers a sample a frame there and shows the path tracer
    from about 0.2 s after the camera stops.
  - **Seen** (`output/heightfield_t3/`): the seahorse path traced and
    lit. The sky light fills the shadows and tints the faces that see
    the sky blue; 256 samples are clean.
- **T3b as built** (2026-10-06): the material and the lens, in the same
  `PATH_WGSL`.
  - **The coat.** Lambert under a GGX gloss coat: reflectance `Gloss`
    at normal incidence (Schlick's Fresnel on a scalar), roughness
    `Roughness` (alpha its square), Smith's separable masking. The
    diffuse takes what the coat's Fresnel does not. The lights' NEE adds
    the coat's physical BRDF times the rig's irradiance. A bounce picks
    the coat's lobe with the probability of its Fresnel at the view
    (clamped to 0.1-0.9) and samples it by the visible normals (Heitz
    2018); otherwise Lambert's cosine lobe, carrying the albedo times
    what the coat lets through. A gloss of 0 is no coat at all, not
    Schlick at 0 -- Schlick is nonzero at grazing angles even there, and
    that broke the white furnace before it was gated out.
  - **Emission.** `Glow` adds the albedo times itself at every surface
    a path meets: the fractal lights itself and, by bounces, the ground
    around it. The firefly clamp counts it as a light.
  - **Depth of field.** A thin lens: each sample's ray starts from a
    point of a disc of radius `Aperture` (view widths) and aims where
    the pinhole's ray meets the focal plane, at view depth `Focus`. A
    focus of 0 is the target's distance, so the default lens is sharp
    where the camera looks. Aperture 0 is the pinhole, and the same bits
    whatever the focus.
  - **Fog stays the coverage fade.** Single scattering of a uniform
    environment through a homogeneous medium is exactly `L(1 - T)` over
    the surface's `T` -- the lit tier's fade toward the background,
    which the tonemap's background blend already is. The sky being the
    background colour, the two are the same picture at Sky light 1; at
    other values the fog stays the background's colour, as a viewer
    would expect of a horizon. A separate march would add noise and
    nothing else. A coloured fog, or sun shafts, would need the march
    (T5).
  - **Config:** `gloss` (0.04, a dielectric's: stone, varnish --
    **reverted to 0 in T3c**, below), `roughness` (0.5), `emission` (0),
    `aperture` (0), `focus` (0, the target); the panel's Gloss,
    Roughness, Glow, Aperture and Focus, shown when the tier path traces.
    All five animate. (T3c moved them to `escape.path` and put the lens
    in the target's terms.)
  - **Gates, all passing:**
    - **Glow alone** (no light, no sky) is the albedo times the
      emission, worst error 0.
    - **The furnace with a coat:** an albedo-1 plane under a uniform
      sky L. The 0.04/0.5 coat averages 0.995 L (the single-scattering
      microfacet's known loss, held under 3%), a near-mirror (1, 0.05)
      1.0007 L; neither exceeds L by more than 0.5%.
    - **The lens:** a pinhole is bit-identical at two focus distances;
      a lens 20 cells wide focused on a chequered plane matches the
      pinhole to 0.001; focused 140 cells short, the chequer's contrast
      falls from 0.302 to 0.022.
    - `escape-terrain-seahorse` re-baselined (the default coat);
      113 escape baselines and `release.py check` pass.
  - **Seen** (`output/heightfield_t3/`): `coat.png`, the default, a
    faint sheen on the faces toward the sun; `dof.png` (aperture 0.03),
    the target sharp and the near and far ground soft; `glow.png`
    (emission 0.6, sky light 0.3), the filaments lit from within. Each
    about 6 s at 1080p and 256 samples: the coat costs little.
- **T3c as built** (2026-10-06): mode D's solids path traced, on a core
  the terrain now shares.
  - **The core** ([src/escape/path_core.rs](../../src/escape/path_core.rs)):
    the settings (`path_settings(config, target)`), the uniform (with a
    row band), the WGSL -- the random stream, the lens, the coat, and
    `pt_path`, the integrator from a path's first surface on -- and
    `PathSum`, the per-pixel sum and its resolve. A geometry supplies
    `pt_eye`, `pt_scene_begin`, `pt_scene_next` (the surface a ray
    leaving a surface reaches, its normal on the side the ray sees),
    `pt_scene_visible` and `pt_sample` (the camera ray, the first
    surface, and what coverage and fog mean there). The terrain's
    `PATH_WGSL` is now that and nothing else; its tests pass unchanged.
  - **The solid's geometry** (`IFS_PATH_WGSL`, `assemble_ifs_path` in
    the assembler): the walk's shader with its entry point replaced, so
    the distance function, colouring, rig and a lens are the walk's own
    text. The camera ray marches as the walk's does (ball entry to exit,
    stopping within a pixel, the walk asked for a hundredth of one); a
    ray leaving a surface leaves as the walk's shadow rays do (two pixels
    off, from four out, a hit at a tenth of that or a ten-thousandth of
    the ball). A surface is the walk's: `ifs_normal` at the pixel's
    footprint, the albedo the relight's colouring, palette and
    brightness. Fog is the rig's colour mix, after the clamp.
  - **Normals stay outward.** The core used to turn every normal toward
    the ray, which a height field's two-sided walls need. A solid's
    grazing camera ray can stop within its tolerance BESIDE the
    silhouette, where the gradient faces slightly away; turned round, it
    sent the bounce into the solid. Measured on a cube in the furnace: 4%
    of the energy lost, all at the silhouette. The turn now lives in the
    terrain's surface, and the furnace is exact.
  - **Banded** ([src/escape/renderer/solid_path.rs](../../src/escape/renderer/solid_path.rs)):
    a sample is several walks, and a walk at 1080p is already hundreds of
    milliseconds, so a sample is a pass of row bands under the walk's
    watchdog budget, sized by the walk's own model with the path's
    marches counted (`(1 + bounces) * (steps * (1 + lights) + 7)` per
    pixel). The viewport's bands are a sixth of an export's. Where a
    whole frame fits a dispatch, a dispatch takes several samples. The
    resolve reads a part-done pass row by row (rows above the band have
    one sample more), and a row no sample has reached is empty. It traces
    display pixels: the jitter is the antialiasing, and a path-traced
    export renders at supersample 1.
  - **Tiers** (`escape.solid_tier`, `RenderTier`): **Lit by default**,
    so every solid saved before this renders as it did. Auto walks while
    anything moves and path traces once the walk settles, shown from 8
    samples; Path Traced skips the walk and shows from the first band.
    Any walk restarts the path tracer (whatever made it walk changed the
    picture). `render_with` path traces at `samples` when the tier is
    not Lit; so does the browser's export, which had never handed mode D
    its flame (`set_ifs`, the coarse pass, the lights) and now does.
  - **Config:** the path tracer's settings moved from the terrain to a
    shared `escape.path` (`PathTraceConfig`): samples, bounces, sky
    light, gloss, roughness, glow, aperture, focus -- ConfigPaths
    `EscapePath*`, tracks `Escape.Path.*`. The terrain keeps its own
    tier; the solid's is `EscapeSolidTier` (`Escape.SolidTier`). The
    lens is in the TARGET's terms -- aperture a fraction of the distance
    to it, focus a multiple of it -- so it keeps its look as the camera
    dollies: a terrain's world is view widths with the target 1.3 away,
    a solid's is the attractor's, the target the eye's offset away. The
    panel's `show_path_tracing` block serves both sections.
  - **Two decisions taken back, after the user's look in the app.** The
    first solids were worse than the lit tier: grey, speckled dark faces.
    - **The coat.** T3b's default 0.04 coat is physically fair, but most
      of a fractal solid's visible surface is faces at a glance, where
      Fresnel takes the coat to a mirror of the sky -- the background's
      colour -- and its lobe, chosen at least a tenth of the time,
      returns it weighted. Measured: the same render without the coat is
      red where the coat's is grey. **Default gloss is 0**, Lambert
      alone.
    - **One light a surface.** Picking a light by power saved a fifth of
      a sample, but a face only one of two lights reaches then reads all
      of it or none, sample to sample: speckle over the lit faces. Back
      to every light, with no shadow ray at shadow strength 0.
    - And with no coat there is no draw to choose a lobe, so Lambert's
      random stream is T3a's again: the T3a seahorse
      (`output/heightfield_t3/seahorse-pt.png`, the user's reference for
      "flawless") re-renders at 1080p and 256 samples to within ONE
      level, mean 8e-7. Its baseline is back to that look.
    - Coverage was suspected and ruled out: the path tracer's mean
      coverage is the lit tier's (0.1007 against 0.1010 on the
      tetrahedron), 99.9% inside patches the walk sees as solid, the
      same at 96 and 384 march steps.
  - **Gates, all passing:**
    - **The furnace on a solid:** a cube (eight half-scale corner maps,
      convex), albedo 1, sky L, no light: every pixel L exactly at 1 and
      3 bounces.
    - **Sun only:** the cube's face interiors equal the lit tier's
      relight to 4e-5.
    - **Bands:** three samples as whole frames and as one-row bands are
      bit-identical sums, twice over.
    - **Coverage:** the tetrahedron's, as above.
    - Every solid formula and colouring assembles and validates, with
      and without a lens.
    - Baselines: `escape-ifs-solid-tetrahedron-path` (the preset, 16
      samples, 800x600, about 8 s) new; `escape-terrain-seahorse` back to
      Lambert. 114 escape baselines and `release.py check` pass.
  - **Measured** (GTX 1660 SUPER, 1080p, two bounces, the presets' key
    and fill): the tetrahedron about 1.3-1.6 s a sample against 0.34 s
    for its lit render (0.4 s at no bounce), the Menger sponge about 4 s
    against 0.7 s. The first bounce is the cost: diffuse rays wander into
    the fractal's own structure, and a march near surfaces takes small
    steps. 64 samples of the tetrahedron are clean, antialiased and
    match the lit tier's colours.
  - **Found on the way:** a config saved WITHOUT `version` loads as v0,
    and the v2-to-v3 migration takes the render mode from the flame --
    2D -- over a top-level one. The first visual config was written by
    `serde_json` directly and loaded as a flame. Configs are saved
    through `to_json` now; the loader is unchanged.
  - **Open:** stratified samples (fewer for the same noise) would change
    the noise of every path-traced picture, the seahorse's included --
    the user's call. The loader could keep a top-level render mode it
    finds.

**T4 — Simulations.**
- **Built.**
  - `sim.terrain`.
  - The shared solid camera on `SimConfig`, with its ConfigPaths and
    the generalized panel.
  - The grid-resolution colour pass; the relief stage run for the
    terrain.
  - The live lit tier: the maximum mipmap rebuilt each step.
  - Repeat tiling on the periodic boundary.
- **Gates.**
  - A running simulation in 3D at 1080p stays inside the interactive
    budget.
  - Terrain-off configs are byte-identical.
  - A `sim-terrain-*` visual baseline.

**T5 — Reach and polish**, each its own decision when it comes up:
- a denoiser: à-trous guided by albedo and normal, which
  `shaders/atrous.wgsl` already implements for flame normals;
- a far field: a second, coarser footprint around the first, for
  horizon views of escape and a smooth handover as the footprint
  follows the camera (H11);
- a gradient sky and an HDRI environment;
- output tiling for very large stills;
- a filmic tonemap curve;
- volumetric sun shafts;
- presets.

## 10. The user's answers (2026-10-05)

1. **The camera.** The escape solid camera, not the flame camera. The
   user's reasons:
   - it is the better renderer: higher quality, and at least as fast;
   - "the quality of voxels or splatting leaves much to be desired".

   No option ever splatted. The first draft borrowed only the flame
   camera's FIELDS for rays. But the choice stands on its own merits:
   H2, H4, and the reordered phases.
2. **Beyond the tile:** just the background colour, for now (H6). The
   environment light is the same colour (section 4).
3. **The escape height:** distance by default (H8).
4. **The interior:** plateau by default (section 5).
5. **Order:** "might be moot depending on the camera choice". With
   mode D's camera and pipeline, escape comes first (T2) and
   simulations follow (T4), since the escape engine hosts the
   pipeline. Path tracing (T3) sits between them and lands on IFS
   solids as well.
6. **Lights:** fixed in the world (H7).

Two more, the same day:
7. **Re-rendering during a deep zoom:** yes, "if we can re-render at
   <15 ms rates on average hardware". Decided per device by measuring
   (H11).
8. **The gallery modules skip terrains** (H2).

## 11. The footprint's resolution, and the camera (decided, 2026-10-05)

The user's question, after T2c: "Any chance we can tie the Footprint to
the camera frustum?" The first answer below was the clipmap. A
measurement before building it changed the answer, and the user chose
the second: **the footprint's resolution tied to the screen, rendered
in tiles, its colour filtered** (built as T2d). The clipmap waits for a
terrain that runs toward the horizon.

**The problem.** A footprint is uniform: every texel is the same size
on the ground. A perspective view is not: a texel near the eye covers
many screen pixels and one at the horizon a fraction of one. So the
near ground and the cliff faces show the footprint's texels, and the
far ground is sampled more finely than the screen can show.
- Raising the footprint everywhere pays for the far ground too:
  16384² is 64 times 2048²'s pixels, and at the measured 22 ms per
  direct 2048² that is over a second per footprint.
- Antialiasing cannot help near the eye: where a texel is larger than
  a pixel, extra rays land on the same interpolated texels. It adds
  real detail only where texels are about a pixel or smaller.

**Three ways to tie it to the camera:**
1. **Nested footprints, fitted to the frustum (a clipmap).** Several
   footprints, each twice the span of the one inside it, all `n²`:
   the finest where the ground is nearest, coarser ones outward to
   the far edge of the view.
   - **Where the frustum comes in.** The camera decides how many
     levels there are and where each sits: level `k` covers the band
     of ground at distances `[d_k, 2 d_k]` inside the view, so its
     texels are about a screen pixel there.
   - **What it costs.** Four levels at 2048² give the nearest ground
     the density of a 16384² footprint, for a sixteenth of its
     pixels.
   - **Deep zoom stays the 2D renderer's.** Every level is a 2D render
     at a power-of-two zoom of the same region, so one reference
     orbit can serve them all.
   - **What it subsumes.** T5's far field, and H11's handover: moving
     in, the next finer level already exists, so nothing pops.
   - **What the walk needs.** The ray steps through the levels from
     the finest out, each with its own maximum mipmap. A band where
     the levels blend, so the seam is not a step in the ground.
2. **One footprint, warped to the view** (the perspective shadow
   map's trick). Its texel grid is the screen's grid projected onto
   the ground: dense near, sparse far, in one render.
   - **The cheapest in pixels, and the most invasive.** The escape
     renderer needs a projective pixel-to-plane map, which its lens
     hook might host.
   - **The walk** happens in warped coordinates, where a ray's height
     is no longer linear across a cell. Both the bilinear-patch leaf
     and the mipmap's bound get harder.
3. **No footprint near the eye: each ray evaluates the fractal.**
   March against the height computed where the ray is, mode D's way:
   every step an escape iteration.
   - Unlimited detail, at about a hundred 2D renders' cost per frame.
   - A candidate for the last refinement of a path-traced still, not
     for the interactive view.

**First recommended: the first**, as its own phase before T3. The user
agreed ("Let's add a clipmap"); then the measurement:

**What the extra resolution buys** (the seahorse at 1920x1080, mean
difference from a 8192² footprint, by screen band):

| band | 2048² | 4096² |
|---|---|---|
| far (the tile's top) | 4.2 | 3.4 |
| middle | 2.3-3.0 | 1.8-2.4 |
| near (the bottom) | 1.9 | 1.4 |

- **Largest far, not near**, and nearly all on detailed pixels (13.0
  against 0.06 on smooth ground).
- **Near the camera, the gain is samples per pixel.** A 2048² footprint
  already gives about 1.2 texels a screen pixel at the tile's near
  edge, but each texel is one point sample of the fractal; the rays of
  a pixel land on the same one or two texels and average nothing new.
  At 8192² they average real fractal samples, as 2D antialiasing does.
- **Far away, the need is filtering:** both sparkle there, the texels
  smaller than a pixel and point-sampled.
- **A clipmap could not help in this view.** The whole default tile lies
  0.9 to 1.8 footprint widths from the eye, a range of two: its finest
  level would cover nearly all of it. Nested levels pay where the
  ground spans a wide range of distances -- terrain toward the horizon,
  a low pitch, a camera near the ground.

**Decided** (the user, 2026-10-05): the footprint's resolution tied to
the screen (about the antialiasing factor's texels a pixel where the
ground is nearest), rendered in tiles so 8192² fits, and mip-filtered
colour. The clipmap comes later, with terrain toward the horizon (the
far field, T5). Fly mode moves to the end of the plan.

## 12. Ground to the horizon (decided and built as T2e, 2026-10-06)

**The user's feedback after T2d:**
- "Everything is presented on a square plate in front of the camera.
  Zooming and panning are rather awkward because they control what's
  on top of the plate."
- "The end goal here is to make the plate cover the entire view."
- "Maybe something like one big stretched footprint, then divide it up
  into sections for progressive rendering? We could always use 2k x
  2k (or whatever is fastest), and just make them smaller in the pixel
  space to increase quality."
- "Fog would be on by default, but set to just before the max render
  distance."

**The shape: sections.** The ground is a quadtree of square sections
of the plane, out to a maximum distance from the eye.
- **Every section is the same render.** One escape render of a fixed
  side, a section's texels being its side over that.
- **Section sizes double with distance.** So the texels per screen
  pixel stay about the antialiasing factor everywhere (section 11's
  rule).
- **Built near first and coarse first**, a few a frame: a whole coarse
  view lands quickly, then refines.
- **Cached by their place in the plane.** A pan renders only the
  sections it uncovers. A dolly brings finer sections in near the eye
  and retires coarse ones, so a deep zoom refines rather than pops
  (H11's handover).
- **The far edge is hidden by fog:** on by default, reaching the
  background colour just before the maximum distance.

This is section 11's clipmap in the user's terms, at the point where
it pays: section 11 measured that nested levels buy little over a
plate spanning a 2x range of distances, and a lot over ground to the
horizon.

**Estimated** (a CPU model of the default framing at 1080p: rays to the
ground; a section split while its texels are coarser than its nearest
point wants; memory at the tile's 20 B a sample):

| maximum distance | AA | ground on screen | 1024² sections | to fill, direct path | memory |
|---|---|---|---|---|---|
| 4 view widths | 1 | 87% | 24 | ~110 ms | ~0.5 GB |
| 8 view widths | 1 | 98% | 34 | ~160 ms | ~0.7 GB |
| 8 view widths | 2 | 98% | 80 | ~370 ms | ~1.6 GB |
| 16 view widths | 1 | 100% | 38 | ~175 ms | ~0.8 GB |

- **Section side: 1024².** Per pixel it costs what 2048² does (4.4 ns
  measured; 512² costs 30% more). It fits the view more finely, so it
  needs half the pixels and half the memory of 2048² sections.
- **Memory** at AA 2 is the pressure. An 8-bit albedo (the chain
  already filters it) and texels sized for the slant of distant ground
  rather than its width would both cut it.

**What changes:**
- **The geometry.** The walk steps from section to section: each its
  own maximum mipmap; the section under a ray found through a
  quadtree index.
- **The heights must agree across sections.** A distance's estimate is
  stored in the plane's units, not a section's cells. A count's or a
  relief's range is measured once over the coarse sections.
- **Seams.** Neighbouring sections at different levels sample the
  surface at different spacings, which leaves a step at their border;
  a band where the finer section blends into the coarser closes it.
- **The camera stays mode D's.** But a pan moves the target over a
  landscape that stays put, and a dolly approaches it. The square
  plate, `FRAME_DISTANCE`'s framing of it and the footprint's single
  size all go.
- **Deep zoom.** Sections are placed relative to an anchor held in
  exact decimals and re-based as the camera travels, so the shader
  still sees only eye-relative numbers.

**Order: before T3** (the user's choice, 2026-10-06). The path
tracer's rays -- shadows, bounces -- walk whatever the ground is, and
should walk the sections from the start rather than be retrofitted,
the same reason section 11 gave for the clipmap.

### T2e as built (2026-10-06)

[src/escape/footprint.rs](../../src/escape/footprint.rs) (the
sections) and [src/escape/terrain.rs](../../src/escape/terrain.rs) (the
walk over them). The plate is gone: so are its framing, T2d's single
footprint size and tiles, and H11's live-or-release choice, since
sections stream.

- **A section** is one escape render of 1025² samples: 1024 cells,
  with its edge samples on its grid points, so neighbours share them.
  It is rendered at rotation 0, on a dyadic grid fixed in the plane.
  - **Its place:** level `L`, a side of 2^L anchor widths, and its
    index on that level's grid.
  - **Storage:** texture arrays, a layer a section, about 21 MB each
    (the raw samples, the albedo with its mip chain, the maximum
    mipmap). The arrays are sized to what the view wants plus a
    quarter, growing in eights to 64 (~1.3 GB).
- **The anchor is the config's.** An eight-octave band of zoom, and the
  centre truncated to a decimal lattice at least 2^12 view widths
  apart. So the sections are a function of the config. Once all
  wanted sections are in, the ground is exactly those, so the picture
  is the config's and the picture size's, the viewport's and an
  export's alike; while some are missing, other cached sections stand
  in. A pan or zoom out of the band or the lattice cell starts the
  sections over.
  - Found by a test: with the anchor taken from the view's history, a
    reused engine drew a different picture from a fresh render.
- **The world** is the current view width, x along Re, y along Im,
  with the origin at the eye's ground point. The shader sees small
  numbers at any depth.
  - **Heights are fractions of the view width**, applied in the walk.
    So a dolly in flattens the far ground and raises the near detail:
    the terrain looks like itself at every zoom.
  - **The View's rotation turns the camera's heading**; the sections
    themselves are rotation-free.
- **Selection, each frame:**
  - A root grid (at most 3x3) of sections at least `far` wide around
    the eye.
  - Each section is split while its texels are coarser than
    `supersample × detail` a screen pixel at its nearest point, inside
    the view widened by a fifth and within `far`, and no finer than
    2^-14 view widths.
  - Past 64 sections, the texels coarsen until the set fits.
  - The default view at 1080p wants 9 roots and 42 leaves.
- **Building:** roots first, then nearest first, each section an
  escape render in chunks.
  - The app runs up to four steps a frame, as many as fit ~8 ms at the
    measured section time. `render_with` builds them all before a
    sample is drawn.
  - A section the view no longer wants is dropped mid-render; the
    least recently wanted is evicted when the atlas is full.
- **The walk** finds the finest ready section under a point through a
  quadtree in a storage buffer, and goes region by region: the square
  over which one section answers, traced in that section's cells.
  - Shadows and occlusion use the same lookup.
  - A ray entering a section below its surface hits its side: the
    outer edge, or a step between levels; both are shaded as walls.
  - There is no stitching between levels. No seam is visible, and the
    gate below finds no ray falling through the ground.
  - A ray entering the ground's box is looked up a hair inside it:
    exactly on the edge a rounding put it outside. That cost 442 of
    19,200 rays on the CPU-reference gate until fixed.
- **Fog fades the ground's COVERAGE**, not its colour, so the tonemap
  composites the actual background. A colour fog mixed in linear light
  showed as a pale band against a background composited after the
  tonemap's gamma.
  - It reaches 99% at `far` (default 8 view widths), from a start
    `haze` brings nearer (default 1: about a third of the way out).
  - Past `far` there is no ground. With haze 0 the edge shows.
- **Config:** `detail` (texels per pixel against the antialiasing
  factor, default 1), `far` and `haze` replace `resolution`. The panel
  gains Distance, Haze and Detail; its fog sliders went (the terrain's
  fog is its own).
- **Gates, all passing:**
  - The camera's conventions, including rotation turning the heading;
    the picture key ignoring the view.
  - A section's samples on its grid points, and neighbours sharing
    their edges, to 1e-9 of a side.
  - Selection: the count against section 12's estimate, every leaf
    within reach, nearer finer, within the atlas at 4x antialiasing.
  - The canonical anchor: a nearby view keeps it, a new band moves it.
  - **No ray falls through the ground:** 14,814 rays under the
    horizon, all hits, over 15 sections of mixed levels.
  - Sections reused: a light edit renders none, a small turn one, a
    tenth-width pan none; a picture edit renders them all. A reused
    engine's frame equals a fresh render's.
  - The T1 CPU-reference gate on one section: 0 disagreements on all
    four terrains, distances to 6.8e-4 cells. The filter, the shadow
    bias and the relight-cache gates all still pass.
  - 2^60 renders to the horizon; `escape-terrain-seahorse`
    re-baselined; 112 escape baselines and `release.py check` pass.
- **Measured** (GTX 1660 SUPER, 1080p, a fill from nothing):

  | | sections | fill | a section |
  |---|---|---|---|
  | direct 2^3, far 4 | 48 | 174 ms | 3.6 ms |
  | direct 2^3, far 8 | 61 | 192 ms | 3.1 ms |
  | direct 2^3, far 8, AA 2 | 50 (coarsened to fit) | 186 ms | 3.7 ms |
  | perturbed 2^60, far 4 | 46 | 14.6 s | 318 ms |
  | perturbed 2^60, far 8 | 58 | 23.6 s | 406 ms |

- **Open:**
  - **Deep zoom is slow:** a deep section costs about 10x its share of
    the single 2048² footprint's time. Sections far apart in their own
    pixels do not share a reference orbit, the likely cause; a shared
    reference with wider relocation is the lever.
  - Shadows from off-screen ground come only from the coarse roots.
  - A count's or a relief's range grows as sections arrive, so their
    heights shift slightly while the view fills.
  - The 2^60 render shows a hard horizontal colour line across the
    far plain. Likely a colour band of the picture far outside the
    original view; a step between section levels is not ruled out.
  - **Seen** (`output/heightfield_t2e/`): the seahorse to the horizon,
    the whole set, a low pitch, a turned view, the interior as a hole,
    and 2^60.
