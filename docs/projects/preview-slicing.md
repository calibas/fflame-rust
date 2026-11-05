# Previews a frame at a time (option D)

**Status:** planned 2026-11-04, not started (the user: "write up a plan
for D and save it in the projects, but we wont work on it yet").

This is option D of the thumbnail discussion of 2026-11-04. A and B
were built on branch `thumbnail-previews`, and C was declined. With A
and B, a gallery shows its cards at once and makes its previews one at
a time, the visible ones first, and a preview costs a quarter of a
thumbnail or less. Every preview is still one blocking render, though,
so a frame waits for all of it. This plan makes a preview a job that
advances a few milliseconds a frame, so no single frame waits for a
whole render.

## Why

The facts, from the code and the survey behind the discussion:

**Desktop.** The app renders a preview after the UI and the viewport
are submitted, and before the frame is presented. The render blocks
inside `renderer::render`, in `device.poll(PollType::Wait)` between:
- a flame's dispatch batches (`render_with`'s `while total_rendered < target` loop);
- an escape picture's chunks (`while !settled`);
- a simulation's batches of 512 steps;
- a terrain's sections;
- the tail's readback (`finish`).

**Web.** A browser has no blocking wait, so `poll(Wait)` returns at
once. A spawned preview therefore encodes its batches back to back on
the page's only thread, until it reaches an awaited readback.

**What B left alone.** B made flames and solids 3-8 times cheaper
(FRACTAL-BROWSER.md, *Rendering*). A simulation's preview still runs
its whole step count, because its picture is the run at that count.
The presets' totals, as cell-steps (grid area × steps):

| Preset | Cell-steps |
|---|---|
| Brusselator Beans | 3.3e9 |
| GG Snowflake (2D) | 5.2e9 (at a 256² grid now, so about a quarter of the 512² figure) |
| GG Snowflake (3D) | the same, plus a terrain still |
| Oregonator Wave Trains | 9.8e8 |

These are the ones a time slice would turn from a stall of seconds
into a trickle.

**Other blocking callers on the same path:**
- the Textures panel's previews (one blocking generation a frame);
- Save as texture (`cache_saved_texture_from_run`);
- Save Online's 512 thumbnail, rendered on the UI thread before the
  upload's thread starts.

## The design

**A cooperative render.** `render` (and the engines under it) gets a
mode in which every one of the waits above is a yield point rather than
a blocking poll:
- the GPU work is submitted;
- the future returns `Pending` until it is done, checked with a
  non-blocking `device.poll(PollType::Poll)`, or a
  `queue.on_submitted_work_done` flag;
- the loop then goes on.

The CLI, the exports and the gallery module keep the blocking waits.
Their callers drive the future straight through with `pollster`, and a
future that returns `Pending` without a waker that fires would spin or
hang there. So this is a flag on the job (`RenderJob::cooperative`, or a
`RenderProgress` hook), not a change to every caller.

**Driven like the planner's slices.** `scene::slice` already does this
for the deep-zoom planner on the web: a future polled once a frame with
a no-op waker, which yields at a tick past its budget. A preview job
uses the same driver:
- the gallery holds one job: its config, its pinned future, the
  renderer it renders with;
- the app polls the job after the UI, for a budget of a few
  milliseconds (decision 1);
- a finished job's image is saved and uploaded, as now, and the next
  missing preview's job starts.

The same driver runs on the desktop and the web, which is the point on
the web: the yield points are what give the page its frames back.

**Cancel.** A gallery not drawn this frame (`take_shown`, from A) drops
its job and destroys its renderer. Dropping frees nothing on WebGPU,
which is why `render` destroys its throwaway renderer.

**One renderer across previews (maybe; decision 3).** Each preview
builds a fresh `FlameRenderer` and compiles its shaders from nothing,
because the shader cache is per renderer. The gallery module already
keeps one renderer and resizes it (`wasm/render`, `render_with_live`).
Keeping one for previews would warm the cache across them. Measure how
much of a preview is compile first (step 0).

## What it cannot do

**A pipeline compile is one call.** wgpu creates a pipeline
synchronously here, so the first preview needing a new shader still
stalls for its compile. Heavy variations compile slowest, and slower
again in a flame with finals or linked transforms, because the driver
inlines `apply_variations` at each of its three call sites. Step 0
measures how long. Slicing removes the waits between GPU work, not the
compiles.

**The GPU is shared.** A preview's work runs on the app's queue beside
the viewport's. A sliced job keeps each frame's share small, but the
viewport's frame time still carries it.

## What has to be checked on the way

- **The escape renderer's process-wide statics.** These are the
  progress counters `RENDER_DONE` / `RENDER_WANT`, the path tracer's
  in-flight count, and the TDR budget shifts it learns
  (`gpu_tuning.json`). A preview job and the viewport's escape renderer
  advancing in the same frames would share them: the viewport's
  progress readout and the learned budgets could be thrown off. Either
  make them per renderer, or keep a preview from learning a budget.
- **The quality governor.** It times frames, and a frame with a slice of
  preview work in it is slower for a reason the governor cannot act on.
  The histogram readback drops the governor's timing anchor for exactly
  this (`app/mod.rs`, "dropping the timing anchor makes the governor
  skip the one delta that contains it"). A preview slice should do the
  same, or the governor should count the slice's budget as spent.
- **Memory.** A job holds its renderer between frames: a `FlameRenderer`
  for the tail, an `EscapeRenderer`, or a `SimRenderer` of up to a 512²
  grid. One job at a time keeps that to one renderer. On the web, the
  renderer must be destroyed when the job ends or is cancelled.

## Order of work

0. **Measure.** Time every preset's preview, and the frame times while a
   gallery fills: the worst simulations, the 3D escape category, a
   flame with a heavy variation. Split each preview into compile and GPU
   time. This decides whether step 4 is worth it, and what the budget
   should be.
1. **Awaitable waits.** Add a non-blocking wait-for-the-GPU future, and
   the cooperative flag on the job, without changing what the blocking
   callers do. A test renders the same config both ways and requires
   identical pixels.
2. **Yield points** in the flame batches, escape chunks, simulation
   batches (`SimRenderer::advance_steps` already takes a budget),
   terrain sections and the tail's readback.
3. **The preview job.** The gallery's driver from A holds the job and
   polls it each frame within the budget; it cancels on `take_shown`
   false. Desktop and web share the driver. The escape statics and the
   governor's anchor are handled here.
4. **Renderer reuse** across previews, if step 0 shows compiles matter.
5. **The other blocking callers** move to the job: the Textures panel's
   previews, Save as texture, and Save Online's thumbnail (decision 4).

## Decisions to make

1. **The budget:** a fixed few milliseconds a frame (4? 8?), or what the
   quality governor's target leaves.
2. **Pause while busy:** whether previews pause while the viewport is
   being dragged, edited or playing an animation (the governor already
   knows when it is).
3. **Renderer reuse** (step 4): after step 0's numbers.
4. **Scope of step 5:** all three other callers, or only the Textures
   panel's previews. That panel already has its own cost gate,
   `textures::may_generate`.

Not in this plan: **C**, shipping the presets' previews with the app,
which was declined on 2026-11-04.
