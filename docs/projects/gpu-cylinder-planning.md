# Planning cylinders on the GPU (plan, 2026-09-22)

A plan, not a record: **no code has been written for any of this.** It
asks for decisions at the end (§11), and phase 0 is a measurement whose
result can stop the whole thing.

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
