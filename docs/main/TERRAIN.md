# Terrains, Solids and the Path Tracer

**Overview:** The 3D views other than the flame's point cloud. An escape
picture or a simulation's field can be drawn as a **terrain**: a height
field, lit, seen through a camera over it. Escape mode D draws a
**solid**: a 3D IFS attractor's distance field, ray-marched. The two
kinds share a camera, a lighting rig and material, two render tiers (a
lit tier and a path tracer), and the path tracer's core. This doc
describes the terrains whole, and mode D where it shares their
machinery.

**See also:**
- [CAMERA.md](CAMERA.md): the cameras these views are seen through, the
  terrains' target lift and orbit floor, and fly mode.
- [ESCAPE.md](ESCAPE.md): the escape engine, including mode D's distance
  fields and the camera lens.
- [SIMULATION.md](SIMULATION.md): the grid a simulation terrain is made
  from, and its relief stage.
- [RENDERER.md](RENDERER.md): the flame renderer, whose tail (tonemap,
  effects, readback) every view here hands its image to. The 3D flame's
  own solid rendering (splats, shade pass) is the flame renderer's.
- [EXPORT.md](EXPORT.md): exports, and *GPU memory during an export*.
- [UI.md](UI.md): the View panel, where the camera, atmosphere, depth of
  field, lighting and material controls live.
- [EFFECTS.md](EFFECTS.md): the effect chain these pictures share.

**Design record:**
- [heightfield-3d.md](../archive/terrain/heightfield-3d.md): the plan and
  its dated record: sections to the horizon, the walk, the tiers, the
  path tracer, the denoiser, the sky (T1 to T5).
- [terrain-height.md](../archive/terrain/terrain-height.md): the height
  map's quality (H1 to H6): curves, invert, the lock, the prefilter and
  softness, the smooth surface, an export's section cap.
- [materials.md](../archive/projects/materials.md): one material per
  picture, read by every tier.
- [ifs-distance-rendering.md](../archive/escape-time/ifs-distance-rendering.md):
  mode D.
- [escape-sim-followups.md](../projects/escape-sim-followups.md): open
  work, including the solid path tracer's speed.
- [solid-rendering-todo.md](../projects/solid-rendering-todo.md): lighting
  and material work deferred (environment maps, point lights).

**Code locations:**
- [src/escape/terrain.rs](../../src/escape/terrain.rs): `TerrainRenderer`:
  the atlas, the maximum mipmap, the walk, the relight, the terrain's
  half of the path tracer; the WGSL (`INGEST_WGSL`, `HF_HELPERS_WGSL`,
  `GROUND_WGSL`, `PATH_WGSL`)
- [src/escape/footprint.rs](../../src/escape/footprint.rs): `EscapeTerrain`,
  the escape picture's sections, their planner and anchor, the escape
  terrain's camera
- [src/escape/terrain_tiers.rs](../../src/escape/terrain_tiers.rs):
  `TerrainTiers`, the tiers by frame and by still
- [src/escape/path_core.rs](../../src/escape/path_core.rs): the path
  tracer's integrator, settings, resolve and denoiser
- [src/escape/renderer/solid_path.rs](../../src/escape/renderer/solid_path.rs):
  mode D's path tracer, in banded passes
- [src/sim/terrain.rs](../../src/sim/terrain.rs): `SimTerrain`, a
  simulation's terrain
- [src/escape/assembler.rs](../../src/escape/assembler.rs): `IFS_RIG`,
  the lit rig both kinds splice in
- [src/escape/ifs.rs](../../src/escape/ifs.rs): `SolidCamera`,
  `solid_frame`, `solid_camera`, mode D's rig packing
- [src/config/escape.rs](../../src/config/escape.rs): `TerrainConfig`,
  `PathTraceConfig`, `RenderTier`
- [src/config/sim.rs](../../src/config/sim.rs): `SimTerrainConfig`
- [src/config/fractal_config.rs](../../src/config/fractal_config.rs):
  `SolidShadingSettings`, the lights and the material

---

## The 3D views

| View | What it is | Drawn by | Where |
|---|---|---|---|
| 3D flame | the chaos game's points, projected; optionally splatted into a solid and shaded | the flame renderer | [RENDERER.md](RENDERER.md) |
| Mode D's solid | a 3D IFS's attractor as a distance field, ray-marched | `EscapeRenderer`'s IFS walk | [ESCAPE.md](ESCAPE.md); this doc for its tiers and path tracer |
| Escape terrain | the escape picture as a height field, to the horizon | `EscapeTerrain` over `TerrainRenderer` | this doc |
| Simulation terrain | one channel of a simulation layer as a height field | `SimTerrain` over `TerrainRenderer` | this doc |

What the last three share:

| | Mode D | Escape terrain | Simulation terrain |
|---|---|---|---|
| Camera | `SolidCamera` (`solid_camera`) | `SolidCamera` (`footprint::terrain_camera`) | `SolidCamera` (`sim_terrain_camera`) |
| Lit tier | the IFS walk and relight, `IFS_RIG` | the height-field walk and relight, `IFS_RIG` | the same as the escape terrain |
| Path tracer | `path_core` with the distance field (`solid_path.rs`) | `path_core` with the height field (`PATH_WGSL`) | the same as the escape terrain |
| Tier | `escape.solid_tier`, default Lit | `escape.terrain.tier`, default Auto | `sim.terrain.tier`, default Auto |
| Path settings | `escape.path` | `escape.path` | `sim.terrain.path` |
| Lights | fixed in the fractal (or the camera's, a toggle) | world-fixed | world-fixed |
| Fog | the flame's fog fields, in both tiers eased in over the fog's length 1/k past its start (`ifs_fog_amount`: no line where it begins) | the terrain's Far and Haze | Far and Haze on a repeated ground |

Every one hands the tail an `Rgba32Float` image in the accumulator's
layout: rgb linear, alpha coverage. Coverage 0 lets the tonemap's
background through, which is how a sky, a hole and a transparent PNG
work.

The terrains are built under the `terrain` cargo feature (on by default;
it implies `engine-escape`). The gallery's wasm modules build without it
and render a terrain config as its 2D picture.

---

## Render tiers

`RenderTier` (`src/config/escape.rs`) says how a solid or a terrain is
rendered. The Lighting panel's Rendering section shows it
as **Render tier**: Auto, Lit, Path traced.

| Tier | Viewport | Export |
|---|---|---|
| `Lit` | the lit tier only: direct light, traced shadows, horizon occlusion | the lit tier's antialiasing grid |
| `PathTraced` | the path tracer only, shown from the first sample (noisy while moving) | path traced at `samples` |
| `Auto` | the lit tier while anything moves, the path tracer once nothing does | path traced at `samples` |

The defaults differ:
- **Mode D's `solid_tier` defaults to Lit**, so a solid saved before the
  path tracer renders as it did.
- **A terrain's `tier` defaults to Auto.**

**Motion blur** ([ANIMATION.md](ANIMATION.md), *Motion blur*): the tiers'
view is a function of the jitter and a moment (`TierInputs::moments`).
The lit grid's passes go round the moments as a Latin square over the
jitter grid, a pass for every moment at least; the path tracer's
samples are the moments' in turn. A camera's move is blurred at the
frame's own cost over one ground; a picture changing within the shutter
needs a ground a moment, which an export draws and the viewport does
not.

**The terrains' viewport** (`TerrainTiers::viewport`) keys everything on
the view, the ground's version, the path settings and the tier. Any
change restarts both accumulations, so "moving" means the key changed
this frame. Then:
1. **The lit tier first.** One jittered render a frame from the
   antialiasing grid (`EscapeRenderer::sample_grid`, `supersample²`
   positions), folded into a running mean. Lit stops when the grid is
   done.
2. **While an escape terrain's sections are still arriving**, Auto stays
   lit and Path traced takes one sample a ground. Each section that lands
   restarts the accumulation, so samples taken meanwhile would be thrown
   away, and they would slow the sections.
3. **Path traced:** as many samples a frame as fit 12 ms at the measured
   cost (between 1 and 64), up to `samples`. Auto shows the path tracer's
   picture from `PATH_SHOW_SAMPLES` (8). Below that its noise reads worse
   than the lit picture.

**Mode D's viewport** follows the same rule in
`EscapeRenderer::trace_solid_viewport`. Auto path traces once the walk has
settled, and a change of anything restarts it.

---

## Lighting and material

### One rig, one material

Every 3D view reads `FractalConfig::solid_shading`
(`SolidShadingSettings`):
- **The rig:** shading strength, ambient, diffuse, specular and shininess
  (the highlight), the occlusion strength (`ssao_strength`), and up to
  four lights (`SolidLight`: a kind, intensity and colour, and the
  kind's own settings -- *Kinds of light* below).
- **The material:** `gloss`, `roughness` and `glow`.

The splat-only settings (solid strength, surface thickness, SSAO radius,
normal smoothing, gap fill, shadow maps) are the 3D flame's alone.

**The lights' space differs:**
- **Mode D (2026-10-19):** fixed in the fractal's space by default --
  azimuth counter-clockwise about its z axis from +x, elevation above its
  xy plane, a terrain's convention -- so they stay put as the camera
  orbits or flies. `SolidShadingSettings::lights_follow_camera` (the
  Lighting panel's "Lights follow the camera") puts them in
  the camera's space instead: azimuth 0, elevation 0 a headlight, the
  lights turning with every orbit, as every solid was lit before. Config
  v6: a solid saved before loads with it on and renders as it did; the
  3D look turns it off on entering a solid; the View panel's Fly Mode
  section shows **Fix lights in the fractal** for a solid with it on
  (`ui::render_mode::lights_to_world`: each directional light's angles
  turned into the fractal's at this camera, `ifs::camera_light_to_world`
  -- the picture as it stands, 0 pixels apart in both tiers -- and the
  flag off, one undo step). Switching the toggle itself converts nothing:
  the angles are read in the other space, and the light moves. Both
  spaces are turned into world directions in `pack_rig3`. Points and the
  headlamp are not affected.
- **Flames:** camera space, always: the shade pass lights the projected
  picture (`visibility::light_frame` greys the toggle).
- **Terrains:** world-fixed, the 2D relief's light. Azimuth is
  counter-clockwise from east on the plane, elevation is above it
  (`terrain::light_direction`).

### Kinds of light

A `SolidLight` is one of three kinds (`LightKind`;
[lights-in-the-fractal.md](../archive/projects/lights-in-the-fractal.md)):

| Kind | Where it is | Where it is offered |
|---|---|---|
| **Directional** | a direction at infinity: azimuth and elevation, in the spaces above | every 3D view |
| **Point** | `position` in the world (mode D's coordinates, f64) | mode D's solids |
| **Headlamp** | the camera's eye, lifted by `lift` camera distances, aimed down the view | solids and both terrains |

A point's and a headlamp's power at distance `d` is
`intensity / (1 + (d / reach)^2)`: the inverse square far off, finite at
the light, half at the reach. A point's reach is in world units; a
headlamp's is in **camera distances** (the eye's distance to its
target), so the lamp keeps its reach as the camera zooms. A headlamp's
`cone` is its beam's half-angle (180 lights all round), softened over
the outer tenth. `visibility::light_kind` answers which view offers
which kind; a kind a view does not offer is off there, and the panel
says why. A terrain's world is the view's -- its origin and unit move
with every pan and zoom -- so a light placed in it would not stay put;
the 3D flame's splat pass lights in screen space, directional alone
(`SolidLight::directional_on`).

How it is drawn:
- **GPU:** a light's first `vec4` holds its direction or its position
  (in mode D an offset from the target, as every position there is), its
  second `vec4`'s `w` its kind (0, 1, 2), and `fdata[28 + i]` its reach
  in the world's units and its cone's cosines. The CPU places the
  headlamp and turns its reach into world units (`ifs::pack_rig3`,
  `terrain::pack_rig`; the terrain's `fdata` is 32 slots for it).
- **The rig's** `ifs_light_at(i, p)` gives a light's direction, distance
  and gain at a point -- the offset scaled before it is squared, for deep
  views -- and `ifs_rig` takes the surface's position, which the relight
  rebuilds as the eye plus the ray times the hit distance.
- **Compiled in only when used:** unless a light that lights is a point
  or a headlamp (`SolidShadingSettings::placed_lights`), the rig's
  `ifs_light_at` is the directional one (`assembler::ifs_rig`). The
  positional code costs registers wherever it sits: the Menger's path-
  traced sample took 158 ms at 320x180 with it present and unused,
  against 130 without -- the beam's lesson (`IFS_MAX_BEAM`) again. Mode
  D's walk, relight and path pipelines carry it in their keys; a
  terrain builds its placed-light walk, relight and path tracer the
  first time a view has such a light (`TerrainRenderer::ensure_placed`).
  The first point or headlamp is a recompile.
- **Shadow rays stop at the light:** the walk's `ifs_shadow`, the
  terrain walk's trace and both geometries' `pt_scene_visible` take the
  light's distance as their end. A directional light's end is past
  everything, so its numbers are as they were.
- **The walk's key** holds each light's kind, position and lift beside
  its direction (they decide the shadow rays); reach and cone only scale
  what reaches, a relight.

**The panel** (`ui::lighting_panel`, Lights): each light's Kind;
Directional, its Azimuth and Elevation; Point, its Position (dragged at
the camera's scale), **Place at Camera** (the eye's position and a reach
of the camera's distance -- fly inside, drop a light; a light first made
a point is placed so), and Reach on a log scale about the camera's
distance; Headlamp, its Reach, Cone and Lift; every light, Intensity.
The animation picker offers each light's parameters by its kind.

Gates: a point light far off with a reach past everything is the
directional light it approaches; inside the sponge's central cavity a
headlamp's shadows-on picture is its shadows-off one (98.5%; 0 if the
shadow ray ran past the lamp) and a narrow cone darkens the frame's edge;
a point-lit cube and a headlamp-lit plane path trace to their lit tiers;
a headlamp-lit plane is its closed form to 4.1e-6
(`renderer::solid_path` and `terrain` tests).

**An untouched rig is a default light.** For a flame, an untouched rig
(shading strength 0) means classic unlit colour. For a solid or a terrain
it would be a flat silhouette, so `SolidShadingSettings::rig_untouched`
switches in a default instead:
- **A terrain** gets a default sun, `DEFAULT_SUN` = azimuth 135°,
  elevation 30°, white at intensity 1;
- **mode D** gets a key light at the panel's light-0 default (35°, 40°)
  in camera space when its lights follow the camera; fixed in the
  fractal, the same light as the default camera (yaw 0, pitch 0.42) sees
  it, to the degree (`ifs::default_world_key`), which taking the rig
  over writes to light 0 (`render_mode::rig_taken_over`).

Either way the rig is strength 1, ambient 0.12, diffuse 0.88, no
highlight, occlusion strength 1. The material is left out of the test,
so giving a surface a coat or a glow never switches the default light
off. The panel says so (`view.lighting_default_rig`), and any change to
the rig replaces the default.

**Shading strength** mixes the lit tier from the bare albedo (0) to the
fully lit colour (1). The path tracer lights a touched rig fully at any
strength. The highlight is weighted by the strength in both tiers: at 0
neither draws it, at 1 both draw it whole.

**Ambient** is light from every direction. The lit tier adds
`albedo x ambient x occlusion`; the path tracer adds the ambient to the
sky its bounces see (`path_core::path_ambient`), so an open surface gets
the same `albedo x ambient` and the geometry occludes it. An untouched
rig adds none to the path tracer: its default ambient stands in for the
sky there. At 0 bounces the path tracer has no ambient, as it has no sky.
(Until 2026-10-19 the path tracer read no ambient, and the slider did
nothing in a path-traced view; `the_path_tracer_sees_the_ambient`.)

**Occlusion** darkens the ambient only. A direct light's occluder is its
shadow ray, in both tiers; the occlusion estimate stands in for the sky
and the bounces the path tracer traces. Until 2026-10-19 it darkened the
lit tier's direct light as well, which the path tracer has no
counterpart for: the lit tier's recesses came out darker than the path
tracer's (the sponge's direct light 3.5% short in all,
`occlusion_leaves_the_direct_light_alone`). An indirect-light
approximation -- the albedo times the occluded share -- was measured and
left out: it moved the lit tier further from the path tracer.

**The 3D look.** A view entered as a solid or a terrain from one that is
neither (`ui::render_mode::look_3d`; UI.md) gets Linear tone mapping at
exposure 2.0 and gamma 1.0, saturation 1.0, and, when the rig is
untouched, strength 1.0 and ambient 0.0 -- a touched rig whose light 0
is the default key light. The solid presets carry the same.

**The material** (`path_core::lit_material`) is the same in every tier:
- **Gloss** (0 to 1): a clear coat's reflectance head on. 0, the default,
  is no coat; 0.04 is a dielectric's.
- **Metallic** (0 to 1; 2026-10-19): how much of the surface is metal.
  The coat's reflectance head on is `mix(gloss, albedo, metallic)`, a
  colour, and the diffuse is times `1 - metallic`: a metal reflects in its
  own colour with nothing under it. A coat exists where Gloss or Metallic
  is above 0. In the fog row's w (`ifs_metallic`, `fdata[10].w`) for the
  rig's shaders; `metallic` in the 3D flame's shade pass.
- **Roughness** (0.02 to 1): the coat's GGX roughness (alpha is its
  square).
- **Glow:** the colour's own light.
- **Reflectance** (0 to 1, default 1; 2026-10-19): how much of its colour
  the surface reflects. The albedo every tier lights is the colour times
  it -- the diffuse, a metal's coat colour and every bounce -- while the
  glow and the unshaded colour (Shading strength below 1) keep the
  colour's own strength, and a dielectric coat's highlight does not
  depend on it. In the studio row's z as its complement
  (`ifs_reflectance`, `fdata[32].z`: a uniform that never wrote it
  reflects all), read by `ifs_rig` and the path tracer (`pt_path`,
  `pt_first`'s guide); solids and terrains, not the 3D flame (its shade
  pass has no bounces; the panel hides the row there). The Lit tier and
  the path tracer's direct light scale by it exactly; a bounce brings the
  square or less, so with the exposure raised by its inverse the lit
  faces hold and the shadows the bounces alone light fall by it. Why it
  exists: a palette at full strength is a surface no real material is,
  and in an enclosed view its bounced light fills the shadows (a Fire
  palette's red bounces almost without loss). Gates:
  `reflectance_darkens_the_bounced_light_faster` (the Mandelbulb: Lit and
  the direct light 0.500 at 0.5, the bounce-lit shadows 0.236; the glow to
  the byte), and the terrain's flat plane at 0.5 (to 1.5e-8).

Every tier evaluates the coat alike on the direct light -- both escape
and simulation tiers and the 3D flame's shade pass: GGX's distribution,
Smith's separable masking and Schlick's Fresnel (per channel for a
metal), the diffuse taking what the Fresnel does not. The lit tiers'
coat also reflects the ambient, standing in for the sky the path
tracer's bounces see. The path tracer's bounce picks the coat's lobe by
its Fresnel at the view, leaning to it by the metallic (wholly at 1).

**Energy compensation.** One scattering off GGX's microfacets loses the
light that bounces between them: a white metal at roughness 0.7 under a
uniform sky returned 69% of it (measured), and at roughness 1 the
integral says about a third. The coat's
lobe is scaled by `1 + F0 (1 / E - 1)` (Kulla and Conty 2017; Turquin
2019), E its directional albedo at reflectance 1, in every tier
(`ifs_coat_energy`, `coat_energy`). E is a 16-term fit to the coat's own
integral (`renderer::coat::COAT_ESS_FIT`, `coat_energy_wgsl` generates the
WGSL for every shader): the compensation within 1.6% from cos 0.2 up and
-11% to +5% at a glance (`the_coat_albedo_fit_is_the_integral`). Karis's
EnvBRDFApprox, fitted to height-correlated masking, was up to half off
for ours. Under a uniform sky a white metal now returns it within 0.5%
at roughness 0.3 to 1 (`the_coat_and_the_glow`); a metal's direct light
is the lit tier's and the path tracer's alike within 0.3% on the sponge
and the Mandelbulb (`a_metal_is_lit_alike_in_both_tiers`).

### The studio

A polished or metal surface shows what it reflects, and under a dark
uniform sky that is nothing: a metal reads black. **Studio light** (the
path block's `studio`, 2026-10-19) lights the scene from a procedural
photo studio in place of the sky's light, while the camera still sees
the background or the gradient (`ifs_studio` in the rig):
- an overhead **softbox** a little behind the subject (radius 0.65 rad,
  radiance 2) -- what a top face seen from above mirrors;
- two **strips** behind to either side (0.22 rad, 1.2) -- the rims;
- a dim **fill** panel behind the camera (0.8 rad, 0.35);
- **walls** at 0.06 and a **floor** at 0.02, neutral white throughout.

Its brightness is Sky light over the exposure (`path_core::studio_level`),
so a radiance of 1 reads white at any exposure, as the background does.
Its frame (`fdata[32].y`): a solid's turns with the camera when its
lights do (0: x right, y up, z toward the viewer); fixed in the fractal
when they are (2), its basis in `fdata[33..35]` the camera's at the
solids' usual view, yaw 0.9 and pitch 0.42 (`ifs::studio_basis`), where
the shipped solid presets stand, so they keep their look; a terrain's is
the world's with z up (1). In `fdata[32]` for both tiers (the terrain's
uniform grew from 32 vec4s to 36).

- **The path tracer:** a bounce that escapes sees the studio plus the
  ambient (`pt_sky`; the environment is then the ambient alone).
- **The lit tier** traces no rays, so it has closed forms, each
  occluded as the ambient is:
  - the diffuse: the walls and floor each a half-space's share, each
    light's irradiance clipped by the surface's horizon (Lagarde and de
    Rousiers 2014) -- within 1.3% of the integral. Solid angle times
    cosine was 8% high for the softbox and gave a face beside it half its
    light;
  - the coat's reflection (`ifs_studio_reflect`), times its Fresnel at
    the view: the lobe as a cone of radius 0.5 alpha about the mirror
    direction, each light's share of it (its radiance where the cone is
    inside it, its solid angle over the cone's where the cone is wider,
    smoothed across the edge), blended toward the irradiance about the
    lobe's dominant direction as the coat roughens. Fitted to the path
    tracer's integral: within 5% in the mean at every roughness. A flat
    face reflects a single direction and can be a third to a half off
    near a light's edge (a Menger's top face, a flat terrain). Spherical
    Gaussians, tried first, were 8% off in the mean and twice the
    softbox's radiance at its centre.

Measured against the path tracer (`the_studio_lights_both_tiers_alike`,
`a_plane_under_the_studio_is_lit_alike`): the cube's diffuse 1.001, its
metals 1.11 and 0.87; the Mandelbulb at the default occlusion 0.97, 1.06
and 1.04; a flat plane's diffuse 0.999; rolling ground's metal 0.986.
Off, neither tier takes the studio's code path.

A picture saved before the material moved kept its coat and glow in a
path tracer's settings. `lift_path_material` lifts them into
`solid_shading` on load. The old paths (`Escape.Path.Gloss`,
`Sim.Path.Gloss`, and their roughness and emission) are aliases of
`SolidGloss`, `SolidRoughness` and `SolidGlow`.

### HDRI skies

A photographed sky all round (docs/archive/projects/hdri-environment.md):
`escape::environment`, bundled in `assets/environments/` and named by id
(`ENVIRONMENTS`) -- Poly Haven's Sunflowers (Pure Sky) and Rogland Clear
Night, 1K equirectangular Radiance files, CC0 -- and **Starfield**, stars
alone, no file. For solids and both terrains, in both tiers; one text
reads it everywhere (`ENV_WGSL`, spliced into the rig: `fdata[36..45]`,
group 0's bindings 14 and 15), each renderer holding the sky in an
`EnvSlot`.
- **Its frame** is the world's, z up, whatever the lights follow
  (`ifs_env_dir`; read in the studio's frame, a camera's at the solids'
  usual view, its horizon stood at that camera's pitch), turned by
  Rotation, its horizon raised or lowered by Horizon (the sky above and
  below the line stretched to fit, the zenith and the nadir fixed:
  `environment::warp_elevation`; the sun moves with it). It is scaled so
  the sky's median shows at its catalog level (`EnvDef::level`) through
  the exposure and gamma -- a picked colour's `shown`, kept linear --
  times Sky light. A camera ray that misses sees it (`ifs_sky_seen`), in
  both tiers, unless Show as backdrop is off.
- **Its sun is lifted out of the map** (`EnvMap::sun`): the texels above
  1000 times the sky's median, joined to the brightest, holding at least a
  tenth of the light -- the Sunflowers sun is nine texels holding 47% of
  it. It becomes the rig's next free light (directional, its irradiance
  over pi), so both tiers shade and shadow it and polished reflections
  meet it through the lights' own sampling; the map keeps the rest, the
  sun's texels filled from around them. An untouched rig's stand-in key
  (a terrain's default sun) gives way to it; under a sky without one (a
  night's) the stand-in stays, or the scene would be dark.
- **The path tracer** reads the map at its finest level (`ifs_env`).
- **The lit tier** reads it prefiltered: its diffuse light as nine
  spherical harmonics of the irradiance (`irradiance_sh`, taken on
  loading over the map at 128x64 and again when the sky turns or its
  horizon moves; within 2% of the integral on Sunflowers' faces), and a
  coat's reflection from the texture's mips -- level k the map seen
  through a GGX lobe of roughness k/5 (`EnvMap::reflection_levels`, the
  split sum by filtered importance sampling, 170 ms for a 1K sky on
  loading), read about the lobe's dominant direction (`ifs_env_reflect`).
  Against the path tracer (`an_hdri_sky_lights_both_tiers_alike`,
  `a_plane_under_an_hdri_sky_is_lit_alike`): a convex cube's diffuse and
  gloss within 1%, its metals within 10%; a flat plane's diffuse within
  1%, metal hills 9%; the Mandelbulb 2-11% dark, where the occlusion
  estimate stands in for its recesses, as the studio's does.
- **Stars** (`ifs_env_stars`, Stars in the panel; `environment_stars`):
  procedural, fixed to the map -- they turn and rise with it, above its
  horizon only. The sphere in cube-face cells, 160 by 160 a face, a star
  in about one cell in eleven; its brightness by a power law, its colour
  orange through white to blue-white. A star is a pixel across at any
  zoom (its radius 0.6 of the camera's pixel) and display-referred (1
  shows the brightest at white, at any exposure): sharp, and too faint to
  light anything. Polished reflections show them, fading out by
  roughness 0.1. Choosing a night sky in the panel (`EnvDef::stars`)
  brings them out at 1.
- The web build has no skies yet (its loading is for the user's
  decision: docs/archive/projects/hdri-environment.md).

### The walk and the relight

The lit tier is two passes, as mode D's is:
1. **The walk** finds each pixel's surface. It writes a 16-byte geometry
   record (`ifs_pack_geom`): the normal, the raw occlusion, four raw
   per-light shadow terms, and the hit distance (0 is no surface).
2. **The relight** applies `IFS_RIG` (`ifs_rig`) to the record and the
   albedo.

The strengths are applied in the relight, not where they were measured.
So a change to a light's colour or power, the material or the fog is a
relight and not a walk. The walk's key (`walk_key`) holds only what the
walk reads: the camera, the shadow switch and sharpness, occlusion's
reach, the jitter, the height map, the far distance, and each light's
direction and whether it is on (which decide the shadow rays).

The rig's text is shared. `assembler::ifs_rig_jittered` splices it into
the terrain passes with the sub-pixel jitter where mode D splices its
lens.

### The UI

The **Lighting panel** (`ui::lighting_panel`; its sections by
view in [UI.md](UI.md#the-lighting-panel)) holds, for a solid
or a terrain:
- **Rendering:** the render tier and the path tracer's samples, noise
  threshold (adaptive sampling), bounces and denoiser;
- **Sky:** the sky colour (the background), the gradient, Sky light;
- **Lights:** Light 1 to 4 with their Kind and its settings (*Kinds of
  light*) and Intensity;
- **Shading:** Shading Strength, Ambient, Occlusion Strength and
  Occlusion reach (a solid's formula parameter, a terrain's field);
- **Shadows:** Shadows and Shadow sharpness (likewise); a solid's Ray
  tolerance (below);
- **Material:** Diffuse, Highlight, Highlight size, Gloss, Roughness,
  Glow;
- **Surface** (an escape terrain): the surface texture, the lake's tint
  and roughness.

The occlusion rows hide when the tier is Path traced: occlusion is the
lit tier's stand-in for the sky the path tracer traces.

**Ray tolerance** (a solid's; `SolidShadingSettings::ray_tolerance`):
how close a ray leaving a surface -- a shadow ray in either tier
(`ifs_shadow`), the path tracer's bounces and shadows (`pt_ifs_leave`) --
may pass the solid and count as hitting it. Its hit test is the smaller
of a few pixels' share of the bias and the solid's ball times
`ifs_ray_tolerance()`: a ten-thousandth at Fine (the default), three at
Medium, ten at Coarse (rig `fdata[32].w`, the multiple less one). It is
a fraction of the solid, not of a pixel, on purpose: a hit test at a
pixel's width erased the Menger's sub-squares under grazing light
(`ifs_shadow`'s comment). Measured (path-tracing-research, *Found on the
way*): at 1080p Medium is 1.24-1.4 times faster, Coarse 1.5-1.9; the
Menger keeps its picture (640x360: RMSE 3.3% at Fine and Medium, 3.8% at
Coarse), the Mandelbulb's fine shadows darken (Coarse: 3.1%, its error
10% to 31%). Fine is the pictures as they were, to the bit. An untouched
rig's rows show what the first change makes it (UI.md). The View
panel's Depth of Field shows the path tracer's Aperture and Focus, or a
note that the lit tier draws everything sharp.

---

## The path tracer

`src/escape/path_core.rs` is the integrator, its settings, the resolve
and the denoiser. Two geometries render through it: a terrain's height
field (`PATH_WGSL` in `terrain.rs`) and mode D's distance field
(`solid_path.rs`).

### What a geometry supplies

In WGSL:
- `pt_eye()`: where the camera's rays start.
- `pt_scene_begin()`: what an invocation loads once.
- `pt_scene_next(o, d, travelled)`: the next surface a ray reaches, its
  normal on the side the ray sees.
- `pt_scene_visible(o, d, travelled)`: whether a shadow ray gets away.
- `pt_sample(px, py)`: one sample of a pixel, as (radiance × coverage,
  coverage). The camera ray and the first surface are the geometry's,
  because what coverage and fog mean differs between them. The rest of
  the path is the core's `pt_path`.
- `pt_tile()`: the part of the frame the sum holds.

The geometry calls `pt_first(h, d)` with a sample's first surface, which
the denoiser's guides are made of. The rig's accessors (the camera, the
lights) come from the same text both share.

### Settings

`PathTraceConfig` is a block of its own: `escape.path` for mode D and
the escape terrain, `sim.terrain.path` for a simulation's.

| Field | Default | UI | Means |
|---|---|---|---|
| `samples` | 256 | Samples (1 to 4096) | the viewport's target, and an export's count; sampling adaptively, the most any pixel takes |
| `noise_threshold` | 0 (off) | Noise threshold (0 to 16) | adaptive sampling: a pixel stops once its noise, in display levels of 255, and its neighbours' are under this (*Adaptive sampling*) |
| `bounces` | 2 | Bounces (0 to 8) | bounces after the first surface; 0 is direct light only |
| `environment` | 1.0 | Sky light (0 to 4) | the sky's brightness as a light |
| `aperture` | 0 | Aperture (0 to 0.2) | the lens radius, a fraction of the distance to the target; 0 is a pinhole |
| `focus` | 0 | Focus (0 to 8) | the focal plane's distance, a multiple of the target's; 0 is the target |
| `denoise` | off | Denoise | filter the noise (below) |
| `sky_gradient` | off | Gradient sky | the background alone, or a gradient to `zenith` overhead |
| `zenith` | (0.22, 0.40, 0.72) | Zenith | the sky's colour straight up, picked in sRGB |
| `studio` | off | Studio light | light from a photo studio in place of the sky's, Sky light its brightness (*The studio*) |
| `environment_map` | none | HDRI sky | a bundled photographed sky that lights and is reflected, in place of the background's, the gradient's and the studio's light (*HDRI skies*), or Starfield |
| `environment_rotation` | 0 | Rotation | the sky turned about the up axis, degrees |
| `environment_horizon` | 0 | Horizon | the elevation the sky's horizon stands at, degrees (the UI offers -45..45; the config holds -80..80): to set it against the solid |
| `environment_stars` | 0 | Stars | procedural stars over the sky, their brightness (1, the brightest at white; 0, none) |
| `environment_backdrop` | on | Show as backdrop | the camera sees the sky (a skybox), or the background |

`path_core::path_settings_from` turns the block into a pass's
`PathSettings`:
- **The environment** is the background colour as the accumulator holds
  what the tonemap shows as that colour, times Sky light (`shown`).
  `shown` undoes the Linear tonemap's exposure and gamma and the sRGB
  decode the background is composited with. So an albedo-1 surface lit by
  the sky alone, at Sky light 1, reads exactly as the sky behind it.
- **The gradient's zenith** is brought over the same way.
- **The lens is in the target's terms**, so it keeps its look as the
  camera dollies: the aperture times the target's distance, the focus
  that distance times `focus` (1 when 0).
- **A sample's radiance is clamped** at ten times the brightest light:
  the enabled lights' summed intensity, the environment, the zenith, the
  glow, and at least 1 -- an HDRI sky's sun counted with the lights it
  draws beside (`solid_path`). That removes the fireflies a rare bright
  path would scatter. **The light from past the first surface is clamped
  apart, at the brightest light itself** (`clamp_indirect`; Cycles' Clamp
  Indirect): a polished metal's glints of a sun seen in a second surface
  are rare and far brighter than the picture, and at the whole clamp
  alone they stood out more the more samples, for a while (user-reported
  2026-10-20, in a gold solid's crevices). Measured on that scene: the
  pixels standing out at 32 samples 301 -> 106, the picture 1% darker
  (`a_gold_solids_glints_settle`, the measurement
  `sun_threshold_speckles`).
- **Bounces** are capped at 16; the seed is fixed at 1.

### A sample

`pt_path`, from a path's first surface:
1. **The glow:** at every surface, the albedo times the glow.
2. **Every light**, each sampled over its angular size with a shadow ray
   (next-event estimation). The angular radius comes from the shadow
   sharpness, `min(0.5 / sharpness, 0.5)` (`light_radius`). The shadow
   ray's visibility is mixed in by the shadow strength, and none is
   traced at strength 0. Every light is sampled, not one picked by
   power: picking one was measured as speckle on faces only one light
   reaches.
3. **The direct light's response:** the diffuse (in the rig's units), the
   coat's GGX lobe, and the rig's Blinn-Phong highlight, exactly as the
   lit tier draws it.
4. **A bounce:** the coat's lobe, sampled by the visible normals (Heitz
   2018), chosen with the coat's Fresnel probability; otherwise
   cosine-weighted Lambert. Russian roulette from the second bounce.
5. **A ray that escapes** sees the sky: the environment, or the gradient
   by the rig's profile (`ifs_sky_t`), or the studio (*The studio*).
   One that left by the coat's lobe and escapes inside a directional
   light's cone also meets the light (below).

A hole (albedo alpha 0) ends the path. Past the first surface it sees the
sky through the hole.

**Path regularisation** (Kaplanyan and Dachsbacher 2013; Cycles' Filter
Glossy; 2026-10-20): past a diffuse bounce a coat is no sharper than
roughness 0.4 (`PT_REGULAR_ROUGH`) -- light that found a glossy surface
after a diffuse one is the rare bright path a few samples keep as a
firefly. Measured against unclamped references, glossy solids came out
1-4% nearer; a metal, which never bounces diffusely, is untouched.

**Multiple importance sampling** (Veach and Guibas 1995; 2026-10-19). A
polished coat's peak is narrower than a light's cone, so the lights' own
samples rarely land in it, and when they do they are worth a thousand
times the light and the firefly clamp clips them: a polished metal under
one light read 0.068 against the truth's 0.42 (`mis_keeps_the_picture_and_lowers_its_noise`).
Now a bounce by the coat's lobe that escapes inside a directional
light's cone adds the light's radiance -- its irradiance pi I spread over
the cone's solid angle (`pt_light_pdf`) -- and the coat's share of each
estimate is weighted by the power heuristic against the other's density
(`pt_coat_pdf`: the lobe's choice times the visible normals' G1(v) D(h)
/ 4 n.v). The diffuse stays the lights' alone (its density never nears
a light's), a point or a headlamp has no area to meet, and at shadow
strength 0 -- the lights reaching through the solid, where no bounce can
see them do so -- it is off. Under a partial shadow strength the part of
a light that reaches is weighted and the part let through the solid is
whole. Unbiased: with and without (and no clamp) within 0.3%; the noise
at 16 samples 4.7 times lower on a polished metal, 1.9 at roughness 0.3.
Gloss 0 and metallic 0 take no part.

### Random numbers and reproducibility

Each draw is a point of a shuffled, Owen-scrambled Sobol (0,2)-sequence
(Burley 2020): the pixel and the draw's place seed it, and the sample
index walks it. The samples of a pixel are stratified in each 2D
decision (the jitter, the lens, a light's cone, a bounce). Measured
against independent draws, that is a third less error at equal samples.

The sum is the same bits however the samples are split into dispatches,
and however the frame is split into tiles:
- **Each pixel adds its samples to a running sum in order**
  (`pt_sum`: premultiplied radiance, coverage).
- **A sample's numbers are keyed by its pixel's place in the whole
  frame**, its index and the seed.

`path_samples_are_batch_invariant` and `a_still_in_tiles_is_the_still_whole`
hold this.

### Adaptive sampling

With a noise threshold (`noise_threshold`, levels of 255; 0 is off) a
pixel stops taking samples once it is clean enough, and the time goes to
the noisy ones (path-tracing-research.md, V2):
- **Each pixel keeps statistics** beside its sum, in the same buffer
  after the frame's sums (no new binding; the solid path tracer is near
  a browser's storage-buffer count): its samples' luminance squared, its
  odd samples' luminance, its own count -- negative once converged -- and
  its noise at the last check.
- **The check** (`PATH_ADAPT_WGSL`, `PathSum::check_if_due`) runs at
  fixed counts -- `ADAPTIVE_MIN_SAMPLES` (16), then each time the count
  has grown by an eighth -- so a picture does not depend on how its
  samples were batched. `error` measures each pixel's noise: its mean's
  standard error from the moments, as the display shows it -- how far one
  error above the mean lands through the Linear tone map's exposure and
  the display's power on it (`PathSettings::display`, `shown` inverted).
  `converge` flags a pixel converged where its noise and its eight
  neighbours' are under the threshold, so an unconverged pixel keeps its
  neighbours sampling and a converged one wakes when a neighbour's noise
  rises; it lists the pixels still sampling, a workgroup's together, after
  the statistics, and `publish` writes the list's length at its head. The
  count comes back without waiting.
- **The trace takes the list.** Once a check has run, `main` takes a
  list entry a thread (`PathParamsGpu::from_list`) in place of a band's
  rect, so a warp's 32 lanes are pixels that all still sample. A pixel
  draws from its own count, so its samples are the same stream however
  the frame is dealt: each pixel's sum is the plain tracer's at its count,
  to the bit. (A workgroup sharing out its own block's pixels needs a
  barrier, and a barrier in this kernel made the march 3.4 times slower on
  a GTX 1660 SUPER, so the check lists them instead.)
- **The resolve and the denoiser** read each pixel's count.
- **Done** is the samples asked for, or the check's count of pixels still
  sampling coming back as none. A new threshold does not restart the
  picture: the next dispatch checks under it (`PathSum::retune`).
- **The solid's driver** prices a pass over the list per entry-sample,
  measured (the pixels still sampling are the costly ones: a crevice's
  long marches, about five times the frame's average on the Mandelbulb),
  by the last list length that came back until the newest does, and sends
  the whole list at several samples a dispatch where it fits. A
  terrain's sends the whole list a dispatch.
- A still drawn in tiles converges tile by tile: a pixel at a seam does
  not see its neighbour across it, so it may stop a check sooner or later
  than the whole frame would stop it -- noise, not a seam.

Measured (`adaptive_quality`, 2026-10-20; path-tracing-research.md has
the table): the time even sampling takes to the adaptive picture's error,
over the adaptive picture's time, at thresholds 8, 4 and 2 with at most
256 samples. At 4, averaged over the Menger, the Mandelbulb and the
Mandelbox matte, glossy and metal: 1.95x by the gate's error (linear
luminance), 1.44x by the error as the display shows it. It pays where a
few pixels hold the noise -- the glossy Mandelbox 4.1x (2.8x as shown) --
and breaks even on the Mandelbulb, whose noisy pixels are its costly
ones. Off by default.

### Resolve and the denoiser

**The resolve** (`PATH_RESOLVE_WGSL`) writes the premultiplied mean over
the coverage as colour and the mean coverage as alpha: the accumulator
contract.

**The denoiser** (`PATH_DENOISE_WGSL`, run by `PathSum`) is edge-avoiding
à-trous wavelet filtering (Dammertz et al. 2010) with SVGF's variance
guidance (Schied et al. 2017), over one accumulation rather than over
frames:
1. **`prepare`:** each pixel's mean light divided by its mean albedo, so
   the filter sees the light alone and the colouring's detail, which
   lives in the albedo, is multiplied back untouched. That light no
   brighter than twice its brightest neighbour's on the same surface
   (normals within 37 degrees; `DN_CLAMP_FACTOR`) while its excess is
   within two of its standard errors (`clamp_z`; always under four
   samples): one rare path -- a bounce that found a small light, one
   sample far from the rest -- does not stand out of the filter as a dot,
   and a metal's real glint, bright in every sample, stays. (It loosened
   by the sample count before, and paths clamped at 16 samples came
   through at 32 as speckle.) With it, the light's variance and the features: the normal
   and the distance, with the distance's gradient.
2. **`atrous`** at strides 1, 2, 4, 8, 16 -- 16 dropped from 8 samples on,
   8 from 32 (`denoise_steps`) -- a 5x5 B3 kernel whose taps are weighted
   by the normals' agreement, the distances' against the gradient, and
   the lights' against the noise.
3. **`finish`:** the light times the albedo, with the resolve's coverage
   -- moved from the pixel's own mean by no more than 1.5 of its standard
   errors (its samples' luminance moments), judged on its largest
   channel; past that the move shrinks as (bound / gap)^2, so detail the
   filter would average away goes back to the samples' own, and the
   picture converges to their mean. From 8 samples, eased in over 24
   (`DenoiseTrust`): switched on whole, and as one error times
   `max(1, sqrt(16 / n))` -- a bound shrinking faster than the noise --
   more samples showed more speckle (user-reported 2026-10-20;
   `more_samples_are_never_speckier`).

Measured (path-tracing-research.md, the `denoise_quality` gate: three
solids matte, glossy and metal at 4, 16 and 64 samples, against 4096
unclamped samples): the denoised error averaged 0.866 of the raw, from
0.986 before the bound and the neighbourhood clamp; on a fractal solid
most of what remains is the colouring's sub-pixel variation, which no
filter of the light reaches.

**Specular and diffuse apart** (D5), where the material has a coat or a
metal: a sample's specular share -- its first surface's coat and metal
terms of the direct light, and all a first bounce brings back when it
took the coat's lobe (`pt_spec`; the Blinn-Phong highlight stays with the
diffuse, being no lobe a bounce samples) -- is gathered with the guides,
each share with its own luminance moment. `prepare` demodulates both by
the same albedo, `atrous` filters each by its own noise, the specular's
passes reaching by the roughness (none below 0.1, as the diffuse's from
0.5) but whole while samples are few (at least `4 / n`), and `finish`
adds them back; the neighbourhood clamp and the confidence bound act on
the whole. A polished reflection is the view's own, not its neighbours':
filtered with the diffuse it blotched. Measured: never more than 2.3%
worse than not splitting, up to 7.7% better, the glossy Mandelbulb's
coarse error 14-15% lower at 16 and 64 samples; a matte surface is the
same picture. The guides are 48 bytes a pixel (`GUIDE_BYTES`); the two
specular lights are textures of their own, made only when split.

Two lights may differ by four of the noise's standard deviations and
still be averaged (`DENOISE_SIGMA`). Past 8 samples (`DENOISE_FULL`) that
width narrows as `sqrt(8 / n)`, so as samples gather the filter stops
crossing anything.

The guides are gathered with the samples:
- the first surface's whole reflectance (`pt_first`): the albedo under
  the coat, plus the coat's own share;
- the direct light the albedo does not shape (a highlight, a coat's
  glint), taken as white. Divided by a saturated albedo's near-zero
  channels it came back as bands of the wrong colour. Its share is
  reckoned from the light's centre, the lobe widened by the light's
  size, with no shadow ray: the same every sample, so the guide holds no
  noise of its own.

The filter costs about a sample of a terrain at 1080p, so it runs on a
schedule: every count up to 4, then whenever the count has grown by a
quarter, and at least every quarter second while anything new is in the
sum. A still, and the viewport's last sample, resolve now
(`resolve_now`). A tile of a denoised still is drawn with
`DENOISE_REACH` pixels more around it.

### Mode D's banded passes

A solid's sample costs several marches: the camera ray, a shadow ray a
light, a march a bounce. At 1080p a walk is already hundreds of
milliseconds. So `solid_path.rs` traces a sample as a pass over the
frame's rows in bands the GPU's watchdog never notices:
- **Bands are sized by measurement under a model's cap.** Each call is
  timed against its rows, and a band is as many rows as the target time
  holds. A row not yet measured is priced at the cost model, or twice the
  costliest row since the picture restarted, whichever is more. The
  model (calibrated to about 300 ms, `MODEL_BAND_MS`) caps every
  dispatch.
- **A band is at least `MIN_BAND_PX`** (eight rows of 1920) in whole
  workgroups.
- **A band priced past `DISPATCH_CEILING_MS`** (500 ms) goes across the
  frame in tiles.
- **One call at a time.** The viewport sends a call only once the last
  has finished (`SolidPath::busy`), so each is measured alone.
- **Budgets:** the viewport gives the tracer 40 ms a frame
  (`VIEWPORT_FRAME_MS`), an export 250 ms a call (`EXPORT_FRAME_MS`).
- **Losing the device is learned from.** A device loss while the
  tracer's calls are out halves the ceiling and the cap, and is
  persisted in the tuning file as `path_shift`. A call measured past
  700 ms halves them for the session.

It is slow. The shipped Menger sponge at 1080p takes seconds a sample
(256 samples is tens of minutes on a GTX 1660 SUPER). The measurements
and the candidate speed-ups are the TODO in
[escape-sim-followups.md](../projects/escape-sim-followups.md).

---

## The terrain renderer

`TerrainRenderer` draws a height field made of **sections**. An escape
terrain's ground is many sections at different levels of detail; a
simulation's is one section, optionally repeated. Everything below is
shared by the two.

### Sections and the atlas

A section is a grid of `n x m` samples, each a raw height value and an
albedo. Sample `(i, j)` sits at cell `(i, j)`, `j` running north (the
picture's top). The sections live in an **atlas**, texture arrays of one
layer a section (`ensure_atlas`):

| Texture | Format | Holds |
|---|---|---|
| raw | `R32Float` | each sample's raw height source |
| albedo | `Rgba16Float`, full mip chain | each sample's colour, premultiplied, coverage in alpha |
| max mipmap | `R32Float`, power-of-two sides | the hierarchy the walk steps over |

At the escape terrain's 1025² samples a layer is about 21 MB
(`atlas_layer_bytes`). The max mipmap's sides are rounded up to powers of
two because a texture's mips halve rounding down while the node grid
halves rounding up, and only at powers of two do the two agree.

**The ground** (`Ground`, set by `set_ground`) places sections in the
world. Each `GroundSection` has an atlas layer, the world position of its
sample (0, 0) and its texel size. A **quadtree** over a root grid
(`GroundNode`) finds the finest ready section under a point. `Ground`
also carries the slab the walk clips to (`top`, `floor`), the raw-to-
height map's mode, whether the one section repeats, and whether the
surface is smooth.

### The maximum mipmap

The walk skips empty space with a maximum mipmap (Tevs, Ihrke and Seidel
2008):
- **Level 0** holds each cell's highest corner, so no bilinear patch rises
  above it.
- **Each level above** holds the maximum of a 2x2 block below.

"Highest" is the raw extreme the height map takes to the top: the maximum
of a count or a relief, the minimum of a distance. So a height or a flank
width is a uniform that re-walks the ground, not a rebuild.

A smooth surface needs a different level 0 (`BUILD_WGSL`):
- **A simulation's** spline is of the raw values, so level 0 is the
  cell's 16 control points in Bezier form, whose range hugs the cell far
  closer than the B-spline points' own.
- **An escape terrain's** spline is of the mapped heights, through a map
  that is not linear. The map is monotone and the spline a non-negative
  weighting of its 4x4 real samples, so their extreme bounds it.

**Which extreme** a node keeps is the raw value the map takes highest.
For a tile or a simulation's heights that is the maximum. An escape
terrain's sections rank their values by height (`rank` in the build):
- the plateau's sentinel first;
- then the land, in the map's direction: a distance falls as the raw
  value rises, a count or a relief rises, and **Invert** turns each the
  other way;
- then a hole's sentinel;
- a lake's last, so the land round a lake still bounds its block.

`hf_bound` is the kept value's height. Turning Invert or the smooth
surface on or off builds every section's mips again and re-renders
none. Inverted terrains used to keep the plain map's extreme and bound
every node by the relief's top instead. Below that top the walk could
never skip a node, so it marched cell by cell, many times the steps,
slow enough to lose the device and restart over and over
(`an_inverted_terrain_walks_as_fast_as_a_plain_one`).

### The walk

Per pixel (`WALK_WGSL`):
1. **The camera's ray**, `ifs_ray`, offset by the sub-pixel jitter.
2. **`hf_trace`.** The ray is clipped to the ground's slab and root grid
   (a repeated ground has no edge, only the far distance), then walked
   region by region. Each region is the square over which one section
   answers (`hf_lookup`), traced in that section's cells by
   `hf_trace_section`.
3. **`hf_trace_section`**, the hierarchical traversal. From the top
   level, a node the ray stays above is stepped over and the level goes
   up; a node it comes within reach of is descended. At a leaf, the exact
   intersection with the cell's surface. `HF_MAX_STEPS` (4096) is a
   guard, not a budget.
4. **The normal**, the occlusion and a shadow ray per light. Then the
   geometry record.

The ground ends at the view depth `far`. The ray's length is
`far / (d · forward)`.

**`hf_lookup`** walks the quadtree from the root grid's square under the
point, keeping the deepest ready section it passes. The region returned
is the empty quadrant the descent stopped at, or the last node's own
square. So while finer sections are missing, a coarser one answers for
their ground.

**Walls.** A ray that enters a section through its side, below the
surface, has hit a wall: the ground's outer edge, or a step between
sections of different detail. It is lit by the side's outward normal and
is open to the sky. Only a point on the boundary counts, so a point a
rounding under a steep flank stays the flank.

### The surface

Between samples the surface is one of two things:
- **A bilinear patch** a cell, the default for an escape terrain.
  `hf_patch` solves the ray against the patch exactly: the gap is a
  quadratic in the ray parameter, rebased at the cell's entry so a far
  eye loses no precision. The shading normal is the corners' own normals
  (central differences), interpolated, so the light is smooth across a
  faceted cell.
- **A uniform cubic B-spline** through the samples, continuous to its
  curvature: a simulation's always, an escape terrain's with **Smooth
  surface** (`terrain.smooth`). `hf_spline_hit` samples the gap (a
  degree-six polynomial along the ray) at 8 points across the cell and
  refines the first crossing by false position. The normal is the
  spline's gradient.

A bilinear patch creases at every grid line, which shows as stepped
shadow edges and serrated ridges wherever the ground is magnified. The
spline removes them at a cost: measured at 2.6x a path-traced sample's
time at 1080p (terrain-height.md, H5). So it is off by default for an
escape terrain: a still's choice, and the viewport's where it is
affordable.

At an escape terrain's section edge the control points continue in a
straight line, `2 P_edge − P_next` (`hf_extends`). Neighbours share their
edge samples, so each side's spline passes through them and the two meet
without a step. A simulation's single section holds its edge, or wraps
when it repeats. Turning Smooth surface on or off re-bounds every cached
section's mipmap (`reshape_section`) and re-renders none.

### The height map

A section stores its raw source. The walk maps it to a height in the
world (`hf_f`), by the ground's mode:

| Mode | Source | Height |
|---|---|---|
| 0 | a tile set from heights (`set_tile`) | the value itself |
| 1 | the distance estimate | `H · exp(−d / w)`: `d` the distance in the world, `w` the flank width |
| 2 | the escape count | `H · rise(raw)`, logarithmic by default |
| 3 | the relief field | `H · rise(raw)`, linear by default |
| 4 | a simulation's heights | the value itself; the slab is their measured range |

`hf_rise` is a count's or a relief's rise over the range the ground
measured, 0 to 1, by the **Curve** (`TerrainCurve`, `fdata[2]`):
- **Auto:** the source's own. Logarithmic for a count, linear for a
  relief.
- **Log:** `log(1 + raw − lo) / log(1 + span)`.
- **Linear:** `(raw − lo) / span`.
- **Mesa:** the log rise topped out early,
  `(1 − e^(−6v)) / (1 − e^(−6))`. The set's busy boundary becomes a flat
  top instead of a forest of spikes.

**Invert** (`terrain.invert`) maps the land to `H − h`, for every source.

The interior's sentinels map to fixed heights:
- **a plateau** to the top `H`;
- **a hole** to the slab's floor;
- **a lake** to 0, the plain's level.

Their raw values sort so that a mipmap's extreme still bounds the highest
point. For a distance the plateau is −1e30, not the set's own 0, so an
inverted map can tell it from land at distance 0 and leave it on top.

### The surface texture

The escape relief's texture (`EscapeShading`: Grain, Paper, Rock or the
simulation texture, its strength and scale, and Where) is bumped into
an escape terrain's surface: the shading normal tilts, the geometry
does not ([surface-textures.md](../projects/surface-textures.md)). A
simulation's terrain has none. A terrain's is drawn in the Lighting &
Material panel's Surface section, a plane's relief's in the Escape
panel.
- **Mapping:** triplanar. Each of the three axis projections is
  evaluated where its weight (|n|⁴, normalised) counts, its gradient
  lifted into the world.
- **Anchored to the plane:** octave *k* has wavelength 2^*k* of the
  plane. Its phase is the world's origin (the eye's ground point) in its
  cells, mod 1024: the anchor's exact bits (`FixedPoint::bits_mod`)
  plus the eye's offset in f64 (`footprint::surface_texture`). A
  pan, a turn or an anchor change keeps every point's texture;
  heights need no phase.
- **Its size:** `texture_scale` pixels of a 1080-line view at the
  target. As the view zooms, the kind's octave window slides over the
  fixed octaves (`kind_window`: Grain 1 octave, Paper 3, Rock 4, with a
  fractal falloff), so the texture keeps its size on screen and never
  steps.
- **Filtered:** an octave fades out between four and two pixel
  footprints a wavelength (the relight's and the path tracer's `foot`).
  The simulation texture is copied from the section renderer's on the
  GPU, with a mip chain, and sampled at the footprint's level.
- **Masked** by the true normal's slope (Where).
- **The bump:** the surface gradient, scaled so the tilt does not change
  with the scale (`SURFACE_BUMP`).
- **Lit tier:** the relight bumps the normal it unpacks. A light the
  true surface faces away from stays dark, since the walk's shadow test
  was the true surface's. The ambient takes the sky the tilted normal
  faces over the true one's, so a face in shade shows the texture too.
- **Path tracer:** `pt_surface` bumps the normal before the two-sided
  flip. `PtHit::ng` keeps the true normal: the next ray leaves along it,
  and no light or bounce passes under it.
- **Cost:** a texture edit relights and re-walks nothing (not in
  `walk_key`); `picture_key` ignores it, so no section re-renders.
  Nothing at all with the kind None.

### Relight, albedo and fog

**The albedo** is filtered over a ray's share of a pixel at the hit
(`hf_albedo_at`), widened where the surface is seen at a slant:
- where that share is under a texel, between samples: bilinearly, or by
  the spline on a smooth ground;
- where it covers several texels, from the mip chain, so distant ground
  is the average of its texels rather than whichever one a ray struck.

A wall takes the colour at its top. A simulation's ground can instead be
**coloured by its own height** (`set_height_colour`, `fdata[21]`): the
palette at the height, so bands follow the contours down the steepest
face.

**Fog fades coverage, not colour.** The relight multiplies the ground's
alpha by `exp(−density · max(depth − start, 0))`, so the tonemap
composites the actual background behind it. A colour fog mixed in linear
light could never match a background composited after the tonemap's
gamma, and showed as a pale band.

**Accumulation** (`ACCUM_WGSL`) folds each jittered render into a running
mean, premultiplied, stored straight. A sample that misses leaves the
colour alone and lowers the coverage, so a silhouette's edge is the
surface's colour at a fraction of its alpha.

### Shadows and occlusion

Lit tier:
- **Shadows.** For each light the surface faces, a shadow ray is traced
  through the same ground (`hf_trace` with a softness). The ray measures
  how close it passed the surface relative to its distance, a penumbra
  as mode D's shadow rays measure one: **Shadow sharpness** (default 12)
  is the inverse of the light's size. **Shadows** (default 0.98; 0.7 before config v5) is how
  dark a shadow goes; 0 traces no shadow rays. The ray starts off the
  surface by a bias that grows with the distance from the origin and the
  eye, or the surface shadows itself in speckles.
- **Occlusion** is the horizon's (`hf_occlusion`). In eight directions it
  finds the steepest rise within the reach, at three distances, as the
  sine of its elevation; the occlusion is one minus their mean. It is a
  height field's own occlusion and deterministic, with no noise to
  accumulate away. **Occlusion reach** is a fraction of the ground's
  width; the rig's occlusion strength scales it.

The path tracer uses its lights' cones for shadows and needs no
occlusion: it traces the sky.

### Holes and lakes

- **A hole** has albedo alpha 0 and sinks to the slab's floor
  (`hole_floor`: a sixth of the relief, at most 0.01 view widths, below
  the plain). The background shows through it, and in the path tracer,
  past the first surface, the sky does.
- **A lake** is flat at the plain's level, so the set's rim stands round
  it. Its albedo is the **Lake tint**:
  - the lit tier draws it in that colour;
  - the path tracer renders any cell whose four corners are all lake as
    water: a flat normal, a coat reflecting 0.02 head on (a mirror that
    grows stronger toward the horizon, by Fresnel), at the **Lake
    roughness** (0.02 is still water).

A height field is two-sided in the path tracer: the normal is turned to
the side the ray sees.

---

## The escape terrain

### The world and the camera

The escape picture's plane is the ground. The shader's world is the
**current view width** (`4 · 2^−zoom_log2`):
- x along Re, y along Im, z up;
- the origin at the eye's ground point.

So the shader never sees a large number, at any depth.

The camera is mode D's, read for a plane (`footprint::terrain_camera`):
- **The target** is the view's centre, resting on the relief's top `H`
  (a locked relief's top, in current widths) and lifted by
  `terrain.target_lift`.
- **The eye** is `FRAME_DISTANCE` (1.3) view widths from the target, so a
  zoom is a dolly.
- **`cam_pitch`** is above the horizon (default 0.42 rad, about 24°).
- **`cam_yaw`** turns about the target. 0 looks up the 2D picture, and
  the 2D view's `rotation` turns the heading too.
- **`cam_fov`** is the vertical field of view (default 0.7 rad).

There is no roll. See [CAMERA.md](CAMERA.md) for the gestures, the orbit
floor and fly mode.

The terrain is active when `terrain.enabled` is set, the formula draws a
plane (a solid IFS is already 3D), and the build has the `terrain`
feature (`EscapeConfig::terrain_active`).

### Sections on a dyadic grid

The plane is cut into square sections on a dyadic grid fixed in the
plane. Each section is one ordinary escape render of the same size:
`SECTION_SAMPLES` = 1025 samples a side, 1024 cells, its edge samples on
its grid points so neighbours share them. Sections at twice the distance are
twice as wide, so their texels stay about `supersample × detail` a screen
pixel where they are nearest.

A section's key is `(level, i, j)`: a side of `2^level` anchor widths,
and its place on that level's grid. The grid hangs from an **anchor**: a
point of the plane in exact decimals and a zoom whose view width is a
level-0 section's side.

**The anchor is the config's** (`canonical_anchor`):
- its zoom is the floor of an eight-octave band (`ANCHOR_BAND`);
- its centre is the view's centre truncated toward zero to a decimal
  lattice at least 2^12 anchor widths apart (`ANCHOR_LATTICE`).

So the sections, and once they are all in the picture, are a function of
the config and the picture's size, the viewport's and an export's alike.
A pan or zoom out of the band or the lattice cell starts the sections
over. The view's offset from the anchor is subtracted in fixed point
(`view_offset`), so it holds at any depth.

### Making a section

A section is the escape picture rendered at rotation 0 over the
section's square (`section_config`). `footprint_config` forces
supersample 1: the terrain's own accumulation antialiases what the screen
sees.

The ingest (`INGEST_WGSL`) turns the render into atlas samples in three
passes, rows flipped so the picture's top is north:
1. **`filter_main`:** every texel's raw value, border included. With a
   **Prefilter** (`terrain.prefilter`: Off, 2x2, 3x3) the section renders
   at that supersample, and a texel is the mean of its samples that
   escaped, or the interior's where most did not. Detail finer than a
   texel then averages instead of aliasing into spikes. A distance is
   stored in texels.
2. **`soften_main`:** with a **Softness** (`terrain.softness`, 0 to 4
   texels), a Gaussian along the rows. It is weighted by the land alone,
   so a shore is not pulled toward a sentinel, and the interior is left
   as it is.
3. **`store_main`:** the Gaussian down the columns, then the texels, their
   albedo, and the range.

The albedo is the mean colour of a texel's escaped samples, or the
interior's colour:
- **a plateau:** the colouring's interior colour, or the background's
  where the colouring leaves it undrawn;
- **a hole:** transparent;
- **a lake:** its tint.

**Softness is in texels, not plane units.** A texel is about
`supersample × detail` of a screen pixel where the ground is near, so a
softness in texels is a softness on screen, and the sections stay
cacheable. For it, a section renders with a border of `ceil(3σ)` texels
(`section_filter`), so the blur at its edge reads its neighbour's ground
and shared edges agree exactly. The colours are not blurred.

**The prefilter costs its square** in section render time and in the
section renderer's memory, about 148 bytes a render pixel, within the
escape renderer's budget. The budget can clamp the factor
(`effective_supersample`), and the ingest reads the factor actually used.

**The range.** For a count or a relief, the store pass records each
section's own minimum and maximum (`ordered`, atomics), per atlas layer.
`reduce_range` reduces them over the ground's sections, so the heights
are this ground's and not whatever was panned past or zoomed through
before. `read_range_back` copies the result out without waiting, and
`poll_range` collects it on a later frame (`measured_range`).

**Auto contrast**, when on, is measured once from the anchor's own 2D
view (`contrast_probe_config`) and pinned for every section. Fitted each
its own way, every section was a palette turn of its own.

**What the sections depend on** (`picture_key`, `palette_key`): the
config with everything the 3D view alone reads set to its defaults. That
is the view's centre, zoom and rotation, the camera, the path tracer, the
solid tier, the downsample and the relief's light. The terrain's source,
interior, lake tint, prefilter and softness stay in. The palette, the
background and (for a 2D IFS) the flame are keyed by content. So a
camera, light or path-tracer edit re-renders no section.

### The section planner

`wanted_sections` decides, each frame, which sections the view wants:
1. **Roots:** a grid of sections of the coarsest level at least `far`
   wide, covering the disc of `far` view widths about the eye's ground
   point.
2. **Split** a section into its four children while its texels are
   coarser than `supersample × detail` a screen pixel at its nearest point
   from the eye. The split is limited to sections inside the view
   (widened by a fifth) and nearer than `far` in view depth, and goes no
   finer than `FINEST_REL` (2^−14 of the view width).
3. **Past the cap**, coarsen: the texel target is multiplied by the
   least factor that fits, bracketed in steps of 1.5 and then halved
   eight times (to within 0.16%). A view that needs coarsening spends
   the whole cap, and a moving view changes the sections at its margin a
   few at a time. Stopping at the first step of 1.5 that fit changed the
   whole ground's level at once wherever a moving view crossed a step: a
   video of a slow zoom went from 78 sections to the export's cap of 128
   between two frames, its finest detail visibly brighter before than
   after.

**The nearest ground in view** floors every distance in step 2. A box
knows only the relief's slab. With the eye inside the slab (a low
**Target lift**), every box under the eye contains the eye, at distance
0, and split to the finest level. That column lies under and behind the
camera where no pixel looks, and on a measured view it was 50 of 59
sections. The viewport fills sections nearest first, so the ground in
view waited behind the column, coarse, and started over with every turn.
- **The probe:** `TerrainRenderer::probe_nearest` traces one primary ray
  per 8x8 block, each block's last row and column, so the frame's bottom
  row is included. It reads the nearest hit back without waiting. It
  runs whenever what the walk reads changes.
- **The floor** (`next_near_floor`) is half that hit, rounded down to a
  power of two. It falls at once and rises only once the ground has drawn
  back fourfold, so a wavering measure cannot flip the sections between
  two splits.
- **Settling:** `update` stays busy while a probe is in flight, so an
  export's loop settles on a ground the floor has seen. On the desktop
  that loop waits on the GPU between updates, so the sequence is the
  same every time. A browser's export loop never yields to the event
  loop a read lands on, so there an export settles without the floor,
  as before it, and the viewport's frames apply it.

The result is ordered roots first, then nearest first. The planner works
in 3D, from the eye, and culls by the frustum, so a lifted target or an
eye looking up needs nothing extra.

A **hidden root** is one no primary ray reaches once its leaves are in:
split, or out of view. It still serves shadows, bounces and the moment
before its leaves arrive. In a deep (perturbed) picture it renders at
`COARSE_SAMPLES` (257 a side), a sixteenth of a section's. The roots
are the widest sections, and they cost the most a sample at depth.

**Detail** (`terrain.detail`, 0.1 to 8, default 1) is texels per screen
pixel, against the antialiasing factor.

**The cap:**
- **The viewport's** is `MAX_SECTIONS` = 64.
- **An export's** is fitted to the card by `fit_export_sections`: 256
  (`EXPORT_MAX_SECTIONS`), then 128, then 64, the first whose atlas and
  the rest of the export's peak (`export_headroom`, estimated high) the
  card will give. wgpu cannot say what memory is free, so it asks for it
  in pieces of up to 256 MB, stopping at the first refused, and gives it
  all back at once. Only a discrete GPU is asked: unified memory can
  grant far past what is comfortable and pay in paging, a browser may not
  allocate until a buffer is used, and a software renderer is the
  reference. They keep 64.

A terrain needs at least 64 texture array layers on the device
(`EscapeTerrain::allocation_error`).

### Building and caching

`EscapeTerrain::update` runs a frame's work:
- **A new picture** (the key changed) drops every section and resets the
  range. **A new anchor** drops every section.
- **The atlas** holds what the view wants plus a quarter, rounded up to
  eights, between 8 and the cap. Growing it remakes it, and the sections
  start over.
- **Missing sections render in chunks**, roots first, then nearest
  first: up to `steps` chunks a frame. The app gives as many steps as fit
  about 8 ms at the measured section time (1 to 4), and none while an
  export runs in the background. A section the view stops wanting is
  dropped mid-render. A full atlas evicts the section wanted longest ago.
- **The ground is set** from the ready sections only when the camera or
  the sections changed (`set_ground`), so a still view accumulates.
  While some are missing, other cached sections stand in. Once all are
  in, the ground is exactly the wanted ones, so the picture is the
  config's and not the cache's history. The ground's key includes the
  height and the lock's scale, because the slab top is set there and the
  walk clips every ray to it.
- **Every section in:** the section renderer gives back its per-pixel
  memory (`EscapeRenderer::release_pixels`) until the next section is
  wanted. At a 3x3 prefilter that is about 1.5 GB.

### Height sources and the interior

**Height from** (`TerrainSource`):
- **Distance estimate** (default): `H · exp(−d / w)`. The set is the
  plateau, every filament on smooth flanks of **Flank width** `w`. Where
  the formula has no derivative, the escape count is used instead, and
  the panel says so.
- **Escape count:** the smooth count, on a log curve by default; spiky
  toward the set.
- **Relief field:** the 2D relief's own height source, the field chosen
  in the Escape panel's relief settings; linear by default.
- **Paper creases:** a folding formula's paper (Origami's; see
  [ESCAPE.md](ESCAPE.md), relief shading), whatever the 2D relief draws,
  linear by default. A crease's slope runs out to one paper size (the
  line spread) and then levels off (`CREASE_TERRAIN_REACH`). Unbounded,
  the rise out to the horizon would set the measured range and flatten
  every crease near the camera. The sheet is continuous: every crease
  keeps its fold's direction along its whole length. Offered only for
  formulas with creases.

**Interior** (`TerrainInterior`): **Plateau** (flat at the top, in the
interior's colour), **Hole** (transparent, sunk to the floor) or **Lake**
(water at the plain's level).

### Heights are the view's, unless locked

**Height** (default 6%) and **Flank width** (default 1/150) are
fractions of the current view width, applied in the walk. A dolly in
flattens the far ground and raises the near detail: the terrain looks
like itself at every zoom. A count's or a relief's range is the ground's
own, measured as it is set.

**Lock height at this zoom** (`terrain.locked`, `lock_zoom`,
`lock_range`) fixes one landscape in the plane instead:
- **The lengths become the lock's.** The height, the flank width,
  occlusion's reach and a hole's depth are fractions of the lock zoom's
  view width. `EscapeConfig::terrain_scale` is `2^(zoom − lock_zoom)`
  current widths, held within 2^±24, and is applied wherever the walk,
  the camera and the planner read them.
- **The range is pinned.** A count's or a relief's range is the one the
  view measured when the lock was set, read back without waiting and
  handed to the panel. Locking therefore changes nothing on screen.
- **Zooming in approaches the landscape**, whose hills grow as you near
  them.

**Lock here** moves the lock to the current zoom and measures the range
again. The panel's **Range** edits the pinned range. A pinned range also
makes an export's count heights the viewport's: unlocked, an export
measures its own finer sections.

The camera's target still rests on the relief's top, so zooming in on a
locked terrain approaches a point at the peaks' altitude: a valley falls
away, and past the lock out of reach. The top is `height` lock widths,
which is 2^(zoom − lock_zoom) times that in current widths. Four zooms
past a lock at height 0.3, it stands about 5 view widths above the plain.
Ground that far below the eye is in the haze, which thickens from about
a third of `far` (8 widths) and ends at `far`. So the picture darkens
as you zoom and then goes to the background. A relief whose ground lies
mostly below its top shows this soonest; Origami's paper creases are
such a relief.

Ways down, by decision (2026-10-12) rather than a camera that follows
the land:
- type a negative **Target lift**, about −0.3 × 2^(zoom − lock_zoom)
  to reach the plain;
- **Lock here** at the new zoom;
- or unlock.

### Atmosphere

The ground reaches **Far** view widths (default 8) in view depth. Past
it there is no ground. **Haze** (default 1) is how much of that distance
the fog takes (`terrain_fog`): it starts at `far · (1 − 0.65 · haze)` and
reaches 99% at `far`. At 0 there is no fog and the ground's edge shows.
The terrain's fog is its own; the flame's fog fields are mode D's.

### Settings

`TerrainConfig` (`escape.terrain`), all skip-if-default and all
animatable (`Escape.Terrain.*`):

| Field | Default | UI (panel) |
|---|---|---|
| `enabled` | off | Show as a 3D terrain (Escape) |
| `source` | Distance | Height from (Escape) |
| `height` | 0.06 | Height, 0 to 30% (Escape) |
| `de_width` | 1/150 | Flank width (Escape, distance only) |
| `curve`, `invert` | Auto, off | Curve, Invert (Escape) |
| `locked`, `lock_zoom`, `lock_range` | off | Lock height at this zoom, Lock here, Range (Escape) |
| `interior`, `lake_tint`, `lake_roughness` | Plateau, (0.02, 0.05, 0.07), 0.05 | Interior (Escape); Lake tint, Lake roughness (Lighting: Surface) |
| `detail` | 1 | Detail (Escape) |
| `prefilter` | 1 | Prefilter: Off, 2x2, 3x3 (Escape) |
| `smooth` | off | Smooth surface (Escape) |
| `softness` | 0 | Softness (Escape) |
| `far`, `haze` | 8, 1 | Far, Haze (View: Atmosphere) |
| `tier` | Auto | Render tier (Lighting: Rendering) |
| `shadow`, `shadow_sharpness`, `occlusion` | 0.98, 12, 0.006 | Shadows, Shadow sharpness (Lighting: Shadows); Occlusion reach (Lighting: Shading) |
| `target_lift` | 0 | Target lift (View: 3D Camera) |

The camera angles are mode D's (`cam_pitch`, `cam_yaw`, `cam_bank`,
`cam_fov`); the antialiasing is the escape picture's `supersample`
(View: Image). The path tracer is `escape.path`.

---

## The simulation terrain

`SimTerrain` (`src/sim/terrain.rs`) draws one channel of a simulation
layer as a height field, on the same `TerrainRenderer` and tiers.

**The world is the grid's cells:** x right, y up the picture (north), z
the height. `height` is a field unit's height as a fraction of the grid's
width, so a sample's height is the field value times `height × grid
width` cells. A negative height turns the land upside down.

**The ground is one section** (`set_grid`, `GRID_WGSL`):
- **The height** is the relief stage's output over the terrain's own
  layer, channel and **Softness** (cells, default 1)
  (`SimRenderer::terrain_inputs`), the same stage the Relief colouring
  uses.
- **The albedo** is the colour stack evaluated once a cell.
- **The surface** is always the smooth spline, its max mipmap the Bezier
  bound.
- **The slab** is the heights' own range (mode 4), measured as they are
  made, its floor a hundredth of the ground's side below the lowest. A
  bound of 64 field units stands in where no range is measured.

**Edge** (`SimTiling`):
- **Grid alone:** the background beyond the grid.
- **Repeat:** the grid copied to the horizon. It is offered only on the
  periodic boundary, where the copies meet without a seam. The section
  gets one more sample each way, its first again, and `hf_lookup` reads
  the copy a whole period away.

Far and Haze apply only to a repeated ground (in grid widths, as the
escape terrain's). A single grid draws everything out to
`cam_distance + 4` grid widths, with no fog.

**Coloured by its height.** When the colour stack is the palette over the
height's own channel, a point is coloured by the palette at its own
height (`height_colour`), as the 2D picture is at its value. Bands then
follow the contours however steep the face. The stack qualifies when it
is one Channel colouring of the terrain's layer and channel: the base
colouring, or the one enabled colour layer at full opacity, Normal blend,
not gathered.

**The camera** (`sim_terrain_camera`):
- **The target** is `(target_x, target_y)` on the grid, as fractions,
  resting at half a field unit's height and raised by `target_lift` grid
  widths.
- **The eye** is `cam_distance` grid widths away (default 1.3).
- **The angles** are the solid camera's: pitch from the horizon (default
  0.6 rad), yaw 0 looking north, no roll.

**When the ground is remade** (`ground_key`): whenever the field or how
it is drawn changes. That includes every step of a running simulation, so
a run shows the lit tier and the path tracer gathers once it rests. The
key leaves out the terrain's camera and rendering settings, and keys the
palette by content, so a video's camera-only track does not remake the
ground each frame.

The lit tier's antialiasing grid is fixed at 2x2 (`SUPERSAMPLE`). The
path tracer reads `sim.terrain.path` (`Sim.Path.*`).

`SimTerrainConfig` (`sim.terrain`, `Sim.Terrain.*`): `enabled`, `layer`,
`channel`, `softness` (1), `height` (0.1) and `tiling` in the Simulation
panel; `far` (8) and `haze` (1), `tier` (Auto), `shadow` (0.98),
`shadow_sharpness` (12), `occlusion` (0.02) and the path block in the
Lighting panel; the camera in the View panel.

---

## Exports

The CLI, thumbnails, video and the desktop app's exports go through
`render_with` (`src/renderer/render.rs`). The browser's export builds the
same engines itself, in `src/app/mod.rs`.

**The escape terrain** (`render_escape_terrain`):
1. Fit the section cap to the card (`fit_sections_for_export`).
2. Render every section the export's own view wants before a sample is
   drawn.
3. Draw the still (`EscapeTerrain::render_still`).
4. Hand the image to the escape tail.

The progress bar gives the sections a quarter of its length when path
traced, nine tenths when lit.

**A simulation terrain** runs the simulation to its step count, makes the
ground from that field, then draws the still.

**The still** (`TerrainTiers::still`):
- **Lit tier:** the antialiasing grid, `supersample²` jittered renders.
- **Otherwise:** path traced at `samples`, in batches of about a quarter
  second at the measured cost (1 to 64 samples, starting at 2), with a
  wait between them. The wait is a blocking poll on the desktop and none
  in a browser. A cancelled export stops between batches.

**Still tiles** (`still_tile`, `STILL_TILE_SIDE` = 2048) keep the memory
bounded, at about 80 bytes a pixel of the terrain's buffers:
- A still up to 2048² pixels is drawn whole.
- A larger one is drawn in tiles of about that side, each copied into an
  `Rgba32Float` picture of the frame's size as it finishes.
- A tile's rays and samples are the frame's own (`set_frame`), so the
  tiles make the same pixels as the frame whole.
- A denoised tile is drawn with the denoiser's reach more around it and
  keeps only its own pixels.

**Mode D** path traced exports in bands of the export budget
(`render_solid_still`). A path-traced solid renders at display
resolution: its jitter is the antialiasing.

**In the app**, a terrain's custom-size PNG runs in the background
(`App::export_terrain_background`, `src/app/config.rs`) on a device of its
own (`headless_device`), with the progress bar. Meanwhile the viewport
freezes and gives back its terrain's memory. See
[EXPORT.md](EXPORT.md#gpu-memory-during-an-export), which also measures
the peaks.

**Memory, in sum:**
- an atlas section is about 21 MB: 64 is about 1.3 GB, 256 about 5.4 GB;
- the section renderer is about 148 bytes a render pixel while sections
  render, then released;
- a still's buffers are about 80 bytes a pixel, bounded by the tiles.

---

## Open items

- **The solid path tracer is slow.** Measurements and candidates are in
  [escape-sim-followups.md](../projects/escape-sim-followups.md).
- **Scripts cannot reach the terrain or the path tracer.** There are no
  terrain functions, and `config.set` will not create the skip-if-default
  groups. That is the open decision D2 in
  [escape-sim-followups.md](../projects/escape-sim-followups.md).
  Animation tracks do reach them.
- **No lens on the terrains.** Their rays carry the jitter where mode D's
  carry the lens; see
  [camera-lens-everywhere.md](../projects/camera-lens-everywhere.md).
- **Deferred from the plan:** HDRI environments, sun shafts and terrain
  presets ([heightfield-3d.md](../archive/terrain/heightfield-3d.md)).
- **A locked terrain's camera** rests on the locked top, not on the
  ground under the view's centre, which the GPU holds and which moves as
  finer sections land (terrain-height.md, H3).
