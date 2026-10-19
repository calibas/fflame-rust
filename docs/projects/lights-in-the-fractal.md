# Lights in the fractal

Planned 2026-10-19, at the user's request: "I want to be able to place
lights freely inside the fractal space, not just outside. I want an
option for a 'headlamp', so adjustable light that's attached to the
camera view. I want to be able to light sections up when moving around
'inside' the 3d fractals." And a bug: "The Ambient lighting setting
doesn't seem to do anything."

Scope: every 3D view the escape engine draws -- mode D's solids and the
terrains (escape and simulation) -- in both tiers, the lit tier and the
path tracer. The 3D flame's splat shade pass keeps directional lights
(its lights are screen-space; see *Not in this plan*).

## Where lights are today

`SolidShadingSettings::lights` (`config/fractal_config.rs`): up to four
`SolidLight`s, each **directional** -- an azimuth and an elevation, an
intensity and a colour. Mode D's are in camera space (rotated into the
world once, `ifs::pack_rig3`); a terrain's are world-fixed
(`terrain::pack_rig`). Each light is two `vec4`s of the uniform's
`fdata` (11 + 2i: direction and power; 12 + 2i: colour, `w` unused). A
light is read in four places:

1. **The walk** (lit tier): one traced shadow ray per light toward its
   direction (`ifs_shadow`, out to the bounding sphere), stored raw in
   the geometry record (four unorm8 terms).
2. **The relight**: `ifs_rig` -- Lambert, the coat, the highlight, times
   the stored shadow term; the ambient times the occlusion.
3. **The path tracer** (`path_core::pt_path`): each light sampled over
   its angular radius (`pt_cone`), an unbounded visibility ray
   (`pt_scene_visible`).
4. **The terrain's** own walk and packing (`terrain.rs`), the same rig.

**Why Ambient does nothing (measured):** the path tracer never reads it.
Its only fill light is the environment, the background times Sky light,
on rays that escape after a bounce. So in the Path traced tier, and in
Auto once the view settles -- a terrain's default -- the slider is dead.
Mean brightness of a 96x72 render at Ambient 0 / 0.5 / 1: a sponge solid
96.6 / 116.9 / 117.5 lit, 114.7 / 114.7 / 114.7 path traced; the default
terrain 19.3 / 32.0 / 37.9 lit, 20.5 / 20.5 / 20.5 path traced.

## Order of work

### L1. Ambient in the path tracer

The ambient becomes uniform environment light the path tracer's bounces
see, added to the sky: an escaped bounce ray brings back
`sky(d) + ambient`. That is the lit tier's own term traced rather than
estimated -- an open Lambert surface gets `albedo x ambient` in both
tiers, and the path tracer occludes it by the geometry where the lit
tier multiplies by its occlusion estimate. The camera's view of the sky
is unchanged (it is the tonemap's background).

- An **untouched** rig contributes no ambient to the path tracer, so
  every picture already path traced with the default rig is unchanged.
  A touched rig's Ambient (0.25 as the panel ships it) now brightens
  path-traced shadows -- a visual change to such saved pictures,
  accepted as the fix.
- At 0 bounces the path tracer draws direct light only, so no ambient,
  as no sky.
- The firefly clamp counts the ambient among the lights.

Gate: the cube (convex, so a bounce never meets it again) lit by the
ambient alone path traces to the lit tier's `albedo x ambient` within
the noise; the sponge's path-traced brightness rises with Ambient.

**Done 2026-10-19.** The cube matches the lit tier exactly on every face
pixel (one bounce is the whole of it, so there is no noise); the
sponge's light rises 0.185 per 0.2 of Ambient, twice alike. The escape
visual suite is unchanged within tolerance (the two configs carrying a
touched ambient move little under their sky).

### L2. The model: three kinds of light

`SolidLight` gains, skip-if-default so old files stay as they are:

- `kind`: **Directional** (as now, the default), **Point** (a position
  in the world), **Headlamp** (attached to the camera).
- `position: [f64; 3]` (Point): world coordinates.
- `reach: f32`: the falloff's length. A light's power at distance `d` is
  `I x R^2 / (R^2 + d^2)` -- the inverse square far from the light,
  finite at it, half at `R`. A Point's reach is in world units; a
  Headlamp's is in **camera distances** (the eye's distance to its
  target), so the lamp scales with the zoom: it lights the same share of
  the view at any depth.
- `cone: f32` (Headlamp): the spot's half-angle in degrees, 180 for no
  cone; the edge softened over its outer tenth.
- `lift: f32` (Headlamp): how far above the eye it sits, in camera
  distances (0, at the eye). A lamp at the eye casts every shadow behind
  what casts it, out of sight; a little lift shows them, which is what
  reads as depth.

Shadows of a Point or a Headlamp are soft by the same Shadow sharpness as
a directional light's (a light's angular radius, `light_radius`), so no
size parameter.

GPU: a third `vec4` per light, `fdata[28 + i]`: (reach in world units,
the cone's outer cosine, its inner cosine, the kind). The first `vec4`
holds the position -- an offset from the target in mode D, as every
position there is -- for a Point or a Headlamp; the CPU places the
headlamp (eye + lift x up) and converts its reach. The terrain's
`fdata` grows from 28 to 32 slots for it.

Paths: `SolidLightParam { index, param }` with the new params `kind`,
`x`, `y`, `z`, `reach`, `cone`, `lift`; animatable as the others.

**Done 2026-10-19**, as above. The position is f64 and travels as
`ConfigValue::Double` (a track's keyframe as a Float); the f32 params
share one setter with the exporter (`set_light_param`). The 3D flame's
shade pass draws directional lights alone (`SolidLight::directional_on`).

### L3. Mode D, the lit tier

- The walk: a Point's or a Headlamp's shadow ray goes toward the light
  from each surface point and stops at it (`ifs_shadow` takes the
  light's distance as its end); skipped where the surface faces away.
- The relight: the surface's position is the eye plus the ray times the
  hit distance, both of which it has; from it, each light's direction,
  falloff and cone. `ifs_rig` takes the position.
- The walk's key: each light's kind and position as well as its
  direction (they decide the shadow rays). The headlamp moves with the
  camera, which re-walks anyway.

**Done 2026-10-19.** `ifs_light_at` in the rig's shared text gives a
light's direction, distance and gain at a point (the offset scaled
before it is squared, for deep views); `ifs_rig` takes the surface's
position. A directional light's numbers are unchanged -- its gain is 1
and its shadow ray's end past the ball -- and the escape, solid and
simulation visual suites pass unchanged (125, 6, 107). Gates: a point
light far off with a reach past everything is the directional light it
approaches (worst relative difference 1.9e-3, its angle across the
cube); inside the sponge's cavity a headlamp's shadows-on picture is
98.5% of shadows-off -- and 0 with the shadow ray not stopped at the
lamp, which is what the gate is for -- and a 10-degree cone leaves the
corners black (`a_far_point_light_is_a_directional_one`,
`a_headlamp_lights_the_inside_of_a_sponge`).

### L4. Mode D, the path tracer

- A Point or a Headlamp is sampled over its angular radius about the
  direction to it, its visibility ray stopping at it
  (`pt_scene_visible_to`, each geometry's bounded visibility); the same
  falloff and cone as the rig.

**Done 2026-10-19**, as `pt_scene_visible(o, d, t_end, travelled)` in
both geometries. Gate: a point-lit cube path traces to the lit tier's
picture (mean relative difference 4.5e-4, worst 5.8e-3 -- the path
tracer's jitter across a pixel, over which the light's distance changes;
`a_point_lit_solid_is_the_lit_tiers`).

### L5. Terrains

The same three kinds on the escape and the simulation terrains: the
packing (world positions in the terrain's world, the headlamp from its
camera), the terrain walk's bounded shadow march, its path tracer's
bounded visibility.

**The headlamp done 2026-10-19; a point light deferred.** The terrain's
world is the current view's -- its unit the view width, its origin the
eye's ground point -- so both move with every pan and zoom, and a light
"in the world" there needs a frame of its own first (the plane's
coordinates and a height in some fixed unit). A point light on a terrain
is off, and the panel says so. The terrain's `fdata` grew to 32 slots.
Gates: a flat plane under a headlamp is its closed form to 4.1e-6, and
the path tracer's is the lit tier's (mean 1.6e-4;
`a_headlamp_on_a_plane_is_its_closed_form`,
`a_headlamp_lit_plane_is_the_lit_tiers`).

### L6. The panel

Each light: a Kind picker (Directional, Point, Headlamp; a flame offers
Directional alone). Directional: Azimuth, Elevation. Point: X, Y, Z,
**Place at camera** (the eye's position now, and a reach of the camera's
distance -- fly inside, drop a light), Reach. Headlamp: Reach, Cone,
Lift. All: Intensity, colour. The animation picker offers the new
parameters.

**Done 2026-10-19.** `visibility::light_kind` answers which view offers
which kind; one it does not is greyed in the picker and said under the
light. A light first made a point is placed at the camera. The reach of
a point is a log slider about the camera's distance (its value shown
unclamped past either end). The animation picker offers each light by
its kind -- lights 1 and 2 always, as before, and any other that is on.
Gates: the View panel lays out with a light of each kind in every view;
each view offers its kinds; the exporter applies every light parameter
offered as the app does.

### L7. Docs

TERRAIN.md (*Lighting and material*), CAMERA.md (the headlamp), the
animation targets, PR.md.

**Done 2026-10-19:** TERRAIN.md *Kinds of light*, CAMERA.md *The
headlamp*.

## Not in this plan

- **The 3D flame's splat shade pass** lights in screen space from a
  depth buffer; a point light there needs world positions rebuilt from
  depth. A flame offers Directional alone, and a flame config carrying
  another kind (pasted rig) treats it as off.
- More than four lights (the geometry record holds four shadow terms).
- Placing a light by clicking a surface; a fly-mode key to drop one.
- Emissive geometry (the material's Glow is the albedo's own light, not
  a light that shines on anything else).
