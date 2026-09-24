# Editing the picture by its words (plan, 2026-09-24)

Cylinder targeting draws every sample through a word of its plan: a
sequence of the flame's maps (transform and arm, in the order the chaos
game applies them). A word is exactly one piece of the picture, so
removing a word removes that piece and nothing else. This plan uses
that to replace the old Path Editor. It adds a **trim slider** that drops
the minor branches that flicker in during an animation, and a **Words
panel** where a user removes the branches they don't want.

## 1. What prompted it

In an animation that rotates transform 1 of the true Grand Julian
(`output/flame-zoom/grand-julian-zoom{1,2}.fflame`, 1.8 degrees apart,
zoom 88), a faint overlapping section flickers in and out. It is real (an
untargeted render has it too, barely visible). `what_changes_between_frames`
compared the two frames' plans:

- **Shared words:** 3,734. Every one has transform 3 (`t3a0`) as its
  last-applied map.
- **The flicker:** 274 words only in the first frame, 2.3% of its
  probability, whose last map is transform 1's first arm (`t1a0`). They
  branch off the shared words at the very last map. As transform 1
  rotates, that piece of its image swings across the view.
- **Ordinary refinement:** the second frame's own words branch 3-10 maps
  deep, as frame-to-frame change does.
- **A second flicker:** one word of the blurred glow flips from
  transform 2's arm 1 to arm 0 between the frames.

## 2. Decisions (taken)

1. **A branch is measured by its share of the VIEW**: the sum over its
   words of probability x efficiency (the share of its samples that
   land). That is how much of the picture it is, not how likely its
   history is.
2. **A removal removes that piece of the picture**: the words whose
   last-applied maps match. The pattern elsewhere in a history is left
   alone, so the fractal itself is unchanged (that would be xaos).
3. **The Words panel replaces the Path Editor**, which nothing uses. The
   PathMap colour mode is a separate feature and stays.

## 3. The word tree

A plan's words, grouped by their last-applied maps: depth 1 by the last
map, depth 2 by the last two, and so on. It is the tree the inverse walk
already grows from the view inward. Each node has a share of the view,
the sum of its words'. `Cylinder` gains the word's efficiency (the walk
measures it and currently discards it). Words from the other planners
count as efficiency 1.

## 4. Trim (phase 1, done)

- **The rule.** A branch is dropped when its share of the view is less
  than `trim` times its largest sibling's. Measuring against the largest
  sibling rather than the parent means an even split is never trimmed
  (a power-15 julian's arms, 1/15 each), while a sliver beside a dominant
  branch is. `scene::word_tree`.
- **Only near the view.** Trim reaches `cylinder_trim_levels` levels of
  the tree, counted from the view (default 2). Measured on the two
  frames, removing what trim 0.05 removes (the share of the view taken):

  | levels | zoom1 | zoom2 |
  |---|---|---|
  | 1 or 2 | 0.53%, exactly the `t1a0` family | nothing |
  | 3 | 0.70% at trim 0.01, 17.6% at 0.03 | 17.0% at 0.03 |
  | all | 31% at 0.03, 66% at 0.1 | 32% at 0.03 |

  At one or two levels the result is the same at every trim from 0.01 to
  0.3. From three levels on, trim cuts real structure: deep in the tree,
  uneven splits are ordinary geometry, not slivers.
- **Unmeasured words.** A word whose replays landed nothing has an
  efficiency of 0, and that means not measured, not measured as nothing.
  The walk forces such words rather than dropping them. The true Grand
  Julian's renewal words (the blurred glow, a quarter of its probability)
  all measure 0 at depth. Ranked as zero-share slivers, any trim took
  them, and every pixel of zoom2 changed. So a branch is judged by the
  words in it that were measured, and its unmeasured words go with it. A
  branch in which nothing was measured is not judged, and is kept. 94 of
  the flicker's 274 words are unmeasured, and with this rule they go too.
- **Where.** After the plan, when it is packed for the GPU. The renderer
  keeps the full plan (`cylinders_full`), so moving the slider retrims
  without replanning. The kept words' mass is the tone map's scale, so
  what remains keeps its brightness.
- **Config.** `FractalConfig::cylinder_trim` (0 = off; skipped in files
  when 0) and `cylinder_trim_levels` (skipped when 2). Both are
  `ConfigPath`s and undoable. They sit in the View panel under the
  targeting checkbox: a logarithmic slider, and a depth field shown when
  trim is on.
- **Stateless.** Each frame is trimmed on its own, so playback and export
  agree. A branch near the threshold could still pop; if it does,
  hysteresis across frames is phase 4.
- **Result** (`the_zoom_examples_side_by_side`, GPU, deterministic RNG,
  trim 0.05 at two levels):
  - **zoom2** is bit-identical to its untrimmed render.
  - **zoom1** loses the grey overlapping cells in its dark regions, and
    looks like zoom2. Its other pixels differ only by noise, because the
    tone map's scale moved with the 2.3% of mass removed.
  - **Frame comparison:** `what_changes_between_frames` prints the
    levels-by-trim table and what trim removes.

## 5. Removals (phase 2)

- **Config.** `FractalConfig::word_removals`: a list of patterns, each a
  run of symbols (transform and arm) matched against a word's
  last-applied maps. Undoable.
- **In the walk,** so a removal holds at every zoom:
  - a child whose word ends with a removed pattern is dropped;
  - a child whose word is a proper suffix of a removed pattern (its piece
    contains the removed one) is not kept as it stands but carried
    deeper, until its words either match or diverge.
  - A child that must be refined but has no points to carry is kept
    whole: the removed piece then stays in that sliver, and this is
    counted in the trace.
- **The other planners** (affine, Möbius) get the same filter after the
  plan, dropping the matching words; one too short to split stays.
- **Gate.** A removed branch is gone at the view it was removed at and
  at deeper and shallower ones, with the rest of the picture unchanged
  (targeted renders against targeted renders, removed pixels against
  kept).

## 6. The Words panel (phase 3)

- **The tree of the current plan:** each branch labelled by its maps
  (e.g. `T1·a0` for transform 1's first arm), with its share of the view
  and its word count. Expandable, the largest first.
- **Per branch:** remove (adds the branch's pattern) and solo (draw only
  this branch while the button is held, to see what it is).
- **Beside it:** the removed patterns with restore buttons, and the trim
  slider.
- **The old Path Editor goes:** the panel, `GpuPathFilter`,
  `path_filter.wgsl`, `check_path_filters`, `set_path_filters`, and the
  filter buffer. The PathMap colour mode stays. Canonical shader dumps
  that held the filter code are regenerated deliberately.

## 7. Later (phase 4, only if needed)

- Hysteresis for trim across animation frames (the "lowest common
  denominator"): a branch kept in the previous frame keeps a lower bar.
- Hover highlight of a branch in the viewport.

## 8. Order

Phase 1, measured on the two frames, then 2, then 3.
