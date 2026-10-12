# Origami creases: relief and terrain from the folds

**Status 2026-10-12:** built on branch `escape-expansion` (the relief
field, the terrain source, the settings, tests, docs). After the user's
first review, the model became the continuous one: the first version's
terrain had cliffs between sections. A Paper preset waits on the preset
review. First project of the escape-time expansion.

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
- *σ<sub>i</sub>* is the fold's direction, from the pattern (+1 valley,
  −1 mountain), negated by Flip. It is the same on every layer.
- The height is *h* = Σ decay<sup>*i*</sup> σ<sub>*i*</sub>
  |*d<sub>i</sub>*|: a straight rise or fall with the distance from each
  crease, the same slope on both sides.

Each |*d<sub>i</sub>*| is a continuous function of the pixel, because
the fold map is. So *h* is continuous: one sheet, with flat facets.

The **gradient is exact**, carried through the folds rather than
differenced. Every fold is a reflection, so the point is an orthogonal
map of the pixel, *M p + t*, and the gradient of |*d<sub>i</sub>*| is
sign(*d<sub>i</sub>*) *M*<sup>T</sup>*n<sub>i</sub>*. The facets come
out flat and crisp at any zoom, where differencing the height would lose
its digits in f32 at depth. The prototype checks the carried gradient
against finite differences: their median difference is 2e-4.

**Why not reverse a crease on a face-down layer.** The first version
did. A layer that an odd number of earlier folds had turned face down
saw its creases reversed, because a flat-folded crease is a valley on
one face of the paper and a mountain on the other. In 2D it read well,
since the relief lights only slopes. In the terrain it tore the sheet.
Where a later crease crosses an earlier fold's line, its sign flips
across that line, so *h* jumps by 2·decay<sup>*i*</sup>|*d<sub>i</sub>*|.
The jump grows away from the vertex. The prototype measured steps
600-1200 times what the slope allows between neighbouring pixels; the
user saw them as cliffs between sections.

No continuous faceted height can carry the flat-folded assignment.
Take a vertex where crease 1 crosses crease 0, with rays at 0, φ, π and
−φ. A piecewise-linear height there needs its slope jumps κ<sub>r</sub>
across the rays to sum to zero around the vertex:
Σ κ<sub>r</sub> n<sub>r</sub> = 0. That forces κ<sub>φ</sub> =
κ<sub>−φ</sub>, so crease 1's two halves keep the same direction.
Crease 0's halves bend by different amounts instead
(κ<sub>0</sub> − κ<sub>π</sub> = −2 cos φ κ<sub>φ</sub>). This is
exactly what the continuous model produces: |*d*<sub>1</sub>| kinks
along crease 0 through the reflection. It is how a gently folded sheet
bends. The alternating valley/mountain pattern belongs to paper folded
flat, which a height field cannot show.

The prototype compares the two (`cmp-layered-*` and `cmp-continuous-*`
in `output/origami/`). The continuous one never steps more than its
slope allows, and in 2D it reads as faceted paper too.

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
  paper exactly. The sheet must be continuous: no step between nearby
  samples may exceed what the steepest slope allows. Putting the
  face-down flip back fails this check.
- `the_crease_field_matches_its_reference` (GPU, ignored without one):
  the stored height and slope match `origami_creases_at` at 99% of
  pixels or more, for two 2D settings and the terrain's reach. Measured
  on the continuous model: 19,200 of 19,200 pixels, 19,194 of 19,200,
  and 19,200 of 19,200. One renderer draws all three, so each case is an
  edit of the last. Dropping the recolour bypass fails it (0 of 19,200
  on the second case).
- Shader validation over every formula: the splice appears exactly
  where the formula has creases. Config round trips and wire strings
  are covered.
- Visual configs: `escape/origami-creases.fflame` (2D, Alternate, decay
  0.7) and `escape/origami-creases-terrain.fflame`.

## Open

- A Paper preset (the user is reviewing presets).
- The two terrain bugs found while reviewing this (Invert's march, and
  the low camera's sections) are fixed on the same branch; see
  [TERRAIN.md](../main/TERRAIN.md).
