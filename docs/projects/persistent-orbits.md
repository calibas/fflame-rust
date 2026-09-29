# Persistent orbits: keeping each thread's chaos-game orbit between dispatches

Status: **planned** (2026-09-28). Not started.

Every compute dispatch today restarts every thread's orbit from scratch.
Where a flame takes longer than the burn-in to forget that start, the
untargeted render is wrong, not just noisy. This plan keeps each thread's
orbit in a GPU buffer, so a dispatch resumes it and only an edit to the
flame restarts it.

## 1. Why: the blue band

`output/flame-zoom/julian-disc-blue1.fflame` (untargeted) and
`-blue2.fflame` (targeted) are one view of julian-disc at zoom 1,598. The
untargeted render has a blue band across the middle. The targeted one
does not, and the targeted one is right.

**The mechanism.**

1. Each dispatch starts every thread at a random point in [-1, 1]² with
   colour coordinate 0.0 (`main_template.wgsl`, top of `main`), and plots
   after `burn_in` iterations: 20, in the app (256 iterations per thread)
   and in exports (1,024).
2. julian-disc's disc transform (weight 15) multiplies the colour by 0.95
   and adds nothing, its colour being 0. Only the julian (weight 0.5, one
   step in 31 on average) raises it. So a restarted orbit's colour stays
   exactly 0 until its first julian step, past the burn-in in more than
   half of the threads.
3. The middle band is where long runs of the disc transform land: in a
   converged chaos game, about 80 disc steps after a julian step, colour
   0.008-0.023 (the saved palette's entries 2-6, dark red). A restarted
   orbit's first disc run crosses the same band at colour 0: entry 0, a
   bright blue about 20 times brighter than those reds.

**Measured** (2026-09-28):

| test | result |
|---|---|
| f64 chaos game on the CPU | the band in entries 2-6; entries 0-1 each under 0.6% |
| float32 chaos game in numpy | identical to f64: not float32 precision |
| the GPU replaying "julian, then n disc steps" | lands at the CPU's step counts, 6,927 against 6,926: not GPU arithmetic |
| the restart scheme in numpy, app settings (256 per thread, burn-in 20) | 18% of band samples at colour exactly 0 |
| the same, export settings (1,024 per thread) | 4.2% |
| the same, burn-in 200 | 0.0% |
| both modes on the GPU, a palette with one colour per range | untargeted: 20% of band pixels in entries 0-1; targeted: none |
| the targeted render, simulated on the CPU | the chaos game's colour and light in every band; the plan covers 99.8-100% of the band |

The targeted render is immune because a forced sample's colour is
`H·c + G`, folded from the plan's word, and the band's words contain the
julian step: `G` sets the colour, not the restarted orbit's `c`.

**The class.** Anything that takes longer than the burn-in to converge:

- colour, where one transform dominates with a high colour speed and a
  rare one moves it (this case);
- position, where a nearly neutral map carries points slowly;
- the per-thread registers of 27 stateful variations (1-7 slots each,
  e.g. `apollonian_gasket`'s and `cubic_julia`'s colour registers,
  `curliecue2`'s point sequence), reset every dispatch today.
  [intra-iteration-state-and-accum.md](../archive/intra-iteration-state-and-accum.md)
  made cross-dispatch persistence a non-goal, on the assumption that such
  state converges fast enough to be invisible;
- the importance-sampling window, whose epochs (plots held back for the
  first `importance_window` of every `2 · importance_window` choices)
  restart at every dispatch.

## 2. What changes

| | today | after |
|---|---|---|
| a dispatch starts a thread | at a random point, colour 0, burn-in 20 | where its last dispatch left it |
| a restart | every dispatch | when the flame's dynamics change, a respawn, or a refresh (§3.7) |
| burn-in paid | every dispatch: 7.8% of the app's iterations, 2% of an export's | once per restart |
| a pan or zoom | no different: every dispatch restarts anyway | keeps them: the attractor does not depend on the view |
| iterations per thread | trades each orbit's depth against breadth | only slices the work into dispatches |

Thread counts: the app's frame-time governor dispatches 1-128 workgroups
of 64 (64 to 8,192 threads) and changes the count between frames; the
headless renderer (`render.rs`) dispatches 128 (8,192), and the high-res
exporter at most 128.

## 3. Design

### 3.1 The state buffer

One entry per thread, for the largest dispatch (8,192 threads):

```wgsl
struct Orbit {
    p: vec4<f32>,       // x, y, z, and point_w (HAS_W)
    colour: vec4<f32>,  // speed-mode rgb, and color_index in .w
    gen: u32,           // the generation it belongs to (§3.2)
    fuse: u32,          // burn-in left
    prev_xform: u32,    // XAOS_ENABLED
    is_window: u32,     // IMPORTANCE_SAMPLING
    is_weight: f32,
    ab_remaining: u32,  // HAS_ANALYTIC_BLUR
    ab_slot: i32,
    _pad: u32,
}                       // 64 bytes: 512 KB for 8,192 threads
```

The stateful variations' slots follow, `8,192 × total_slots` floats,
where `total_slots` is the shader builder's size of `thread_state`
(`shader_builder_v2.rs`). They change only with the shader, and a shader
change comes with a generation change.

**Binding 8**, unused in both layouts. The storage-buffer count rises
from 11 to 12 in `FlameRenderer`'s compute layout (`gpu/pipelines.rs`)
and from 12 to 13 in the high-res exporter's (`export/high_res.rs`, which
also binds `sample_counter` at 6). The device asks for the adapter's own
limit (`gpu/device.rs`; its comment still counts 10). If a target adapter
reports fewer (step 0 checks), the fallback keeps the count: the orbit
buffer takes binding 16 and holds the frame-coverage counters at its head,
as `array<atomic<u32>>`, orbit fields read and written through `bitcast`;
the per-frame clear then covers only the counters.

The buffer survives a resize: orbits do not depend on the view. It is
created zeroed, so every thread's `gen` is 0 until its first dispatch.

### 3.2 The generation

`params.orbit_generation: u32`, carved from `_pad_shadow1` (after
`importance_window`, which was carved from the same pad run), so no offset
moves; mirrored in `GpuParams` (`gpu/buffers.rs`) and `header.wgsl`.
Starts at 1, so the zeroed buffer reads as not started. The renderer
increments it when the orbit key changes (§3.3).

### 3.3 The orbit key: what restarts orbits

Anything that changes the dynamics, or the state the orbit carries: a hash
of the **whole** `Flame` (transforms, finals, linked, subflames, xaos,
weights, colours, colour speeds, direct colour, variations and their
parameters), the render mode, `preserve_z`, the colour mode and the speed
factor. Hashing the whole flame, not a chosen list, because a missed field
leaves orbits on an old attractor, and an extra restart costs one burn-in.
(`flame_key`, which the planner uses, leaves out direct colour and
subflames, so it is not enough.)

Not: the view (zoom, pan, rotation, camera), palette, tone mapping,
effects, plans.

Computed where flame edits land: `FlameRenderer::update_flame` and
`load_config`. A slider drag on the flame changes it every frame and
restarts every frame, as today.

### 3.4 The shader

At the top of `main`, after `rng_init`: read `orbits[thread_id]`. If its
`gen` is `params.orbit_generation`, resume `current`, `point_w`, `color`,
`color_index`, `fuse`, `prev_xform_idx`, the importance window, the
analytic-blur residual and the variation slots. Otherwise start as today:
random point, colour 0, `fuse = params.burn_in`, the unconditioned xaos
pick, the state-init block. After the loop, write them back with
`gen = params.orbit_generation`.

Unchanged:

- the random streams (`rng`, `ct_rng`, `is_rng`), seeded per dispatch
  from `params.seed`;
- the bad-value respawn;
- targeting's forced samples, which restore `current` and `color_index`
  after the plot;
- the per-dispatch counters (`fc_*`).

All of it behind a template flag, `PERSISTENT_ORBITS`, off by default
until step 5, so every gate can compare the two in one build.

**Seeds after a reset.** The deterministic seed is
`12345 + frame_counter · 2654435761`, and `reset_iteration_counter`, which
runs whenever the accumulation restarts, zeroes the counter. After a pan,
a persisted orbit therefore meets the seed sequence it met after the
previous restart, and a contracting orbit driven by the same choices
converges onto the same trajectory: the new accumulation replots the
previous one's first samples, in the new view. They are valid chaos-game
samples, so the picture is right. Step 1 decides whether to keep the
counter running across resets that keep the generation.

### 3.5 Counting plotted samples

Brightness is normalised by `total_iterations`, counted in `compute_pass`
as `threads × (iterations_per_thread − burn_in)`. That is wrong once a
thread's burn-in is paid only when it starts. The high-res exporter has
already had this class of bug: a sample count that did not match its
dispatch made its exports dimmer than the app's at low sample counts.

The replacement: an exact model on the CPU, with no readback. Threads that
start together form a cohort, a prefix range of thread indices with the
burn-in it has left. A dispatch of `N` threads advances only the cohorts
below `N`, and a cohort plots `ipt − min(ipt, fuse left)` per thread. A
generation change starts one cohort of all threads; the governor growing
past every thread started so far starts another. Burn-in longer than
`ipt` spans dispatches. Respawns stay uncounted, as today.

A GPU counter checks the model in a test: each thread adds its plotted
count to one atomic at the end of the dispatch, as the coverage tallies
do.

Also to check: whether the importance window's held-back plots are counted
today. With persistence the window no longer restarts at each dispatch.

### 3.6 The governor

Threads above this frame's dispatch keep their state. When the governor
grows the dispatch again they resume, if their generation still matches,
or start, as §3.5's cohorts describe.

### 3.7 Flames with more than one attractor

With reducible xaos, or separate attracting sets, how threads split
between the parts is drawn when they start. Today every dispatch redraws
it. Persisted, it is drawn once: with 8,192 threads, a part holding 1% of
the orbits gets about 82, a brightness error around 11% that no longer
averages out (and fewer threads when the governor sheds load).

**Refresh**: each dispatch, the threads with
`(thread_id + dispatch_index) % P == 0` restart. Cost `burn_in / (P · ipt)`:
0.12% at P = 64 in the app. It brings back a little of the transient this
plan removes, so P, and whether refresh is always on or only for flames
with xaos, are decided by measurement (step 4).

### 3.8 The high-res exporter

Its own layout, dispatch loop and sample count (`export/high_res.rs`). The
same buffer, binding, generation (one per export) and cohort count, fixed
at 128 workgroups. Without it, large exports would keep the band.

### 3.9 Settings and docs

- **Burn-in** (SystemSettings) now means iterations per restart, not per
  dispatch. Its tooltip and `docs/main/RENDERER.md` say so.
- **Iterations per thread** no longer trades depth against breadth.
- **The frame-time governor** (`app/mod.rs`: `Batch`, `Knee`) shortens
  the dispatch before shedding workgroups, but only under targeting since
  2026-09-28, because until orbits persist a shorter dispatch is a shorter
  orbit. Once they persist, it shortens every dispatch (step 6).
- **CLAUDE.md**'s render-pipeline paragraph is updated.
- **The archived state doc**: its non-goal gets a note that the state now
  persists.

## 4. Open questions, decided by measurement

1. **The starting colour: 0, or random?** Check what the reference
   renderers do from their sources (JWildfire, flam3). Do not assume it.
2. **The first dispatch after a restart.** Its transients stay in the
   accumulator, as `1/k` of it after `k` dispatches. Measure the band after
   1, 10 and 100 app frames. If the first frames are visibly wrong, give
   the first dispatch after a generation change a longer burn-in, at the
   cost of a frame with nothing plotted after each edit.
3. **The refresh period, and when it applies** (§3.7).
4. **The binding** (§3.1).

## 5. Steps

Each a commit, with `python scripts/release.py check` and the gates it
touches.

0. **Before any change**, on the current build:
   - the band's blue-entry share at app and export settings;
   - mean brightness at 64, 256 and 1,024 iterations per thread on a few
     flames (julian-disc, grand-julian, a gasket, one 3D);
   - the visual suite's current state;
   - the storage-buffer limit the adapters report on the desktop build and
     in Chrome and Firefox (the web build logs it at device creation).
1. **The core**: the buffer, the generation, the orbit key, the shader's
   load and store of point, colour and fuse (behind `PERSISTENT_ORBITS`),
   and §3.5's count. Gates: the first three below.
2. **The rest of the state**: xaos previous transform, `point_w`, speed
   colour, the importance window, the analytic-blur residual, the variation
   slots. Gates: stateful registers; importance sampling's bit-identity at
   `q = p`.
3. **The high-res exporter**, §3.8. Gate: the independence gate through
   the tiled path.
4. **Refresh**, §3.7, measured on a flame with isolated xaos groups.
5. **On by default.** Re-baseline the visual suite's 151 flame configs
   (escape-time and simulation are unaffected) and the benchmark hashes.
   Review every image that moves past the tolerance, expecting noise except
   where a flame mixes slowly. Update the docs (§3.9).
6. **The governor shortens every dispatch.** Since 2026-09-28 it cuts
   iterations per thread before workgroups only while targeting is active
   (`Batch::shorten`): julian-disc at zoom 1,598 with 1,000 per thread had
   fallen to one workgroup and 1.1 million samples a second, and shortening
   brought it to 79-87 million at full width. With persistent orbits a
   shorter dispatch costs no orbit depth, so `shorten` becomes true for
   every render. Gate: the independence gate with the governor shortening,
   and the governor's tests (`governor_tests`) with `shorten` on for an
   untargeted batch.
7. **Remove the flag**, or keep it as a setting if a flame is found that
   renders worse. Decided then.
8. **Governor controls in Rendering** (asked for, 2026-09-28; after the
   rest): a toggle for the governor, and a manual workgroup count for when
   it is off.

## 6. Gates

- **The picture no longer depends on iterations per thread.** The same
  flame and total iterations at 64, 256 and 1,024 per thread agree block
  by block. Today they do not where a flame mixes slowly: 18% of the band's
  samples at 256 against 4% at 1,024. This is the general invariant; the
  band is its first case.
- **The band**: `julian-disc-blue1`, untargeted, with one colour per
  palette range: blue-entry share in the band at most the f64 chaos game's
  (under 0.6%).
- **Brightness**: equal mean brightness at 64, 256 and 1,024 per thread,
  and over a dispatch sequence with the governor's count varying; §3.5's
  count equal to the GPU counter, exactly.
- **Restarts**: flame A for K frames, then B, matches a fresh render of B
  block by block; a pan or zoom leaves the generation alone and pays no
  burn-in.
- **Stateful registers**: a `cubic_julia` Branch Blend flame agrees at 64
  and 1,024 per thread.
- **Isolated xaos groups**: each group's share of the light is stable over
  time with refresh on.
- **Unchanged**: the targeted-against-untargeted gates, the sticky
  bit-identity gate, importance sampling's bit-identity at `q = p`, the
  release check, and the full GPU sweep at the end, in the background.
- **Performance**: plotted samples per second up about 8.5% at 256 per
  thread (all 256 plotted instead of 236) and 2% at 1,024; dispatch time
  unchanged within noise.

## 7. Costs and risks

- **Every flame render changes.** 151 visual baselines, and old PNGs do
  not reproduce bit for bit.
- **A wrong sample count shifts brightness**, by up to 8% in the app
  (§3.5).
- **A missed restart leaves stale orbits** (§3.3). Hashing the whole flame
  errs toward restarting.
- **Flames with several attractors average less** without refresh (§3.7).
- **One more storage binding**, or a merged buffer (§3.1).
- **Two kernels to keep in step**: the renderer's and the high-res
  exporter's.
