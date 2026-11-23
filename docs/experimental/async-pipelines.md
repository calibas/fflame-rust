# Compiling pipelines off the frame

**Status:** planned 2026-11-22, not started. Waiting for wgpu's async
pipeline creation ([gfx-rs/wgpu#10438](https://github.com/gfx-rs/wgpu/pull/10438),
open) to land in a release. Not for 0.6 (the user's call).

## Why

A pipeline compile stops the UI for as long as it takes, on both
platforms, for different reasons:

- **Desktop:** pipelines are built synchronously on the frame loop's
  thread (`create_compute_pipeline` inside each renderer's
  `ensure_*_pipeline`). The window stops drawing until the driver
  finishes.
- **Web:** the browser compiles in its GPU process. The JavaScript call
  returns at once, but every submission after it -- the UI's own frames
  included -- waits behind the compile, so the page freezes. Only
  `createComputePipelineAsync` keeps the queue free, and wgpu 29 calls
  the sync `createComputePipeline` (`wgpu-29.0.3/src/backend/webgpu.rs`,
  `create_compute_pipeline`).

The compile-times branch (2026-11-22) cut the worst compiles 5-18x
(a solid's path tracer now about 0.6-0.8 s on the desktop's driver, a lit
solid 110-260 ms, a 2D formula a median 36 ms), but a browser compiles
many times slower than that, and an M2 through Chrome took over 30 s
for one before. Shorter compiles make the freeze shorter; this makes it
go away.

## What the wgpu PR gives

`Device::create_compute_pipeline_async` and
`Device::create_render_pipeline_async`, each returning
`impl Future<Output = Result<_, Error>>`:

- **Web:** calls `createComputePipelineAsync` / `createRenderPipelineAsync`.
  The queue stays free while the browser compiles.
- **Native:** a future that is already resolved: it still compiles
  synchronously, on the calling thread. Compiling on a background thread
  is listed as not included.
- **Errors** come back through the future (`Error::Validation` or
  `Error::Internal`), not through the error scope. A lost device resolves
  to an invalid pipeline.

So the web side comes from wgpu; the desktop side needs a thread of ours.

## The design

**One pipeline job type** (`gpu::pipeline_job`): a compile that is
`Pending`, `Ready(pipeline)` or `Failed(message)`, keyed by what the
renderer keys its pipelines by today.

- **Web:** `create_compute_pipeline_async`, awaited in
  `wasm_bindgen_futures::spawn_local`; the result lands in the job.
- **Desktop:** the same call on a worker thread (`std::thread::spawn`
  with the device cloned: wgpu's native `Device` is `Clone + Send +
  Sync`), the result sent back through a channel the renderer polls.
  One compile at a time to start with: the driver compiles with threads
  of its own, and two heavy compiles at once mostly slow each other.
- **Either way,** a finished job asks egui for a repaint (the frame loop
  honours those since the 0.6 UI review), so the picture appears without
  waiting for input.
- **A newer key supersedes a pending one:** its result is dropped when it
  lands, and nothing waits on it.

**The renderers that compile on a user's action go through it:**

| Where | What compiles |
|---|---|
| `shader_cache.rs`, `renderer/compute_kernel.rs` | the flame's chaos-game shader (variation and feature changes; the sticky superset already keeps most edits from compiling) |
| `escape/renderer.rs` | escape-time: the direct and perturbed paths, mode D's walk, fields, the recolour and relight passes |
| `escape/renderer/solid_path.rs` | a solid's path tracer |
| `escape/terrain.rs`, `escape/path_core.rs` | terrains, lit and path traced |
| `sim/renderer.rs` | a simulation's model and colouring |
| `renderer/effect_chain.rs`, `renderer/effect_passes.rs` | effects |

The fixed shaders made once at startup (tonemap, accumulate, the planner's,
the export folds) stay synchronous: they compile before the first frame,
and quickly.

**While a job is pending:**

- The renderer keeps showing its last picture and submits nothing that
  needs the new pipeline. The path tracer already does this for its own
  compile (`SolidPath::compiling`, an empty submission whose completion
  is the compile); the job replaces that.
- The progress bar says "Compiling..." (a new `RenderProgress` state), so
  a pause reads as work, not a hang.
- Edits keep flowing into the config; the next frame after the job lands
  renders the config as it is then.

**Exports, the CLI, thumbnails and scripts** await the job: blocking on
the desktop (as they block on renders now), awaited on the web
(`render_preview_async` already is).

## Steps

1. **Upgrade wgpu** to the release with the PR. Check its other changes on
   their own first (the shader dumps, the visual suite, the browser
   storage-buffer limits).
2. **The job type**, with tests that need no GPU: pending, ready,
   superseded, failed.
3. **Escape-time first** (`escape/renderer.rs`, the solid path tracer,
   terrains). Its compiles are the longest, and the path tracer's compile
   probe shows the shape already. Then simulations and effects; the flame
   shader last, since the sticky superset already hides most of its
   compiles.
4. **The "Compiling..." state** in the progress bar and the phone's menu.
5. **Exports and headless paths** awaiting jobs; the gallery modules and
   pyfflame checked, since they share `render_with`.

## Open questions

- How the egui renderer's own pipelines (made by egui-wgpu) behave. They
  compile once at startup and are small; probably left alone.
- Whether the web's async compile also avoids the macOS Chrome device
  loss on a long compile, or only keeps the page responsive while it runs.
  The M2 that lost its device should be the first test.
- A device lost while a job is pending resolves to an invalid pipeline
  (per the PR): the job should report it as failed, and the web's Reload
  notice take over.
