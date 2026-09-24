# Deep zoom for flames: open items

The running list for cylinder targeting and the inverse walk. The work
behind each item is written up in the design docs:

- [flame-deep-zoom.md](flame-deep-zoom.md): the stages, and the precision plan (stage 3).
- [inversive-targeting.md](inversive-targeting.md): the inverse walk and its completeness.
- [gpu-cylinder-planning.md](gpu-cylinder-planning.md): planning on the GPU, and on the web (§17).

**Priorities (2026-09-23): coverage first, then performance.** Coverage
means more flames, deeper zooms, and images without holes. Testing on
other platforms (macOS/Metal, Firefox) is planned for later.

Status: **open**, **investigate** (measure before building), **done**.

---

## Coverage

### C1. Precision past ~64k zoom: stripes -- done (2026-09-23)

**Seen:** stripes past about 64k zoom.

**Cause:** f32. A forced point is computed in world coordinates, and the
view centre (`GpuParams::pan_x/pan_y`) is f32 too. At the saved
grand-julian view (x = 0.268, y = -0.111, 1280x720), f32 spacing against
the pixel size is:

| zoom | f32 spacing in x |
|---|---|
| 32k | 0.18 px |
| 64k | 0.35 px |
| 131k | 0.70 px |
| 1e6 | 5.4 px |

At 0.35 px, each pixel column holds either two or three representable x
values. Points snap to them, so neighbouring columns get density in a
2:3 ratio, which shows as stripes. y is 4x finer at this view (|y| < 0.125),
so the stripes should appear first as vertical bands.

**Fix:** stage 3 of [flame-deep-zoom.md](flame-deep-zoom.md), built as
[deep-zoom-precision.md](deep-zoom-precision.md). The replay's deep steps
run as offsets from f64 reference orbits. Per sample, the render is
within 0.02 px of f64 at the 99th percentile on grand-julian, random1
and julian-disc at 1e4-1e8, and grand-julian renders cleanly to 1e10.

**Found on the way:** the old replay was also displaced, by about 4e-7
world units at every depth (0.3 px at 1e4, 3 px at 1e5), from the GPU's
transcendental functions, not f32's rounding.

**Left over:**
- The GPU planner in offsets: past a view radius of about 2e-6 (zoom
  ~2e6), plans are made on the CPU, 3-5x slower.
- Zoom past f64's pan (about 1e12).
- The references' cost: on the web, a plan of ~10k words takes about
  0.5 s longer (1.2 s to 1.7 s warm, at 60 Hz), since they are computed
  on the page's thread. On twelve threads it is 3-7 ms.

- The forced word's image of a reference point is computed on the CPU at
  any precision (`BigFloat`). That is the reference orbit.
- Each sample is carried as an offset from it, through forward
  difference forms of the flame's kernels. [ifs-perturbation-delta.md](ifs-perturbation-delta.md)
  §3 has the construction; the CPU inverse forms exist and are gated.
- The splat becomes (reference − centre), in f64, plus the offset in
  f32. Nothing near the view is ever an f32 world coordinate.
- The kernels to port are only those the analysis accepts (affine,
  roots, disc, bubble), because only those flames are targeted.

### C2. `pre_blur` -- done, v1 (2026-09-23)

**Before:** rejected by the analysis as not affine. **Now:** a blur that
forgets its input (a renewal, v1 below) is planned. A smaller blur is
refused with its numbers (C2c).

**The idea:** keep blur out of the plan and let the forced replay apply
it. That is **biased**:

- Holes: a word whose blurred image reaches the view, but whose
  unblurred cylinder does not, is never forced, so its contribution goes
  missing.
- Wasted samples: forced samples get blurred off the view, and the share
  of samples that land falls with zoom.

**The sound version is cheap in principle.** `pre_blur` moves the point
by at most 3x its weight, in a random direction (JWF's six uniforms
minus 3, scaled by the weight). It runs after the affine and before the
variation. So the walk can treat it as a bounded dilation:

- Grow the preimage disc by 3w at each backward step through a blurred
  transform. Completeness stays by construction.
- The replay already applies the blur, since it runs the transform's
  own code.

**What that implies:**

- A word whose last-applied transform is blurred cannot localize below
  about 3w x |J|. Below that scale the true image there is smooth; there
  is nothing to zoom into.
- A word whose blur comes earlier, deeper inside it, has that blur shrunk
  by the maps applied after it, and zooms normally.

**Needs:**

- The analysis must accept `pre_blur` beside an analysable variation, as
  a transform with slack.
- Every place the walk maps a disc backward must add the slack.

**The canonical case** is the true Grand Julian (the first flame of
`assets/presets.fflame`, extracted to
`output/flame-zoom/true-grand-julian.fflame`). Transform 0, drawn about
6% of the time, is `pre_blur` 10 and `bubble` 0.2 on an identity affine.
A blur reaching 30 swamps its input, so it emits a smooth blob on the
disc of radius 0.2 whatever comes in: a **renewal**. Transforms 1-3 are
julian roots.

**Design, v1 (2026-09-23): renewal symbols.**

- **Analysis.** The planner's analysis strips `pre_blur` from a copy of
  the transform and records its reach, 3x the weight, in the frame the
  kernel sees. That is allowed when it is the transform's only
  pre-phase variation. The escape engine's analysis still refuses it.
- **Renewal, or refused.** A blurred transform is a renewal when its
  reach covers the attractor's image in its frame plus the kernel's
  preimage radius: every point then has a positive chance of landing
  anywhere in the transform's output. Bubble's inner branch holds a
  preimage of every image point within radius 2. Anything else is
  refused with its numbers ("a partial blur is not planned yet"), not
  planned with holes.
- **Sample and replays with the blur.** The walk's attractor sample and
  its CPU replays draw the blur, as JWF does. Replays are seeded by word
  and point, so a plan is still the same however it runs. The GPU
  planner and the render run the variation's own code, blur included.
- **A renewal child is kept, never carried**, since its region is the
  whole attractor. It is kept or dropped by geometry, not by replays: at
  depth, a smooth part can land 1e-7 of the time, and 400 replays would
  drop it and leave the view black. So the view is pulled back through
  the node's word, along every branch and with a radius bound. The child
  is kept when that region can reach the renewal's output disc. Replays
  only measure its efficiency.
- **References.** A word that starts with a renewal starts its
  reference after it, at points of the rest's region (its node's own),
  with `m >= 1`, so the blurred step always runs as the shader's own
  absolute code.
- **Gates.** Coverage against an untargeted chaos game with the blur, at
  views it can reach. The targeted picture against the untargeted one.
  A view inside the blob (the smooth part) must not come out black. The
  per-sample gate on its words.

**Built and measured.** `Backward::renewal`, `Backward::reaches`,
`forward_blurred`, `analyse_2d_maps_blurred_sliced`. On the true Grand
Julian (`what_the_true_grand_julian_is`):

| view | words (renewal-first) | coverage | renewal words: mass, efficiency | the rest: mass, efficiency |
|---|---|---|---|---|
| the preset's (1.8) | 26 (1) | 1.0000 | 6%, 1.00 | 94%, 0.96 |
| 1e2 on the attractor | 4,035 (354) | 0.9999 | 54%, 0.15 | 46%, 0.90 |
| 1e3 on the attractor | 1,065 (109) | 0.9997 | 89%, 0.04 | 12%, 0.93 |
| 1e3 inside the blob | 1 (1) | unreachable | 100%, ~0 | none |

- Coverage is against an independent chaos game with the blur, at views
  it reaches.
- Targeted against untargeted
  (`a_targeted_true_grand_julian_render_is_the_untargeted_render`): the
  targeted render lights 0.984, 0.994 and 0.999 of what the reference
  lights at 10, 1e2 and 1e3, with brightness 0.334/0.329 at 10.
- Its julian words hold per sample at 1e4-1e8: 99th percentile under
  0.003 px.
- Inside the blob the single word kept is the renewal itself, by
  geometry. Its replays hit zero times, and the replay rule would have
  dropped it.
- Plans of flames without a blur are unchanged, bit for bit.

**What v1 does not do: the blob is sampled at its own rate.** A
renewal word forces a whole blob through the rest of its word, and only
the part that reaches the view lands. At depth those words carry most of
the plan's probability (89% at 1e3) and land 4% of the time, so the
targeted render's efficiency falls toward zero there (it is correct, and
the picture's soft glow is about a quarter of the view at 1e3). See C2b.

### C2b. The blob's efficiency at depth -- open

To land a renewal word's samples, the blob would have to be drawn
already inside its word's region, with the word's probability scaled by
the chance of that. The chance is a smooth integral over the blur and
the attractor; the draw inside the region needs a per-sample weight,
which the renderer's unit deposits do not carry. Needs either weighted
deposits (the histogram carries `color_scale` = 100 per unit today) or
an exact conditional draw of the blur. Research, not a transcription.

### C2c. A blur too small to be a renewal -- open

A `pre_blur` whose reach does not cover the attractor's image in its
kernel's frame, or one beside anything but bubble, is refused with its
numbers. Planning it needs the walk's regions dilated by the reach at
each blurred step (the original C2 sketch above), so the blurred
transform's children can be carried.

### C3. random1 at 1e3 misses 2.6% -- open

Every other view measured covers 97% or more; grand-julian covers 100%.
The measurements are in [inversive-targeting.md](inversive-targeting.md)
§31-§32.

### C4. Schottky flames: a Möbius analysis -- open

Two corpus flames. They are off the escape-time / deep-zoom diagonal
because the inverse walk's analysis does not model Möbius maps.

### C5. Flames over the forward word cap -- open

Three corpus flames. They would need their own routing.

---

## Performance

### P1. Cache each word's landings across views -- measured, not worth building

**Measured (2026-09-23, `what_could_a_cache_reuse`, the current walk):**
the share of a plan's expanded nodes that the previous plan had already
expanded.

| move | at 1e3 | at 1e6 |
|---|---|---|
| pan 1/4 view | 28% | 31% |
| pan 1 view | 10% | 17% |
| zoom in 1.5x | 20% | 11% |
| zoom in 4x | 2.6% | 2.0% |
| zoom out 2x | 25% | 11% |

A plan's work sits at the depth where words fit the view, and those words
move with the view. So a cache keyed by word saves 1.0-1.45x on typical
moves, the same answer an earlier session recorded
([inversive-targeting.md](inversive-targeting.md) §28). The expectation
that it would be the biggest win was wrong.

What already covers moves: the standby plan (twice the view's radius) is
swapped in on the frame a pan or zoom-in lands inside it. What would
make plans faster at depth: the GPU planner in offsets (C1's leftover),
which the CPU now stands in for past ~2e6 at 3-5x the time.

### P2. Patterns instead of paths -- investigate

Idea (2026-09-23): track *patterns* in how points reach each pixel,
instead of explicit paths, so a moved view need not be planned from the
root again.

- **Where the patterns are exact:** for a view V inside a cylinder
  f_u(A), the words starting with u that meet f_u(V) are u followed by
  plan(V) -- exactly, since f_u is one-to-one. Other words meet f_u(V)
  only where cylinders overlap. So a plan is a finite set of patterns
  whenever the overlaps come in finitely many shapes. In the literature
  that is the IFS *finite type* condition, and its "neighbour maps"
  f_u⁻¹∘f_u' are the patterns.
  - With finitely many, planning is a walk on a small automaton: its
    cost does not grow with depth, and a pan re-walks only the last few
    steps.
  - Exact for affine flames with suitable (e.g. algebraic) ratios.
  - For nonlinear flames, deep cylinders are close to affine, so the
    neighbour maps converge (bounded distortion). The patterns are then
    approximate, and completeness needs a margin.
- **Measure first:** along the walk for the corpus flames, count the
  distinct neighbour maps up to a tolerance as depth grows. If the count
  levels off, an automaton planner is feasible. If it keeps growing, it
  is not.
- **Cheaper, and sure to help:** P1, the per-word cache.
- **From the render itself:** the compute shader already knows each
  deposited sample's recent symbols (the `PATH_TRACKING` machinery; under
  targeting, the forced word plus the free symbols). A census of those
  addresses per region of the view says which longer words land in a
  zoomed-in sub-view, and how often. It is statistical, so on its own
  it cannot rule out holes; it can seed the walk, or order it.

### P3. julian-disc plans are large and inefficient -- open

Complete, but they force many more words than the view needs.

### P4. Keep-or-carry on the GPU -- open (optional)

GPU plans take 80-130 ms. Most of the GPU's work is checks for children
that end up kept anyway. Deciding keep-or-carry between the replay and
the check in the same submission, and returning counts rather than a
byte per point, would cut that ([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §16).

### P5. The first-plan frame in the browser -- open

One frame of 38-50 ms per flame per session: the GPU process compiles
the planner's six kernels synchronously
([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §17). Options:

- Async pipeline creation: the real fix. It needs a wgpu patch or upgrade.
- Merge Plan Eval and Plan Eval Gathered: less compile time, probably not
  a shorter frame.
- Compile the planner's kernels when the flame loads.

---

## Editing by words

### E1. Trim, removals and the Words panel -- in progress (2026-09-24)

Plan: [word-editing.md](word-editing.md). A trim slider drops the minor
branches of a plan's word tree that flicker in during an animation; a
Words panel removes branches by hand, replacing the Path Editor.

- **Phase 1, trim: done.** At trim 0.05, two levels from the view, the
  flicker in the Grand Julian animation frames goes (all 274 of its words)
  and the other frame's render is bit-identical. Unmeasured words
  (efficiency 0, such as the renewal glow) go with their branch and are
  never ranked as slivers.
- **Phase 2, removals: done.** `FractalConfig::word_removals`. The walk
  never makes a word ending with a removed pattern, and refines a word
  that holds one. A removal holds from zoom 5.5 to 1408 on the Grand
  Julian frame, with nothing kept whole.
- Phase 3 (the Words panel, the Path Editor removed): next.

## Other

- **O1. Other platforms -- later.** The planner's kernel has never run on
  Metal, where fast-math has broken our shaders before (see CLAUDE.md).
  Firefox has not been run.
- **O2. Merge `ifs-distance` into `main`** -- when you decide.
- **O3. A CLAUDE.md entry** pointing here and to the design docs.

## Done

- 2026-09-23: phase 3 (plans on the web), phase 4 (gathers on the GPU),
  and the crash when a web plan was dropped mid-flight
  ([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §16-§17).
