# Flame Renderer

**Overview:** `FlameRenderer` runs the chaos game on the GPU and turns
its samples into a picture: a compute pass plots into a `u32`
histogram, an accumulate pass folds batches of it into an
`Rgba32Float` accumulator, optional solid-rendering passes light and
blur it, and a tone-map pass draws it into an `Rgba8Unorm` texture that
the viewport shows and the exporters read back. The same renderer
provides the tail (tone map, effects, readback) for the escape-time and
simulation engines.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — system overview
- [BUFFERS.md](BUFFERS.md) — every buffer, binding and struct layout named here
- [SHADERS.md](SHADERS.md) — the shader builder, template flags, the shader cache, sticky shaders
- [COLOR.md](COLOR.md) — colour modes, palette, histogram encoding, accumulator semantics
- [TONE-MAPPING.md](TONE-MAPPING.md) — the tone map's modes and formulas
- [EFFECTS.md](EFFECTS.md) — density and colour effects
- [EXPORT.md](EXPORT.md) — PNG, tiled and CPU-histogram exports, video, GPU memory
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — deep zoom: cylinder targeting, planners, replay
- [CAMERA.md](CAMERA.md) — the 3D camera and projection
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md), [TERRAIN.md](TERRAIN.md) — the other engines that share the tail
- [TRANSFORMS.md](TRANSFORMS.md) — the flame algorithm the compute pass runs

**Code locations:**
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) — `FlameRenderer`, `OrbitFuses`, `plan_view`
- [src/renderer/render.rs](../../src/renderer/render.rs) — `render`, `render_with`, `RenderJob`, `RenderEngines`, `RenderOutput`, `RenderError`
- [src/renderer/shade_pass.rs](../../src/renderer/shade_pass.rs), [src/renderer/dof_pass.rs](../../src/renderer/dof_pass.rs) — solid rendering's post passes
- [src/renderer/density_stats.rs](../../src/renderer/density_stats.rs) — the 8-word tail readback (`BoundsTracker`)
- [src/renderer/sticky.rs](../../src/renderer/sticky.rs) — the sticky variation superset
- [src/renderer/effect_chain.rs](../../src/renderer/effect_chain.rs) — `EffectChainRunner`
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs), [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) — `FlameBuffers`, `FlamePipelines`
- [src/app/mod.rs](../../src/app/mod.rs) — the event loop, the frame, the governor (`Batch`, `Knee`)
- [src/app/gpu_updates.rs](../../src/app/gpu_updates.rs) — config changes to GPU calls, overwrite mode, `freeze_viewport`

---

## The pieces

| piece | where | what it owns |
| --- | --- | --- |
| `FlameRenderer` | `renderer/compute_kernel.rs` | the passes, the counters, solid state, orbit generations, the Focused Rendering plan, the output texture `fractal_texture` |
| `FlameBuffers` | `gpu/buffers.rs` | histograms, the accumulator pair, palette and curve textures, uniform buffers, optional feature buffers |
| `FlamePipelines` | `gpu/pipelines.rs` | the accumulate, histogram-blur, analytic-blur and tonemap pipelines; the `ShaderCache` |
| `ShaderCache` | `shader_cache.rs` | the generated flame compute shader, its pipeline and bind-group layout, the variation init pipeline, an 8-entry pipeline LRU |
| `StickyVariations` | `renderer/sticky.rs` | which variations recent flames used (renderer state, never serialized) |
| `render` / `render_with` | `renderer/render.rs` | one complete headless render, for every engine |
| `App::render` | `app/mod.rs` | the interactive frame |

Constructors:

| constructor | per-pixel flame state | used by |
| --- | --- | --- |
| `FlameRenderer::new(device, queue, format, w, h, flame)` | full | the WASM custom-size export |
| `FlameRenderer::with_palette_size(..., palette_size)` | full | the app's viewport, `render()` for flames, video, the gallery module, tools |
| `FlameRenderer::for_tail(..., palette_size)` | 1x1 | `render()` for escape and simulation configs |

`for_tail` builds `FlameBuffers` at 1x1 — the two histograms and the
accumulator pair, 64 bytes a pixel, are never read when another engine
supplies the image — while `fractal_texture` and the size the tone map
normalises by are the full frame. It is not for drawing a flame.

The shade pass is sized on first use: every renderer starts it at 1x1
and `run_shade_pass` resizes it the first time a lit 3D solid runs.

---

## A frame

```
compute_pass ×N ──▶ accumulate_pass ──▶ sample density ──▶ shade ──▶ DoF ──▶ density ──▶ tonemap ──▶ colour ──▶ display /
 (chaos game)        │ analytic blur       │ (fractions)     (lit     (solid    effects     (into      effects    readback
 init pass if dirty  │  convolve, upscale  │                  solid)   DoF)                fractal_
 histogram cleared   │ histogram blur H,V  │                                               texture)
 at batch start      │ accumulate, swap    │
```

| step | call | runs when |
| --- | --- | --- |
| 1. iterate | `compute_pass` | every frame that iterates; one dispatch |
| 2. analytic blur | inside `accumulate_pass` | a flame with `analytic_*` blur transforms (`blur_buffer_count > 0`) |
| 3. spatial filter | inside `accumulate_pass` | `filter_radius > 0` |
| 4. accumulate | `accumulate_pass` | at the end of each batch of dispatches |
| 5. sample-density fractions | `update_density_stats` (app, async) / `apply_exact_density_fraction` (one-shot, blocking) | every frame / once before the final tone map |
| 6. shade | `run_shade_pass` | 3D, lighting active, depth region present |
| 7. depth of field | `run_dof_pass` | 3D, `dof_blur_strength > 0`, depth region present |
| 8. density effects | `EffectChainRunner::run_density_effects` | any enabled |
| 9. tone map | `tonemap_pass` / `tonemap_pass_with_input` | every frame |
| 10. colour effects | `EffectChainRunner::run_color_effects` | any enabled |
| 11. readback | `read_fractal_pixels` or the effect chain's `read_color_output_pixels` | exports |

The tone map's input is the first that exists of: the density-effect
output, the DoF output, the shade output, the current accumulator.
Both `App::render` and `render_with` make that choice the same way;
steps 2–7 are flame-only.

### 1. The compute pass

```rust
pub fn compute_pass(&mut self, encoder, queue, device,
    num_workgroups, iterations_per_thread, burn_in,
    zoom, pan_x, pan_y, rotation,
    camera_rotation_x, camera_rotation_y, camera_bank,
    camera_x, camera_y, camera_z, speed_factor,
    clear_histogram: bool, clear_paths: bool) -> u64
```

In order, it:

1. Draws the dispatch seed. With `deterministic_rng` (from the config)
   the seed is `12345 + frame_counter × 2654435761`, and
   `frame_counter` restarts at every accumulation restart; otherwise it
   is random.
2. Settles the orbit generation (see *Persistent orbits*).
3. Takes the depth-prime flag for solid rendering (see below).
4. Uploads the whole `GpuParams` uniform.
5. Counts the samples the dispatch will plot (see below) and adds them
   to `total_iterations` — or, in overwrite mode, sets it to them.
6. If `clear_histogram`, clears the histogram
   (`FlameBuffers::clear_histogram`) — and with it the frame-coverage
   counters and the analytic-blur splat buffer. In overwrite mode the
   solid depth region is cleared too, so a changing shape is not
   occluded by the previous one's surface. If `clear_paths`, clears
   PathMap's path ids.
7. Rebuilds the analytic-blur kernels if the view or flame changed
   (`maybe_rebuild_blur_kernels`), which can reallocate the low-res
   blur buffers.
8. Runs the variation init pass if variation parameters were written
   since it last ran (`init_dirty`): `ceil(pairs / 64)` workgroups of
   the init pipeline, filling the init-derived parameter slots.
9. Rebuilds the compute bind group if the shader's bindings changed
   (`ComputeBindGroup::bindings` against
   `FlamePipelines::compute_bindings`; see [BUFFERS.md](BUFFERS.md)).
10. Dispatches `num_workgroups × 1 × 1` workgroups of the trajectory
    pipeline for the render mode (`FlamePipelines::get_trajectory_pipeline`,
    from the `ShaderCache`). The shader's workgroup is `(64, 1, 1)`.

**The dispatch.** `num_workgroups × 64` threads, each running
`iterations_per_thread` iterations. Every iteration selects a
transform, applies it, and — once the thread's burn-in has run out and
the transform's opacity test passes — plots a point that lands in
frame: four `atomicAdd`s of `(r·s, g·s, b·s, s)` into the pixel's
histogram words, with `s = 100 × weight` (the weight being 1 unless a
depth, occlusion, targeting or importance factor applies). The encoding is in
[COLOR.md](COLOR.md).

**The count it returns** is the samples that plotted, and every caller
that accumulates must pass it on rather than `threads × iterations`:

- with persistent orbits, `OrbitFuses::dispatch(threads, ipt, burn_in, period, seed)` — an exact CPU model of each thread's burn-in left, so the count needs no readback;
- without them, `workgroups × 64 × (ipt − burn_in)`.

Respawns after a bad value re-fuse a point but are not counted.

### Persistent orbits

Each thread's orbit outlives its dispatch. The orbit buffer (binding 8,
template flag `PERSISTENT_ORBITS`, `16 + ORBIT_SLOTS` words a thread)
holds its point, colour, burn-in left, xaos previous transform,
importance window, analytic-blur residual and the stateful variations'
slots; a dispatch resumes them and stores them back. The random streams
are still seeded per dispatch from `params.seed`.

- **Generations.** `params.orbit_generation` starts at 1 and is bumped
  when the *orbit key* changes: a hash of the whole flame (its JSON,
  `orbit_flame_hash`), the render mode, `preserve_z`, the colour mode,
  the speed factor and the orbit's word count. A thread whose stored
  generation differs starts afresh. Pan, zoom, palette and tone-map
  edits keep the orbits.
- **Variation state.** A flame whose shader carries variation slots
  (orbit words above `ShaderCache::ORBIT_FIXED_WORDS`) restarts its
  orbits at every restart of the accumulation — a pan, a reset, every
  overwrite-mode frame — because a state can be a clock rather than a
  register that settles (`curliecue2`'s walker), and JWildfire
  initialises variation state per render.
- **Burn-in** is paid once per restart, not once per dispatch. So
  `iterations_per_thread` only slices the work: the picture does not
  depend on it, and a short dispatch costs no orbit depth.
- **Motion-blur moments** (`set_moment`, `set_continuing_frame`;
  [ANIMATION.md](ANIMATION.md), *Motion blur*): a frame's later moments
  take over the orbits on their flame's new key without a restart -- the
  moments are close, and an orbit settles onto the next attractor in a
  few iterations -- so a frame pays one burn-in; their samples add to the
  frame's in overwrite mode too. Measured: no bias at 0.15% of a block's
  density.
- **Capacity.** The buffer is sized for at least `128 × 64` threads
  from the start, so the governor widening again never loses orbits; a
  wider dispatch grows it once, which restarts the orbits.
- **Closed xaos groups.** A starting thread's first transform is a
  stratified pick (a Kronecker sequence in the thread index), so each
  isolated group gets its share of orbits. Where the xaos walk itself
  decides the group (`Flame::xaos_walk_splits_orbits`), a refresh
  restarts the threads with `(thread_id + seed) % 4 == 0` each dispatch
  (`ORBIT_REFRESH_PERIOD`).

Design record:
[persistent-orbits.md](../archive/projects/persistent-orbits.md). The
gates are in `src/renderer/orbit_gates.rs`, comparing renders by
density (`src/renderer/density_gates.rs`).

### Batches

The histogram holds a **batch** of dispatches: it is cleared at the
first dispatch of a batch and folded into the accumulator at the last.

| path | dispatches per batch | workgroups × iterations |
| --- | --- | --- |
| `render_with` | `BATCH_SIZE = 4` | `128 × job.iterations_per_thread` |
| the app, accumulating | `accumulation_batch_size = 4` | the governor's shape |
| the app, overwrite mode | 1 | the governor's shape |

### 2–4. Accumulate

```rust
pub fn accumulate_pass(&mut self, encoder, queue, device, samples_this_frame: u64)
```

`samples_this_frame` is the batch's plotted count. Before the
accumulate dispatch itself it runs, on the batch's histogram:

- **Analytic blur** ([analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md)).
  Transforms whose blur is an `analytic_blur` or
  `analytic_gaussian_blur` variation (up to `MAX_BLUR_BUFFERS = 4`)
  splatted their mean point into a low-resolution buffer during the
  chaos game. Here each slice is convolved with its kernel (a CPU
  Monte-Carlo sample of the variation's offset distribution through the
  view's pixel mapping, rebuilt only when the view or flame changes),
  then cubic-B-spline upscaled and added into the main histogram with
  dithered stochastic rounding; the dither seed advances every frame.
- **The spatial filter** (Apophysis `filter`): when `filter_radius > 0`,
  a separable bilateral Gaussian — horizontal pass into the scratch
  histogram, vertical pass back. The bilateral density sigma is scaled
  from the batch's mean density, counted as the untargeted samples a
  targeted batch stands for.

Then the accumulate dispatch (`shaders/accumulate.wgsl`, workgroup
`(8, 8, 1)`, `ceil(W/8) × ceil(H/8)` workgroups) reads the previous
accumulator and the histogram and writes the other accumulator. The
textures then swap, and the accumulate and tonemap bind groups are
rebuilt to point at the new pair.

Every accumulator pixel holds `rgb` = the density-weighted mean colour
in `[0, 1]` and `a` = the raw cumulative hit count. The blend is chosen
on the host:

| mode | `blend_factor` | `use_fixed_ema` | colour update |
| --- | --- | --- | --- |
| overwrite (live preview) | 1.0 (unread) | 2 | previous discarded |
| cumulative mean (default, `use_dynamic_blend`) | 0.0 | 0 | `α = new_density / total_density` |
| fixed EMA | `FractalConfig::blend_factor` | 1 | `α = blend_factor` |

Overwrite is its own mode. It used to be `blend_factor ≥ 0.99` in mode
0, which a Fixed Blend Rate of 0.99-1.0 also sent: the shader cleared
every batch while the host kept counting them into `samples_in_buffer`,
and the picture faded.

Density is cumulative in every mode. Cumulative mean is the reference
algorithm and reaches an f32 precision floor around 10⁹ hits a pixel;
fixed EMA stays precision-stable indefinitely at the cost of dim early
frames. With the solid depth region present, a pixel whose nearest
depth moves closer by more than 1.5 × `surface_thickness` discards its
history (the depth-tightening reset, tracked in `accum_depth_buffer`).
Formulas and history: [COLOR.md](COLOR.md),
[accumulator-unification.md](../archive/projects/accumulator-unification.md).

**Counters.**

| counter | grows by | reset by | read for |
| --- | --- | --- | --- |
| `total_iterations()` | each dispatch's plotted count (set to it in overwrite mode) | `reset_iteration_counter`, `update_flame` | the app's `max_iterations` stop |
| `samples_in_buffer()` | each batch's plotted count (set to it in overwrite mode) | `reset_iteration_counter`, but not `reset_iteration_counter_keep_buffer` | the tone map's `sample_density` |
| `samples_accumulated()` | each batch's plotted count | `reset_iteration_counter`, `update_flame` | the Levels histogram's statistics |
| `effective_iterations()` | each batch, outside overwrite mode | resets | nothing reads it |

### 5. Sample density

Brightness is normalised by `sample_density`, the work the accumulator
holds per pixel. `refresh_sample_density` writes it at the start of
every `tonemap_pass` and `tonemap_pass_with_input`, as two 4-byte
writes into `TonemapParams`:

```
sample_density = max(samples_in_buffer
                     × solid_density_fraction
                     × frame_coverage_fraction
                     × cylinder_iteration_scale()
                     / (W × H), 1e-6)
levels_density = max(sample_density / cylinder_iteration_scale(), 1e-6)
```

| factor | 1.0 unless | measured by |
| --- | --- | --- |
| `solid_density_fraction` | `solid_strength > 0`, 3D, depth region present | occlusion-survival counters in the histogram tail (1 thread in 1024), clamped to `[0.005, 1]` |
| `frame_coverage_fraction` | `FractalConfig::auto_exposure` | the frame-coverage counters (binding 16): plot attempts in frame over attempts; refused below 32 hits or near u32 saturation |
| `cylinder_iteration_scale()` | Focused Rendering is drawing a plan | `Cylinders::iteration_scale` — see [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) |

The interactive path reads the two measured fractions asynchronously
every 16 frames (`update_density_stats`, `BoundsTracker`) and smooths
them (`0.7 × old + 0.3 × new`); a one-shot render reads them once,
blocking, before its final tone map (`apply_exact_density_fraction`).
Whole-struct writes of `TonemapParams` (`update_tonemap`,
`set_transparent_mode`) compute a plainer `sample_density`; the
per-pass refresh overrides it.

### 6–7. Solid rendering (3D flames)

Solid rendering applies to `RenderMode::ThreeD` when `solid_strength > 0`
or lighting is active (`SolidShadingSettings::active`, i.e.
`shading_strength > 0`). Design record:
[solid-rendering.md](../archive/projects/solid-rendering.md); backlog:
[solid-rendering-todo.md](../projects/solid-rendering-todo.md).

- **Occlusion.** The histogram grows a fifth word a pixel, the
  nearest-depth region (`FlameBuffers::set_solid_depth_region`), which
  the shader's `SOLID` block maintains with `atomicMax` on an inverted
  ordered-float encoding (0 = no sample). A sample deeper than the
  nearest by more than `surface_thickness` has its weight scaled by
  `1 − solid_strength`.
- **Depth priming.** The first batch after a reset (`needs_depth_prime`)
  records depth only and plots nothing, so early samples are not gated
  against an empty depth buffer.
- **The tail.** After the depth region: 8 words — a subsampled world
  bounding box (1 thread in 16) and the occlusion-survival counters —
  then four light-space shadow maps of `1024²` words each.
- **Shadow maps** are splatted by the main pass toward up to four
  enabled lights, at splat resolution. Their fit (centre, radius) is
  frozen per accumulation run from the measured bounds, clamped to 12
  view extents around the view centre. The app re-fits at most once a
  run and only in its first 64 batches (`maybe_refit_shadow`, which
  makes the caller reset); `render_with` runs six warm-up dispatches,
  awaits one exact bounds read to re-fit (`refresh_shadow_placement`),
  and resets.
- **The shade pass** (`ShadePass`, `shaders/shade.wgsl`): Blinn-Phong
  over normals reconstructed from the depth region — a full-image
  normals pass smoothed by up to three à-trous iterations
  (`normal_smoothing`), or an inline estimate — with SSAO, gap fill,
  the shadow-map lookup, the material's coat and glow, and post-lighting
  fog. It writes an `Rgba32Float` output with a temporal blend of 0.85
  (0 in overwrite mode) and skips itself when nothing changed and the
  blend has settled.
- **Fog ownership.** With lighting active, depth fog moves from the
  splat (`GpuParams::fog_strength` is written 0) into the shade pass.
  The flip needs an accumulation reset, which `process_gpu_updates`
  forces.
- **Depth of field** (`DofPass`, `shaders/dof.wgsl`): at-splat DoF is
  compiled out under `SOLID`, so a post pass does it: a circle-of-confusion
  prepass, a separable max-dilate, then a disk gather over the shade
  output or the accumulator. It runs only when its input changed.
- **Not bit-reproducible:** the in-batch depth race means two solid
  renders differ slightly; the `solid-*` visual tests compare with a
  tolerance.

**The lighting rig is shared.** `FractalConfig::solid_shading` — four
directional lights (azimuth and elevation in camera space, colour,
intensity) and the material (`gloss`, `roughness`, `glow`;
[materials.md](../archive/projects/materials.md)) — is read by this
shade pass, by the escape engine's solid walk and path tracer, and by
both terrains. See [TERRAIN.md](TERRAIN.md).

### 8–11. Tone map and the tail

- **Density effects** run on the HDR input before the tone map, in the
  effect chain's `Rgba32Float` ping-pong, and need the device's
  `FLOAT32_FILTERABLE` feature (they are skipped without it). Colour
  effects run on the tone-mapped `Rgba8Unorm` image after it. See
  [EFFECTS.md](EFFECTS.md).
- **`tonemap_pass(queue, encoder)`** draws a fullscreen triangle
  (`draw(0..3)`, `shaders/tonemap.wgsl`) into `fractal_texture`,
  reading the current accumulator through the stored bind group.
  **`tonemap_pass_with_input(device, queue, encoder, view)`** builds a
  bind group around another `Rgba32Float` view in the accumulator's
  layout — the shade or DoF output, a density effect's output, or
  another engine's image. The accumulator is read with `textureLoad`,
  so it needs no filtering.
- The tone-map modes (Linear, Logarithmic, Density visualisation), the
  curve LUT, Levels, highlights and the background blend are in
  [TONE-MAPPING.md](TONE-MAPPING.md). `update_tonemap` uploads the
  whole `TonemapParams`; `update_curve_lut` writes the 256-entry curve
  texture.
- **Transparency is a tone-map mode.** `set_transparent_mode(queue,
  transparent, premultiplied, config, ipt)` rewrites `TonemapParams`
  with `transparent_mode`: 0 opaque (blended over the background), 1 a
  flame's straight-alpha split (a flatten over black reproduces the
  opaque export), 2 premultiplied, 3 straight alpha for an escape or
  simulation image, whose coverage is its alpha.
- **Readback.** `read_fractal_pixels(device, queue, transparent,
  background)` copies `fractal_texture` through one persistent,
  grow-only staging buffer. Transparent keeps RGBA; opaque composites
  over the background on the CPU and sets alpha to 255.
  `encode_png_from_rgba` writes the PNG, with metadata when given.

---

## The tail for the other engines

Escape-time and simulation pictures are not chaos games. Their
generators write an `Rgba32Float` image in the accumulator's layout,
and from there the flame renderer does the rest: density effects, the
tone map (`tonemap_pass_with_input`), colour effects, readback. Steps
1–7 do not run for them.

| | headless (`render_with`) | interactive (`App::render`) |
| --- | --- | --- |
| escape, 2D | `render_escape` → `escape_tail` | the app's lazy `escape_renderer`, re-rendered while `escape_dirty` |
| escape terrain | `render_escape_terrain` → `escape_tail` | `escape_terrain` |
| simulation (and its terrain) | `render_sim`, whose tail is inline | `sim_renderer`, `sim_terrain` |

The headless paths call `load_config` first: it is the one call that
keeps every tail input — palette texture, tonemap uniform, curve LUT,
background, Levels — in step with the flame path. The palette views come from the
renderer: `palette_view`, `escape_palette_view(stepped)`, and
`palette_generation`, a counter bumped on every palette write that the
escape engine keys its row bands on.

If you add a pass to the tail, say what it does for all three engines.
See [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md) and
[TERRAIN.md](TERRAIN.md).

---

## `render` and `render_with`

Every headless render goes through `src/renderer/render.rs`: CLI
export, the WASM API's export, thumbnails, the in-app PNG export on the
desktop, the gallery module (`wasm/render`) and video.

```rust
let job = RenderJob::new(&config, 1920, 1080)
    .with_iterations(10_000_000)
    .with_transparent(true);
let out = render(&device, &queue, job, &mut NoProgress).await?;
// out.rgba_data, out.width, out.height, out.total_iterations,
// out.render_time_ms, out.frame_coverage
```

**`RenderJob`**

| field | default | builder |
| --- | --- | --- |
| `config` | — | `RenderJob::new(config, width, height)` |
| `target_iterations` | `None` (the config's `max_iterations`) | `with_iterations` |
| `iterations_per_thread` | 256 | `with_iterations_per_thread` |
| `burn_in` | 20 | `with_burn_in` |
| `transparent`, `premultiplied` | false | `with_transparent`, `with_premultiplied` |
| `engines` | `None` (a fresh engine per render) | `with_engines(&mut RenderEngines)` |

`with_persistent_orbits` exists only under `#[cfg(test)]`, for the
gates' restarted-orbit baseline.

**`RenderEngines`** is engine state a looping caller keeps alive
between jobs: `sim` (the field and step counter, so frame *n* continues
frame *n − 1*'s run), `escape` (allocations and the warm reference
orbit), `terrain` and `sim_terrain`, each behind its cargo feature. The
video loop passes one; still renders pass none.

**`RenderOutput`**: `width`, `height`, `rgba_data`, `total_iterations`
(flames: dispatched iterations; escape: `max_iter`; simulation:
`sim.steps`), `render_time_ms`, and `frame_coverage` (always 1.0 for
the non-flame engines).

**`RenderError`**: `NoPaletteFound`, `PixelReadFailed`, `Cancelled`,
`EngineMissing` (the config names an engine this build lacks),
`OutOfMemory` (an allocation failed — caught with a wgpu error scope
rather than rendering black), `TooLarge` (refused before allocating).
`RenderProgress` reports progress and carries cancellation; `NoProgress`
ignores both. `unknown_names(config)` lists escape and simulation names
this build has no entry for.

**`render(device, queue, job, progress)`**:

1. Refuses sizes the device cannot hold before allocating anything
   (`EscapeTerrain`, `EscapeRenderer` and `SimRenderer` each have an
   `allocation_error`).
2. Builds a renderer — `for_tail` for escape and simulation,
   `with_palette_size` for flames — inside an out-of-memory error scope.
3. Turns the sticky superset off: a one-shot render must compile the
   specialised shader its reproducibility contract was made with.
4. Calls `render_with`, then `destroy()` — dropping frees nothing on
   WebGPU.

**`render_with(renderer, device, queue, job, progress)`** renders into
a caller-owned renderer of the right size (call `resize` first if the
size changed). Escape configs go to `render_escape`, simulation configs
to `render_sim`; a build without the engine returns `EngineMissing`.
For a flame:

1. `load_config`, which also plans Focused Rendering inline.
2. If shadows are wanted, the shadow warm-up described above.
3. The loop: `compute_pass` with 128 workgroups, the histogram cleared
   at each batch's first dispatch and the path ids at the very first;
   `accumulate_pass` every `BATCH_SIZE = 4` dispatches with the batch's
   plotted count, and once more for a partial final batch. The loop
   stops when the *dispatched* count (`128 × 64 × ipt` a dispatch)
   reaches the target.
4. `set_transparent_mode` if transparent.
5. The tail: an `EffectChainRunner` only if an effect is enabled,
   `apply_exact_density_fraction`, shade, DoF, density effects, tone
   map, colour effects, readback, and the chain destroyed after the
   read.

---

## The app's frame

### The event loop

The window redraws only when something needs it (`Event::AboutToWait`
in `src/app/mod.rs`). It requests redraws continuously while a flame is
iterating below `max_iterations`, an animation or audio plays, input
arrived within the last 700 ms, a viewport resize is pending, fly-mode
keys are held, a simulation runs or catches up to a timeline target,
or the progress bar is a frame stale — and once more on the frame
iteration stops. With vsync on it requests a redraw each time; off, it
waits until the target frame time. During an export it redraws at ten
frames a second for the progress bar. Otherwise it sleeps
(`ControlFlow::Wait`). Escape renders that have not settled request
their own redraws.

### `App::render`

1. **UI first.** egui draws, showing the *previous* frame's
   `fractal_texture` (or the colour-effect output). A changed viewport
   size resizes the renderer here.
2. **Updates.** UI responses are handled, then `process_gpu_updates`
   turns the config manager's pending actions into renderer calls (see
   below) and updates overwrite mode.
3. **Render.** `freeze_viewport` is checked; then the escape or
   simulation generator runs if that is the mode; `sync_cylinders`
   keeps Focused Rendering's plan current (a `true` return means a
   full `load_config`); a newly arrived plan or a shadow re-fit resets
   the accumulation; the governor shapes the dispatch; then compute,
   accumulate at batch end, `update_tonemap`, `update_density_stats`,
   shade, DoF, density effects, tone map, colour effects; then submit.

Every 30 frames, outside animation playback, the accumulator is read
back for the Levels histogram (`renderer::histogram`).

### Overwrite mode

While parameters change, the accumulator shows only the newest batch
instead of averaging a stale picture into a new one
(`set_overwrite_mode`; design record
[overwrite-mode-fsm.md](../archive/overwrite-mode-fsm.md)). It is on
while a flame, view or palette edit that does not itself reset the
accumulation arrived in the last 100 ms (`update_overwrite_mode`) and
during animation playback, and never in escape mode. In overwrite mode every frame is a batch of one, the
accumulate discards the previous image, the solid depth region resets
each batch, the shade pass does not blend temporally, and a stateful
flame restarts its orbits every frame.

When the window lapses, cumulative-mean mode calls
`reset_iteration_counter_keep_buffer` — the accumulator keeps the last
preview batch, so `samples_in_buffer` must keep describing it — and
fixed-EMA mode calls `reset_iteration_counter` and clears both
accumulators.

### The frame-time governor

The dispatch is sized to the frame budget (`Batch`, `Knee`,
`governor_step`, `adjust_iter_scale` in `src/app/mod.rs`). The budget is
the display's refresh interval with vsync, else `1 / target_fps`; the
signal is the wall time between iterating frames, median-filtered over
three.

- **`Batch`** is the full batch: 128 workgroups of
  `iterations_per_thread`. `shape(knee, scale)` maps the governor's
  scale onto it by **shortening the dispatch first**, down to
  `ipt_floor` = `min(ipt, max(3 × burn_in, 64))`, and only then shedding
  workgroups. Persistent orbits make a shorter dispatch cost no
  trajectory depth.
- **`Batch::min_scale`** is one workgroup at the floor's length, the
  smallest batch.
- **`adjust_iter_scale`**: a single frame four or more budgets late
  sheds proportionally at once; a median above 1.3 sheds by 1.15; a
  median below 1.05 grows by 1.15.
- **`Knee`**: at the floor and still over budget for eight frames, the
  governor probes wider widths (doubling, four frames each) and holds
  the widest that cost at most 1.25 × the narrower — on a GPU that runs
  the whole batch at once, width costs nothing and shedding it only
  loses throughput.
- **Filling the frame** (`fill_step`, `gpu::timer`): the governor only
  sheds from the full batch, and under vsync the frame lasts the refresh
  interval however little the GPU did -- a light flame's full batch used
  about 2 ms of a 16.7 ms frame, so the app drew 490M iterations a
  second where the CLI drew 4.2G. At the full batch, the trajectory
  dispatch is timed on the GPU (timestamp queries, one measurement in
  flight, never waited on), and the dispatch is made LONGER -- `fill`
  times the iterations a thread, at full width -- toward half the frame
  budget (`FILL_TARGET`), growing at most 1.25x a measurement, falling at
  once, at most 64x and 16,384 a thread. Persistent orbits make the
  length free for the picture. A late frame gives back the fill before
  the scale. The fill starts again from the batch at every restart of
  the accumulation, in overwrite mode and during playback (a change may
  change the cost), and a measurement tagged before the restart is not
  used. Without timestamp queries (some browsers) it stays at the batch.
  Measured at 754x786 on a GTX 1660 SUPER: 4 billion iterations of Julian
  Disc in 1.6 s (8.1 s before; the CLI 0.94 s), 1.8 s at the default 256
  a thread; Grand Julian 5.5 s (8.1 s; the CLI 2.9 s); heavy flames,
  which the governor already sheds, unchanged.
- **The pacing cap** (`BatchCap`, `capped_scale` in `src/app/governor.rs`):
  under vsync, while a flame plays or is edited, the batch may not grow
  past where the frame's whole GPU span (the frame timer's) fills 70% of
  the budget, and above it it comes down at the governor's slew. Under
  vsync a frame on time reads the refresh however full the GPU is, so the
  governor grows a heavy batch until frames miss, and they then alternate
  long and short. The span is split by a linear model into the dispatch
  (its timed cost a thread-iteration, times what the frame dispatched)
  and the rest, so the cap does not move with the batch. Not in a still,
  which the fill lengthens instead, nor without vsync, where the frame
  intervals follow the GPU already. Measured at 60 Hz, a 2D flame whose
  full batch was about two frames' work: 45 fps, 40% of the frames over
  25 ms and 43% under 8, uncapped; 59.8 fps and none either way, the batch
  held at 35%, capped. Light flames (a full batch under the cap) keep the
  full batch.
- **`Batch::while_planning`**: while a Focused Rendering plan is being
  made, the dispatch is capped to the floor's length and the governor
  stops measuring — the planner shares the render's queue.
- **`Batch::fixed`**: with the governor off (Rendering menu,
  `SystemSettings::frame_governor`), every frame dispatches
  `SystemSettings::manual_workgroups` (default 128) at the full length,
  still capped while a plan is made.

The governor resets its timing when iteration stops. The same switch,
the Rendering menu's Quality Governor, also sets a flame's render scale
from the samples a frame plots, and governs escape-time, terrains and
simulations (`src/app/governor.rs`;
[quality-governor.md](../projects/quality-governor.md)).

### Configuration to GPU

All edits flow through `ConfigManager`; `process_gpu_updates`
(`src/app/gpu_updates.rs`) maps its pending actions onto the renderer:

| action | renderer calls |
| --- | --- |
| `update_flame` | `update_flame` — flame, scene state, solid settings; may recompile and recreate depth or feature buffers; does not reset |
| `update_shading` | `set_solid_shading`, or `update_flame` plus a reset when the change flips depth capture, shadow capture, a shadowed light's direction, or fog ownership |
| `update_view` | `set_deterministic_rng`, `update_iterations` (rewrites `GpuParams`) |
| `update_palette` | `set_palette_size` if it changed, `update_palette`, `set_color_mode`, `update_path_features` |
| `update_tone_curve` | `update_curve_lut` |
| `reset_accumulation` | `reset` |

`reset(encoder, queue, …)` clears both accumulators, the histogram
(except a census tail), the accumulator depth buffer and the path ids;
zeroes the counters; arms depth priming; and drops the shade pass's
temporal history. Its view arguments are unused. It does not clear the
orbit buffer.

**`load_config(device, encoder, queue, config, palette, ipt, burn_in)`**
is the full reset point, used for file and preset loads, undo and redo
(`App::import_config`) and every headless render: it plans Focused
Rendering, recompiles shaders as needed, uploads transforms, variation
parameters, attachments, subflames, xaos and bias tables, sets solid
state and the depth region, resizes and uploads the palette, writes
`GpuParams` and `TonemapParams` and the curve LUT, clears the
accumulation and resets the counters.

**Sticky shaders.** `load_config` and `update_flame` compile against
the *sticky superset* — the flame's variations plus recently used ones
at weight 0, in a canonical order — so toggling a variation is a
pipeline-cache hit rather than a rebuild. Every path that packs buffers
from a caller's flame (`resize`, `update_path_features`,
`set_variation_params`) must use the same map
(`StickyVariations::augmented`), or the compiled offsets and the packed
parameters disagree and the flame collapses to one pixel.
With sticky on, even a fresh renderer compiles the canonical order,
which changes f32 summation order (a ULP-class difference), so
`set_sticky_enabled(false)` turns it off where output must match a
baseline: `render()` and the video loop do. `sticky_stats` and
`shader_rebuild_stats` report it. See
[SHADERS.md](SHADERS.md);
[sticky-shader-compilation.md](../archive/projects/sticky-shader-compilation.md).

### Resize

`resize(device, encoder, queue, w, h, flame, …)` rebuilds `FlameBuffers`
at the new size (packing against the sticky map), repacks a Focused
Rendering plan's table before any bind group is made, re-applies the
solid depth region and census tail, marks the init pass dirty, restores
the xaos buffer and analytic-blur slots, rebuilds every bind group,
recreates `fractal_texture`, and resets. The caller must then restore
the palette, colour mode, path features, tone map and curve LUT, and
re-register the texture with egui — the app does all of these.

### Freezing during exports

A background export runs on its own device and competes for the same
card. While `export_status` is active, `freeze_viewport`
(`src/app/gpu_updates.rs`) makes the frame render nothing — the last
picture stays on screen — and destroys the engines that rebuild
themselves: the escape renderer and both terrains. The flame renderer
and a simulation's run are kept. Thawed, the engines are made again and
render from scratch. See [EXPORT.md](EXPORT.md), *GPU memory during an
export*.

---

## Focused Rendering hooks

Focused Rendering (deep zoom; config `cylinder_targeting`) lives in
`src/scene/` and its own shader arms; the renderer holds the plan and
decides when to make one. The hooks:

| hook | role |
| --- | --- |
| `load_config` | plans inline, or — for the expensive plans, with `set_background_planning` on (the app's renderer) — leaves it to the background job |
| `sync_cylinders` | every app frame: replans when the view or flame moves, swaps in the standby plan, polls the background job; `true` asks for a `load_config` because the shader arm changed |
| `take_plan_arrived` | a plan landed; the app resets the accumulation |
| `plans_running`, `planning_elapsed` | keep frames coming; cap the dispatch (`Batch::while_planning`) |
| `cylinder_iteration_scale` | the factor in `sample_density` |
| `targeting_state` | what the panel reports (`TargetingState`) |
| `plan_view` | the same decisions for the tiled exporter |

See [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md).

---

## Exports

The routing between this renderer and `HighResExporter` (tiled
sample-emit or CPU histogram, above one storage-buffer binding), 2x
supersampling, PNG metadata, the video loop and GPU memory during an
export are in [EXPORT.md](EXPORT.md). Points that touch the renderer:

- The video loop (`src/animation/export.rs`) owns one renderer and one
  `RenderEngines` for the whole video and calls `render_with` per
  frame, with sticky off.
- On the web, the app's custom-size PNG export (`src/app/mod.rs`) runs
  its own copy of the flame loop and tail rather than calling `render`,
  and frees the iteration buffers (`free_iteration_buffers`) before
  colour effects allocate.
- `destroy()` releases the renderer's buffers explicitly; on WebGPU a
  dropped buffer is freed only when the JS garbage collector notices.

---

## GPU memory

Per pixel, for a `with_palette_size` renderer at the frame's size:

| resource | bytes a pixel | when |
| --- | --- | --- |
| histogram + scratch histogram | 32 | always |
| accumulator pair (`Rgba32Float`) | 32 | always |
| `fractal_texture` (`Rgba8Unorm`) | 4 | always (also for `for_tail`) |
| solid depth region + accumulator depth | 8, plus 16 MiB of shadow maps | solid or lit 3D |
| shade pass (four `Rgba32Float`) | 64 | lazily, once a lit solid runs |
| DoF pass (four `Rgba32Float`) | 64 | lazily, once solid DoF runs |
| path ids | 4 | PathMap colour mode |
| density effect ping-pong (`Rgba32Float` × 2) | 32 | an enabled density effect |
| colour effect ping-pong (`Rgba8Unorm` × 2) | 8 | an enabled colour effect |

Layouts and fixed-size buffers: [BUFFERS.md](BUFFERS.md).

---

## Common tasks

| task | where |
| --- | --- |
| Add a pass to the tail | `render_with` and `App::render` (and `escape_tail`, `render_sim` if it applies to them); keep the input-priority chain identical in both |
| Change the accumulation formula | `shaders/accumulate.wgsl`, `AccumulateParams` in `gpu/buffers.rs`, `accumulate_pass` |
| Change brightness normalisation | `refresh_sample_density`; [TONE-MAPPING.md](TONE-MAPPING.md) |
| Add a per-frame uniform | `GpuParams` and `header.wgsl` together, and every `GpuParams { … }` literal (five in `compute_kernel.rs`, one each in `gpu/buffers.rs` and `export/high_res.rs`) — see [BUFFERS.md](BUFFERS.md) |
| Change what restarts orbits | the orbit key in `compute_pass`, and `OrbitFuses` if the burn-in rule changes |
| Tune the governor | `Batch`, `Knee`, `adjust_iter_scale` in `src/app/mod.rs`; its tests are in the same file |

---

## Design record

- [persistent-orbits.md](../archive/projects/persistent-orbits.md) — orbits kept between dispatches
- [accumulator-unification.md](../archive/projects/accumulator-unification.md) — the accumulator's units, `sample_density`, `Rgba32Float`
- [solid-rendering.md](../archive/projects/solid-rendering.md) — occlusion, shading, shadows, DoF
- [analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md) — the analytic blur
- [sticky-shader-compilation.md](../archive/projects/sticky-shader-compilation.md) — the pipeline LRU and the sticky superset
- [unified-rendering-api.md](../archive/projects/unified-rendering-api.md) — `render` / `render_with`
- [unified-render-pipeline.md](../archive/projects/unified-render-pipeline.md) — direct histogram against sample emit
- [overwrite-mode-fsm.md](../archive/overwrite-mode-fsm.md) — overwrite mode
- [materials.md](../archive/projects/materials.md) — the shared material
- [video-loop-and-sim-timeline.md](../archive/projects/video-loop-and-sim-timeline.md) — one video loop, `RenderEngines`
