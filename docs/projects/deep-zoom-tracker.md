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

### C2. `pre_blur` -- open

**Today:** rejected by the analysis as not affine.

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

### P1. Cache each word's landings across views -- open

**The biggest expected win for panning and zooming in the app.** Much of
a plan's cost is working out where the sample points go under each word
(replays), and which of them land near each node (gathers). Where a word
sends the sample does not depend on the view. Today every pan or zoom
replans from the root and recomputes all of it.

- **Cache, per flame:** each replayed word's landings (the sample points
  it sends where, or the compact form the walk reads), bounded in memory
  and evicted least-recently-used.
- **Reuse:** a moved view walks again, but asks the evaluator only for
  words it has not seen. Keep, carry and drop decisions stay per view,
  since they depend on the view; only the view-free answers are reused.
- **Measure first:** how many words nearby views share (small pans, 2x
  zooms), which bounds the win. Then plan time per pan, before and after,
  in the app.
- **Fits the standby:** the standby plans a disc twice the view's radius,
  and a cache warmed by it makes the next tight plan cheap.

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
