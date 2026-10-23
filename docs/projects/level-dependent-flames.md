# Level-dependent flames (Moran sets)

Tried 2026-10-23 as a Rhai variation; the variation shipped as an example
the same day (branch `level-dependent-flames`). Levels in the flame engine
(2) and in mode D (3) are planned below, not started: the user is testing
the example first. The user's words: "I do like the level-dependent
flames idea though. I'm wondering if there's a way we can test it quick";
after the test, "keep track of the Moran variation set work from before,
I do want to add it"; then, "Let's implement #1, and plan out 2 & 3. I
want to test things first."

## The idea

A flame's chaos game draws the attractor of one set of maps. A
level-dependent IFS -- a Moran construction -- gives every level of the
construction its own set: a point is

    x = f(1, i1) o f(2, i2) o ... o f(K, iK) (x0)

one map per level, the coarsest level (1) outermost, each level drawing
its map from the set its schedule names. No single set of maps has that
as its attractor, so the plain chaos game cannot draw it.

What the schedule decides:
- **Periodic** (coarse, fine, coarse, fine, ...): only an ordinary IFS
  of composed maps. Nothing new; a flame can already draw it.
- **Split** (the coarse set above level s, the fine set below): the
  large-scale shape of one fractal made of small copies of another --
  a triangle of ferns, a carpet of dragons. New.
- **Sturmian** (the golden-ratio sequence) or any other schedule that
  never repeats: a blend at every scale. New.

What prompted it: J. Chen and L. Xi, *A new fractal dimension for
discrete structures: Frostman meets Moran*, Chaos, Solitons & Fractals
211 (2026) -- a dimension for discrete Moran sets, no rendering. Moran
*carpets* as an escape-time digit test were considered the same day and
dropped: they look like the Sierpinski carpet and add nothing.

## 1. The example (built)

`assets/formulas/variation-moran.rhai`, in the Formula Editor's examples
(`storage::formula_store::EXAMPLES`): a Rhai variation that ignores its
input and builds each point itself, as the escape-paths variation does.
Five sets, each fitted to the unit square: Sierpinski triangle,
Sierpinski carpet, Barnsley's fern (Barnsley's weights), Heighway's
dragon, Levy's C curve. Parameters: `coarse`, `fine`, `schedule` (Coarse
above the split, Alternate, Sturmian), `split` (1-16), `depth` (2-40,
default 24). The colour is the coarsest level's map.

Six presets (`Moran: ...`), each one identity transform holding only the
variation, Direct Color 1, zoom 3 around (0.5, 0.5): Triangle of Ferns
(split 2: 9 ferns), Triangle of Ferns, Four Levels (81), Carpet of
Dragons, Dragon of Carpets, Carpet and Levy C, Sturmian, Dragon and Levy
C, Sturmian. `escape::moran` writes them from the file
(`write_presets`, then spliced into `assets/presets.fflame`) and keeps
them to it (`the_presets_carry_the_example`); its other tests run the
variation on the CPU: `every_set_is_drawn_in_the_unit_square`, and
`the_levels_compose_coarsest_outermost` (a triangle-and-carpet point
peels back through its schedule's sets level by level, its colour naming
the outermost map, for all three schedules; in the other order most
points do not peel).

What the first renders showed (`output/moran/`, gitignored):
- The triangle of ferns is the clearest case. The carpet of dragons
  reads as one; the dragon of carpets is blocky where carpets overlap.
- Alternate and Sturmian schedules give textures neither set has alone;
  for triangle and fern the two are hard to tell apart.
- **The fern as the coarse set is messy** (fern of triangles, fern of
  dragons): its main map shrinks only to 0.85, so the first levels'
  copies pile onto one another. None of the presets uses it.
- The word starts from a fixed point (0.5, 0.5): a random start left
  the square's images visible in the slow fern. The slowest set sets the
  depth; 24 was enough for every render.

## What the engine can already do (tested 2026-10-23)

Two configs built by hand from the example's maps
(`output/moran/unrolled/`), rendered against the presets:

- **Unrolled.** One copy of the set's transforms per level, K levels;
  xaos lets each level's copies go only to the next level's (level 1
  back to level K); only level 1's copies plot (opacity 1, the rest 0),
  and level 1 sets the colour (colour speed -1). The orbit then runs
  through one word of K maps per plotted point. **The same pictures as
  the variation** for the triangle of ferns (94 transforms at depth 24),
  81 ferns (92), carpet of dragons (54) and the dragon-and-Levy
  Sturmian (48). Dimmer: one iteration in K plots, and the Log tone map
  divides by every iteration.
- **The split shortcut.** The fine set as a subflame, read by one
  opacity-0 transform R holding `subflame_wf` -- the subflame's own orbit
  persists in the thread's state and stays on the fine attractor -- then
  s copies of the coarse set: xaos R -> level s -> ... -> level 1
  (plotted) -> R. **The same pictures** with 7, 13 and 9 transforms, one
  iteration in s + 1 plotted.

So levels need no new GPU code: they can be expanded into an ordinary
flame, xaos and opacity carrying the schedule. Not measured yet: the
cost per plotted point. Each iteration's xaos pick reads every
transform's weight twice (`select_transform_xaos`), so an unrolled flame
of n transforms pays about K x 2n weight reads per plotted point.

## 2. Levels in the flame engine (planned)

The user's own transforms per level, expanded into the flame the GPU
sees.

**The model.** `Flame::levels: Option<Levels>` -- a flame-level field,
skip-if-default, so older builds ignore it (the `Flame` visitor drops
unknown keys; a new key inside a `Transform` would fail an older build's
load) and the orbit key follows it through `flame_hash` with no change.
The sets are the flame's own transforms and its subflames: each already
has the Transforms panel through the editing target (`EditingTarget::
Subflame`), serialization, and a place in the GPU buffers, and the split
shortcut needs the fine set as a subflame anyway. `Levels` holds the
schedule over those sets and the depth.

**The expansion.** One function, `Flame::expanded()` (name to choose),
returns the ordinary flame: the split shortcut when the schedule ends in
one set forever, the full unroll otherwise. It is applied where the
renderer takes a flame -- `FlameRenderer::load_config` and
`update_flame` (`renderer/compute_kernel.rs`), which every path goes
through (`render_with`: app, CLI, thumbnails, video, gallery) -- and the
editor keeps showing the sets, never the copies.

**Limits it inherits.** `MAX_TRANSFORMS = 128` covers every copy: 24
levels of a 4-map set is 96, an 8-map carpet allows 16. The variation
parameter slots (1600 floats) and per-thread variation state are per
copy, so a parametric set multiplies them; `gpu_capacity_error` must see
the expanded flame. The flame's own xaos is refused while levels are on
(the levels own the xaos). Focused Rendering refuses xaos, so level
flames have no flame deep zoom.

**Steps.**
1. `Levels` and the expansion, no UI: tests that the expansion's xaos
   admits exactly the next level, that it round-trips, that a level
   flame renders the variation's pictures (by density, `compare_blocks`;
   the two draw from different streams, so with a tolerance), and the
   capacity refusals.
2. Brightness: only one iteration in K (or s + 1) plots, so the tone map
   should count plotted iterations -- scale `sample_density` by the
   expansion's plotted fraction. (Apophysis does not compensate hidden
   transforms; this is our own feature, so it may.)
3. The UI: a Levels section in the Transforms panel -- schedule, sets,
   depth -- with the sets edited through the editing target as
   subflames are now.
4. The six presets in the native form; `.flame` export writes the
   expansion (xaos and opacity are Apophysis's, `subflame_wf`
   JWildfire's), so other programs can render a level flame.
5. Measure the cost per plotted point; only if it is slow, a `LEVELS`
   shader flag picking among the next level's copies alone (each
   level's copies are contiguous) in place of the full xaos pick.

**Decisions before step 1:**
- How a schedule is written: the three of the example (split, alternate,
  Sturmian) over two sets, or a pattern string over any number of sets
  ("AAB" repeated, "A x 3 then B").
- The colour: the coarsest level's transform's (as the example), or the
  running colour along the whole word (each copy's own colour and speed,
  as an ordinary flame does -- free with the expansion).
- Whether the sets are subflames (the plan above) or a set tag on each
  transform.

## 3. Levels in mode D (planned)

Mode D draws a flame as a distance field by walking inverse maps
([ESCAPE.md](../main/ESCAPE.md); `escape/ifs.rs`, the CPU reference
`scene/ifs_estimate.rs`). It already honours xaos, so an expanded flame
reaches it -- but the test drew the wrong set: **the unrolled carpet of
dragons in mode D shows the union of every level's piece**, extra
dragons over the carpet's middle, because mode D ignores opacity and the
chaos game plots only level 1.

**Steps.**
1. **Mode D honours opacity 0**, for every flame: a plotted point's
   outermost map is a transform that plots, so the walk's first inverse
   (the CPU's handover prefix, `seed_beam`, and the shader's level 0)
   admits only transforms with opacity above 0; xaos then admits the
   rest. Today any flame with a hidden transform draws more in mode D
   than the chaos game plots. Test: a two-map flame with one map hidden,
   mode D against the chaos game's coverage.
2. **Mode D's own expansion.** The subflame shortcut has no inverse, so
   mode D takes a different one: the full unroll for a schedule that
   never repeats, and for a split, s coarse copies and then one copy of
   the fine set whose xaos returns only to itself -- the fine levels
   below the split, exactly, in s + |fine| maps. (That flame would trap
   the chaos game's orbit in the fine set, so it is mode D's alone.)
3. **The walk's cost and colour.** The beam tries every map at every
   level and lets xaos reject most; a per-level range would try only the
   level's copies. The address colouring counts in base n (every copy),
   so it should read each copy as its set's map.
4. **The fern.** Its stem map is singular (rank 1), which mode D refuses
   -- Barnsley's fern is not among mode D's classical presets. A level
   flame with the fern needs a stem with some width in mode D, or mode D
   learning to skip a singular map.
5. The 3D solid's walk (`ifs_walk3`) after the plane's.

Tests: the split expansion against the full unroll (the same distance
field), and the level flame in mode D against the chaos game's coverage
of the same flame.

## Related: the multifractal tests (Cawley-Mauldin)

The same day: R. Cawley and R. D. Mauldin, *Multifractal decompositions
of Moran fractals*, Advances in Mathematics 92 (1992); their Moran sets
keep the same ratios at every level, so they are not level-dependent.
Tested on three all-affine flames (`output/multifractal/`); neither idea
is being built:

- **Natural weights** p_i = t_i^d (t_i = sqrt|det|, d solving
  sum t_i^d = 1) even out the density -- clearly on an unequal-ratio
  spiral, barely on Barnsley's fern, whose weights are near natural. Only
  meaningful for linear-only flames, which none of the bundled presets
  with 2-6 transforms is.
- **Local-dimension colour** (alpha_i = log p_i / log t_i as each
  transform's colour) works with no code; the user dropped it
  2026-10-23: "it doesn't really do anything new".
