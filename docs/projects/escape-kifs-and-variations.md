# KIFS solids, and flame variations in escape-time

Planned 2026-10-17. Three parts, in this order of work: **A** the
distance-estimated solids mode D was built to take (KIFS first, then
the Mandelbox and the Mandelbulb), **C** flame variations written in
Rhai, usable by flames and by escape-time formulas alike, and **B**
editable solids, which waits on what C builds. Section **D** records a
decision that came up beside them: the field fractals stay where they
are.

Background: mode D's distance functions
([docs/main/ESCAPE.md](../main/ESCAPE.md), `escape::ifs`), the formula
language ([escape-formula-language.md](escape-formula-language.md)), the
camera lens (`escape::lens`), the variation registry
([docs/main/VARIATIONS.md](../main/VARIATIONS.md)).

## A. Solids by distance estimate (mode D)

Mode D's registry was made for these: the ifs-distance plan
([docs/archive/escape-time/ifs-distance-rendering.md](../archive/escape-time/ifs-distance-rendering.md),
end of §5) names the Mandelbulb, Mandelbox and KIFS distance functions
as `IfsDef`s that need no flame, "catalogue work the day phase 3
lands", so nobody plans a second marcher. Phase 3 landed, and the
quaternion Julia (§8.11 step 1) is the pattern: a def with
`solid: true, needs_flame: false` supplies `ifs_walk3(delta, eps) ->
IfsResult`, and the solid template supplies the camera, the march, the
normals, shadows, occlusion, the relight cache, the path tracer, the
material and light rig, and the four colourings. Parameter slots 0-6
are the template's (levels, beam, steps, shadow, shadow sharpness,
occlusion, the 4D slice), unused ones kept so the slots line up; the
def's own follow.

**A1. KIFS (Solid)**, Knighty's kaleidoscopic IFS (Fractal Forums,
2010; the forms here follow Syntopia's "Distance Estimated 3D Fractals
(III): Folding Space", 2011). Each iteration rotates, folds space into
one fundamental domain, rotates again, and scales about the offset:

```text
z = R1 z;  z = fold(z);  z = R2 z;  z = s z - o (s - 1)
```

- **Folds**, a choice: *tetrahedral* (the Sierpinski tetrahedron's three
  plane folds, `x + y < 0` and its two kin), *cubic* (`abs` of each
  component), *octahedral* (`abs`, then sorting the components
  descending), *Menger* (the octahedral folds and Syntopia's correction
  of `z`, which makes the sponge's twenty subcubes).
- **Parameters**: the fold, the scale `s`, the offset `o` (three), the
  two rotations (three angles each, degrees), the iterations (slot 0).
- **The distance** is `(|z_n| - |o|) / s^n` once `|z_n|` has passed the
  bailout, else zero. A lower bound, not an estimate: every fold and
  rotation is 1-Lipschitz and fixes the origin, so a point past radius
  `|o|` escapes and every point of the set lies within it, and each
  step stretches distances by at most `s`. (Syntopia's `|z_n| / s^n`
  overestimates near the set; the Menger correction is a conditional
  translation, not 1-Lipschitz, so for it the bound is practice, not
  proof.)
- **The ball** the camera frames and the march is bounded by is radius
  `|o|` at the origin (`pack_standalone` learns the def's radius; it
  read `bailout` for the quaternion Julia).
- **The four quantities**: as *Colouring solids* below (they were the
  escape count and the escaped point at first, which follow the
  camera).
- **A CPU twin** of the walk, in Rust, for the gates: the quaternion
  Julia has none, and this one is twenty lines.
- **Gates**: (1) every mode-D combination compiles (by joining
  `IFS_DEFS`); (2) *the bound*: on the CPU twin, for points around each
  preset's set, the distance never exceeds the distance to the
  attractor sampled by its inverse maps (a chaos game of the inverse
  branches), and the GPU's walk matches the twin; (3) the presets --
  the Sierpinski tetrahedron, the Menger sponge, an octahedral flake, a
  rotated KIFS -- rendered and inspected before they ship.

**Colouring solids** (decided 2026-10-18, after the user saw Escape
Level and Orbit Trap change with the camera on KIFS and the quaternion
Julia). A solid is coloured where the march stops, a pixel's width from
the set, and that width is the camera's. Whatever a colouring reads
from the orbit's tail -- when it escapes, where it lands -- is decided
by that pixel: measured (`which_solid_colours_follow_the_camera`),
marching each ray to a pixel, an eighth and a sixty-fourth of one, the
escape level and the escaped point move 0.2 to 1.2 palette units at
every step, never settling. So every solid's quantities come from **the
start of its orbit**, the first `colour_depth` iterations (default 3):

- **Orbit trap**: the orbit's closest approach to the origin within the
  depth, as two coordinates the set's symmetry keeps (KIFS: `z.xy`,
  which the first fold makes the same for every mirror image; the
  quaternion: the scalar part and the part along `c`'s vector axis).
- **Level**: `log2` of the running derivative at the depth, where it
  varies over the set (the quaternion; the Mandelbox and Mandelbulb);
  where it is a constant (KIFS: `s^n`), the iteration of the closest
  approach plus how close it came.
- **Address**: digits of choices the set's symmetry keeps. KIFS: the
  fold history from `address_symmetry` folds on (0 off, 1 the whole
  object, 2 each piece too), because mirror images share their orbit
  after the first fold. The quaternion: which side of the plane across
  `c`'s axis each iterate falls (its binary decomposition), then the
  vector part's angle from the axis at the depth.
- **Gate**: `the_solids_colours_settle_as_the_view_refines`, on each
  def's CPU twin: at the default depth each quantity moves under 0.15
  palette units as the view refines eight times, and settles (the next
  eight at most 0.6 of that). Every new solid's twin joins it.

Measured at depth 3 (mean change, 1 to 1/8 pixel, then 1/8 to 1/64):
the Menger's level 0.027 then 0.005, trap 0.086 then 0.015, address
0.007 then 0.002; the quaternion's level 0.034 then 0.008, trap 0.086
then 0.020, address 0.043 then 0.012. Past depth 5 the colours stop
settling at 448 pixels: deeper iterations are the pixel's again, which
is why the depth is a parameter (raise it when zoomed in). The flame
solids (`ifs_flame_3d`) still colour from the tail: an open item.

**A2. Mandelbox (Solid)** (Tom Lowe, 2010): box fold, sphere fold
(minimum and fixed radius), scale, plus the point (the Mandelbox) or a
constant (its Julia). The distance `|z| / |dr|`, `dr` the running
scale. **A3. Mandelbulb (Solid)** (White and Nylander, 2009): the power
`n` in spherical coordinates, the distance `½ ln|z| |z| / dr` as the
quaternion Julia's. Each with the same gates, its twin and presets, and
coloured by the rules above from the start: the level the running
derivative; the trap the closest approach; the address symmetric --
the Mandelbox's per iteration how many axes the box fold reflected and
which of the sphere fold's three cases applied (invariant under the
cube's symmetries, which commute with both folds), the Mandelbulb's
which hemisphere each iterate falls in (invariant under its turns about
the z axis). Each is checked against the settling gate before its
presets are picked.

**A4, later.** Icosahedral and dodecahedral folds (normals from a cited
source, not derived here); the 2D KIFS as a Rhai formula in mode A,
which the language can already state (Kaliset is its cousin).

Not in A: deep zoom (the walk is `delta + target_offset` in f32,
§8.11's Q3), and editing the walk (B).

## C. Flame variations written in Rhai

What exists: the **camera lens** warps the escape view by any variation
(`escape::lens`; `ShaderBuilder::build_layer_map_at` emits the
variation and a `flame_map`, which the simulation's layer warp uses
too). What does not: a variation inside an escape formula's iteration,
and a variation a user can write.

The constraint that orders the work: the 647 variations exist only as
WGSL. The escape engine's CPU half -- deep zoom's reference orbits and
derived equations, the series approximation, Center on minibrot,
`global:`, the degree -- all runs through the formula language's
interpreter, so a built-in variation inside a formula would leave it on
the direct path (about zoom 14), with no distance estimation. A
variation written in the formula language's Rhai dialect has the
interpreter, the WGSL and the derivation passes, so it gets everything
a formula gets. (The dialect, not the scripting API's Rhai, which runs
on the CPU building configs and cannot run per sample.)

**C1. The transformation entry kind**, Rhai only: `fn variation(p,
params)` from a point to a point, with `PARAMS` as a formula's (the
plan's "Later: transformations as flame variations" in
[escape-formula-language.md](escape-formula-language.md)).
- *For flames*: compiled to a `VariationDef`, 2D WGSL reading its
  parameters through `get_param`, its 3D WGSL passing `z` through as
  JWildfire's 2D variations do; registered at runtime as plug-ins and
  API variations are (`global_registry_mut()`, outside the append-only
  order). The flame config carries the source, as an escape config
  carries a custom formula's; a `.flame` export cannot (JWildfire has
  no such variation) and says so; the API repository is told
  ([docs/RELEASE.md](../RELEASE.md) §3).
- *For escape-time*: callable from a formula as a user function (IR
  inlined), so deep zoom and distance estimation follow where the
  rules derive them; and offered as a lens.
- *Gates*: Rhai twins of a few built-ins (`linear`, `spherical`,
  `swirl`, `polar`, `horseshoe`) render as the built-ins in a flame, as
  a lens, and inside a formula against the same formula written out.

**C2. Hooks**, each its own step: random numbers (`NeedsRng`: a
deterministic per-pixel stream in escape-time, the flame's own RNG in a
flame); JWildfire's accumulator (`NeedsAccum`); per-thread state;
colour writes.

**C3, maybe.** A built-in WGSL variation as a step in a formula,
direct path only, the program marked so every CPU path refuses it.
Decided after C1, by whether the Rhai versions cover what is wanted.

## B. Editable solids (long term)

The KIFS parameters (A1) are the editable part for now: the fold, the
rotations, the scale and the offset span the family. A user-written
walk needs what the formula language does not have: **a 3-vector
type**, end to end -- IR types, the interpreter's values, the WGSL
emitter, the Rhai lowering (`vec3(x, y, z)`, `.x`, `dot`, `length`,
`abs`), and the derivation passes (or none: a solid is direct-path
anyway) -- and a solid entry kind compiling to `ifs_walk3`, its running
scale tracked by the program or derived. Not easy, by that list; it is
the same vector type C needs for 3D variations, so it is built once,
for both, after C1.

## D. Decided: the field fractals stay in mode B

Asked 2026-10-17: should Weierstrass Field, Markus-Lyapunov and the
Standard Map FTLE move to the Simulation engine? No. Their iterations
are not steps: Weierstrass's count is the terms of a sum, the other
two run each pixel's own orbit from its own position and average along
it. No pixel reads its neighbours and nothing carries between frames;
the picture is a function of position, which is why it zooms. A
simulation steps one grid whose cells read their neighbours, and the
grid is the information (it does not zoom). If they move, it is into
the formula language as twins (a never-escaping program and an
averaging colouring), with Weierstrass's analytic gradient for the
hillshade the part to check.

## Order of work

1. [x] A1 KIFS (Solid). **Done 2026-10-17**: `kifs_solid` in
   `IFS_DEFS`, as specified above. Measured: the GPU's coverage equals
   the CPU twin's march on every pixel for seven sets (each fold, two
   with rotations, a turned Menger); the bound holds at 2,000 points per
   set with no set point inside a promised ball (the Menger's
   correction included, though it is not proved); cubic folds at scale
   2 draw the box's silhouette grown by the march's pixel of tolerance
   (660 pixels grown of a 772-pixel ring, none shrunk). Six presets,
   picked from rendered candidates: KIFS Menger, Tetrahedron,
   Octahedron (offset (1, 0, 0): the octahedron flake), Turned (the
   sponge with a rotation either side of the fold), Bloom, Columns.
   Found on the way: an octahedral offset off the axes at scale 2 is a
   blob of shells, and the octahedron's vertices sit on its ball, so it
   frames at zoom 0. **Coloured from the start of the orbit
   2026-10-18** (*Colouring solids*): the KIFS and the quaternion Julia,
   with `colour_depth` and the KIFS's `address_symmetry`; the presets
   re-inspected (Bloom moved to the level; the quaternion's preset to
   the orbit trap and zoom 0.4, where 0.7 cut a lobe).
2. [ ] A2 Mandelbox (Solid).
3. [ ] A3 Mandelbulb (Solid).
4. [ ] C1 Rhai transformations, 2D.
5. [ ] C2 hooks, then B (the vector type) for 3D variations and
   editable solids; A4 and C3 as wanted.
