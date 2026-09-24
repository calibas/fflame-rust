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

## 5. Removals (phase 2, done)

- **Config.** `FractalConfig::word_removals`: a list of patterns, each
  written as text such as `"t1a1 t1a0"`. Each map is a transform and an
  arm, listed in the order the chaos game applies them, so the map
  nearest the view is last (`word_tree::parse_pattern`). A pattern
  matches a word that ends with it. It is a `ConfigPath` (a `StringList`),
  so removals are undoable.
- **In the walk** (`PlanOptions::removals`), so a removal holds at every
  zoom:
  - A child whose word ends with a removed pattern is never made, so it
    is never replayed either. Its points count as accounted for, so the
    node is not forced whole for want of it.
  - A child whose word is a proper suffix of a removed pattern (its piece
    contains the removed one) is never cut. It is carried deeper until
    its words either match or diverge. The beam carries such nodes
    rather than forcing them.
  - Such a word kept whole anyway (at the depth cap, a renewal, no points
    to refine it from, or out of time) keeps the removed piece. This is
    counted as `Trace::unrefined`. None were in the gate.
- **The other planners** (affine, Möbius) get the same filter after the
  plan (`word_tree::remove`, in `Cylinders::plan_opts` and
  `plan_sliced`). A word too short to split stays.
- **In the renderer.** The plan on screen is filtered at once when the
  removals change, and a replan (the removals are in the plan key)
  brings the walk's refinement. A standby made with other removals is
  dropped. While the picture is edited by its words (trim or removals),
  a plan is drawn even where it would not pay (`speedup <= 1`), because
  nothing else can draw the edit.
- **Gate** (`a_removal_holds_at_every_zoom`, CPU, the first animation
  frame). It removes `t1a0` (the flicker) plus a pattern one map longer
  than the cut word holding the most of the view (7%), which the walk
  must refine to take out. At zooms from 5.5 to 1408:

  | zoom | words (plain) | view kept | plain words missing / new |
  |---|---|---|---|
  | 5.5 | 1,635 (2,883) | 92.8% | 5 / 148 |
  | 22 | 5,526 (3,600) | 99.4% | 163 / 2,247 |
  | 88 | 5,060 (5,319) | 97.0% | 12 / 27 |
  | 352 | 5,077 (5,077) | 100% | 0 / 0 |
  | 1408 | 94 (94) | 100% | 0 / 0 |

  - No word ends with a removed pattern at any zoom, and nothing was
    kept whole.
  - At 88, the cut word is refined into 27 children, and the removed one
    is never made.
  - At 22 the plain plan cuts shallower, so the walk refines through
    more levels to reach the piece, and the plan grows. This is the cost
    of removing a deep piece at a shallow view.
- **Against trim** (`the_zoom_examples_side_by_side`, GPU). `t1a0` alone
  gives 5,042 words at zoom 88, against trim 0.05's 5,045. The floor and
  the beam move slightly when a branch is never made.
  - Rendered, the removal and the trim differ only by noise (8x8
    block-averaged luminance: mean 0.9, max 5.2). Both take the same 36
    blocks out of the untrimmed picture.
  - Zoom2, which has no `t1a0`, is bit-identical with the removal.

## 6. The Words panel (phase 3, done)

- **The tree of the plan on screen** (`word_tree::Tree`, built lazily
  by `FlameRenderer::word_tree` once per applied plan). Each branch is
  labelled by the map it adds, numbered as the Transforms panel numbers
  them: `T2·1` is transform 2's first arm, and the arm is shown only for
  a transform that has arms. The tooltip gives the whole path in the
  order the maps are applied (`T2·1 → T4`).
  - **What a row shows:** the branch's share of the view and its word
    count. A branch where nothing was measured shows its share of the
    samples instead. Trimmed branches are struck through.
  - **Expanding:** a branch opens to the next map inward. The tree is
    built eight levels deep (four above 50,000 words, since it is rebuilt
    with every move of the trim slider). At most 40 children are listed,
    and the rest are summed on one line.
- **Per branch:**
  - **Remove** adds the branch's pattern to `word_removals`. A removal
    the new one ends with is contained in it and is replaced.
  - **Solo** draws only this branch while the button is held
    (`FlameRenderer::set_word_solo`, a per-frame `UiResponse` field). It
    is transient: not in the config, not in the plan key. Pressing and
    releasing it restarts the picture.
- **Beside it:**
  - the removed patterns, each with a restore button;
  - the trim slider and its depth, moved here from the View panel;
  - the targeting checkbox, since the panel needs a plan.
- **The Path Editor is gone.** The Words panel took its `PanelType` slot
  (flame-only, in the Window menu). Removed with it:
  - `path_editor.rs`, `GpuPathFilter`, `path_filter.wgsl` and
    `check_path_filters`;
  - the filter buffer and binding 8, left as a gap in both the app's and
    the export's layouts;
  - `set_path_filters` and the `UiResponse` field that fed it.

  The two `Params` fields the filters used are padding now, so
  `post_symmetry` keeps its 16-byte boundary. The PathMap colour mode
  stays, and `PATH_TRACKING` now means exactly the PathMap colour mode.
  All eight canonical shader dumps were regenerated. The diff is the
  padding, the dropped struct and binding, and, in the PathMap dump,
  the filter module and its call.
- **Gate.**
  - `the_words_panel_sees_what_is_drawn` (GPU) runs through the
    renderer the app holds:
    - the tree covers every drawn word;
    - solo draws the flicker's 274 words alone, and restarts the picture
      when pressed and released, while the tree stays the plan's;
    - a removal takes the branch out of the tree and the picture at once
      (5,319 → 5,045 words), before the replan.
  - The app, driven into the first animation frame, showed the tree
    (`T4·1` 99.45%, the flicker `T2·1` 0.55%, the glow `T3·2` not
    measured). With `t1a0` removed, the picture looks like the second
    frame, and the Removed list shows `T2·1`.
  - The visual suite passes, 330 of 330.

## 7. Later (phase 4, only if needed)

- Hysteresis for trim across animation frames (the "lowest common
  denominator"): a branch kept in the previous frame keeps a lower bar.
  Tested on an animation (2026-09-24): not needed.
- Hover highlight of a branch in the viewport.

## 9. Names, Always, and opening a path (2026-09-24)

- **Names in the UI.** Most users will not know the mathematical
  terms, and "word" also means words. The code and these docs keep the
  math names; only the UI changes:

  | Code and docs | UI |
  |---|---|
  | cylinder targeting | **Focused Rendering** |
  | a word | a **path**: the transforms that lead into a part of the picture |
  | the Words panel | the **Paths** panel (`paths_panel.rs`, `PanelType::Paths`) |

  The PathMap colour mode keeps its name; it colours by the history
  a sample happened to take.
- **Off / Auto / Always.** A switch replaces the targeting checkbox, in
  the View panel and at the top of the Paths panel, with one status line
  shared by both (`paths_panel::focused_rendering`). Auto is the old
  behaviour: the renderer declines a plan where it does not pay.
  - Always (`FractalConfig::cylinder_always`, skipped when false) keeps
    the plan at every zoom, so the Paths panel works without zooming in
    first. Zoomed out it renders about half as fast as the ordinary way,
    and the status line says so.
  - A trim or removals keep the plan too (`keeps_plan`), as before.
  - The plan key includes `keeps_plan`, so switching to Always replans
    a view Auto had declined.
- **Opening a path splits it.** Zoomed out, every path is whole (the
  view holds all of each: 26 top-level paths on the Grand Julian), so
  the panel could only offer whole transforms.
  - Opening a path that is one word of the plan asks the renderer to
    split it (`FlameRenderer::request_split`, a per-frame `UiResponse`
    field). The walk splits each such word, and every word holding one
    (`PlanOptions::refine`, the same mechanism a removal uses), rather
    than keeping it whole.
  - The set is transient and only grows, so closing a path does not
    replan. It is forgotten when the flame changes, and capped at 256.
  - A path that cannot be split, such as a blur's (its region is the
    whole fractal), shows "No smaller paths here".
  - Gate (`always_keeps_a_plan_zoomed_out_and_opens_a_path`, GPU): at
    zoom 1, Auto declines the plan and Always keeps it, as 26 whole
    paths. Opening one splits it into 26 beneath it, and the rest stay
    whole.
  - The view's share is 0.9824 whole and 0.9817 split, so it is the
    same picture, divided. The split's probability is 0.5% lower,
    because it drops sub-paths that lie wholly off screen.

## 8. Order

Phase 1, measured on the two frames, then 2, then 3.
