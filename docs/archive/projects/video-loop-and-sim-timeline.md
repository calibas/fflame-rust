# One video loop, and a simulation that follows the timeline

**Status: ARCHIVED 2026-09-09 — done.** All four phases built and
gated, plus the review pass in §5. Kept as the record of why the video
path and the simulation timeline are shaped this way. Anything still
outstanding is under **What is still open** below.

**Was:** plan of record, 2026-09-09, branch `simulation-mode`.
Follows the two animation fixes of the same day (`ed5e12b9`, an
integer track dropped on interpolation; `d80796a9`, a simulation video
frame running the flame's chaos game first).

## What was built

| Phase | Commit | Landed |
|---|---|---|
| 1 | `1869600e` | `RenderEngines` on the job; `plan_steps` and `advance_to`; the CLI's simulation export stops being quadratic |
| 2 | `1d12a4d8` | One video loop for every mode; the fast loop and its copy of the flame section deleted |
| 3 | `092f9d64` | The timeline drives the grid in-app, budgeted, with the hold rule |
| review | `58bd910c` | Three defects found by driving the real app (§5) |
| 4 | this commit | EXPORT.md, UI.md, `simulation-fractals.md` D5; archived |

**Measured, before → after:**

| | before | after |
|---|---|---|
| CLI sim video, 0→20,000 steps ×100 frames | 18.3 s | 4.6 s |
| App sim video, 720p ramp | 17.24 ms/frame | 13.85 ms/frame |
| Flame video, 1e9 iterations | 430 ms/frame (old fast loop) | 431 ms/frame |
| Density effects in video | dropped, silently | bit-identical to PNG |
| Solid depth-of-field in video | dropped, silently | matches PNG within the depth race |
| `Sim.Steps` track in-app | moved a cap, changed nothing | drives the grid |

**Five things came out that this plan did not predict**, all recorded
in the commits and in §5 below:

1. **Density effects were dropped for a SECOND reason.** This document
   said the CLI loop ran them because it goes through the still path.
   It does call the stage; the stage declined every time, because the
   export device never requested `FLOAT32_FILTERABLE`. Both loops
   dropped them, by independent mechanisms, and unifying alone would
   have fixed one. The gate caught it only because it required the
   effect-on and effect-off videos to DIFFER rather than trusting the
   call existed.
2. **The hold did not hold.** The rule refused falling targets
   correctly and then the driver fell through to the transport, where
   Run was still engaged. Found by driving the real app, not by the
   unit tests — it is about who ELSE may move the grid.
3. **Ownership was too broad**, and would have frozen a free-running
   simulation whenever any animation played.
4. **Per-dispatch progress cost 18% of a flame frame** on the CLI.
5. **A test error, not a code error, cost the longest detour**: a
   comparison animation pinned zoom to 1.0 while the config used 1.5,
   so a solid render looked 42% dark. The lesson is the same one as
   (1) — a gate is only worth what its inputs are.

## 0. What is being asked for

Three things, in the user's words, lightly condensed:

1. *"Do we need a fast loop? Can't we just exclude unused features from
   the loop dynamically? I don't want different routes if we don't
   need them."* — One video export loop, not two. Whatever it does not
   need for the current mode, it skips by checking the config, the way
   the still path already does.
2. *"I don't care if we overshoot on flame renders, especially if it
   means things are faster."* — Settles the one real divergence
   between the two loops (§1.3). The still path's dispatch shape wins.
3. *"Can we make the Animation panel scrubber and playing a step track
   in-app functional as part of this as well?"* — and, on reflection,
   *"Rewinding the scrubber might be awkward."* It is; §3 says how it
   is handled.

## 1. What is there now

### 1.1 Two video loops

| | `export_animation` (`src/animation/export.rs:1399`) | `export_animation_fast` (`:1902`) |
|---|---|---|
| Called by | the CLI, `export-animation` | the app's export dialog |
| Per frame | `crate::renderer::render()` — a FRESH `FlameRenderer` and a fresh engine, through the still path | its own copy of the still path's flame section, then tonemap, colour effects, readback |
| Renderer across frames | new each frame | one, kept |
| Simulation across frames | reseeded and rerun from zero each frame — **quadratic**; the form D5 in `simulation-fractals.md` rejects | one `SimRenderer`, advanced to the frame's `sim.steps` |
| Density effects | **never run** -- it CALLS the stage, but the device never asked for `FLOAT32_FILTERABLE`, so the stage declined every time (found in phase 2) | **never run**: the stage is not called at all |
| Solid-mode DoF pass | run | **never run** |
| Device features | `CLEAR_TEXTURE` only | `CLEAR_TEXTURE` only |
| Final dispatch | full batch (overshoots by up to one dispatch) | trimmed to the target (`workgroups_for_remaining`) |
| ffmpeg | written from the render thread | writer thread, `sync_channel(16)` |
| Lines | 348 | 699 |

The fast loop is not a different algorithm. It is the still path's
flame section pasted into a frame loop, plus two things the still path
lacks: state kept across frames, and a writer thread so ffmpeg's pipe
never stalls the GPU. Everything else is duplication, and the
duplication is where the bugs have been: it was copied before density
effects and the solid DoF pass existed and never got them; the escape
arm was added to both loops; the simulation arm was added to both and
gated in only one. Every stage added to the tail from now on would
have to be added twice.

### 1.2 What the still path already offers

`render_with` (`src/renderer/render.rs:272`) renders into a
**caller-owned** renderer and exists precisely so a caller looping
over many renders can reuse one — `shader_bench` and the sticky tests
use it that way. Every stage in its tail checks its own config and
costs nothing when off: that IS "excluding unused features
dynamically". What it cannot do today is keep an engine alive:
`render_sim` (`:658`) and `render_escape` (`:833`) create theirs per
call, and `render_sim` seeds and runs all of `steps` every time.

### 1.3 The one divergence, decided

The still path dispatches `NUM_WORKGROUPS = 128` every batch, so a
render overshoots its target by up to one dispatch (up to 8.4 M
iterations at the export default of 1024 per thread). The fast loop
trims its last dispatch to land on the target. Unifying on the trimmed
form would move all 330 visual baselines (the visual configs ask for
1,000 iterations and get millions). **Decision: the untrimmed form.**
The user does not care about the overshoot; it keeps every baseline
where it is; and the trimmed form's own comment records that it was
added to fix a per-frame budget that no longer exists in that shape.
`workgroups_for_remaining` and its tests go with the fast loop.

### 1.4 The simulation in-app today

The frame driver (`src/app/mod.rs:2672`) advances the grid by
`steps_per_frame` while `sim_running`, by 1 on a Step, else 0, and
`sim.steps` is a **cap**: reaching it pauses; 0 means uncapped. The
animation controller writes track values with `update_param_silent_on`
every display frame (`src/app/animation_update.rs:154`), and a scrub
does the same on every slider event (`src/app/ui_handlers.rs:1292`).
So with a `Sim.Steps` track:

- **Playing:** the cap moves each frame. If Run is engaged the grid
  advances by `steps_per_frame`, unrelated to the track; if paused,
  nothing moves. The picture does not follow the timeline.
- **Scrubbing:** the cap moves; nothing else happens.
- At t = 0 the track writes 0, which the driver reads as *uncapped*.

Video export, by contrast, treats `sim.steps` as a **target**: advance
to it, reseed if it went down. The same field means two things on two
paths — recorded as an inconsistency on 2026-09-09 and resolved here
(D4).

## 2. Decisions

- **D1 — One loop, built on `render_with`.** The video loop owns a
  device, one `FlameRenderer` (sticky off, as `render()` does — a
  one-shot renderer gains nothing from it and exports must render the
  specialised shader their reproducibility rests on), the engine
  state, and the ffmpeg writer thread. Per frame: evaluate tracks,
  apply to a copy of the config, `render_with`, send the pixels.
  The app and the CLI call the same function. `export_animation_fast`
  and its copy of the flame section are deleted, not kept "for
  performance": the phase-2 gate (§4) is the measurement that says they were
  not needed.
- **D2 — Untrimmed dispatch.** §1.3.
- **D3 — Persistent engine state travels on the job.** `RenderJob`
  gains an `engines: Option<&mut RenderEngines>` slot holding
  `Option<SimRenderer>` and `Option<EscapeRenderer>`. Given one,
  `render_sim` advances the existing renderer instead of building and
  seeding a new one; `render_escape` reuses its renderer (and its
  allocations) across frames. Without one, both behave exactly as
  today — the still path, thumbnails and the gallery see no change.
  This is what makes the CLI's simulation video stop being quadratic,
  as a side effect of using one loop.
- **D4 — Under the timeline, `sim.steps` is a target.** One function
  on `SimRenderer` owns the rule:

  ```
  advance_to(device, queue, cfg, target, budget) -> reached
  ```

  Reseed if `target < step_index` (the rule is not invertible; D5 of
  `simulation-fractals.md` already says so), then step toward
  `target`, at most `budget` steps this call (`None` = all of them),
  and report whether it arrived. The exporter calls it with no budget.
  The app calls it with a per-display-frame budget whenever **the
  timeline owns the step count**: the controller is playing, or a
  scrub has been applied and the grid has not yet reached it. Outside
  those, the driver is unchanged — `steps` is the cap, Run/Pause and
  Step work as they do, and the run holds where the timeline left it.
  Under the timeline the value is literal: a track at 0 is the seed,
  not "uncapped".
- **D5 — Catch-up is budgeted, never blocking.** `run_steps` already
  measures milliseconds per step to size its submissions
  (`src/sim/renderer.rs:2495`); the budget is derived from that
  measurement and a frame-time allowance (the escape renderer's
  chunk-time idea, ~8 ms interactive). A target the grid cannot reach
  this frame is reached over the next ones, with the picture
  recoloured every frame on the way. The UI never waits on a
  simulation.
- **D6 — Playback is paced by the grid, not the clock.** The escape
  mode's settle-then-jump rule (`escape_playback_tick`) generalises:
  hold the controller while the grid is below its target, then
  advance it by everything that elapsed. A model that cannot keep up
  plays slower; it does not skip steps, and every displayed frame is
  the state at a real timestamp — which is what the export will show.
  The helper is renamed to say it serves both engines; its test
  stands.
- **D7 — Rewinding applies on release.** The awkward case. While the
  scrubber is being dragged, other tracks apply live as they do now.
  The step target applies live only while it is **ahead** of the
  grid; a target **behind** it is held until the drag ends
  (`animation_seek_drag_stopped`), then one reseed-and-catch-up runs
  under D5. Without this, a leftward drag reseeds on every slider
  event whose target has dropped below the index the last reseed has
  reached, and the picture flickers through restarts for the whole
  drag. With it, the picture holds during the drag and restarts once.
  The Simulation panel's step readout says what is happening
  (`step 340 / 1500`, "restarting" while a rewind catches up), so a
  held picture is not a frozen one.

- **D8 — Ping-pong never plays the grid backwards.** Loop modes are
  in-app only (the exporter renders 0..duration once), and PingPong
  reverses `direction` at each end (`src/animation/controller.rs:122`).
  On the backward leg a `Sim.Steps` track's target falls every frame,
  and under D4 alone every frame would reseed — a restart per frame
  for the whole leg. So D7's hold generalises: **a backward step
  target is held while the motion is continuous** — a drag in
  progress, or playback in EITHER direction (D9) — and applied once
  by a discrete event: drag release, the `Loop` wrap, or the
  direction flipping back to forward at t = 0. On a ping-pong the grid therefore runs
  forward to the end state, holds it while every other track plays
  back, and restarts once at the turnaround. `Loop` mode needs no
  rule: the wrap from `duration` to 0 is one restart per cycle, which
  is the right picture for a run that starts over. The driver's
  decision is one pure function taking `(index, target, motion:
  Continuous | Discrete)`, tested alongside D5's.

- **D9 — An authored reverse plays correctly; it is just not free.**
  A track that goes DOWN (2000 → 0) under forward playback is not
  ping-pong: time is moving forward and the target is what the user
  wrote, so D8's hold does not apply.
  - **Export:** every frame whose target is below the grid reseeds and
    reruns to it. Total work is the sum of the targets — ~300,000
    steps for 2000 → 0 over 300 frames, against 2,000 forward; from
    the 2026-09-09 measurement, roughly 50 s at 720p and 2 min at
    1080p for that model, and quadratic in the step count. Correct,
    and the ETA reports it. *Optional, if it ever bites:* the state at
    step N does not depend on which frame asked for it, so frames can
    be rendered in ascending-target order (one forward run, no
    reseeds) and handed to ffmpeg in time order — at the price of
    buffering the frames (~2.5 GB raw for 10 s of 1080p at 30 fps, so
    a temp file). Not built now.
  - **In-app: playback never runs the grid backwards. Only the
    scrubber does.** (Decided 2026-09-09 over the alternative of
    paced-but-correct playback: a slow preview is painful to watch and
    does not represent the export anyway.) So D8 is the whole rule,
    and it is one rule: **under playback, whatever the direction, a
    target below the grid is held**; a backward target is applied
    only by a discrete event — scrubber release, the `Loop` wrap, the
    ping-pong turnaround at t = 0. An authored 2000 → 0 track
    therefore plays as a still of the 2000-step state, restarting at
    the wrap; dragging the scrubber to a time is the way to see the
    state at that time. The Simulation panel's readout says so while
    it holds: *held — the track goes backwards; drag the scrubber to
    preview*.
  - **Caveat, inherent:** a rerun uses the CURRENT frame's parameters
    for all of its steps. If a model parameter animates at the same
    time, the forward run's history had it changing along the way and
    the rerun's does not, so a reversed track retraces the forward
    pictures exactly only when the parameters are constant. *Done:*
    the Animation panel's `Sim.Steps` hover text (which needed the
    target picker to gain tooltips at all) and EXPORT.md.

  **Considered and not done now:** keeping periodic field snapshots
  so a rewind restores the nearest earlier state and runs forward
  from it. A snapshot is `grid × layers × 16 B` — 14.7 MB per layer at
  1280×720 — so twenty of them is hundreds of MB, and the honest
  default would be "off". It is the right next step if rewinding
  under D7 still feels slow on real models; it is not needed to make
  the scrubber correct.

## 3. What the user will see

- **Export dialog, any mode:** the same pixels a PNG export of that
  frame's config gives — density effects and solid DoF included.
- **Export dialog, simulation:** the same 17 ms/frame as after
  `d80796a9`, or better.
- **CLI `export-animation`, simulation:** no longer quadratic.
- **Animation panel, play:** the grid follows the `Sim.Steps` track.
  If the model is heavy, playback slows to what the grid can do rather
  than showing the wrong step.
- **Animation panel, scrub right:** the grid follows the slider,
  catching up over a few frames if it has to.
- **Animation panel, scrub left:** the picture holds during the drag;
  on release the run restarts and catches up, with the step readout
  counting.
- **Ping-pong:** the grid runs forward, holds its end state while the
  other tracks play back, restarts once when playback turns forward
  again. Loop: restarts once per cycle.
- **A track written backwards (2000 → 0):** in-app playback shows the
  2000-step state and holds it, with the readout saying why; drag the
  scrubber to see any point of it. Export renders every frame
  correctly, at a cost that grows with the square of the step count;
  the ETA says how long.
- **Run / Pause / Step / Space** while the timeline is playing a step
  track: inert — the timeline owns the step count, and Run is
  disengaged when playback starts. When it stops, they work again from
  wherever the grid is. An animation with no step track leaves the
  transport alone: the run keeps going under it.

## 4. Phases 1–3

Each phase is one commit and leaves the app working.

### Phase 1 — `advance_to`, and the exporter uses it

- `SimRenderer::advance_to` (D4), built from `will_reseed`, `seed`,
  `run_steps`, `color`. The reseed/budget decision is a pure function
  with a unit test: given `(step_index, target, budget)` it says
  `(reseed, steps_this_call, reached)`.
- `RenderJob.engines` (D3); `render_sim` and `render_escape` use the
  slot when given.
- **Gate:** CLI `export-animation` on the ramp in `output/simvideo/`
  (0→2000 steps, 40 frames) — per-frame time flat rather than
  climbing, pixels identical to the current CLI's output (which is
  correct, only slow). The visual suite unchanged (still path
  untouched when no slot is given).

### Phase 2 — One video loop

- Rewrite `export_animation` (D1): owned renderer, `RenderEngines`,
  writer thread from the fast loop, `render_with` per frame. The app's
  dialog calls it. Delete `export_animation_fast`,
  `render_frame_to_completion`, `workgroups_for_remaining` and their
  tests. Keep the timing summary.
- **Gates:**
  - A flame config with a density effect enabled and a 3D solid config
    with DoF: one-frame video frame == PNG export of the same config
    (frame-grab from the mp4 at CRF 0, or read the RGBA before ffmpeg
    in a test hook), compared at a tolerance that only the codec
    explains.
  - The sim ramp on the app's path: ≤ 17 ms/frame (measured
    2026-09-09 after `d80796a9`).
  - An escape animation renders as before (same frame count, same
    settle behaviour, accumulated antialiasing still applied).
  - Every existing animation unit test passes or is deleted with the
    code it tested.

**What phase 2 turned up that this plan had wrong.** The table above
originally said the CLI loop ran density effects, because it renders
through the still path and the still path has the stage. It does call
the stage. The stage declined: density effects bilinear-sample the
Rgba32Float accumulation, which needs `FLOAT32_FILTERABLE`, and the
animation-export device asked only for `CLEAR_TEXTURE` -- while
`app/export.rs`, `export/high_res.rs`, `gpu/device.rs`, `shader_bench`
and `probe/run.rs` all request it when the adapter advertises it. So
BOTH loops dropped density effects, for two independent reasons, and
unifying them would have fixed only one. The device now asks for the
same features every other export path does.

The lesson for the gate: "the code calls the stage" is not evidence
the stage ran. The measurement is. Turning the effect off and
requiring the two videos to DIFFER is what caught this -- the first
run of that gate, on the unified loop, still showed zero difference.

### Phase 3 — The timeline drives the grid in-app

- Frame driver (`src/app/mod.rs:2672`): when the timeline owns the
  step count, call `advance_to` with a budget (D5) instead of the
  running/step-once logic; otherwise unchanged.
- `animation_update.rs`: generalise the pacing helper (D6); the
  simulation reports "at target" the way escape reports "settled".
- Scrub handling (`ui_handlers.rs:1292`) and playback direction: D7
  and D8 — one hold rule for a backward target under continuous
  motion, released at drag stop or the forward turnaround.
- Simulation panel: the step readout shows target and progress while
  catching up; transport greyed while the controller is playing.
- **Gates:** pure-rule tests for the budget/reseed decision and for
  the hold rule (drag, and ping-pong's backward leg); then in-app,
  the scenarios in §3 by hand.

## 5. What the review of phases 1–3 found

**(2026-09-09, commit `58bd910c`.)** A startup hook
drove the real app through four animations and logged the grid every
frame. Three things fell out, two of them bugs the pure-rule tests
could not see because they are about who else is allowed to move the
grid:

1. **The hold did not hold.** On ping-pong's backward leg and on a
   track written to count down, the rule refused every falling target
   correctly -- and then, with nothing committed, the driver fell
   through to the transport, where Run was still engaged from before
   playback started. The step count climbed past 2,000 during the
   "hold", and after playback ended the run kept going under a button
   nobody had pressed. Fixed two ways: playback with a step track now
   DISENGAGES Run when it starts, and ownership (`timeline_owns_sim`)
   is "playing with a step track, or still walking to a target" --
   wider than "a target is committed" -- so the driver, the panel's
   greying and the spacebar all treat a held leg as owned.
2. **Ownership had been playback alone**, which would have taken the
   grid from a free-running simulation the moment Play was pressed on
   an animation that only sweeps a colouring parameter. Now it needs a
   `Sim.Steps` track. Verified: with no step track, playback leaves
   the run free-running and it pauses at its cap as before.
3. **Per-dispatch progress cost 18% of a flame frame on the CLI.** The
   new adapter forwarded every flame dispatch (about 120 a frame at a
   billion iterations) to a reporter that prints and flushes each
   time: 505 ms a frame against 415 ms with reporting off. Rate-limited
   to one report per 100 ms; 431 ms after.

And what it confirmed: the interactive budget does not spiral -- the
measured step cost climbs as the batch shrinks, because the fixed
submit-and-wait overhead is amortised over fewer steps, and converges
where overhead plus work fills the 8 ms; the budget stayed well above
the eight steps a frame the ramp needed. Pacing engaged exactly once,
for four frames, while the reversed track's first target was reached.
A persistent escape renderer gives bit-identical frames for identical
configs, and a zoom-track frame matches a PNG of its own config
bit-for-bit once that config carries the same `f32`-rounded value the
apply path produces (pre-existing: animation values are `f32`). The
unified loop renders a billion-iteration flame frame in 431 ms against
the old fast loop's 430 ms.

## 6. Phase 4 — Record it

- `simulation-fractals.md` D5 status paragraph: replace the
  2026-09-09 note with the outcome. Add D4–D7 here as the reference.
- `docs/main/EXPORT.md` and `docs/main/UI.md`: one video loop; the
  simulation under the timeline.
- Archive this document with a "what is still open" table (snapshots
  under D7 go there).

## 7. What is still open

- Whether the app's dialog should offer the trimmed-dispatch behaviour
  as an option. Not planned: D2 says no one wants it.
- Field snapshots for fast rewinding (D7). Wait for the complaint.
- Ascending-target frame ordering for reversed tracks in export (D9).
  Same: wait for the complaint.
- The `f32` apply path. Animation values reach the config as `f32`,
  so an animated `Escape.ZoomLog2` (an `f64` field) differs from the
  same number typed into a config by about one part in ten million.
  Pre-existing, harmless to look at, and confusing exactly once: a
  video frame will not be bit-identical to a PNG of the "same" config
  unless that config carries the rounded value. Found while verifying
  phase 2; not fixed, because widening the animation value type
  touches every track.
- The `sim.steps == 0` "uncapped" sentinel outside the timeline is
  unchanged. A control that reads 0 as "no cap" and a track that
  reads 0 as "the seed" share a field; D4 draws the line at "who owns
  the step count", which is workable but not pretty. If it bites, the
  cap becomes its own field.
