# Planning cylinders on the GPU (plan, 2026-09-22)

Written as a plan before any code. The decisions (§11) were taken and
are recorded in §12; phases 0 and 1 are built and measured in §13, phase
2 in §14, the escape-time theory is measured in §15, phase 4 in §16,
and the rest of the document is the plan as it was written.

Background: [inversive-targeting.md](inversive-targeting.md) §24–§30
describe the planner this would accelerate -- the inverse walk over an
indexed attractor sample in `src/scene/backward.rs`.

---

## 0. What is being asked, and the short answer

Move the planner's expensive work -- evaluating chosen words on chosen
sample points -- onto the GPU.

**Short answer: yes, and the right cut is narrow.** The GPU should do one
thing, many times: *apply a given word to a given set of sample points
and say which land in the view, and where.* The tree, the index, the
completeness bookkeeping and every decision stay on the CPU. That split
keeps everything that was hard to get right in §26–§27 exactly where it
is, and moves the part that is pure arithmetic.

It matters most on the **web**, where there are no threads: today an
armed flame plans inline on the UI thread there, single-core. A GPU
planner is asynchronous by nature (submit, then read back later), so
it gives the web a non-blocking planner for the first time.

---

## 1. What the planner does now

Per plan (saved view, x546, 12 cores; `where_a_plan_spends_its_time`,
after the sequential replay of §30):

| work | CPU time | what it is |
|---|---|---|
| replays | 1.5 s | ~1.2M word applications: 100 (or 400) sample points per child, to measure the share that lands in the view |
| candidate checks | 1.8 s | ~1.7M word applications: a carried child's candidate points, checked exactly |
| gather | 1.0 s | index lookups: which sample points could belong to a child (binary searches over the landing index) |
| everything else | ~0.1 s | tree, orbit split, completeness check, beam |

Wall time ~0.45 s. A word is 5–20 symbols, so ~30M transform
applications per plan. Measured rates: the CPU planner manages about
10^8 transform applications per second across 12 cores; the GPU's
ordinary chaos game on the same flame ran 4x10^9 iterations in under
6 s in the saved-view picture gate, so **~0.7x10^9/s or more** -- about
7x the whole CPU, before any tuning.

So on desktop the arithmetic (replays + checks, 3.3 of 4.4 CPU-seconds)
would drop to tens of milliseconds, and gather -- which stays on the CPU
in the first design -- becomes the floor: ~80 ms of wall time on 12
cores. **Expected desktop plan: 0.45 s -> ~0.1-0.15 s.** On the web the
CPU times above are summed across threads, so single-core they are
~3-4 s natively and more as wasm -- and the web plans inline on the UI
thread, under a 1.5 s budget that forces whatever is left (complete,
less efficient). On the GPU the arithmetic is the same tens of
milliseconds, asynchronous.

These are estimates from existing measurements; phase 0 replaces them
with measured ones.

---

## 2. The operation

Everything the planner asks of a word is one of two questions about
`S_w(x)` for sample points `x`:

1. **Replay** -- for a child word: of these `n` points (100, then 400),
   how many land in the view disc, their centroid, and the largest
   distance from it. One answer per word.
2. **Check** -- for a carried child: for each of these candidate points
   (up to 1024, or 16384 for a thin child's second search), does it land
   in the view? One bit per point.

Both are "apply the word, test the disc". A word is applied as the
render's replay arm applies it: symbol by symbol, transform in the low
byte, arm forced from the high bits.

---

## 3. What exists to build on

### 3.1 The replay arm of the render kernel -- the closest thing

`main_template.wgsl` under `CYLINDER_REPLAY` already applies a word to a
point: for each symbol, mask the transform, set `ct_forced_arm`, run
`apply_affine`, `apply_variations`, the post-affine. The armed
variations' draws are wrapped to read the forced arm
(`bound.rs::ARMED`). **A planning kernel is that loop with a different
outside**: points from a buffer instead of the free orbit, a disc test
instead of a plot. It would be assembled by the same shader builder, from
the same variation WGSL, so it evaluates *exactly the maps the render
plots* -- which the CPU planner does not (see §7).

### 3.2 The Paths system -- related, but not the same thing

`PATH_TRACKING` records, per sample, the first 32 transform indices of
the chaos game's own random history (4 bits each, so at most 16
transforms, and no arms), and writes it to a per-pixel buffer for the
PathMap colouring; path filters drop samples whose recent history
matches a suffix of up to 8 symbols. So it **observes** the addresses
the chaos game happens to take, where planning must **choose** a word
and apply it to chosen points.

What carries over: the idea that a sample's address is data the GPU can
record cheaply, and the bind-group pattern of optional buffers bound to
dummies when off (`path-buffer-optimization.md`). What does not: the
encoding (16 transforms, no arms -- `grand-julian`'s alphabet is 25
symbols, `julian-disc`'s 51), the per-pixel buffer, and the random
source of the words. It is not a starting point for the planner.

One thing it could do later, and it is not in this plan: under
targeting, record the planned word each forced sample used, per pixel --
a GPU-side picture of which words reach which part of the frame. A
diagnostic, not a planner.

### 3.3 The mode-D inverse walk -- the other GPU walk

The escape engine's IFS mode already walks INVERSE maps on the GPU, per
pixel (`src/escape/ifs.rs`: `ifs_kernel_inverse`, `ifs_kernel_sigma`,
the Jacobians of §5k of
[ifs-measure-by-inverse-walk.md](ifs-measure-by-inverse-walk.md)), and
its measure walk renders a flame's density at any zoom by walking back
from each pixel (§5l there). Two relevances:

- The planner's cloud phase -- the descent from a view too small to hold
  sample points, by inverse maps -- could run on those kernels. It is a
  negligible share of the time today, so this plan leaves it on the CPU.
- It is a second route to deep zoom that needs no plan at all. Whether
  targeting and the measure walk should both exist long-term is a
  question for another day; this plan does not depend on the answer.

### 3.4 Plumbing that exists

- The shader builder assembles per-flame WGSL from only the active
  variations, with a cache; a planning shader is one more template.
- `Device` and `Queue` are cloned into background threads already
  (`app/mod.rs`); the planner thread would hold clones.
- Readback via `map_async` is in `escape/renderer.rs` and `escape/ifs.rs`.

---

## 4. Designs considered

**A. The GPU evaluates batches; the CPU keeps the tree.** Per level of
the walk, the CPU builds the children and their point lists as today,
then submits every replay and every check of that level in one or two
dispatches, reads back, and decides. *Recommended.* It moves 75% of the
CPU time, keeps every decision where it is, and is testable piece by
piece against the CPU.

**B. A, plus gather on the GPU.** The landing index is a sorted
`(cell, index)` array -- binary-searchable in WGSL. Moving gather too
would take the CPU floor away. Worth doing only once A is measured and
gather is shown to be what remains. *Deferred.*

**C. The whole walk on the GPU.** A tree whose size is unknown in
advance, a beam that sorts, a completeness check per node, forcing that
rewrites the tree: all things GPUs do badly and that took two days to
get right on the CPU. *Not recommended.*

---

## 5. Design A in detail

### 5.1 Buffers

| buffer | size | lifetime |
|---|---|---|
| sample points, `vec2<f32>` | 100k x 8 B = 0.8 MB | per flame (with the index) |
| words: concatenated symbols + offsets | a few thousand children a level (~22k a plan) x ~15 symbols x 4 B: well under 1 MB | per level |
| jobs: `(word, first point, count, kind)` | a few thousand x 16 B | per level |
| point lists for checks: sample indices | ~1.7M a plan in all, so a few hundred thousand a level x 4 B | per level |
| results: replay `(hits, sum x, sum y, max r)` per job; check bitmask per job | a few thousand x 16 B + the check bits | per level |

Grown and reused across levels; nothing is allocated per dispatch.

### 5.2 Kernels

One workgroup per job. Each thread applies the job's word to one point
(looping if the job has more points than threads); a replay job then
reduces hits, centroid sums and max radius in workgroup memory and one
thread writes the job's result; a check job writes its bits. No float
atomics are needed (WGSL has none): the reduction is per workgroup.
The centroid's max-radius needs the centroid first -- two passes in
workgroup memory, or the planner accepts the radius about the view
centre instead (the Cylinder's centre and radius are informational for
the replay arm; nothing downstream reads them).

### 5.3 A level, end to end

1. CPU: expand the frontier's nodes into children (gather candidates,
   orbit split) -- as now, in parallel.
2. GPU: first-pass replays for every child (100 points each), **and**
   speculative checks for every child's candidates, in one submission.
   Checking candidates for children that turn out to be kept is wasted
   work -- it was 90% of the CPU's check time in §28 -- but on the GPU
   it costs milliseconds and saves a round trip.
3. CPU: read back; children whose first pass is within `REPLAY_EXTEND`
   need the rest of their 400 points.
4. GPU: those extensions, if any.
5. CPU: decide keep / carry / force, completeness check, beam -- as now.

Two round trips a level, ~15 levels: 30 round trips. On desktop a
round trip is well under a millisecond plus the transfer; on the web it
is about a frame, so a plan takes ~0.5 s there -- asynchronous, so the
page stays live.

### 5.4 Threads, and the render

Desktop: the planner thread holds `Device`/`Queue` clones and submits
its own command buffers; the render keeps rendering the old plan
meanwhile. The two contend for the GPU -- the render will visibly slow
while a plan runs, for ~0.1 s. Acceptable; measurable in phase 2.

Web: no threads. The planner becomes a state machine advanced from
`sync_cylinders` each frame: submit a level, return; next frame, if the
readback has landed, decide and submit the next. The CPU half of a
level is small enough to run on the UI thread without a visible hitch
once the arithmetic is gone -- phase 4 measures that before committing
to it.

---

## 6. Precision

The CPU planner runs in f64; the GPU in f32. The render itself plots
replayed words in **f32 world coordinates** (the replay arm is not
view-relative), so its own ceiling is where f32 stops resolving the
view: somewhere near 1e6-1e7 of zoom. A GPU planner has the same
ceiling as the render it plans for, which is the honest limit -- a plan
the render cannot draw has no value.

What has to be measured (phase 1): agreement between CPU and GPU
answers on the same word and points, as a function of zoom. A disagreeing
**replay** only moves an efficiency estimate; a disagreeing **check**
drops or admits a point, which the completeness machinery then either
tolerates (orbit points are unaffected) or forces around. Gate:
check agreement at 99.9% or better up to the render's ceiling.

---

## 7. Consistency -- a side benefit

The CPU planner evaluates the flame through `ifs_analysis`'s maps
(`Map2` -- f64, rebuilt from the transform's affines and kernels). The
render evaluates the flame through its WGSL. They agree for the
variations the analysis accepts, to f32 rounding -- but they are two
implementations, and a disagreement between them would plan words for a
flame the render does not draw. A GPU planner uses the render's own
code, so that class of mismatch disappears for the arithmetic it takes
over.

It does not disappear completely: the sample, its orbit and the indexes
are still built on the CPU through `ifs_analysis`. Building the sample
on the GPU with the render's maps is possible later (the chaos game is
the render's own business) and would make the whole planner consistent
with the render. Not in this plan.

---

## 8. What it does not change

- **Which flames can be targeted.** The cloud phase still needs inverse
  maps, so only flames `analyse_2d` accepts. The GPU kernel would happen
  to run forward on any flame -- but the walk needs the inverse to start.
- **Completeness.** Same tree, same checks, same forcing.
- **The background thread and the standby** (§28–§29). The GPU planner
  is a faster engine under the same scheduling.

---

## 9. Phases and gates

**Phase 0 -- measure, decide.** A standalone harness (test-only): the
grand-julian flame, a fixed list of real words from a real plan, the
100k sample. Time, on the GPU, (a) 1M replays, (b) 1M checks,
(c) one submit + readback round trip with realistic sizes. Compare with
the CPU. *Gate:* the arithmetic of a whole plan in under 50 ms and a
round trip under 2 ms on desktop. If not, stop and record why.

**Phase 1 -- the kernel, and agreement.** The planning shader from the
shader builder (a `PLAN_REPLAY` template: the replay loop of
`main_template.wgsl` with a point buffer in and results out). *Gate:*
CPU/GPU agreement on checks at 99.9%+ at zooms 1e2, 1e3, 1e4, 1e6, and
the replay share within 1/100 of the CPU's for 99% of words.

**Phase 2 -- replays and checks per level, desktop.** `Backward` gains
an optional GPU evaluator; the walk batches each level. *Gates:* every
existing completeness and picture gate unchanged; plan time measured;
the render's frame rate during a plan measured.

**Phase 3 -- the web.** The state-machine planner. *Gate:* no frame
over 16 ms on the UI thread while a plan runs, in the browser.

**Phase 4 (optional) -- gather on the GPU**, if phase 2 shows it is the
floor.

---

## 10. Risks

- **Round-trip latency dominates.** Thirty round trips are cheap on
  desktop and a frame each on the web; if a level cannot be done in two,
  the web plan stretches. Mitigation: submit the speculative checks with
  the first replays (§5.3).
- **Contention with the render.** A plan and the render share the GPU;
  the render stutters while a plan runs. Measured in phase 2; if it
  matters, plans can yield (submit between frames).
- **f32 near the ceiling.** Checks that disagree with the f64 CPU near
  the render's precision limit. The render cannot resolve those views
  either; phase 1 measures where the disagreement starts.
- **A second copy of the replay loop.** The planning kernel and the
  render's replay arm must apply a word identically, or the plan and
  the picture disagree. Both must come from one WGSL function, spliced
  into both templates -- not two loops kept in step by hand.

---

## 11. Decisions for you

1. **Design A** (GPU arithmetic, CPU tree) -- or do you want the larger
   step (B) planned in detail too?
2. **Web first or desktop first?** Desktop is simpler and measurable
   first; the web gains the most. The phases above do desktop first.
3. **Phase 0's gate** -- is "under 50 ms of arithmetic and a round trip
   under 2 ms" the right bar, or is a smaller win on desktop worth it for
   the web alone?
4. **The mode-D measure walk** (§3.3) is a different route to the same
   deep zoom. Not needed to decide now; worth a conversation before too
   much more is built on either side.

---

## 12. Decisions taken

1. **Design A**: GPU arithmetic only, the tree stays on the CPU.
2. **Desktop first.**
3. **Phase 0's bar** was left open ("not sure"). Taken as: go on if the
   GPU does a plan's arithmetic at least 3x faster than the twelve CPU
   cores, round trips included, and gives the CPU's answers.
4. **Escape-time and deep zoom.** The theory put forward: the flames
   that convert to escape time are the ones that can be deep-zoomed.
   For armed flames today the two sets coincide by construction -- the
   inverse walk starts from `analyse_2d`, the same analysis mode D
   needs, and refuses whatever it refuses. Whether the sets coincide
   beyond that is a corpus measurement, due after the GPU phases.

---

## 13. Phases 0 and 1, measured (2026-09-22)

**Built:** `shaders/core/plan_eval.wgsl` (the entry point),
`ShaderBuilder::build_plan_eval`, `src/scene/plan_gpu.rs` (`PlanGpu`:
`evaluate` answers in-disc per point, `endpoints` returns where each
point lands), and the test `the_gpu_answers_as_the_cpu_does` in
`backward.rs`. Real words from real grand-julian plans -- every kept
word and every expanded node's word -- applied to the 400 verification
points and to 256 random sample points each, on both sides.

**One function for both.** Applying one forced symbol now lives in
`shaders/core/replay.wgsl` (`ct_apply_symbol`, with the forced arm and
its two helpers), appended to the definitions only when
`CYLINDER_REPLAY` is on. The render's replay arm and the planner's
kernel both call it, so the risk in §10 ("a second copy of the replay
loop") is closed rather than managed: a word cannot land in the view
for the planner and somewhere else for the picture. Untargeted shaders
are byte-identical (the file is not appended). The planner's kernel
used to treat a symbol's hide flag as a miss; the render discards it
(the plot is gated by the free iteration's own hide), and the shared
function does what the render does. No flame the analysis accepts has
a hiding variation, so this changed no answer.

**Two things had to be found first.**

- **Eight storage buffers per stage** (WebGPU's default): the flame's
  group 0 takes four, so the planner's jobs, words and point indices
  share one buffer (`plan_data`).
- **Derived parameter slots.** julian's `cpower` (and every
  `wgsl_init` variation's derived slots) are filled by the render's
  init pass. Without it every word sent every point to radius one and
  the kernel reported no hits at all. `PlanGpu::new` runs the same
  pass.

### 13.1 Speed

GTX 1660 SUPER against the planner's own CPU path on twelve threads
(rayon), whole batches of a plan's words at once, best of five:

| zoom | points | symbol applications | GPU | CPU, 12 threads | ratio |
|---|---|---|---|---|---|
| 1e2 | 1.71M | 16.9M | 5.3 ms | 187 ms | 35x |
| 1e6 (run first) | 1.90M | 27.2M | 6.4 ms | 294 ms | 46x |
| 1e4 (run second) | 2.81M | 36.7M | 9.1 ms | 393 ms | 43x |

The GPU's own work (submit to mapped) is 1.6-3 ms of that; packing the
jobs on the CPU is 2.5-4 ms and reading back under 1 ms. **Only the first
view a test runs times cleanly**: after the CPU comparison has held
twelve cores for a second, the next view's batches measured 2-3x slower
in whichever order the views ran, packing included, so the slowdown is
the machine's state and not the words (running the views in reverse
order moved it with them). One small job's round trip: 0.14-0.33 ms.

**The bar is passed by an order of magnitude**: 35-46x against 3x.

### 13.2 Agreement

The plan's gate was per-point agreement of 99.9%. Measured:

| zoom | per point | f32 step at the centre | moved >= 0.1 r | same keep/carry call |
|---|---|---|---|---|
| 1e2 | 0.99991 | 1.5e-6 r (0.00 px) | 1.8e-6 | 1.0000 |
| 1e3 | 0.99923 | 1.5e-5 r (0.01 px) | 3.5e-5 | 0.9996 |
| 1e4 | 0.99437 | 1.5e-4 r (0.11 px) | 1.4e-6 | 0.9944 |
| 1e6 | 0.91055 | 1.5e-2 r (10.7 px) | 2.1e-5 | 0.9154 |

Per point fails at 1e4, and **per point was the wrong gate.** With the
GPU's end points read back (`PlanGpu::endpoints`) against the CPU's:

- **Every disagreement below 0.1 r is at the rim.** 156, 2415 and
  15798 of them at 1e2, 1e3 and 1e4, and each one sits within twice its
  own displacement of the view disc's edge. The displacement is f32
  rounding: under 1e-3 r at 1e4. The f32 render puts those points on
  the GPU's side of the rim too, so there the GPU is the more faithful
  answer, not the less. The rim of the disc is outside the frame except
  at the corners.
- **Different points** (moved 0.1 r or more): 3, 115, 4 and 40 per
  million-odd, at most 3.5e-5 of all points. These are branches taken
  the other way (julian's `atan2` cut, most likely): the same class of
  event the render's own f32 replay has.
- **Replay shares** differ by more than 1/100 for 2-5% of words, up to
  0.235 at 1e3 and 0.85 at 1e4. These are words whose entire image is a
  speck on the rim -- measured image spreads of 8e-5 to 2e-3 r against
  displacements of 6e-6 to 7e-5 r -- so the rim's rounding moves a large
  share of a tiny image. That moves an efficiency estimate and nothing
  else; what changes a plan is the keep-or-carry call it makes at
  `CUT_EFFICIENCY`, which agrees for 99.4% of words or better wherever
  the render resolves a pixel.

**The gate as restated and asserted**, where one f32 step is smaller
than a pixel (1e2-1e4 here): points that land somewhere else under
1e-4 of all points; every disagreement that moved less than 0.1 r at
the rim; the keep-or-carry call the same for 99% of words. At 1e6 one
f32 step is ten pixels -- past the render's own ceiling for this view
(§6), so it is reported and not gated. The real test of the GPU's
answers is phase 2's: plans made with them must pass the same
completeness gates the CPU's plans pass.

### 13.3 What phase 2 inherits

- **Packing is as big as the GPU's work.** 2.5-4 ms to pack a batch
  against 1.6-3 ms on the GPU. Phase 2 packs each level's jobs as the
  walk produces them; the point indices are the bulk and are already
  flat vectors on the CPU side.
- **Round trips are cheap on desktop** (0.14-0.33 ms), so a level can
  afford the two §5.3 plans for: replays, then checks.
- **Replays want positions, not just shares**: the planner's replay
  records where the landed points sit (centre and radius). The
  `endpoints` mode returns them; phase 2 decides whether to reduce on
  the GPU or read positions back.

---

## 14. Phase 2, built and measured (2026-09-22)

**Built.**

- **`Evaluate`** (`backward.rs`): the walk's one question -- apply each
  job's word to each of its sample points; does it land in the view? --
  answered in batches. `CpuEval` answers with `Backward::lands` in f64 on
  rayon; `PlanGpu` answers with the kernel.
- **`Backward::expand_level`** replaces the per-node `expand`. The rules
  are the same; a level's questions are asked for all of its nodes at
  once, in five batches: seeds, replays (first pass), replays (second
  pass), checks, top-ups. The CPU work between them -- children,
  gathers, decisions -- runs over the nodes in parallel. `close` settles
  each node in its children's order.
- **Speculation.** An evaluator whose round trip costs more than extra
  answers (`Evaluate::speculative`: the GPU) is asked, with a level's
  replays, everything the walk might ask next -- the second pass and
  every child's checks -- and the walk uses only the answers it would
  have asked for. Plans are identical with and without it; batches per
  plan fall from 80-100 to 48-60 at five times the points.
- **`GpuPlanner`** (`plan_gpu.rs`): the device and the kernel for the
  flame planned last (~10 ms to build per flame). A kernel that fails
  validation makes that flame plan on the CPU instead of panicking the
  planner thread. `Backward::plan_for` uses it when `PlanOptions::gpu` is
  given.
- **The app** plans on the GPU by default, both on the planner thread and
  inline (export, the command line, tests). `FFLAME_PLAN_CPU=1` or
  `FlameRenderer::set_plan_on_gpu(false)` plans on the CPU. Desktop only;
  the web still plans inline on the CPU (phase 3).
- **A kept word's centre and radius are now the view disc.** Nothing
  downstream reads them for an inverse-walk plan -- the kernel's table
  holds the symbols, the probability and the colour fold -- and
  computing them needed positions an evaluator does not return.

**The walk is unchanged.** `dump_plans` writes a plan exactly: every
word's symbols and the bits of its probability, then the totals. Over 25
views (grand-julian, julian-disc, random1 at 1e2-1e6, two positions each,
and the saved missing-pieces view), the batched walk on the CPU
reproduces the per-node walk's plans **bit for bit**, all 25.

**The GPU's plans** differ where its answers do, at the rim:

| view | CPU words | GPU words | in both |
|---|---|---|---|
| grand-julian 1e2 | 4055 / 2291 | 4059 / 2291 | 4044 / 2291 |
| grand-julian 1e3 | 3422 / 4479 | 3424 / 4482 | 3420 / 4473 |
| grand-julian 1e4 | 6883 / 3782 | 6944 / 3689 | 6746 / 3623 |
| grand-julian 1e6 | 3175 / 2545 | 3408 / 3516 | 1794 / 1972 |
| missing-pieces (546) | 8724 | 8724 | 8721 |
| julian-disc 1e2 | 2631 / 8101 | 2631 / 8100 | 2629 / 8051 |
| random1 1e2-1e4 | 1914 / 1471 / 963 / 1355 / 2365 | 1914 / 1471 / 962 / 1356 / 2371 | 1914 / 1471 / 962 / 1354 / 2337 |

At 1e6 -- past the render's f32 ceiling for this view (§13.2) -- half
the words differ, as the answers do. Completeness is the gate, measured
against an independent chaos game on 3000 in-view samples
(`the_gpu_plans_as_completely_as_the_cpu`): **the GPU's plan covers
exactly what the CPU's does at every view measured** -- 0.9977, 0.9973,
0.9980 and 0.9913 on both sides at 1e2-1e3, and 0.9913 at the saved
view. From 1e4 the independent game cannot measure: 400M iterations land
fewer than 300 samples in the view, and at 1e6 none -- which is why
targeting exists. (A first run reported 1.0000 and 0.9888 at 1e4; those
were fractions of about ninety samples, 0.9888 being one miss in 89, and
the test now declines to report under 300.)

### 14.1 Time, alone

`where_a_plan_spends_its_time`, grand-julian at 1280x720, twelve cores:

| zoom | CPU plan | GPU plan | GPU answering | CPU gathering |
|---|---|---|---|---|
| 1e3 | 297 ms | 94 ms (124 speculative) | 18 ms (34) | 54 ms |
| 1e4 | 319 ms | 77 ms (101) | 13 ms (26) | 47 ms |
| 1e6 | 329 ms | 89 ms (162) | 26 ms (60) | 57 ms |

Alone, the GPU plans 3.5-4x faster, and the answering is no longer the
cost: **gathering candidates on the CPU is now the largest part** of a
GPU plan (phase 4's question). Speculation costs 30-70 ms alone -- five
times the points -- and pays for itself only when round trips are
expensive, which is the next table.

### 14.2 Time, beside the render

`a_plan_on_the_gpu_shares_it_with_the_render` draws grand-julian frames
while the view moves, and times each plan from the moment it starts (the
250 ms settle delay before it is the view's). The planner's batches
share the render's queue, so each waits behind the frame running.

| frames | planner | plans | frames while planning (median / p95) |
|---|---|---|---|
| as the app draws: 128 workgroups (~3 ms), 60 Hz | CPU | 337-416 ms | 4.6 / 18.2 ms |
| | GPU | 137-189 ms | 2.9 / 8.2 ms |
| heavy: ~12 ms back to back | CPU | 367-409 ms | 10.2 / 18.3 ms |
| | GPU | 370-412 ms | 10.2 / 11.7 ms |

**The GPU planner is never the worse choice.** Where the app draws as it
does -- its governor never dispatches more than 128 workgroups, which for
this flame is about 3 ms -- plans are 2.2-2.5x faster and the frames are
smoother (the CPU planner's twelve threads crowd the app's own). Under
the heaviest frames the plans tie and the frames are still smoother.

Measured and not kept:

- **Before speculation**, under heavy frames the GPU plans took 814-983
  ms against the CPU's 616-699 (settle included): 80-100 batches, each a
  frame's wait.
- **A second device** on the same GPU, for a queue of its own: no change
  (plans 778-960 ms). The GPU runs the render's frame first either way.
- **Lighter frames while a plan runs** (an eighth of the dispatch; the
  samples are a preview the plan's arrival resets): 446-496 ms under
  heavy frames. Not needed where the app draws as it does, so not built;
  a flame heavy enough to fill the frame at 128 workgroups is where it
  would matter.

### 14.3 Found on the way (open)

- **Views with no plan -- answered in part.** julian-disc from 1e3 and
  random1 from 1e4 returned `ViewIsEmpty`, and their plans at shallower
  views were badly incomplete (0.311 and 0.879). Both were the walk's
  faults, fixed in `inversive-targeting.md` §31: children the sample
  cannot see were dropped without a replay, and the cloud could not seed
  where the dominant map is nearly neutral. A view holding at least one
  sample point now plans completely (0.968-1.0). A view deeper than the
  sample's resolution still needs the cloud, and for random1 the cloud
  still cannot widen its region there -- open.
- **Coverage cannot be checked independently from 1e4.** The chaos game
  that measures it cannot reach a view that deep often enough. Plans
  there are complete by the walk's own point count (§27 of
  `inversive-targeting.md`) and by the pictures, not by this check.
- **Gathering is the floor.** See §14.1.

---

## 15. Escape time and deep zoom: the theory, measured (2026-09-23)

The theory (§12, decision 4): the flames that convert to escape time are
the ones that can be deep-zoomed. `do_escape_and_deep_zoom_go_together`
asks both questions of every flame in the corpus -- the 45 `.flame`
files in `output/` (43 flames) and the 10 configs in `output/flame-zoom/`:

- **Converts**: mode D's `pack_flame` accepts it, planar or solid.
- **Deep-zooms**: `Cylinders::plan` returns a plan with a speedup above
  one at 1e3 or 1e5 times the flame's own zoom, centred on one of three
  points of its attractor. The points come from the planning kernel
  itself: 256 scattered seeds, each carried through its own random
  48-symbol word by the render's maps, which works for any flame the
  kernel builds -- analysable or not.

| | deep-zooms | does not |
|---|---|---|
| **converts** | 6 | 4 |
| **does not** | 2 | 43 |

(The planar analysis alone, `analyse_2d`, gives the same table. First
measured 5 / 5 / 2 / 43; `julian-disc` moved to the diagonal with the
fixes of `inversive-targeting.md` §31.)

**49 of 55 on the diagonal, and each of the six off it is a gap in one
tool, not a different kind of flame:**

- **Converts, but the forward planner's word cap stops it** (3):
  `cup_(3d)`, `cup_(3d)-nopreservez`, `linear3d-test`, all
  `TooManyWords` (4404-6232 against 4096). They are analysable, so the
  inverse walk could plan them -- but `Cylinders::plan` sends only armed
  flames there.
- **Converts, but the inverse walk returns an empty plan** (1):
  `random1`, at views deeper than its sample resolves -- the cloud
  phase's open problem (§14.3).
- **Deep-zooms, but does not convert** (2): `schottky1`, `schottky2`.
  Four `mobius` transforms, targeted through forward ball bounds; the
  planar analysis does not read Möbius maps yet (`affine_role`'s note:
  conformal invertible maps are "the next candidates").

One caution on reading it: the forward planner works to a time budget,
so under load a flame near it can time out. Run beside the other GPU
tests, `schottky2` once came out "not deep"; alone it is deep. The table
above is from a run alone.

The 43 that do neither are blocked, on the targeting side, by things
both tools lack today rather than by geometry: a DC/RGB colour variation
(`ColourNotAffine`, 20), xaos (8), a variation with no bound or inverse
(`rays`, `cross`, `roundspher3D`, `julian` the analysis refuses: 10), the
forward word cap (3), no invariant ball (1), a planner timeout
(`spherical`, 1). So the
diagonal is partly shared limits -- both tools need each map to be
readable as a map -- and the corpus is small, and mostly flames made to
test other features.

**What it supports**: the property both tools rest on is the same --
every map has something computable about it (an inverse, or a bound on
where it sends a disc) -- and where that holds, both tools can in
principle work. Every disagreement found is a gap to close, not a
counterexample. Closing them is concrete: route analysable flames over
the forward cap to the inverse walk, let the cloud phase seed where a
neutral map will not widen a region, and teach the planar analysis
Möbius maps.

---

## 16. Phase 4: gathers on the GPU (2026-09-23)

After phase 2 and `inversive-targeting.md` §31, gathering candidates on
the CPU was the largest part of a GPU plan: 50-60 ms of wall time on
twelve cores. Profiled, 85% of it was binary searches -- about 3.5
million cell lookups per plan, each two searches in a 100k-entry index,
one per cell per alphabet symbol -- and the rest the cell lists and the
fill.

**Built.**

- **The gather, split** (`backward.rs`): `expand_cells` (a node's cells,
  with neighbours, sorted and deduplicated) is now made once per node
  instead of once per child, and `gather_seen` takes the candidates from
  one index over those cells.
- **`Evaluate::gather_lands`**: plain jobs and gathers asked together. A
  gather is `gather_seen` plus a check of each candidate against the
  child's word; back come its candidate count and the candidates that
  landed. The default (`gather_on_cpu`) gathers on the CPU and asks
  `lands` for everything, so the CPU walk is unchanged.
- **The walk** asks with `gather_lands` wherever it gathered: seeds, and
  top-ups. On a speculative evaluator a level's children are not gathered
  in step 2 at all: their gathers ride in the replays' batch.
- **On the GPU** (`shaders/core/plan_gather.wgsl`, `PlanGpu::attach`,
  `PlanGpu::fused`): the walk's indexes -- every landing index and the
  grid, concatenated, ~30 MB for grand-julian -- are uploaded once per
  flame. One submission then runs the replays, `gather_ranges` (a thread
  per cell of each gather: its run, by two binary searches),
  `gather_scan` (a workgroup per gather: the runs' positions, the total,
  the step), `gather_fill` (a thread per output slot: the entry at
  `slot * step`), and `plan_eval_gathered` (the check, a thread per slot).
  The candidates never leave the GPU; the per-gather counts and, per slot,
  the sample index where it landed come back.

**Exact.** The selection is integer arithmetic, and it is the CPU's:

- `the_gpu_gathers_as_the_cpu_does`: 123 random gathers (cell lists of 1
  to 1500 sample points, with and without neighbours, every kind of
  index, caps from 1 to 16384, an empty list) -- 156,394 candidates, the
  same on both sides, and the same 140,789 landing.
- `dump_plans`: all 25 views plan bit for bit as the GPU planned with CPU
  gathers; the CPU's plans are bit for bit as before the restructure.

**Time.** Grand-julian at 1280x720, twelve cores and a GTX 1660 SUPER:

| zoom | CPU plan | GPU plan, CPU gathers | GPU plan, GPU gathers |
|---|---|---|---|
| 1e3 | 435 ms | 162 ms | 91 ms |
| 1e4 | 490 ms | 143 ms | 79 ms |
| 1e6 | 449 ms | 226 ms | 130 ms |

The gather's wall time fell from 49-69 ms to 5-6 ms (what is left is the
nodes' cell lists, on the CPU). Beside the render
(`a_plan_on_the_gpu_shares_it_with_the_render`):

| frames | CPU plans | GPU plans | frames while planning, p95 (CPU / GPU) |
|---|---|---|---|
| as the app draws (128 workgroups) | 486-602 ms | 120-136 ms | 19.5 / 5.2 ms |
| heavy (~12 ms each) | 507-580 ms | 370-389 ms | 26.9 / 13.1 ms |

With every child replayed (§31) the CPU plans grew by a third; the GPU
plans are now four times faster as the app draws, and faster under heavy
frames too, where in phase 2 they only tied.

**What is left in a GPU plan** is the GPU's own work (27-79 ms submit to
mapped) and packing and reading (25-30 ms). Most of the GPU's work is
the speculative checks: every child's candidates are checked in the
replays' batch, and about nine in ten children are kept and never use
theirs. Deciding keep-or-carry on the GPU, between the replay and the
check in the same submission, and returning replay counts rather than a
byte per point, is the next step if plans need to be faster.
