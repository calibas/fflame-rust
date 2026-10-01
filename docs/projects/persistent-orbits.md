# Persistent orbits: keeping each thread's chaos-game orbit between dispatches

Status: **done** (2026-10-01), branch `persistent-orbits`, steps 0-8:
every renderer keeps its orbits, with no setting to turn them off; the
governor shortens every dispatch; and the Rendering menu can switch the
governor off and set the workgroups by hand. Results in §8.

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

*As built:* each thread owns `ORBIT_WORDS = 16 + ORBIT_SLOTS` consecutive
`u32`s, the slots following its own 16 words rather than a separate
array, and every field is stored as bits (`bitcast`): words 0-3 the point
and `point_w`, 4-7 the speed colour and `color_index`, 8 the generation, 9
the fuse, 10 the xaos previous transform, 11-12 the importance window and
weight, 13-14 the analytic-blur residual, 15 padding, then the variation
slots. `ORBIT_SLOTS` is the shader builder's size of `thread_state`, a
constant in the WGSL that `shader_cache` reads back, so the renderer
knows the stride without asking the builder again. The buffer is made at
the first dispatch, with room for the app's widest (128 workgroups), and
grown when a dispatch is wider; growing drops the orbits and restarts
them. The sketch it was built from:

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

**Binding 8**, unused in both layouts. *Measured* (the layout tests in
`gpu/pipelines.rs`): a plain flame's shader with orbits binds 5 storage
buffers, one with every optional binding on, 9. Since 2026-09-29 `FlameRenderer`'s
compute layout holds only the bindings its shader's WGSL uses
(`gpu::pipelines::used_bindings`; the layout of all of them held 11
storage buffers and a laptop's Chrome allows 10, so no flame rendered
there). So the orbit buffer is one more storage buffer only in a shader
built with `PERSISTENT_ORBITS`, and it needs no gating of its own beyond
its declaration's use: a plain flame's shader uses 4 storage buffers
(transforms, histogram, variation params, xaos), one with every optional
binding on -- PathMap's path ids, auto exposure's counters, the plan,
importance sampling's table -- 8, so 5 and 9 with orbits. 9 is over
WebGPU's minimum of 8 but within the 10 the laptop reports; step 0
records what Chrome and Firefox report on the target machines, and
`a_flame_renders_within_a_browsers_storage_limit` gains the orbit
buffer. If an 8-limit device must run everything at once, the fallback
still applies: the orbit buffer takes binding 16 and holds the
frame-coverage counters at its head, as `array<atomic<u32>>`, orbit
fields read and written through `bitcast`; the per-frame clear then
covers only the counters.

The high-res exporter (`export/high_res.rs`) bound a fixed layout of 12
storage buffers (it also binds `sample_counter` at 6), 13 with the orbit
buffer. *Step 3 gave it the per-shader layout:* the band flame's export
shader binds 5, and 6 with orbits.

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

*Amended after step 8 (reported in the app):* except on a flame that
carries variation state (`ORBIT_SLOTS > 0`). There every restart of the
accumulation -- a pan, a reset, each overwrite-mode frame -- restarts the
orbits too. A variation's state can be a clock rather than a register
that settles: `curliecue2` ignores its input and walks on from its state,
so its picture is the walk since the state started, and kept through a
pan it showed the walk's next stretch instead of a fresh one. JWildfire
initialises a variation's state for every render, and any view change is
a new render. The cost is a burn-in, and a register's settling, per pan,
on those flames only. Gate: `a_pan_restarts_variation_state` -- panned
from A to B, `curliecue2` is exactly a fresh render at B (without the
restart, every lit pixel was off); overwrite frames restart it every
frame.

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

*Decided (step 1): the counter still resets.* A deterministic render
already replots exactly the same samples after every reset -- same seeds,
same random starts -- and that is what makes it deterministic; a persisted
orbit converging onto the previous trajectory is the same thing, less
exactly. A render from scratch, which is what the CLI and the visual suite
make, starts a new generation at counter 0 and reproduces bit for bit.

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

*As built (step 4).* Two cases, told apart on the CPU from the xaos chain
(`Flame::xaos_closed_classes`, `xaos_walk_splits_orbits`):

- **The first pick decides the group** -- isolated groups, the common
  case. A thread starting draws its first transform from a Kronecker
  sequence in its index (`thread_id · 2^32/φ + seed`) instead of
  independently, so every range of threads starting together holds each
  group's share to within a thread or two. No restart, no cost.
- **The walk decides it** -- a transform an orbit can start at leads
  into two or more closed groups. Refresh, with P = 4 and the restarted
  threads chosen by `(thread_id + seed) % P == 0`, so the CPU's count
  (`OrbitFuses`, now one burn-in counter a thread) knows which.

Every other flame -- no xaos, or one closed group -- needs neither: every
orbit ends in the same group.

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
   *Measured (step 1, `dbg_how_long_a_restart_shows`):* the band flame
   from a fresh start at 256 a thread has 11 blocks of blue light off by
   more than 5 standard errors after 1 frame (it is a restarted frame),
   1 after 10, none after 100: a sixth of a second at 60 frames a second.
   No longer first burn-in.
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
   *Found in steps 2-3, to do here:* every place that counts samples must
   count what `compute_pass` says plotted.
   - `render.rs` and the exporter count every iteration dispatched, burn-in
     included, and the live app counts what plotted, so today a CLI render
     normalises by 256/236 more samples than the app shows at the same
     settings. With persistence both count what plotted (behind
     `RenderJob::with_persistent_orbits` and
     `HighResExporter::new_with_orbits` until this step), and the
     difference goes.
   - The app's WASM export loop (`app/mod.rs`, the `temp_renderer`) counts
     as `render.rs` does.
   - The live loop multiplies the last frame's count by the batch size.
     That is wrong as soon as frames in a batch differ -- the first after a
     restart pays the burn-in, the governor changes the width -- so it
     sums each frame's count instead.
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
  unchanged within noise. *Measured (§8): the dispatch costs 1-5% more at
  256 and 1,024 a thread, up to 10% at 64, and plotted samples a second
  rise everywhere -- 4-6% at 256, 32-40% at 64.*

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

## 8. Results

Measured on the desktop (GTX 1660 SUPER, Vulkan), 2026-10-01. The gates
are in `src/renderer/orbit_gates.rs` and compare by density and by each
colour channel's light, block by block over replicates
(`src/renderer/density_gates.rs`), never through the tone map.

### Step 0: before (persistence off)

- **The picture depends on iterations per thread.** The band flame at
  128x128, 8 replicates, equal total work, 64 and 256 a thread against
  1,024: 25 blocks differ by more than 5 standard errors, the worst by 79%
  in density; the blue light is gone at 64 a thread and 20% short at 256.
- **Against long orbits** (16,384 a thread, restarted): 256 a thread has
  19 blocks off, up to 62%, and 75% of the blue light.
- **Visual suite**: 238/238 on Windows at the merge of #120.
- **Storage buffers**: the browsers' limits are still to be recorded on
  the target machines (the laptop's Chrome reports 10).

### Step 1: the core (persistence on)

| gate | result |
|---|---|
| the picture does not depend on iterations per thread | 64 and 256 against 1,024: no block outside noise; density and light ratios 0.9995-1.0042 |
| the band is gone | against the long orbits, 256 a thread: no block off, density 1.0010, red 1.0008, blue 1.0131; 64 a thread: one block, 4.1% |
| a varying dispatch keeps its brightness | the gasket at 64 a thread over 256, 16, 128, 64, 256 and 8 workgroups against 256 each time: ratio 1.0000, z -0.5 |
| the count is the GPU's | the cohorts' count equals the shader's own tally of plot attempts exactly, over 16 dispatches of varying width and length, burn-in spanning dispatches, and an edit |
| an edit restarts the orbits and a pan does not | a pan and a zoom keep the generation and pay no burn-in; an edit starts a new one and pays it once |

**Performance** (`dbg_what_persistent_orbits_cost`): the app's frame, 128
workgroups, compute and accumulate, persistence off then on, alternated
over three rounds.

| flame | per thread | dispatch, ms | plotted samples/s, millions |
|---|---|---|---|
| gasket | 64 | 0.369 → 0.399 (+8.0%) | 977 → 1,315 (+34.6%) |
| | 256 | 0.774 → 0.801 (+3.4%) | 2,497 → 2,619 (+4.9%) |
| | 1,024 | 2.401 → 2.401 (0.0%) | 3,425 → 3,494 (+2.0%) |
| julian-disc (the band) | 64 | 0.321 → 0.333 (+3.9%) | 1,124 → 1,574 (+40.0%) |
| | 256 | 0.550 → 0.568 (+3.3%) | 3,518 → 3,693 (+5.0%) |
| | 1,024 | 1.473 → 1.500 (+1.9%) | 5,586 → 5,593 (+0.1%) |
| random1 | 64 | 0.328 → 0.343 (+4.7%) | 1,099 → 1,527 (+38.9%) |
| | 256 | 0.576 → 0.596 (+3.5%) | 3,358 → 3,520 (+4.8%) |
| | 1,024 | 1.586 → 1.613 (+1.6%) | 5,184 → 5,203 (+0.4%) |
| bubble-3d | 64 | 0.352 → 0.370 (+5.1%) | 1,023 → 1,416 (+38.4%) |
| | 256 | 0.651 → 0.668 (+2.7%) | 2,972 → 3,138 (+5.6%) |
| | 1,024 | 1.855 → 1.892 (+2.0%) | 4,435 → 4,433 (0.0%) |
| gasket + cubic_julia (stateful) | 64 | 0.425 → 0.469 (+10.4%) | 848 → 1,117 (+31.7%) |
| | 256 | 1.005 → 1.050 (+4.4%) | 1,923 → 1,997 (+3.9%) |
| | 1,024 | 3.335 → 3.377 (+1.3%) | 2,466 → 2,484 (+0.7%) |

A dispatch costs a near-constant 0.01-0.04 ms more -- the load before the
first iteration and the store after the last -- so its share falls as the
dispatch lengthens. What a render converges by is plotted samples a
second, and that rises at every setting: the burn-in that every dispatch
paid is paid once. By the rule set for this plan (always on if the cost is
under 10%), persistence is always on; step 7 removes the flag unless a
flame is found that renders worse.

### Step 2: the rest of the state

The shader already persisted it in step 1; these are its gates.

| gate | result |
|---|---|
| stateful registers (`a_stateful_register_does_not_depend_on_iterations_per_thread`): `cubic_julia` Branch Blend at a colour speed of 0.02, 64 and 256 a thread against 1,024 | restarted: red light 32% and 82% of 1,024's, green 2% and 64%, every judged block off by thousands of standard errors; persistent: every ratio 1.0000-1.0003, no block off |
| importance sampling's bit-identity at `q = p` (`a_neutral_bias_renders_what_the_feature_off_renders`, now run both ways) | 0 of 65,536 bytes differ, restarted and persistent |

Density is the same either way on the Branch Blend flame: the register
colours the set, it does not move it. Restarted at 1,024 a thread is not
the truth either -- its red light is 5% under persistent's.

`RenderJob::with_persistent_orbits` turns persistence on for the headless
renderer, so the importance gate can run through it.

### Step 3: the high-res exporter

`HighResExporter::new_with_orbits` builds the exporter's shader with
`PERSISTENT_ORBITS`, binds an orbit buffer for its 128 workgroups, one
generation for its life, and counts what plotted (`OrbitCohorts`), as the
renderer does; `new` keeps restarted orbits until step 5. Its layout and
bind group now hold only the bindings its shader uses.

| gate | result |
|---|---|
| the exporter does not depend on iterations per thread (16 replicates) | restarted: 64 a thread has no blue light and 72% of the density (it counts the burn-in), 256 has 15 blocks off up to 34%; persistent: 64 and 256 against 1,024, no block past 2.7 standard errors |
| an export is the app's picture (first run as "counts what the app counts", whole view only -- see the retraction below) | the whole view's density 1.002-1.003 of the app's, red 1.002-1.003, blue 1.01-1.02; restarted at 1,024 a thread the export reads 0.981 (the burn-in it counts); block by block once the harness passed the rotation |
| storage buffers | the band flame's export shader: 5, 6 with orbits (13 before) |

**Tracker P11 is closed by persistence.** Targeted julian-disc at 1e3,
`dbg_orbit_length_by_density`, against 4,096 a thread restarted:

| iterations a thread | landed plan: worst dense block, restarted | persistent | plan on the way: restarted | persistent |
|---|---|---|---|---|
| 64 | 18% (234 blocks past 5 standard errors) | none past 5 | 18% (252) | 0.7% (13) |
| 256 | 4.1% (106) | none | 4.0% (198) | 0.3% (1) |
| 1,024 | 0.8% (7) | none | 0.8% (26) | 0.4% (2) |

The residue on the way is the reference's own: it restarts every 4,096
iterations.

**Found on the way, not this plan's, not fixed:**

- ~~**An export's picture of a deep view is not the app's.**~~
  *Retracted (2026-10-01): a harness error, not the exporter.*
  `render_blocks` drew the app's side with rotation 0 and no camera,
  while the exporter used the config's: the band flame is rotated by -4
  degrees, and a rotated picture against an unrotated one is dense blocks
  59% apart with the whole view equal. With the view passed whole, the
  export is the app's picture block by block (`an_export_is_the_apps_picture`:
  no block past 3 standard errors, whole view 1.0007). The same fix gives
  each replicate's fresh orbits a warm-up (`Run::warm`, 4,096 iterations
  a thread unaccumulated): with two frames a replicate, the restart's
  transient was half of one render and an eightieth of the other it was
  compared with.
- ~~**`rng_nextf` returns exactly 1.0**~~ *Fixed (2026-10-01).* `f32(u32)
  rounded the top 128 values up to 2^32, one draw in 2^25, so
  `rng_nextf() < opacity` dropped a plot at opacity 1 and `u32(rng_nextf()
  * n)` -- 109 call sites in the variations -- could be `n`. It is now the
  top 24 bits, which an f32 holds exactly: at most `1 - 2^-24`. The
  targeting word draw had the same conversion and takes the same. Every
  render's exact pixels move, by noise: the visual suite stays within
  tolerance (330/330, no baseline rewritten), and the count gate is exact
  again -- 20 runs of about 30 million draws each, where the old
  conversion dropped about one plot a run. A draw of exactly 0 is now one
  in 2^24 rather than 2^32; every `log` of a draw in the variations is
  already guarded (`max(u, 1e-30)` or `u + 1e-10`).
- **The exporter's GPU histogram is u32, scaled by 100**: a pixel holding
  more than 43 million samples wraps. Seen only at 34 billion samples on
  a 128x128 gasket.

**The gates' statistics.** With persistence, replicates rendered by one
renderer continued each other's orbits, so their spread understated the
noise; `render_blocks` now restarts the orbits each replicate. Colour
light is tiered by its own reference (a block dense in density can hold
almost no blue). At 8 replicates, 5 standard errors is crossed by chance
about once a run; the gates use 16. The band gate holds blocks to 10%, and
blue to the whole view only: its reference restarts every 16,384
iterations, and the light a restart makes -- the blue -- shows in single
blocks of it, up to 6% in density and 48% in blue, where persistent orbits
at 64 and at 256 a thread agree with each other to 4 standard errors
(`dbg_short_dispatches_precisely`).

### Step 4: closed xaos groups

`dbg_isolated_groups_share`: the right group's share of the density over
24 fresh renders of 256 a thread, for a group holding 1% of the weight.

| flame, arm | 128 workgroups, 64 frames | 16 workgroups, 64 frames | 128, 256 frames | 16, 256 frames |
|---|---|---|---|---|
| isolated, restarted | +-1.1% | +-4.1% | +-0.6% | +-2.1% |
| isolated, persistent, independent first picks | +-9.9% | +-31.8% | | |
| isolated, persistent, stratified first pick | +-0.9% | +-5.5% | +-0.9% | +-6.0% |
| hub, restarted | +-1.1% | +-4.0% | +-0.6% | +-2.1% |
| hub, persistent, no refresh | +-10.8% | +-26.9% | +-13.8% | +-38.5% |
| hub, persistent, refresh every 4 | +-2.8% | +-9.0% | +-1.7% | +-4.8% |
| hub, persistent, refresh every 16 | +-7.0% | +-22.5% | +-4.6% | +-11.5% |

Every mean is the weight share, 0.0094-0.0111. The stratified pick's
spread at 16 workgroups does not shrink with frames: 1,024 threads hold
10 or 11 of a 1% group, a floor of +-5% for as long as the governor runs
that narrow; the threads above keep their orbits, and the full 8,192
holds the share to +-1%. Refresh every 4 costs `20 / (4 · 256)`, 2% of
the iterations, on the flames that need it, and brings back a quarter of
a restarted render's transient on them.

| gate | result |
|---|---|
| isolated groups keep their share | isolated +-1.1%, hub +-3.8% (bar 5%), means within noise of 1% |
| the count is the GPU's, now through a refresh (the hub) | exact but for the opacity draw, 24 dispatches |
| closed groups on the CPU (`closed_class_tests`) | no xaos, isolated, bridged, a one-way split, an unreachable group |

### Step 5: on by default

`FlameRenderer`, `RenderJob` and `HighResExporter::new` keep their
orbits; restarted orbits remain as the gates' baseline only. Every place
that counts samples counts what `compute_pass` says plotted: `render.rs`,
the exporter, the app's WASM export loop, and the live loop, which now
sums a batch's frames. The shader cache's start-up shader is built with
orbits, so the first config load is not a rebuild.

**The visual suite**: 324 of 330 images within tolerance (they change by
noise: every sample's path differs). The 6 past it, each reviewed against
its baseline:

| image | change | why |
|---|---|---|
| `2d-simple-linear` | a filled square becomes 8,192 dots | its one transform (`a = d = 0.5`, `linear` 2) is the identity: no attractor. Restarted orbits drew their random start square; persistent ones keep their fixed points |
| `variations-curliecue2-smoke` | a small pentagon becomes rays across the frame | `curliecue2` walks 0.001 a step from per-thread state: reset each dispatch, it never got past 256 steps; kept, it walks the whole render, as JWildfire's per-thread state does |
| `variations-hypertile-poincare` | the disc's interior glow dims, the light on the boundary circle | restarted orbits started inside the disc and plotted on their way out |
| `2d-plastic-sierpinski` | colour and brightness move in the streams | an identity-affine transform: slow mixing |
| `3d-cpow3_wf-smoke`, `variations-watchlist-misc-smoke` | mean difference 3.0 and 2.07 against a limit of 2.0 | noise on sparse flames |

Baselines updated. Storage buffers, with orbits on by default: a plain
flame at WebGPU's minimum of 8 binds 5; everything at the laptop's 10
binds 9 (`a_flame_renders_within_a_browsers_storage_limit`).

### Step 6: the governor shortens every dispatch

`Batch::shorten` is gone: every render's dispatch shortens first, to 64
iterations a thread, then sheds width. The floor had to move with it --
the governor's smallest scale was a fixed 1/256 of the full batch, which
once the dispatch shortens first is 8 workgroups of 64 at 1,000 a thread
(the governor test for heavy flames, a workgroup 20 ms, caught it at 164
ms a frame); it is now one workgroup of the shortest dispatch
(`Batch::min_scale`). The knee tests now use frames over budget even at
the shortest dispatch, the case the knee is for once shortening comes
first.

| gate | result |
|---|---|
| `governor_tests` (14), every batch shortening | pass; the floor is one workgroup of 64 |
| a varying dispatch keeps its brightness: widths 256, 16, 128, 64, 256, 8 and lengths 64, 256, 1,000, 64, 20, 512 against 256 x 64 | whole view 1.0000, no block off |

### Step 7: no setting

The rule set for this plan was: always on if the cost is under 10%. It
costs nothing measurable -- plotted samples a second rise at every setting
-- and no flame was found that renders worse: every image that moved in
the visual suite moved toward the long-orbit picture the reference
renderers draw, except the identity map, which has no attractor to draw
either way. So there is no setting. `set_persistent_orbits`,
`RenderJob::with_persistent_orbits` and `HighResExporter::new_with_orbits`
are test-only (`#[cfg(test)]` or crate-private): restarted orbits remain
as the gates' measured baseline.

The `PERSISTENT_ORBITS` codegen flag stays in `ShaderConstants`. Every
renderer sets it; the harnesses that build a flame shader without running
the chaos game -- the variation probe, the bounds analysis -- leave it
off, and bind no orbit buffer.

### Step 8: governor controls in Rendering

Two system settings, `frame_governor` (default on) and
`manual_workgroups` (default 128, the governor's full batch), through
`ConfigPath::SystemFrameGovernor` / `SystemManualWorkgroups`, and two
items in the Rendering menu: a Frame-Time Governor checkbox, and a
Workgroups submenu (1 to 1,024) that is enabled while the governor is
off. Off, every frame dispatches that many workgroups at the full
iterations per thread (`Batch::fixed`), and the governor neither measures
nor adapts; while a tight plan is made the dispatch is still capped to
the shortest one, which is the planner's cap (tracker P10), not the
governor's. Neither setting resets the accumulation: orbits persist, so
how the work is sliced changes nothing. Past 128 workgroups the orbit
buffer grows once, restarting the orbits. Reset to Defaults resets both.
The new strings are in English only (`locales/en.yml`); the other
languages fall back to it. The compact (mobile) menu does not carry them.
