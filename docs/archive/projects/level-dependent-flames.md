# Level-dependent flames (Moran sets)

> **Archived 2026-10-23** -- experiment complete. The Rhai variation shipped as a Formula Editor example with six presets; levels of the user's own transforms need no feature, since xaos and a subflame already draw them (the recipe is below and in [TRANSFORMS.md](../../main/TRANSFORMS.md#xaos)); mode D's opacity gap, found here, is in [escape-sim-followups.md](../../projects/escape-sim-followups.md). This is the record of what was tried and found.

The user's words: "I do like the level-dependent flames idea though. I'm
wondering if there's a way we can test it quick"; after the test, "keep
track of the Moran variation set work from before, I do want to add it";
then "Let's implement #1, and plan out 2 & 3. I want to test things
first"; and, of plan 2, "Isn't #2 something we can already do with xaos
and/or subflames?" -- it is -- "Let's document what we've found and we'll
call this experiment complete."

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
  of composed maps. Nothing new.
- **Split** (the coarse set above level s, the fine set below): the
  large-scale shape of one fractal made of small copies of another --
  a triangle of ferns, a carpet of dragons.
- **Sturmian** (the golden-ratio sequence) or any other schedule that
  never repeats: a blend at every scale.

What prompted it: J. Chen and L. Xi, *A new fractal dimension for
discrete structures: Frostman meets Moran*, Chaos, Solitons & Fractals
211 (2026) -- a dimension for discrete Moran sets, no rendering. Moran
*carpets* as an escape-time digit test were considered the same day and
dropped: they look like the Sierpinski carpet and add nothing.

## What shipped: the example

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

What the renders showed:
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

## What the engine already draws: levels from xaos and a subflame

Built by hand from the example's maps and rendered against the presets
(2026-10-23; the configs were in `output/moran/unrolled/`, gitignored).
Both drew **the same pictures as the variation**, the colours included.

**The split shortcut** (coarse set C, fine set F, split s):
- F as subflame 0, its transforms visible.
- One transform R: an identity affine holding `subflame_wf` at 1
  (`subflame_id` 0), opacity 0. The subflame's walk persists in the
  thread's state and stays on F's attractor, so each call hands back a
  point of it -- every fine level below the split at once.
- s copies of C's transforms, one copy per level; level 1's copies have
  opacity 1, the rest 0.
- Xaos, every other entry 0: R -> level s's copies; level k -> level
  k - 1; level 1 -> R.
- For the coarsest map's colour (as the example): level 1's copies at
  colour speed -1 (jump to their colour), the rest at 1 (keep). Left as
  the flame's usual colours, it is the running colour along the word.

Triangle of ferns: 7 transforms (split 2), 81 ferns: 13, carpet of
dragons: 9. One iteration in s + 1 plots.

**The unrolled form**, for any schedule (depth K): one copy of each
level's set, K levels; xaos level k + 1 -> level k and level 1 -> level
K; only level 1's copies plot. The orbit runs one word of K maps per
plotted point; past the depth the schedule repeats, below a pixel.
Triangle of ferns: 94 transforms at depth 24; 81 ferns: 92; carpet of
dragons: 54; dragon and Levy, Sturmian: 48. The flame's 128-transform
cap bounds it (24 levels of a 4-map set; 16 of the 8-map carpet), and a
Rhai generator script could build one (scripts set xaos, opacity and
colour speed; they cannot make subflames, so not the shortcut).

Both are dimmer than the variation: only the plotted iterations land,
and the Log tone map divides by every iteration -- one in s + 1, or one
in K. Raise the exposure.

## What was not pursued, and why

**Levels in the flame model** (plan 2): `Flame::levels` naming the
schedule over the flame's transforms and its subflames, expanded into
the flame above where the renderer takes a flame. Since the engine
already draws the result, the model would add only editing a set once
for all its copies (little, with the shortcut's few copies), counting
only plotted iterations in the tone map, and a Levels UI. A one-shot
"Make level flame" action (build the shortcut from the flame and a
subflame, one undo step) was the smaller alternative. Neither was built.

**Levels in mode D** (plan 3). Mode D already honours xaos, so an
unrolled flame reaches it -- but **the unrolled carpet of dragons in
mode D drew the union of every level's piece**, extra dragons over the
carpet's middle: mode D ignores opacity, and the chaos game plots only
level 1. Had it gone ahead:
1. Mode D honours opacity 0 for every flame (the outermost map of a
   plotted point is one that plots) -- recorded as an open gap in
   [escape-sim-followups.md](../../projects/escape-sim-followups.md).
2. Its own split expansion: the subflame shortcut has no inverse, but s
   coarse copies followed by one copy of the fine set whose xaos returns
   only to itself is the fine levels exactly, in s + |F| maps (that
   flame traps the chaos game's orbit, so it would be mode D's alone).
3. The beam tries every map at every level and lets xaos reject most; a
   per-level range would try only the level's copies, and the address
   colouring counts in base n, every copy a digit.
4. The fern's stem map is singular (rank 1), which mode D refuses --
   Barnsley's fern is not among mode D's classical presets.

**The multifractal ideas** (R. Cawley and R. D. Mauldin, *Multifractal
decompositions of Moran fractals*, Advances in Mathematics 92 (1992);
their Moran sets keep the same ratios at every level). Tested the same
day on three all-affine flames (`output/multifractal/`):
- **Natural weights** p_i = t_i^d (t_i = sqrt|det|, d solving
  sum t_i^d = 1) even out the density -- clearly on an unequal-ratio
  spiral, barely on Barnsley's fern, whose weights are near natural.
  Only meaningful for linear-only flames, which none of the bundled
  presets with 2-6 transforms is.
- **Local-dimension colour** (alpha_i = log p_i / log t_i as each
  transform's colour) works with no code; the user dropped it: "it
  doesn't really do anything new".
