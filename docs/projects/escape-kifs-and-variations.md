# KIFS solids, and flame variations in escape-time

Planned 2026-10-17. Three parts, in this order of work: **A** the
distance-estimated solids mode D was built to take (KIFS first, then
the Mandelbox and the Mandelbulb), **C** flame variations written in
Rhai, usable by flames and by escape-time formulas alike, and **B**
editable solids, which waits on what C builds. Section **D** records a
decision that came up beside them: the field fractals stay where they
are.

The direction this serves -- every plug-in a Rhai script, carried in
the config, parameters animatable -- is recorded in
[rhai-plugins.md](rhai-plugins.md) (2026-10-18), with the design rules
C keeps.

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
solids (`ifs_flame_3d`) colour from the start of their inverse walk
since A5.

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
**Done 2026-10-18** (the user's call):
- **The folds** (KIFS Fold 4 and 5) as Syntopia's Fragmentarium examples
  `Icosahedron.frag` and `Dodecahedron.frag` (2010) write them, after
  Knighty's kaleidoscopic IFS (fractalforums.com, "Kaleidoscopic
  (escape time) IFS"). Icosahedral: normals `(-phi, phi-1, 1)`, `(1,
  -phi, phi+1)`, `(0, 0, -1)`, normalised; a prefold once, into the
  fundamental domain (abs, then n1, n2, n3, n2), then each iteration
  abs and n1, a mirror reflecting a point on its positive side.
  Dodecahedral: `(-1, phi-1, 1/(phi-1))` and its cyclic turns; each
  iteration three passes of the three, reflecting a point on its
  negative side. *Decision*: **the golden ratio exact**, where the
  sources' slider holds 1.618 -- only then are the planes the group's
  mirrors. Checked, not taken on trust
  (`the_golden_folds_are_the_icosahedrons_mirrors`): each fold's planes
  map an icosahedron's twelve vertices onto themselves, and the
  icosahedral fold's three meet at H3's angles, pi/2, pi/3, pi/5 (the
  two sources use the icosahedron's two orientations, mirror images).
  The rotations keep our order (before the fold, after it), so with
  turns the walk is ours, not the sources'; at none it is theirs. The
  estimate stays the strict lower bound. The address digit: the
  icosahedral fold's signs flipped and its n1; the dodecahedral's first
  pass's three mirrors. Both joined every KIFS gate: the bound, the
  ball, the colours settling, every mode-D combination compiling, the
  GPU's coverage the twin's (0 pixels apart for both). Presets, from
  `render_golden_kifs_candidates`: KIFS Icosahedron (the source's
  defaults, level), KIFS Icosahedral Flake (scale 2.5, address), KIFS
  Dodecahedron (the source's defaults, level). The orbit trap is busy
  on the dodecahedral folds; it settles (the gate), so that is the
  geometry, not the camera.
- **The 2D KIFS** as `assets/formulas/kifs-2d.rhai`, in the library:
  fold (Square, Diagonal, or Carpet -- the sponge's correction in 2D),
  turn, scale about the offset; the Sierpinski carpet at the defaults.
  Checked against the carpet's own base-3 digits
  (`the_kifs_2d_example_draws_the_carpet`): every point in a hole
  escapes, three to five steps after its hole's level, one step later
  per level on average (4.52, 5.44, 6.52, 7.52, 8.47, 9.41); a corner
  stays.

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

C1's steps (planned 2026-10-18, started while the user was away; the
decisions are marked for review):
- **C1a, the language.** A Rhai source with `fn variation(z, p)` is a
  transformation: `z` the point (complex), `p` the parameters, the last
  expression the new point; `PARAMS` and `INFO` as a formula's, plus
  `INFO.name`, the variation's registry name. In the IR it is UF's own
  `transform:` section (`Program::transform`, a new entry kind): the
  point is `#pixel`, read as the input and assigned as the output --
  *decision*: UF's convention, so a `.uxf` transformation reads into the
  same thing when UF support resumes. The checker allows the parameters,
  `#pixel`, `#pi`, `#e` and local variables; no `prepare` (a variation's
  derived parameters are its `wgsl_init`: later), no random numbers
  (C2). The interpreter runs it (`interp::transform`); the WGSL emitter
  writes `fn variation_<name>(p: vec2<f32>, xform_id: u32, variation_id:
  u32) -> vec2<f32>`, parameters through `get_param`, its helpers
  prefixed with the name so two custom variations never define one
  twice; the 3D body runs the 2D one on `p.xy` and passes `p.z` through,
  as JWildfire's 2D variations do (our builder zeroes `z` under
  `preserve_z = false`, as for them).
- **C1b, registration** (`variations::custom`): a source compiles to the
  same `VariationDownload` a plug-in is, registered with a new
  `Provenance::Custom` (*decision*: neither `Local`, which is a plug-in
  file, nor a download). The name must be a lowercase identifier and is
  refused if a built-in, download or plug-in has it; registering a
  custom name again replaces it, and the registry's version rebuilds the
  shader.
- **C1c, the config carries it.** The flame's `custom_variations`
  (sources, skip-if-empty), registered on load and before every render
  (`render_with`, as `escape::custom::ensure`), so CLI, thumbnails,
  video and the gallery inherit it. A `.flame` export cannot carry it
  and drops the variation with a warning.
- **C1d, the editor.** The Formula Editor in the flame modes too; a
  transformation's source gets "Use as variation", which registers it
  and adds it to the flame's list; it is then in every transform's
  variation picker (category Custom).
- **C1e, escape-time.** The camera lens takes any registered variation,
  so a custom one is a lens with nothing more; calling one inside a
  formula (inlined as a user function) is the step after.

**C2. Hooks**, each its own step: random numbers (`NeedsRng`: a
deterministic per-pixel stream in escape-time, the flame's own RNG in a
flame); JWildfire's accumulator (`NeedsAccum`); per-thread state;
colour writes.

C2's steps (planned 2026-10-18, the user away; the Rhai surface is the
decision to review -- each keeps the script honest Rhai, and each hook
is the `Feature` a built-in declares, so the shader builder has one path):
- **C2a, random numbers.** `rand()`: a fresh draw in [0, 1) each call,
  in any entry. In a variation, the flame's own stream
  (`rng_nextf(rng)`; `NeedsRng`); in a formula or colouring -- or a
  variation `vary` inlines there -- a deterministic stream per pixel,
  seeded by its place, so a still reproduces; the interpreter draws the
  same numbers. An entry that draws has no deep zoom (the reference and
  the pixel would draw differently). The IR's `Predef::Draw`, not
  UF's `#random` (one value per pixel, as UF's).
- **C2b, the accumulator.** `fn variation(z, p, ctx)`: `ctx.accum`,
  JWildfire's `pVarTP` -- the weighted sum of the transform's earlier
  variations (`NeedsAccum`). Zero where `vary` inlines one.
- **C2c, per-thread state.** `const STATE = #{ count: 0, last:
  complex(0.0, 0.0) }` declares the fields and their starting values;
  the body reads and writes them as `this.count` -- Rhai's own way for a
  function called on an object to change it. Slots per (transform,
  variation) per thread (`state_count`, `get_state` / `set_state`, the
  starting values in `wgsl_state_init`), kept with the orbit. Refused
  where `vary` inlines one (a formula's state is its own `start` map).
- **C2d, colour writes.** The body may end with `#{ z: point, color:
  index }`: the palette index the transform's `direct_color` blends in
  (`WritesColor`). Where `vary` inlines one, the point alone.
- Gates: each against a built-in twin where one exists (rand: a blur's
  twin by density; accum: a `pVarTP` reader; colour: a direct-colour
  variation), and the escape stream against the interpreter.

**C4, proposed (2026-10-18): transformations for escape-time beyond
the lens.** The user asked what UF does. Its transformations run once
per pixel before the formula, rewriting `#pixel` in the fractal's
coordinates and able to mark it solid (clipping), and a layer stacks
them; and the same `Transform` classes transform whole orbits as
plug-in parameters -- the trap colourings' "Trap Position" moves every
orbit value before the trap distance is measured (`common.ulb`'s own
documentation; [uf-transforms-review.md](uf-transforms-review.md)).
Ours: the lens (one variation on the screen offset), `vary` (inside a
formula, edited in). Missing: a stack of variations on the sample
point in fractal coordinates, with a clip; an orbit transform a
colouring reads through; a transform plugged into any formula without
editing it. Rhai variations would serve all three. Not planned until
the user says.

C4's steps (planned 2026-10-18, the user's go-ahead; decisions marked):
- **C4a, the model.** `EscapeConfig::transforms`, a list of
  `EscapeTransform { stage, variation, amount, params }`, skip-if-empty;
  the stage one of **Plane** (the sample point, in the fractal's
  coordinates, before the formula -- UF's transformation), **Orbit**
  (each orbit value the colouring accumulates -- UF's trap position)
  and **Iteration** (`z` after every step of the formula). Any
  registered variation with a 2D body, built-in or the user's (whose
  source the flame carries). Paths: `EscapeTransforms` (the list, one
  undo step per structural edit), `EscapeTransformAmount { index }` and
  `EscapeTransformParam { index, param }` (animatable).
- **C4b, one maps flame.** The lens's one-transform flame becomes the
  escape view's maps flame (`escape::lens`): the lens, then each
  transform, one transform each, in group 2 as now, so a browser's
  storage-buffer count does not grow; the glue gives `esc_lens`,
  `esc_plane`, `esc_orbit` and `esc_iter`, each the stage's slots in
  order, `p = mix(p, flame_map(k, p, seed), weight)` -- the amount in
  the transform's weight as the lens's, so a drag recompiles nothing.
  *Decision*: the **amount blends toward the identity** as the lens's
  does (0 none, 1 the variation), where UF's transformations have none.
- **C4c, the templates**, by line substitution so a shader without
  transforms stays byte-identical: Plane rewrites `esc_run`'s entry
  (`pixel = esc_plane(pixel_in)`, so the relief's neighbour orbits and
  the terrain's samples are mapped too); Orbit wraps the colouring's
  accumulation (`coloring_accum(esc_orbit(z), esc_orbit(z_before), ..)`,
  on the direct and the perturbed path alike: the colouring reads the
  full `z` there); Iteration follows the step (`z = esc_iter(z)`).
  *Decisions*: **Orbit reaches only what the colouring accumulates per
  iteration** -- trap and average colourings -- not the final `z` an
  escape count or a smooth colouring reads, which is UF's trap
  position; **Plane and Iteration keep the direct path** (a
  transformed absolute coordinate is not a perturbation of the
  reference's), so with either deep zoom stops at the direct path's
  limit; the derivative is not transformed, so a distance colouring
  under Plane or Iteration is not a distance. Mode A only: mode B's
  fields and mode D keep the lens alone (later).
- **C4d, the panel**: a Transformations section in the escape panel
  (mode A formulas): each entry's stage, variation (the lens's picker),
  amount and parameters (`render_variation_params` through a
  `TransformTarget`), remove and move; Add. A user's variation picked
  here is carried in the flame's `custom_variations`, as the lens's
  now is too.
- **C4e, gates**: without transforms, the shaders byte-identical; a
  Plane `linear` at 1 is the identity; a Plane transform renders as the
  formula run at the mapped point in the interpreter (escape counts on
  the GPU against the interpreter); an Orbit translation equals the
  trap colouring's own centre moved; an Iteration transform equals the
  formula with the map written into its step (a Rhai twin); Plane and
  Iteration turn deep zoom off, Orbit keeps it.
  **Done 2026-10-18**, as above (ESCAPE.md, *Transformations*). The
  gate for Plane against the interpreter became a mirror's exact
  geometry instead (the mapped point is the mirrored pixel's); the rest
  as planned. Measured: Plane `linear` 0 pixels apart; the mirror 6 of
  9,216 (the mirrored pixel's f32 offset); Orbit 0; Iteration 0 for a
  built-in and a program formula. Also: the lens picker now carries a
  user's variation's source too (`variations::custom::carry_change`),
  and the Formula Editor keeps a source a transformation uses. Not
  done: mode B and mode D, clipping (UF's `#solid` from a
  transformation), the colouring's own start reading the Orbit map.

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

B's steps (planned 2026-10-18, the user's go-ahead; the decisions are
marked for review):
- **B1, `vec3` in the Rhai dialect, split into scalars by the lowering.**
  *Decision*: the IR, the interpreter, the WGSL emitter and the
  derivation passes stay scalar -- a vector local is three float locals,
  a vector field three fields, `dot(a, b)` a sum of products -- so
  nothing past `rhai.rs` changes and deep zoom derives through vector
  code as through any other (a GPU thread runs scalar code anyway; the
  `Ty` machinery is matched in some 180 places). The surface:
  `vec3(x, y, z)`; `v.x`, `v.y`, `v.z`, read and set; `+`, `-`, unary
  `-`; `*` and `/` by a number (a vector times a vector componentwise);
  `dot`, `cross`, `length`, `normalize`, `abs`, `min`, `max`, `clamp`,
  `floor`, as functions or methods; a vector in a state map; a `PARAMS`
  entry `vec3(...)` as three float parameters `name.x`, `.y`, `.z`.
  Refused at first, with the reason: a helper returning a vector, and
  an `if` used as a vector value (assign in each arm instead).
  **Done 2026-10-18** (`src/escape/lang/rhai/vec.rs`): measured, a
  formula using every vector operation, a vector state field and a
  vector parameter runs as the same arithmetic written in numbers on
  400 pixels (1e-12), and its WGSL validates. `min`, `max` and `clamp`
  of numbers came with it (a temporary and an `if`).
- **B2, 3D variations.** `fn variation3(v, p[, ctx])`, a vector point,
  compiles to the 3D body (the 2D one from `fn variation`, or `z`
  passed through as now; only `variation3` makes the variation
  `only_3d`); `ctx.accum` a vector there, the result `v` or `#{ z: v,
  color }`. Gates: twins of 3D built-ins (`spherical3D`, `linear3D`,
  `hemisphere`, `zcone`) by density in 3D flames. **Done 2026-10-18**:
  `transformation3` lowers `variation3`, its point a vector local read
  from `#pixel` and the new `#pixelz`, written back to both; the
  emitter builds each body from the union of the hooks either needs.
  *Decision*: a 3D body declares `always_z`, as our 3D built-ins do --
  it computes its z, so `preserve_z = false` does not zero it. Measured:
  the twins equal the built-ins' formulas in the interpreter (1e-12); on
  the GPU, in a pitched 3D flame, by density, worst block |z| 4.0, none
  past 5.
- **B3, editable solids.** A solid entry: `fn start(pos, p)` returns
  the state (`z` a vector, `dr` the running derivative), `fn step(s,
  p)` one iteration, `fn escaped(s, p)` the bail, `fn distance(s, p)`
  the estimate after the last iteration, `INFO.radius` the ball the set
  lies in. It compiles to mode D's `ifs_walk3`, registered as custom
  formulas are and carried in the config; the template colours it by
  "Colouring solids" (the closest approach and the running derivative
  over the first `colour_depth` iterations; an `address` field if the
  state has one). Gates: Rhai twins of the KIFS Menger, the Mandelbox
  and the Mandelbulb, coverage pixel for pixel against the built-ins.
  **Done 2026-10-18** for the Mandelbox and the Mandelbulb (the KIFS
  Menger's twin is left: the built-in's eight folds and two rotations
  are a long source, and the other two exercise every part). A new
  `EntryKind::Solid`; `lang::wgsl::solid` writes the program over a
  `SolidState` and the walk around it; `compile_solid` registers an
  `IfsDef` the user's `custom-` name finds (`ifs::get_ifs`). Decisions to
  review: **the ball is `INFO.radius`, and a panel parameter** (Ball
  Radius) -- a user's set has no formula for its ball; **the estimate is
  `#index`** in `final:`, not a new symbol; **no address colouring unless
  the state has an `address` field**. Measured: the Rhai walks give the
  f64 twins' distances at 400 points each (the Mandelbox exactly, the
  Mandelbulb to 4e-13); on the GPU, coverage equal on all 25,600 pixels
  for both; the Rhai Mandelbox under the trap colouring pixel for pixel
  the built-in's (0 of 172,800).

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
2. [x] A2 Mandelbox (Solid). **Done 2026-10-18**: `mandelbox_solid`,
   Lowe's folds as Syntopia writes them, Mandelbox or Julia, the scale's
   magnitude held to 1.05 or more. The ball (`mandelbox_radius`): past
   `(2L|s| + |c|) / (|s| - 1)` in the largest coordinate a Julia's orbit
   grows; for the Mandelbox Lowe's `2L(|s| + 1) / (|s| - 1)`, held by
   `the_solids_lie_within_their_balls` at 2, -1.5 and a Julia (loose at
   a negative scale: the set is about a fifth of the ball). **Lowe's
   `|z| / |dr|` runs at 0.95 of itself**: whole, it was up to a percent
   over the distance at scale -1.5, which 0.95 clears at 3,000 points.
   Colour by the rules from the start: the level `log2` of the running
   scale, the trap the closest approach's sorted absolute coordinates
   **in units of the ball** (in world units the box, a dozen across,
   cycled the palette as fast again as the others and failed the
   settling gate at 0.26), the address base 12 (axes reflected, sphere
   case). The GPU's coverage equals the twin's on every pixel. Presets:
   Mandelbox (scale 2, trap), Mandelbox Scale 3 (address), Mandelbox
   Inverted (scale -2, level), Mandelbox Julia (scale 3, address).
3. [x] A3 Mandelbulb (Solid). **Done 2026-10-18**: `mandelbulb_solid`,
   White and Nylander's power as Syntopia writes it, Mandelbulb or
   Julia, with Hart's half of the estimate taken at radius 64. The ball
   (`mandelbulb_radius`): `2^(1/(n-1))`, and `1 + |c|` for a Julia.
   **Hart's estimate is no bound for the bulb** (it is not conformal):
   beside a thin part it was measured at 1.7 times the distance to
   points that stay in the ball for 2,000 iterations, while the query
   point leaves in two. A strict bound would cost 1.7 times the steps,
   so the estimate stays the standard one and a **Step Factor**
   (`step_factor`, default 1, down to 0.25) is the user's margin, as in
   Fragmentarium and Mandelbulber; the lower-bound gate holds the bulb
   to half its promise (a lost factor, not percent). The level is `log2`
   of the derivative **per power** (`/ (n - 1)`: the derivative carries
   `r^(n-1)` a step, and at power 8 its log moved seven times as fast as
   the quaternion's, 0.47 palette units a refinement); the trap `(z,
   |xy|)` of the closest approach; the address the hemispheres, then
   the polar angle. The GPU's coverage equals the twin's on every
   pixel. Presets: Mandelbulb (power 8, trap), Mandelbulb Power 4
   (address), Mandelbulb Julia (level).
4. [x] **A5 The flame solids' colourings** (the user, 2026-10-18).
   `ifs_flame_3d` coloured from where the inverse walk escapes, which
   the march's pixel decides, so its level and trap followed the camera
   as the KIFS's did; its address (the branches) was the start of the
   walk already. **Done 2026-10-18**: a colourings' walk at the hit
   point (`ifs_colour_walk3`, only where the march colours, never where
   it steps): `colour_depth` levels (default 3) from the sample's own
   point -- link 0, the final transform undone -- taking at each level
   the child the walk's ranking puts first, which is the walk's own
   path for a beam of one (the chain's prefix is the path its samples
   agree on); the trap the closest approach to the ball's centre, the
   level the band of that approach (the KIFS's rule: an affine walk's
   contraction says nothing of the surface). **Not counted from the
   main walk's start**: a sample near the target starts several links
   down even at zoom 0 (a pixel there is a small offset), and a window
   counted from there would part neighbouring pixels along the links'
   edges. Measured (`which_solid_colours_follow_the_camera`, the
   reference walk as the march's distance): the tail level moves 0.35
   then 0.34 palette units on the Sierpinski tetrahedron, 0.22 then
   0.23 on the sponge, never settling; at depth 3 the level moves 0.033
   then 0.009 and 0.027 then 0.005, the trap 0.070 then 0.025 and 0.059
   then 0.012, the address 0.025 then 0.007 and 0.001. Both join the
   settling gate. **Address Start** (`address_start`, default the first
   map, so the shipped presets look as they did): from the second, the
   address's first digit is shifted out and every copy of a first piece
   reads alike (the sponge's twenty cubes, the tetrahedron's four).
   **Limits**: for a beam wider than one the colour walk follows the
   greedy path, which need not be the winning branch (the address still
   is); and at a deep zoom, where a pixel is below f32's resolution of
   the set's own size, the colour walk sees the target's point from
   every pixel -- the level and the trap are flat there, and the
   address, which the link chain carries, is the colouring for deep
   views.
5. [x] **C1 Rhai transformations, 2D** (C1a-d; C1e's lens). **Done
   2026-10-18**, as C1's steps above, with these changes and additions:
   - **Category `plugin`, not a new "Custom" one** (C1d said Custom): a
     new `VariationCategory` would need the API repository told, and
     the provenance already says whose it is. A decision to review.
   - **The function's signature follows the parameters**: the flame
     shader passes the ids only to a variation with parameters
     (`variation_call`), so a parameterless one takes the point alone.
   - **Registered wherever a flame is read**: `missing_variations_in`
     (else a loaded flame's custom variation was fetched from the API),
     `FlameRenderer`'s build, `load_config`, `update_flame`,
     `render_with`, `unknown_names`.
   - **The editor**: in a flame mode, a transformation gets "Use as
     variation" (formulas and colourings are not offered there), and New
     starts a variation; in Escape mode, "Use as lens", which also sets
     the lens (amount 1 if it was off). Using one prunes the flame's
     list to the sources its transforms or the lens still name, so a
     renamed variation's old source goes. Live re-applies an edit that
     compiles. A UF transformation shows its errors but is not used
     (UF support is paused). The lens menu lists custom variations
     after the measured ones. Example: `variation-whorl.rhai`.
   - **`.flame` export** drops a custom variation's weight, parameters
     and `plugins=` entry, with a warning.
   - **Measured** (the gates): the five twins against their built-ins
     in a three-transform flame, 256 px, 8 replicates -- worst block
     |z| 3.6, none past 5, the whole view's density within 0.2 standard
     errors; neighbouring built-ins told apart. As lenses on a
     Mandelbrot: 0 of 16,384 pixels more than 8 levels apart, each lens
     moving 74-95% of the picture.
   - **Not done**: calling a custom variation inside a formula (C1e's
     second half); removing one from a flame (it goes when nothing uses
     it and another is used); random numbers and the other hooks (C2).
6. [x] **C1e, the second half**: a custom variation called inside an
   escape formula (the user, 2026-10-18). **Done 2026-10-18**:
   `vary("name", z)` or `vary("name", z, #{ param: value })`, in a
   formula, colouring or transformation, inlines the variation's body
   as a helper's (`rhai.rs` `Lowerer::vary`: its positions placed at the
   call, its point a local the body may reassign, `p.name` the map's
   value or the default). **Decisions to review**: the call is `vary`
   with the name as a string, so C3 could take built-ins with the same
   form; **a config carries what an entry calls** --
   `CustomSource::variations`, the sources as they were when the entry
   was used, flat, in `name_of`'s hash -- rather than relying on the
   flame's list or the registry, so an escape config renders anywhere
   and a changed variation recompiles its caller (`with_called_variations`
   fills it from the registered variations, then the library and its
   examples). A 39-literal codemod added the field
   (`scripts/codemod_custom_source_variations.py`). Measured: the
   varied Mandelbrot against the same step written out -- the
   interpreter equal to 1e-12 on 576 pixels; on the GPU 0% of pixels
   apart on the direct path, and at 2^24 on the bent set's edge (found
   by bisection) the same count on all 3,072 pixels, 84 distinct, the
   perturbed path running on equations derived through the variation.
   Not done: a formula's Live re-apply does not follow an edit to a
   variation it calls (Use the formula again); a hybrid's member that
   calls a variation does not carry it.
7. [x] **C2 hooks** (the user, 2026-10-18): random numbers, the
   accumulator, per-thread state, colour writes. **Done 2026-10-18**,
   as C2's steps above, with these changes: **state starts from `fn
   init(p)`**, not a constant `STATE` map -- MacMillan's starting state
   is its parameters -- and **`ctx.weight`** joined `ctx.accum`
   (JWildfire variations read their own amount; MacMillan does). The IR
   gained `Predef::Draw`, `Accum`, `Weight`, `VarColor` and
   `Place::Color`; `wgsl::variation_shape` decides the features and the
   function's arguments in the builder's order. Measured: the MacMillan
   twin equal to the built-in's arithmetic in the interpreter over 50
   calls (1e-12, point and colour); on the GPU, the `blur` and
   `macmillan` twins against the built-ins after a half-weight linear,
   by density and each colour's light, worst block |z| 4.6, none past
   5; a formula's draws on the GPU the interpreter's on all 3,072
   pixels (60 distinct counts). Not done: a variation's state inside a
   formula (refused), state arrays, `rand()` in `init`, writing RGB
   (`WritesRgb`; needs the vector type).
8. [x] **Review UF's transformations** once C is done (the user,
   2026-10-18; UF transformations themselves stay unsupported): what
   the public library's `.uxf` entries do, and which our variations
   already cover. **Done 2026-10-18**:
   [uf-transforms-review.md](uf-transforms-review.md). The library has
   no `.uxf` files; its transformations are 192 classes in 11 `.ulb`
   libraries: 126 point maps, 44 maps needing more (a class-valued
   parameter in 32 of them, else state, the screen or an image), 22
   utilities (clipping, merging, bases). Of the 170 maps, 18 have an
   equivalent here (12 of them plain affines), 40 something close, 112
   nothing. Worth writing as Rhai variations, by the review: circle-
   inversion folds for Kleinian and Apollonian packings (9 classes), a
   2D fold toolkit (box, ball, Kali, abs, triangle, diagonal,
   Sierpinski -- the 2D side of the KIFS's folds; we have no fold
   variation), the hyperbolic tiling fold (hypertile's inverse), an
   angular kaleidoscope fold, general inversion in curves, Newton /
   Halley / Laguerre steps, polylog and Blaschke maps, complex Γ, the
   attractor maps we lack (Hopalong, Martin, Chip, Quadruptwo, Sprott)
   and Pickover's original popcorn, fBm displacement, local lenses. What
   UF has that a flame variation does not: the solid flag, maps as
   parameters of maps (products, quotients, powers of two maps), state
   along a pixel's orbit, the screen, and a function menu -- a shared
   function-menu helper for Rhai variations would cover much of jlb.ulb.
   Which of these to write is the user's call.
9. [x] **B** (the user, 2026-10-18): the vector type (B1), 3D variations
   (B2), solids written in Rhai (B3). **Done 2026-10-18**, as B's steps
   above. Left: the KIFS Menger's Rhai twin; a solid's `prepare` and
   function parameters; vector-valued helpers and `if` values.
10. [x] **A4** (the user, 2026-10-18): the icosahedral and dodecahedral
   folds, the 2D KIFS formula. Done, as A4 above.
11. [x] **C4** (the user, 2026-10-18): transformations for escape-time,
   Plane, Orbit and Iteration. Done, as C4 above.
12. [ ] C3 as the user wants it; the UF review's missing maps
   (uf-transforms-review.md) as Rhai variations, the user's pick.
