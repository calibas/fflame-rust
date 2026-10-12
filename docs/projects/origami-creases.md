# Origami creases: relief and terrain from the folds

**Status 2026-10-12:** built on branch `escape-expansion` (the relief
field, the terrain source, the settings, tests, docs). The user has not
reviewed it yet. A Paper preset waits on the preset review. First
project of the escape-time expansion.

## Goal

The origami butterfly (`ORIGAMI`, `src/escape/formulas.rs`) folds the
plane along a sequence of lines, but its shading does not look like
folded paper. Add relief shading and a 3D terrain source in which the
paper rises or falls with the distance from each crease:
- a central crease down the middle, one side lit and the other in shade
  depending on the light;
- finer creases on top of creases, like a sheet of origami paper.

## The model

Prototyped in `scripts/escape_prototypes/origami_creases.py`; renders
are in `output/origami/`.

Per pixel, per fold *i* -- the formula's own loop, so the number of
creases is the iteration count (at most 64):
- *d<sub>i</sub>* is the point's signed distance, in the wad before fold
  *i*, from fold *i*'s line. It is positive on the side that stays put.
- *parity* counts the earlier folds that reflected the point, mod 2:
  whether this layer of the wad lies face up or face down.
- *σ<sub>i</sub>* is the fold's base direction, from the pattern (+1
  valley, −1 mountain), negated by Flip, and negated again on a face-down
  layer. That last flip is the physics: a crease is a valley on one face
  of the paper and a mountain on the other. Without it the heights stack
  into stepped ridges rather than folds
  (`creases-valley-unlayered.png`, before the model settled).
- The height is *h* = Σ decay<sup>*i*</sup> σ<sub>*i*</sub>
  |*d<sub>i</sub>*|: a straight rise or fall with the distance from each
  crease, the same slope on both sides.

The **gradient is exact**, carried through the folds rather than
differenced. Every fold is a reflection, so the point is an orthogonal
map of the pixel, *M p + t*, and the gradient of |*d<sub>i</sub>*| is
sign(*d<sub>i</sub>*) *M*<sup>T</sup>*n<sub>i</sub>*. The facets come
out flat and crisp at any zoom, where differencing the height would lose
its digits in f32 at depth. The prototype checks the carried gradient
against finite differences: their median difference is 2e-4.

**The height steps where a layer turns over.** The face-down flip that
makes the folds read as paper also makes *h* discontinuous: where a later
crease crosses an earlier fold's line, it is a valley on one side and a
mountain on the other. The 2D relief lights only the slope and never
shows this. The terrain shows it as a step at a flap's edge, which reads
as layered paper. A continuous height would need the unlayered model,
which the prototype showed stacking into ridges.

### Settings (decided 2026-10-12)

- **Pattern:** Valley (every fold toward the viewer), Alternate, or
  Random (from the line seed).
- **Decay:** how much finer each later crease is; 0.6 reads as paper,
  0.85 crowds.
- **Flip:** swaps mountain and valley.

There is no separate crease count: each fold is one crease, so the
iteration count is the crease count.

**Depth was dropped** while building it. The relief's own Height scales
the slope, and a terrain fits its heights to the range it measures, so
a depth would have duplicated one control in 2D and done nothing in 3D.

**The 2D relief has no reach cap**: the prototype's capped version
looked worse. **The terrain has one** (`CREASE_TERRAIN_REACH`, one
paper size, i.e. the line spread). Unbounded, *h* rises linearly out to
the horizon. Over a span 32 wide, the 2D view's area covered 20% of the
height range, and the terrain flattened every crease near the camera
into a groove. With the cap at the spread, that area covers 87% of the
range. Beyond the cap the paper levels into plateaus.

## As built

- **`FormulaFeature::Creases`** (`src/escape/mod.rs`): the formula's
  WGSL also defines `fn formula_creases(p0: vec2<f32>) -> vec3<f32>`,
  the height and its gradient in the plane's units. Origami's mirrors
  `origami_fold` (same side test, same reflection); the Random pattern
  hashes the line seed past the 256 indices the endpoints take.
  `origami_creases_at` is its CPU twin, used as the test reference.
- **Uniform:** a new `crease` vec4 at the end of `EscapeParams`
  (offset 1312, size 1328), in all nine WGSL copies. It holds the decay,
  the pattern, the flip as a sign, and the reach in paper sizes (0 in
  2D). `fdata` was full. `pmap`'s spare-looking `yzw` are in fact the
  layer weight, the relief step and the light azimuth; the stale
  comments now say so.
- **Relief:** `ShadingField::Creases` (GPU code 6, wire `"creases"`,
  `stored_relief` 1). The direct template's `//__CREASES__` splice, for
  crease formulas only, runs at every pixel when `shade_flags == 6`. It
  writes *h* to the relief channel and a tenth of −∇*h* (turned into the
  screen frame, divided by the supersample factor) to the stored-slope
  channels. At the default Height of 10, the first crease tilts 45°.
- **Terrain:** `TerrainSource::Creases`, `shade_flags` 6, and the ingest
  reads it as a relief (mode 3, linear).
- **Recolour:** a crease render always iterates
  (`EscapeConfig::wants_creases`), because the recolour pass has no
  formula code. The section cache keys on `shading.creases` through
  `picture_key`.
- **Config:** `CreaseSettings` in `EscapeShading.creases`, skipped
  while default. Its ConfigPaths are `Escape.Shading.CreasePattern`,
  `CreaseDecay` (animatable) and `CreaseFlip`.
- **UI:** "Paper creases" in the relief field and terrain source
  dropdowns, offered for crease formulas only. A warning appears when
  one is picked on another formula. There are Pattern, Flip and Decay
  controls; the terrain's Decay commits on release, because each change
  re-renders the sections.
- **Script:** `escape.shading_field("creases")` and
  `escape.shading_creases(pattern, decay, flip)`.

## Tests

- `the_crease_gradient_is_the_heights` (CPU): where a second difference
  is zero, no crease lies between the samples, and the central
  difference equals the carried gradient. At least 80% of samples must
  qualify, there must be more than 50 facets, and Flip must mirror the
  paper exactly.
- `the_crease_field_matches_its_reference` (GPU, ignored without one):
  the stored height and slope match `origami_creases_at` at 99% of
  pixels or more, for two 2D settings and the terrain's reach. Measured:
  19,200 of 19,200 pixels, 19,193 of 19,200, and 19,200 of 19,200.
  One renderer draws all three, so each case is an edit of the last.
  Dropping the face-down flip from the shader fails it (0 of 19,200),
  and so does dropping the recolour bypass (0 of 19,200 on the second
  case).
- Shader validation over every formula: the splice appears exactly
  where the formula has creases. Config round trips and wire strings
  are covered.
- Visual configs: `escape/origami-creases.fflame` (2D, Alternate, decay
  0.7) and `escape/origami-creases-terrain.fflame`.

## Open

- A Paper preset (the user is reviewing presets).
- The terrain's layer-edge steps are a property of the layered model;
  revisit if they read as tears rather than flaps.
