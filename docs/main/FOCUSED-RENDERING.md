# Focused Rendering

**Overview:** Deep zoom for 2D flames. A **plan** lists the *words* --
sequences of the flame's transforms -- whose images reach the view.
Every sample then runs the ordinary chaos game's free orbit and is
plotted *forced* through a word drawn from the plan, so it lands in
frame at zooms where the plain chaos game lands almost none. The
picture is the unbiased render's: only how fast it arrives changes.

It is **opt-in**, **2D only**, and works for a **narrow range of
flames** (affine IFS, and the families the inverse walk can plan). Its
coverage work is **paused, not finished**: what is open, measured and
ordered lives in the tracker, not here.

| In the UI | In the code and design docs |
| --- | --- |
| Focused Rendering | cylinder targeting (`cylinder_targeting`, `scene::cylinder`) |
| a path | a word: transforms (and arms) in the order the chaos game applies them |
| the Paths panel | the Words panel, sometimes the Pieces panel (`PanelType::Paths`) |

> **Not the escape engine's deep zoom.** Escape-time fractals zoom deep
> by perturbation: a reference orbit computed in fixed point, each
> pixel iterated as a delta from it, and bivariate linear approximation
> to skip iterations (`src/escape/reference.rs`, `fixedpoint.rs`,
> `bla.rs`). That is a different engine and unrelated code; see
> [ESCAPE.md](ESCAPE.md). The only thing the two share is the IFS
> analysis (`scene::ifs_analysis`, `variations::inverse`), which the
> escape engine's IFS distance mode also reads.

**See also:**
- [RENDERER.md](RENDERER.md) — the flame pipeline this sits inside
- [SHADERS.md](SHADERS.md) — template flags and the shader builder
- [BUFFERS.md](BUFFERS.md) — the bindings, including the plan table
- [COLOR.md](COLOR.md) — the PathMap colour mode, which draws through a plan
- [TONE-MAPPING.md](TONE-MAPPING.md) — `sample_density`, Levels, auto exposure
- [VARIATIONS.md](VARIATIONS.md) / [TRANSFORMS.md](TRANSFORMS.md) — what a word is made of
- [EXPORT.md](EXPORT.md) — the tiled exporter, which plans once per export
- [UI.md](UI.md) — the View panel and the panel system

**Design record** (history, measurements, and the open work):
- [deep-zoom-tracker.md](../projects/deep-zoom/deep-zoom-tracker.md) —
  **start here**: status, measurements, agreed order of work
- [flame-deep-zoom.md](../projects/deep-zoom/flame-deep-zoom.md) — the
  stages, and why a forced sample keeps the picture unbiased
- [inversive-targeting.md](../projects/deep-zoom/inversive-targeting.md)
  — the inverse walk and its completeness; family M
- [gpu-cylinder-planning.md](../projects/deep-zoom/gpu-cylinder-planning.md)
  — planning on the GPU, and on the web
- [deep-zoom-precision.md](../projects/deep-zoom/deep-zoom-precision.md)
  — past f32: the replay in offsets
- [forward-bounds.md](../projects/deep-zoom/forward-bounds.md) — bounds
  derived from a variation's WGSL
- [word-editing.md](../projects/deep-zoom/word-editing.md) — trim,
  removals, solo, the Paths panel, PathMap by path
- [ifs-general.md](../projects/deep-zoom/ifs-general.md) — the shared IFS
  machinery (`InverseDef`)
- Superseded: [ifs-measure-by-inverse-walk.md](../archive/projects/ifs-measure-by-inverse-walk.md),
  [ifs-perturbation-delta.md](../archive/projects/ifs-perturbation-delta.md)

**Code locations:**
- [src/scene/cylinder.rs](../../src/scene/cylinder.rs) — `Cylinders`
  (the plan), `NoCylinders` (why not), the forward planner, `pack` /
  `pack_words` (the shader's table), `iteration_scale`
- [src/scene/backward.rs](../../src/scene/backward.rs) — the inverse
  walk; `PlanOptions`, `TIME_BUDGET`, reference orbits for offsets
- [src/scene/plan_gpu.rs](../../src/scene/plan_gpu.rs) — the walk's
  questions answered on the GPU
- [src/scene/word_tree.rs](../../src/scene/word_tree.rs) — trim,
  removals, solo, the Paths panel's tree, PathMap colours
- [src/scene/final_map.rs](../../src/scene/final_map.rs) — final
  transforms pulled back for the walk
- [src/scene/mobius.rs](../../src/scene/mobius.rs) — family M (Möbius flames)
- [src/scene/ifs_ball.rs](../../src/scene/ifs_ball.rs),
  [src/variations/bound.rs](../../src/variations/bound.rs),
  [src/variations/derive.rs](../../src/variations/derive.rs) — forward
  bounds: by hand, and derived from WGSL by interval arithmetic
- [src/variations/inverse.rs](../../src/variations/inverse.rs) — the
  inverses the walk uses (`INVERSES`)
- [src/scene/forward_delta.rs](../../src/scene/forward_delta.rs) — the
  CPU's forward difference forms (offsets)
- [shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl)
  — the forced prefix; [replay.wgsl](../../shaders/core/replay.wgsl),
  [replay_delta.wgsl](../../shaders/core/replay_delta.wgsl);
  [plan_eval.wgsl](../../shaders/core/plan_eval.wgsl),
  [plan_gather.wgsl](../../shaders/core/plan_gather.wgsl),
  [plan_offsets.wgsl](../../shaders/core/plan_offsets.wgsl) (the GPU planner)
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)
  — `FlameRenderer::sync_cylinders`, the plan job, the standby, `plan_view`
- [src/ui/paths_panel.rs](../../src/ui/paths_panel.rs) — the switch,
  its status line, and the Paths panel
- [src/renderer/density_gates.rs](../../src/renderer/density_gates.rs) —
  comparing renders by density (tests)

---

## What it does, and why

A flame's picture is its **invariant measure** `μ`, which satisfies
`μ = Σ_i p_i (S_i)_* μ`. Unrolled along a complete antichain of words
`A` (a prefix-free set every infinite path passes through once),

```text
μ = Σ_{a ∈ A} p_a · (S_a)_* μ,     p_a = ∏ p(symbol)
```

Restricted to the view `V`, every word whose image misses `V`
contributes nothing. So drawing a word `a` from the words that reach
the view, with probability `p_a / P(A_V)`, applying it to a point
`x ~ μ` from the true chaos game, and plotting `S_a(x)` has expectation
`μ` restricted to the view -- **exactly**, for any set of words built
this way. A loose plan admits words whose image misses the view; their
samples land outside the frame and are discarded. That costs
efficiency, never correctness.

The plain chaos game's share of a small view falls off polynomially
with the zoom. A forced sample costs the word's length, which grows
like `log(1/zoom)`. `Cylinders::speedup` is the ratio,
`1 / (mass · (depth + 1))`, and it grows without bound as the view
shrinks.

**One sample:**

1. The thread's free orbit steps as the ordinary chaos game does. It is
   the persistent orbit ([RENDERER.md](RENDERER.md)), so its point is a
   sample of `μ`.
2. At plot time a word is drawn from the plan by its cumulative
   probability (`ct_pick`).
3. The word is applied to the orbit's point -- one composed matrix, or
   a replay of its symbols (below) -- and the colour coordinate is
   carried through the word's colour fold.
4. The forced point goes through the rest of the plot path as any
   point does: the final transforms, the depth effects, the deposit.
5. The free orbit's point and colour are restored, and it carries on.
   The forced prefix draws from its own RNG stream (`ct_rng`), so it
   never disturbs the free orbit.

---

## Brightness

A forced sample's true weight is `P(A_V)`, the plan's **mass** -- the
share of the measure the view holds. Deposited directly it would be
lost to the u32 histogram's quantisation (at a zoom of 2^20 it is about
2.6e-9). So each forced sample deposits **one**, and the tone map is
told the render did `N / P(A_V)` iterations instead of `N`.

| Piece | What it does |
| --- | --- |
| `Cylinders::iteration_scale` | `1 / (mass · draw_scale)`: the factor the tone map's sample count is multiplied by (`FlameRenderer::refresh_sample_density`; the tiled exporter uses the same expression) |
| `Cylinder::draw` | A word drawn off its probability -- `prob · draw` -- deposits `1 / draw` per sample, so the picture is unchanged and fewer samples go where few land |
| `Cylinders::draw_scale` | The probability-weighted mean draw rate `S`. It is 1 when every word is drawn at its probability, or when the replay table has no room for the per-word weights (`weights_fit`) |
| `levels_density` | Levels measures against the frame's *real* mean, `sample_density / iteration_scale`, written beside it from the same expression |

Two consequences:

- **Editing keeps brightness.** Trim, removals and solo take a subset of
  the plan's words and recompute its mass (`word_tree::subset`), so
  what remains is as bright as it was.
- **A new plan restarts the picture.** Samples drawn under one plan
  carry its mass, so every plan that lands -- a tight plan, a plan on
  the way, a standby swapped in -- resets the accumulation
  (`FlameRenderer::take_plan_arrived`). Carrying the accumulation
  across a plan was considered and dropped (tracker P7).

**Auto exposure** (`FractalConfig::auto_exposure`, the View panel's
Deep Zoom section) is a separate switch. It counts plot attempts and
in-frame landings in the shader (`FRAME_COVERAGE`) and scales the
tone map's count by the measured coverage, so a deep *untargeted* view
does not fade. For a targeted render the two factors cancel: every
forced sample lands in frame, so its coverage is 1. In the app the
coverage is an exponential moving average; the tiled exporter reads
the counters with each dispatch.

---

## Which flames it works for

`Cylinders::plan` refuses some flames before any planner runs, and each
planner refuses others. Every refusal is a `NoCylinders` value, and the
status line prints it.

**Refused before planning** (`Cylinders::plannable`):

| `NoCylinders` | Why |
| --- | --- |
| `Empty` | No transforms, or every weight is zero |
| `Xaos` | The first symbol's probability would depend on the burn-in's last transform, which the table does not carry |
| `PostSymmetry` | Its copies of a forced sample land outside the view the plan was made for |
| `ColourNotAffine` | A `WritesColor` or `WritesRgb` variation is active, so a word's colour cannot fold into two coefficients |

**Not planned at all:** 3D. The planners ask whether a word's image
meets a disc in the plane; in 3D what reaches the frame depends on the
camera and on `z`. The renderer reports `TargetingState::NotPlanar`, and
`visibility::Control::FocusedRendering` greys the switch. A `.flame`
import loads in 3D by design, so it must be switched to 2D first.

**The planners**, chosen per flame:

| Planner | Takes | Where |
| --- | --- | --- |
| Forward, composed | Every transform affine. A word composes to one 2×2 and a translation on the CPU | `Cylinders::plan_inner`, `pack` |
| Forward, bounded | Transforms that are not affine but have a forward bound: a disc that contains the image of a disc. The kernel replays the word | `plan_inner`, `ifs_ball::Bounder`, `pack_words` |
| Inverse walk | A flame with an **armed** variation (one whose draw picks among several images), or with final or linked transforms. The only planner for those | `backward.rs` |
| Family M | Möbius flames: similarities with `linear` and `spherical` (the Schottky flames). Cylinders are keyed by composed map, not by word | `Cylinders::plan_mobius`, `mobius.rs`; gated by `mobius::ENABLED` |

**The forward planner** pushes an invariant disc through each word and
keeps the words whose image meets the view, breadth first. It needs:

- an origin-centred disc every map sends into itself
  (`NoInvariantBall`);
- every map to shrink that disc (`NotContractive`);
- a bound for every active variation (`Unbounded`, naming the
  transform and the variation);
- at most `MAX_WORDS` (4096) words (`TooManyWords`), and at most
  `MAX_DEPTH` (96) symbols a word. A word that reaches the depth cap is
  kept as it is, which is sound;
- some word to reach the view (`ViewIsEmpty`).

Forward bounds come from two places. Five are written by hand
(`bound::BOUNDS`: `spherical`, `bubble`, `julian`, `pre_blur`, `blur`).
On the **desktop**, any other variation's bound is derived from its
shipped WGSL by interval arithmetic over naga IR (`variations::derive`),
retried once on a subdivided input before refusing. The **web** build
has no WGSL front end, so it has the five hand bounds only.

**The inverse walk** pulls the view back through the maps' inverses,
guided by an indexed sample of the attractor, instead of pushing a disc
forward. It plans flames whose dominant maps *expand* over the
attractor, where no forward bound can contract. It needs every
transform readable as maps it can invert:

- **Armed variations** (`bound::ARMED`): `julian` and `juliascope`. Each
  arm (branch of the draw) is its own symbol, packed as
  `transform | arm << 8` (`cylinder::sym_of`), at most `MAX_ARMS` (64)
  per variation.
- **Kernels with inverses** (`variations::inverse::INVERSES`): the
  affine roles, then `julia`, `julian`, `spherical`, `bubble`,
  `hemisphere`, `disc`, `blob`, and the walk-only `elliptic`, `splits`,
  `cylinder` and `juliascope`.
- **Blurs**: a `pre_blur` that is its transform's only pre-phase
  variation, and a transform whose one variation ignores its input (a
  free blur).
- **Final transforms** that are affine, or `bipolar` between the
  affine and post-affine, one `bipolar` at most (`final_map.rs`). The
  walk plans in the attractor's own space, through the finals.
- **Linked transforms** are refused, with the reason, as are final
  transforms attached to some transforms and not others.

A flame the walk refuses is refused outright (`Unbounded`, "the inverse
walk cannot plan this flame: ..."). The forward planner is not tried in
its place.

**Family M** walks probability increments to maps rather than words,
because a Möbius flame's alphabet holds each generator and its inverse.
Its budget is `MOBIUS_TIME_BUDGET` (1 s) and `MAX_NODES`; what it cannot
follow is charged to `Cylinders::lost` and shown.

**Not declined, but wrong:** see [Known gaps](#known-gaps-and-open-work)
for opacity and Speed colour mode.

---

## The plan

`Cylinders` is the plan; each `Cylinder` is one word.

| `Cylinder` field | Meaning |
| --- | --- |
| `word` | Symbols in the order the chaos game applies them, oldest first |
| `prob` | `p_a`, the word's true probability |
| `eff` | The share of its forced samples that land in the view, from the walk's replays; 1 where the planner does not measure it |
| `draw` | How often it is drawn relative to its probability (see Brightness) |
| `cond` | A blur drawn where it lands: the blur's draw restricted to what the rest of the word sends into the view |
| `seeds` | Points the walk found landing in the view, where reference orbits start |

| `Cylinders` field | Meaning |
| --- | --- |
| `mass` | `P(A_V)`, the sum of the kept words' probabilities |
| `lost` | Measure the planner could not account for. Zero for every flame that targets normally; non-zero, the status line says "MISSING" with the percentage |
| `sampling_leak` | Attractor a sampled cover never covered (family M) |
| `efficiency` | Probability-weighted share of forced samples landing in frame. Below 1 is waste, not error |
| `timed_out` | The walk hit its budget and forced its frontier |
| `composable` | Every map affine: one matrix a word |
| `view_centre` | The view it was made for; packing is relative to it |
| `refs`, `offset_rows`, `final_rows` | Reference orbits and map rows for the replay in offsets |

**Budgets.** The walk stops at `backward::TIME_BUDGET` (20 s, a safety
net now that the app plans in the background) or at `INLINE_BUDGET`
(1.5 s) where a plan must block on the web's UI thread. Hitting either
**forces the frontier as it stands**: the plan is complete, only less
efficient. Its words are replayed into a table of at most
`TABLE_FLOATS` (2^25 floats, 128 MiB, one storage binding); the walk
forces its frontier early rather than make words no table could hold
(`FRONTIER_FLOATS`).

**The GPU planner.** Most of the walk's time is one question: apply a
word to sample points and say which land in the view. `plan_gpu.rs`
answers it in batches with the flame's own WGSL plus
`plan_eval.wgsl`, and does the candidate gathers there too
(`plan_gather.wgsl`). The replay is `ct_apply_symbol` in `replay.wgsl`,
**the same function the render's replay arm calls**, so the planner and
the picture cannot disagree about where a word sends a point.

- The planner's arithmetic is absolute f32. Below `GPU_PLAN_RADIUS`
  (2e-6, relative to the view centre's distance from the origin)
  `Backward::gpu_resolves` says no. Then, on the desktop, the CPU checks
  in f64 and the GPU runs the replays in offsets (`plan_offsets.wgsl`);
  on the web the CPU plans.
- Background plans use the GPU by default. `FFLAME_PLAN_CPU` set in the
  environment plans on the CPU instead (desktop).

---

## The shader

Four template flags, all off unless a plan is drawn. With targeting
off the WGSL is byte-identical to a build without the feature
(`cylinder_targeting_off_is_byte_identical`). Entering or leaving
targeting, or changing arm, is a shader change: `sync_cylinders`
returns `true`, and the caller follows with a full `load_config`.

| Flag | When | What it adds |
| --- | --- | --- |
| `CYLINDER_TARGETING` | A plan is drawn | The table at `@binding(15)`, the word draw, the forced prefix |
| `CYLINDER_REPLAY` | The plan is not `composable` | The symbol walk instead of one matrix; `replay.wgsl` |
| `CYLINDER_RELATIVE` | Packing relative to the view is safe (below) | `world_to_pixel` stops subtracting the pan: the point arrives as an offset from the view centre |
| `CYLINDER_OFFSETS` | Replay with reference orbits | The deep steps as offsets (`replay_delta.wgsl`) |

PathMap adds `PATH_TRACKING` (the per-pixel path id and the Origin
styles), and auto exposure `FRAME_COVERAGE`.

**The composed arm.** Every map is affine, so `pack` folds each word
into one affine map on the CPU, 12 floats a word: the 2×2 and its
translation, the colour fold `H, G`, the cumulative probability, and the
word count in word 0's spare float (`COUNT_AT`). Forcing a word of any
length costs one matrix multiply.

**The replay arm.** `pack_words` writes an eight-float header (stride,
count, where the map rows and reference blocks start, the shift, the
weights, the finals), then one record a word: `[cdf, H, G, len, sym0,
sym1, ...]`, at a uniform stride set by the longest word.

- `ct_apply_symbol` runs each symbol's transform exactly as the chaos
  game does: affine, variations, post-affine. For an armed symbol it
  sets `ct_forced_arm`, and the armed variation's draw -- wrapped by
  the shader builder under `CYLINDER_REPLAY` only (`ArmedDef::forced`)
  -- takes that arm.
- **One word per 32 threads.** Samples in a group of 32 share a draw
  (a hash of the group and the iteration), so a warp walks one word.
  Each sample still draws with the plan's probability.
- A blur drawn where it lands (`cond`) is a negative weight naming its
  block; the sample starts from a point of the restricted draw.

**The colour fold.** flam3's colour rule, `c ← c·h + g` per transform,
is affine in `c`, so a whole word is `c ← c·H + G`. The composed and
replayed arms both apply the CPU's `H, G`, so the colour is exact
whatever the position map does.

**Precision.** A forced sample in absolute f32 world coordinates snaps
to f32's grid once its spacing is a sizeable fraction of a pixel, and
pixel columns get unequal shares of it: stripes.

- **Relative** (`CYLINDER_RELATIVE`). The composed arm's translations
  are packed with the view centre subtracted in f64, so the point
  arrives already relative to the view. The replay arm does the same
  where its plan carries references. `relative_is_safe_for` turns it
  off for post-symmetry, an active analytic blur, and finals the plan
  did not carry into its offsets; the picture is the same, it only stops
  resolving sooner.
- **Offsets** (`CYLINDER_OFFSETS`). The CPU runs reference orbits in
  f64 (`Backward::reference_chains`). A word replays in absolute f32 up
  to step `m` -- the last step whose f32 error, carried through the
  rest of the word, stays within `PLOT_TOLERANCE` (a twentieth of a
  pixel on a 4K frame) -- then carries the sample as an offset from a
  reference through each map's forward difference form. The forms are `replay_delta.wgsl`, held to
  the CPU's (`forward_delta.rs`) by
  `the_shader_forward_forms_are_the_cpu_ones`. Final transforms run in
  offsets too (`FinalMap::rows`). The table's shift -- the plan's
  centre less the view's, formed in f64 -- is rewritten every frame, so
  a plan still drawing after a pan stays where it belongs.

---

## In the app

`FlameRenderer::sync_cylinders` runs every frame for a flame, not only
while iterating, so a plan made in the background is picked up even
when the render it restarts had finished. The plan is keyed by the
view (zoom, pan, size), the flame (`flame_key`: only live variations),
the removals, whether a plan is kept where it does not pay, the opened
paths, and PathMap's minimum word length.

**Inline or in the background.** The forward planners cost well under a
millisecond and plan inline, every frame the key changes. A flame the
inverse walk or family M plans (armed, with finals or linked
transforms, or Möbius) waits for the view to hold still for 250 ms
(`targeting_settle_delay`), then plans in the background:

| | Desktop | Web |
| --- | --- | --- |
| Runs on | A worker thread (`cylinder-plan`), cancelled when the view moves on | A future polled each frame for `WEB_PLAN_SLICE` (6 ms) or until it waits on the GPU |
| While it runs | The plan on screen keeps drawing; the status line shows the time | Same |

**Plans on the way** (tracker P8). From `PLAN_ON_THE_WAY` (250 ms), and
at each doubling of the time since, a tight plan hands over a complete
plan of what it has so far: the words kept and the frontier forced. One
is shown only where nothing on screen covers the view (a long pan, a
first plan, a zoom past the standby) or what is shown is an earlier
plan on the way. The final plan is the plan made without them, word for
word.

**The standby** (`STANDBY_MARGIN` = 2). Once a tight plan lands, a second
plan is made for a disc of twice the view's radius and held. When the
view leaves the plan on screen but stays inside the standby's disc -- a
pan of up to one view radius, a zoom out of up to 2× -- the standby is
swapped in on that frame, and a tight plan for the new view is made.
A plan for the view itself is never made with a margin: that would cost
the picture you stop on its efficiency.

**The governor cap** (tracker P10). The planner submits to the render's
own GPU queue, so each of its round trips waits behind the dispatch
queued ahead of it. While a tight plan is made, the dispatch is capped
at the shortest dispatch length (`Batch::while_planning`, and
`Batch::fixed` with the governor switched off) and the governor holds
rather than reading the planner's work as the render's cost. Standby
plans are not capped.

**Auto, Always, kept.** In Auto the renderer declines a plan where
`speedup ≤ 1` (`TargetingState::NotWorthIt`): zoomed out, the ordinary
chaos game already lands its samples in frame. A plan is kept anyway
(`keeps_plan`) with Always (`cylinder_always`), with a trim or a
removal, and in PathMap colour mode.

**The zoom cap.** View controls clamp the zoom at
`FractalConfig::max_view_zoom`: 10,000 normally, 1e30 with targeting on.

**The status line** (`paths_panel::focused_rendering`, in the View
panel's Deep Zoom section and at the top of the Paths panel) prints the
`TargetingState`:

| State | Shown as |
| --- | --- |
| `Off` | Not running |
| `NotPlanar` | 2D only |
| `Declined(NoCylinders)` | Not available, and why |
| `NotWorthIt { speedup }` | Not used at this zoom |
| `Active { words, depth, speedup, lost }` | Running, with the plan's size; a separate clause if `lost > 0` |

---

## Headless renders and export

`FlameRenderer` plans inline unless background planning is switched on,
which only the app does. So `render_with` -- CLI export, thumbnails,
video -- plans in `load_config`, and its first sample already uses the
plan.

The tiled exporter (`HighResExporter`) renders the whole image from one
stream of dispatches and only scatters the samples per tile, so it has
**one view to plan** however it is tiled. `compute_kernel::plan_view`
makes the renderer's decisions once -- removals, PathMap's minimum word
length, the not-worth-it test, trim -- and returns the table, the
shader flags and the iteration scale. Gate:
`a_targeted_export_is_the_targeted_render`.

What `plan_view` leaves out is what the app adds for its panels: opened
paths, solo, and PathMap's per-path colours.

---

## Editing the picture by its paths

A word is exactly one piece of the picture, so removing a word removes
that piece and nothing else. The **Paths panel** (`PanelType::Paths`,
[src/ui/paths_panel.rs](../../src/ui/paths_panel.rs)) shows the plan as
a tree (`word_tree::Tree`), grouped by last-applied maps -- depth 1 by
the last map, depth 2 by the last two -- each branch with its share of
the view (`prob · eff`).

| Control | Config | What it does |
| --- | --- | --- |
| Off / Auto / Always | `cylinder_targeting`, `cylinder_always` | The switch, one undo point |
| Trim | `cylinder_trim` (0 = off) | Drops branches whose share is under this times their largest sibling's, within `cylinder_trim_levels` (default 2) levels of the view. Removes real parts of the picture, on purpose, for slivers that flicker in an animation. Retrims without replanning |
| Remove | `word_removals` | A pattern of maps matched against a word's last-applied maps. The walk makes no word ending with one, and refines a word that holds one |
| Restore | `word_removals` | Takes a pattern off the list |
| Solo (held) | none | Draws one branch alone, restarting the picture |
| Opening a path | none | Splits that word in the plan (`FlameRenderer::request_split`, at most 256 a flame, forgotten when the flame changes) |

A removal is stored as text, maps in the order the chaos game applies
them, the map nearest the view last: `"t1a1 t1a0"` is transform 1 arm 1,
then transform 1 arm 0 (`word_tree::parse_pattern`; 0-based, where the
panel labels the same path `T2·2 > T2·1`). Removals are drawn only
where a plan is: with Focused Rendering on, in 2D.

**PathMap** colours each part of the picture by its path, so it needs a
plan: with Focused Rendering off, or in 3D, it is the palette, bit for
bit. The Path, Path (distinct) and Depth styles colour each word on the
CPU (`word_tree::path_colours`) and pack the colour as the word's fold
with `H = 0` (`word_tree::recolour`), so the kernel needs no change. The
Origin styles read the point each sample started from, in the shader.
A path style reads `path_map_level` maps (default 2), so the planner
splits every shorter word (`PlanOptions::min_len`). Right-clicking the
viewport in PathMap shows the pixel's path and offers to remove it at
several lengths. See [COLOR.md](COLOR.md) for the styles.

---

## Related mechanisms

- **Auto exposure** -- see Brightness above.
- **Importance-sampled selection** (stage 1 of
  [flame-deep-zoom.md](../projects/deep-zoom/flame-deep-zoom.md);
  `FractalConfig::importance`, [src/scene/importance.rs](../../src/scene/importance.rs),
  the `IMPORTANCE_SAMPLING` flag). Biased transform selection with a
  windowed likelihood-ratio correction. A config-only mechanism: it has
  no `ConfigPath` and no UI.

---

## Testing

**Compare pictures by density, not by the tone map.** A tone map lifts
faint light and darkens sparse light, so of two renders of the same
measure the sparser reads darker -- and an untargeted reference is
sparse exactly in a deep view's faint regions. The gates read the
accumulator instead:

- `FlameRenderer::read_density_blocking`: each pixel's weighted count
  per equivalent iteration of the unbiased chaos game (the count over
  `samples_in_buffer · cylinder_iteration_scale`).
- `density_gates::replicated_blocks`: independent replicates summed in
  16×16 blocks, so each block has a mean and a standard error.
- `density_gates::compare_blocks`: two renders compared per block in
  standard errors, and pooled in dense, middle and faint tiers.

| Gate | Holds |
| --- | --- |
| `cylinder_targeting_off_is_byte_identical` | Off is the shader without the feature |
| `a_targeted_grand_julian_render_is_the_untargeted_render` | An armed flame targeted against the plain render |
| `a_plan_on_the_way_draws_the_same_picture` | Plans on the way against the landed plan, by density |
| `a_targeted_export_is_the_targeted_render` | The tiled exporter against `FlameRenderer` |
| `the_offset_replay_holds_per_sample` | The offset replay against f64, per sample |
| `the_shader_forward_forms_are_the_cpu_ones` | `replay_delta.wgsl` against `forward_delta.rs` |
| `bounds_contain_the_real_wgsl` | Every forward bound contains the shipped shader's output |
| `every_working_flame_loses_nothing` | `lost` stays zero |
| `pathmap_colours_by_path` | PathMap styles; off equals the palette |
| `a_dispatch_while_planning_is_short` | The governor cap's shape |

Most are GPU tests marked `#[ignore]`, so run them deliberately. Many
read their flames from `output/flame-zoom/`, which is under the
gitignored `output/` directory: they run only where those files exist.

---

## Known gaps and open work

The open items, their measurements and their order are in
[deep-zoom-tracker.md](../projects/deep-zoom/deep-zoom-tracker.md). The
ones a user can meet:

- **Opacity** (tracker C17). A forced sample is plotted by the free
  orbit's transform's opacity (`should_plot = rng < xform.opacity`), not
  by its word's last transform's. A flame with a transform below full
  opacity draws the targeted picture with the wrong transforms hidden.
  Nothing declines such a flame.
- **Speed colour mode** (C17). The forced sample wears the free orbit's
  step's speed colour, which says nothing of the point it plots.
  Nothing declines it.
- **GPU planner holes at 1e5–1e6** (C16). Down to `GPU_PLAN_RADIUS` the
  GPU planner's absolute f32 can misjudge a child as landing nothing,
  and drop it where its region is unseen. Measured, not yet fixed.
- **Finals** other than affine and `bipolar` are not pulled back (C13);
  RNG-bearing maps need one refinement mechanism (C14); offsets by
  Jacobian chains would replace the per-kernel difference forms (P9).
- **3D** is not planned (E2, an idea).
- **Other platforms.** The planner's kernel has not been run on Metal,
  where fast-math has broken shaders before, nor in Firefox (O1).

---

## Gotchas

**Declined is the common case.** Most flames are refused, and most
views of the flames that are not are too shallow to pay. Read the
status line before assuming the feature is working.

**A `.flame` import is 3D.** Switch it to 2D under Mode before
targeting it.

**Trim and removals change the picture on purpose.** They are artistic
edits saved in the config, not quality settings.

**A plan resets the accumulation.** A long pan with an expensive flame
shows a restart each time a plan lands; that is by design.
