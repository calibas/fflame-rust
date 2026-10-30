# The quality governor: a target frame rate, render scale, upscaling

**Status:** phase 1 measured 2026-10-29 (*Results*), but for allocation
cost. Of the faults it found, escape playback's part-drawn pictures are
fixed (2026-10-30); the flame's uneven pacing goes to the governor
(order of work, item 5). Item 3 is built (2026-10-30): render scale and
the Catmull-Rom upscale, sizes in pixels, an active simulation's
letterbox and the field resampler; so is item 4, FSR, in the viewport
and for simulations. The governor (item 5) is next. All seven decisions taken
2026-10-29: D1-D4 and D6 as recommended; D5 revised (a simulation's
field must go far lower than 50%, a solid to 25%); D7 the user's own (an active
simulation is letterboxed to its grid, never warped).

The user's words: "I want features to improve framerate, while preserving
as much quality as possible. I was thinking all the automated stuff would
be controlled by the governor, and the user would be able to set a target
framerate." It should act in three places: live animation playback in
the app, the preview while sliders move or the view is dragged, and as
optional ways to speed up an animation export.

This is items 1 and 2 of the plan made before motion blur: measurement,
then render scale, better upscaling and a GPU-timed governor. Item 3
(motion vectors: re-projection, temporal blending and upscaling) and the
optical-flow item are not in it; see *Later*.

## What exists

Every engine already holds its frame time somehow, each in its own way:

| What | Where | Knob | Signal | When |
|---|---|---|---|---|
| Flame batch governor | `app/mod.rs` (`adjust_iter_scale`, `Batch`, `Knee`, `fill_step`) | dispatch length, then workgroups; lengthened past the batch when short | frame intervals; the GPU's clock (`gpu::timer::PassTimer`) for the fill | always, with `frame_governor` on |
| Escape playback antialiasing | `app/aa_governor.rs` | supersampling factor | dropped frames under vsync | playback only |
| Escape chunk pacer | `escape::renderer`'s `TimestampPacer` | iterations a chunk | GPU timestamps, wall clock without them | always |
| Mode D preview | `ESCAPE_INTERACTION_WINDOW` (250 ms), `EscapeRenderer::set_preview` | a stride-2 walk: a quarter of the pixels | the time since the last edit | while editing a mode-D view |
| Terrain tiers | `escape::terrain_tiers` (`LIT_FRAME_MS`, `PATH_FRAME_MS`) | lit passes and traced samples a frame; Auto is lit while moving | the queue's completion time | always |
| Simulation | `SIM_CATCHUP_BUDGET_MS` | steps a display frame | wall clock | playback |
| Supersampling size cap | `EscapeRenderer::affordable_supersample` | the grid, reduced until it fits the device's render-pixel budget | memory, not time | always; an export makes up the rest by accumulation |

The last row is the "adaptive antialiasing based on view size": a memory
cap, not a time or content rule.

What none of them does:
- **Lower the resolution.** The viewport renders at egui's logical size
  (`panel_viewer.rs`), and egui stretches it to the screen with linear
  filtering (`register_native_texture(.., FilterMode::Linear)`). On a 150%
  display that already means rendering at two-thirds of the physical
  resolution, upscaled bilinearly.
- **Share a target.** The flame governor's budget is the refresh interval
  under vsync, or `target_fps` without it (`frame_budget`). Nothing lets
  the user ask for 30 fps at better quality.
- **Measure the whole frame on the GPU.** `PassTimer` times one pass.
  Measured on 2026-10-29, the terrain's queue-completion timing read 38 ms
  for a ~10 ms lit pass under load.

## The model

### Contexts

| Context | What the governor does |
|---|---|
| **Still** | Nothing: full quality, refined progressively as now |
| **Interacting** | A viewport gesture or an edit within the last 250 ms (the mode-D window, made app-wide): governed to the target |
| **Playing** | In-app playback: governed to the target, with hysteresis, since each change of rung is a visible change of quality |
| **Exporting** | Never timing-driven. A frame's quality must not depend on the machine's load, or the video flickers. The export's speed options are fixed rungs (*Exports*) |

When interaction or playback ends, the next frame starts at full
quality, as the escape renderer returns to the config's factor today.

### Fidelity, not the look

The governor gives up **fidelity**: resolution, samples, supersampling and
a terrain's section detail. It does not touch the **look**: effects, depth
of field, the render tier, the tone map. A governed frame is the same
picture, softer or grainier. Whether that rule is right is D4.

### The ladder

Each engine has an ordered list of rungs, cheapest-looking loss first.
The order comes from measurement (phase 1), not from guessing. The
candidates:

| Engine | Rungs, roughly in order |
|---|---|
| Escape 2D | supersampling down to 2x, then render scale 75%, 50%, then 1x (measured: *Results*) |
| Escape solid (mode D) | 1x, then 75%, 50%, 25%; the stride-2 preview becomes the 50% rung. 2x and above render the same picture as 2x, so 2x is the ceiling that matters |
| Flames | render scale below about 2 samples a display pixel a frame in 2D, 16 in 3D (*Results*) |
| Terrain | render scale for the lit passes; section detail while moving |
| Simulation | a bound grid's field scale, down to 12.5%, chosen when playback starts and held (below); never a fixed grid |

The ceiling is the setting. The governor never goes above the config's
supersampling, or above full resolution (D1 says which full).

**The floors (D5).** Render scale stops at 50% for escape, flames and
terrains: below that, no upscaler hides the loss. A simulation's field
goes down to its grid's own clamp, 12.5% (`SimGrid::cells_for`), because
real-time animation sometimes needs it that low. A mode-D solid goes
down to 25%: no rung reaches 60 fps at 1080p (*Results*), and a blocky
solid that moves beats a sharp one at 7 pictures a second. Solids are a
low priority throughout: the user doubts live solid animation will be
good on any rung, so they get the shared machinery and no work of their
own beyond it.

### Flames are different

A flame already meets its frame rate: the batch governor sheds samples.
What it loses is grain. Halving the resolution in each axis gives each
pixel four times the samples for the same work, so the noise in each
pixel halves (relative noise goes as one over the square root of the
count), but the picture gets softer. At low sample counts the softer
picture is the better one.

So the flame's rung is chosen by **samples per pixel per frame**, not by
time: below a threshold (from phase 1's measurements), drop a rung. The
batch governor stays as the inner loop. A lower rung also shrinks the
tone map, DoF and effects, which are per-pixel.

The flame's live preview already boosts brightness at low density
(`RenderModeFSM::needs_brightness_boost`). A lower rung puts more density
in each pixel, so check that the boost still triggers correctly.

### Simulations are different too

A simulation's cost is its steps: cells times steps a frame
(`steps_per_frame`). The tail at display size is minor. So the knob that
makes a simulation real-time is its field's resolution, and the user
sometimes has to set it very low to get there. The governor should do
the same when it has to.

But the field is content, not fidelity (SIMULATION.md: "the grid is the
information, not the resolution"). The rule works in cells, so a feature
keeps its size in cells:
- **Lowered:** the live field would be resampled into the coarser grid.
  At first it looks like the same picture, blockier. Then the pattern
  re-forms at its own size in cells, which is larger on the screen, so
  there is less of it.
- **Raised back:** the field would be resampled up, blurred, and the
  finer pattern re-forms over the following steps.

**"Would be": the resampler does not exist yet.** Any change of a bound
grid's cell count reseeds the run today: a window resize, and also the
scale slider, though SIMULATION.md says `SimResample` "interpolates the
live field". `UpdateType::SimResample` sets `resample_sim`, and nothing
reads it. `SimRenderer::resize` reseeds whenever the grid's size changes,
deferring the carry-over as "a phase-3 concern (`resample_into`)". The
governor's scale changes need it, and so does the slider (*The field
resampler*, below). A resize does not: an active run is letterboxed to
its grid instead (*A resize mid-session*).

A rung change in the middle of a run is therefore visible, and not as a
loss of sharpness. So the simulation's rule:
- **A bound grid only.** A `Fixed` grid's cell count is the config's
  contract (reproducible at any output size), and the governor never
  touches it.
- **Chosen once, when playback starts, and held.** Playback means the
  timeline or the panel's transport. The governor measures the GPU time
  of a step at the config's scale. Since cost goes with cells, it
  predicts the largest scale at which `steps_per_frame` steps fit the
  budget, and picks it from a few fixed values (100, 75, 50, 37.5, 25,
  18.75, 12.5%). It changes the scale mid-run only when the pick is far
  wrong: still under half the target frame rate, or more than four times
  over budget.
- **Never while interacting.** A drag that resampled on its first frame
  and again on release would disturb the run twice per drag. Interacting
  keeps whatever scale is in use.
- **Stop restores the config's scale**, as moving the panel's slider back
  would. A still and an export render at the config's grid, re-run from
  the seed as they are today.
- **Below 12.5%, or a fixed grid that still doesn't fit:** playback paces
  itself as it does now (holding until the steps catch up). The step
  rate is the content's speed, so the governor does not cut
  `steps_per_frame`.
- **The new upscalers are offered to simulations too.** The resolve
  already interpolates the field's state before colouring it
  (`SimUpscale`: nearest, bilinear, bicubic). Each new upscaler becomes
  another `SimUpscale` choice: the field coloured at its own size, then
  the new pass to the output. FSR's edge-adaptive pass, for instance,
  keeps a pattern's edges crisp where interpolating the state rounds
  them. Upscaling is cheap next to the steps (the user: "the simulation
  costs are mostly the sim itself"), so any choice is affordable at any
  scale. Governed playback shows the config's choice: at 12.5% a field
  is upscaled 8x, and `Nearest` (the default) shows 8x8 blocks. Whether
  governed playback should switch to a smooth upscaler is a phase 2
  question, to answer by eye. New `SimUpscale` names go on the end of
  `NAMES`; the engine contract does not list them.
- **A simulation terrain** is the same field as its height map, so the
  same rule applies.

Choosing the scale for the whole playback breaks D4 (fidelity only)
once: a governed simulation is a different run from its still. The user
accepted that (D6), with a *Fit to target* button in the Simulation
panel as well: it writes the predicted scale to the config as an
undoable edit, for a still or an export at the real-time scale.

### A resize mid-session

The user starts playback, then resizes the window or goes full screen.
For every engine, the resize changes the cost of every rung, and for a
bound simulation grid it also changes the field.

**Every engine:**
- **A resize is a new cost, not noise.** Cost goes with pixels, so the
  governor re-predicts the rung for the new size from its measured cost a
  pixel, at once. Holding the old rung and shedding over seconds is
  wrong: full screen on a 1080p display can be three times the pixels of
  a docked viewport. The frames right after a resize are not its cost
  (`aa_governor`'s `SETTLE`). A resize restarts the accumulation anyway,
  so a rung change there costs nothing extra on screen.
- **A window being dragged** resizes nearly every frame, and each resize
  reallocates. While the size keeps changing, the current rung is held
  and nothing is re-predicted. The new rung is picked once the size has
  held for about 200 ms (the web already debounces resizes at 100 ms).
  Going full screen is a single jump, so its rung is picked at once.
- Upscaling to physical pixels (D1) costs little at any size.
- An export is not affected: its size is the config's `image_size`.

**A bound simulation grid: an active run never changes its field**
(D7). Today a resize restarts the run, because a bound grid's cell count
follows the window. While a simulation is active (the timeline playing
or the panel's transport running), the field is never warped, stretched,
extended or resampled for a resize:
- **The viewport letterboxes to the run's grid.** This reuses the
  existing letterbox (`viewport_letterbox`): `view_controls::letterbox_rect`,
  the largest rect of the picture's shape centred in the panel, with
  black bars around it. It is handed the grid's size instead of
  `image_size`. The renderer's size follows that rect, as it does with
  the setting on, so the grid's shape is the viewport's. Bars show only
  when the panel's shape differs from the grid's. A resize that keeps
  the shape shows none.
- **The grid keeps its cells, too.** Governed, the governor holds a cell
  budget (the cells whose `steps_per_frame` steps fit the target), and at
  the grid's own shape that budget is the same grid. Ungoverned, the
  active run keeps its grid as well. Either way, `cells_for` is fed the
  scale that returns the run's grid at the new size: the old grid's width
  over the new rect's. So going full screen shows the same field larger,
  at the same speed, upscaled by the simulation's own resolve. The run
  goes on at its step index, without replaying from the seed.
- **Pause or stop releases the letterbox.** The viewport fills its panel
  again, and the bound grid follows it at the config's scale, re-run from
  the seed as a resize does today. The picture at rest is the still the
  export renders. The cell budget is remembered, as `aa_governor`
  remembers its factor, so resuming lands on it without measuring again.
- **With the letterbox setting on**, the viewport already has
  `image_size`'s shape and a window resize never changes the grid's
  shape; the lock changes nothing. A **fixed grid** never follows the
  window at all; its resolve's fit decides how it fills the view.
- **One detail:** an unletterboxed viewport renders under the tab bar
  (its height includes the bar's strip; `panel_viewer.rs`), and a
  letterboxed one does not. The lock takes the grid's own size, strip
  included, so the shape is exact and the rect a few pixels smaller.

### The field resampler

A resize no longer needs one: an active run keeps its grid, and a
stopped one re-runs from the seed. What still changes a live grid's
cells is the scale itself: the panel's slider (whose `SimResample` is
documented and not built) and the governor's rare re-pick mid-run.
Both are uniform, the same shape at another size, so the resampler never
stretches.

`resample_into` (the deferral in `SimRenderer::resize`), from the warp
stage's samplers (`sim::assembler`: the warp already resamples the field
through an inverse map, with nearest, bilinear and Catmull-Rom). By the
kind of state:
- **continuous state** (reaction-diffusion and the other PDE models):
  interpolated, Catmull-Rom by default;
- **integer state** (sandpiles, cellular automata: "integer state rides
  in an f32 channel"): nearest, which is exact. The warp's notes already
  say why bilinear has no meaning there;
- **an agent population**: positions scaled into the new grid, with the
  count kept for the session. `ensure_agents` reseeds on a change of
  count, and the population parameter is a share of the grid's cells, so
  this needs care. As built, an agent model reseeds instead (Physarum,
  DLA); carrying the population is left for when it is wanted;
- **anything it cannot carry** (layer memory slices, if they turn out
  not to resample cleanly) reseeds, as every model does today.

The model's kind of state is a property of its `ModelDef`, with a test
that every registered model has one.

## Render scale and upscaling

**The pipeline.** The engine renders at the internal size (display x
scale). The tail (tone map, density effects) runs at the internal size.
An **upscale pass** draws the display-sized texture that egui shows.
Upscaling after the tone map is correct for a display image, and it is
the input FSR expects.

**The upscalers:**
- **Bilinear**, today's.
- **Catmull-Rom bicubic.** One small pass, sharper than bilinear. The
  default.
- **FSR 1** (AMD FidelityFX Super Resolution 1.0; MIT-licensed; EASU
  edge-adaptive upsampling, then RCAS sharpening). Two passes, ported to
  WGSL, so it works on the web. It is built for 1.3x-2x a side from
  antialiased input, which escape frames are. Flames' grain is not
  antialiased input, and RCAS sharpening may amplify it. Measure before
  defaulting to it anywhere.

**A free gain at 100%.** The upscale pass can target the screen's
physical pixels. A 150% display then gets a bicubic or FSR stretch instead
of egui's bilinear one, at today's render cost (D1).

**Effects in pixels.** Density effects' radii are in pixels
(`density_blur`, `bilateral_blur` and `sharpen`: "radius in pixels" in
their WGSL). At 50%, a 3-pixel blur covers 6 display pixels. So:
- pixel-sized parameters scale by the render scale (`bilateral_blur`
  rounds its radius to an integer: check that it holds up at small
  scales);
- colour effects that are per-pixel (vignette, grain, chromatic
  aberration, the noise generators) move after the upscale, at the
  display size, where they stay sharp. First check how the chain is
  ordered; `distance_field`'s jump flood is not cheap at display size.

**Allocation.** Changing the scale reallocates each engine's per-pixel
buffers, as a resize does today. In the contexts that change scale, an
edit or a camera move restarts the accumulation anyway, so the cost is
the allocation alone. Measure it (phase 1). Allocating at full size and
rendering into a region is the fallback if it shows. That touches every
engine's passes, so not before it is needed.

**A manual setting first.** Rendering menu: *Render scale* (100/75/50%)
and *Upscaler*, so each rung can be judged by eye before anything
automatic uses it.

**As built (2026-10-30).** `SystemSettings::render_scale`, `upscaler`
(Catmull-Rom by default; bilinear; nearest, which the user asked for after
trying the first two: "Catmull-Rom looks a little better than Bilinear,
they both look a bit blurry at 50% upscaling") and `render_physical_pixels` (D1's setting),
above the Rendering menu's chaos-game rows so they act in escape-time
too. `app/render_scale.rs` turns the viewport's points into two sizes:
the display's (points times `pixels_per_point`) and the render's (points,
or the display's with physical pixels on, times the scale). The renderer,
the effect chain and every engine drawn through it render at the second;
`renderer::upscale` draws the tail's last picture (the colour effects'
output or the fractal texture) up to the first, and egui shows it 1:1.
When the two sizes are one, as at 100% on a 1x screen, the pass does not
run. A simulation renders at the viewport's points whatever the setting
(`Control::RenderScale` greys the rows there): a bound grid's cells
follow that size, and its grid scale is its render scale. The upscale is
on the 8-bit tone-mapped picture, as egui's own filter was. Export with
Background and Transparent render at `image_size` as before; Export
Current Image saves the picture as rendered, so at 50% it is half the
size. The path query's pixel is mapped into the renderer's pixels.

**Sizes in pixels, as built (2026-10-30).** A parameter in pixels has
always meant a logical point, so the unit is the *pixel scale*: render
pixels per point (render height over the viewport's height in points,
exactly 1 at full size and for every export). The app sets it each frame
on the flame renderer, the effect chain and the escape renderer, and it
is applied where each parameter is packed for the GPU, never to a config
copy. Scaled:
- the flame's spatial filter radius (`FlameRenderer::set_pixel_scale`),
  the solid shade pass's gap fill (whole pixels, `gap_in_pixels`) and the
  DoF pass's 32-pixel cap;
- the density blur's radius, sharpen's radius, the bilateral blur's
  radius and spatial sigma, chromatic aberration's amount and the
  kaleidoscope's edge offset (`EffectParams::pixel_scale`, the struct's
  last slot, read in the shader);
- escape-time's output pixel (`EscapeParams.px_scale`, a new float that
  `esc_supersample` returns: the supersample factor times the pixel
  scale), which the relief's height, softness and texture scale, the
  distance-estimate colourings and the relief's slope cancellations all
  measure in, so they move together.

Not scaled, deliberately: the colour effects stay before the upscale
(their order is the user's, so only all of them could move after it, and
on a high-DPI screen that would change today's look at 100%), so film
grain's per-pixel hash and the Sobel stencil turn coarser at a lower
scale; the noise-fighting kernels (the 9x9 normal fit, the a-trous
strides, the path tracer's denoiser, the DoF's depth prepass) stay in
render pixels, since at a lower scale each pixel carries more samples;
mode D's march keeps stepping by the integer factor in `flags` (solids
are a low priority); KF's texture-overlay step is baked into the WGSL in
whole render pixels; a terrain's section softness is in texels, which
grow with a lower render height; custom formulas' `#screenpixel` and the
like are the user's. An export at a size other than the viewport's has
the same mismatch for most of these, as it always had.

### FSR 1, as built (2026-10-30)

`UpscaleFilter::Fsr`: AMD's FidelityFX Super Resolution 1.0, EASU then
RCAS, ported line for line from the 32-bit paths of ffx_fsr1.h
v1.20210629 and the ffx_a.h helpers they call (`shaders/upscale_fsr.wgsl`,
both MIT notices kept), two passes through an intermediate picture
(`renderer::upscale`). Where it differs from the source: taps by
`textureLoad`, clamped, rather than gathers through a clamping sampler;
the alpha bilinear and passed through; `FSR_RCAS_DENOISE` on, for flames'
grain; RCAS's two limiters, which divide by zero on a ring of pure black
or white, take HLSL's answer there explicitly (`rcas_lobe`), since WGSL
leaves a NaN's `max()` to the platform and Metal's fast-math keeps none: a
plain guard would have turned the sharpening off for saturated colours on
black. RCAS sharpness 0.2 stops. Catmull-Rom stays the default.

Judged with the loss metric (`upscaler_comparison`, then `frame_metrics.py
upscalers`: each calibration rung below full size drawn up to 1080p by
the viewport's own pass). The viewport's bilinear matches PIL's to the
hundredth (seahorse, 2x at 75%: 4.40 both), so the pass is the filter it
claims. Against the 6x-supersampled references, mae:

| scene, rung | nearest | bilinear | Catmull-Rom | FSR |
|---|---|---|---|---|
| seahorse, 2x at 50% | 5.73 | 4.79 | 5.04 | 5.06 |
| newton, 2x at 50% | 0.60 | 0.67 | 0.61 | 0.59 |
| solid, 2x at 50% | 1.27 | 1.41 | 1.34 | 1.30 |
| 2D flame, 64 samples at 75% | 7.57 | 7.63 | 6.91 | 6.74 |
| 2D flame, 16 samples at 50% | 9.84 | 10.87 | 9.99 | 9.77 |
| 3D flame, 16 samples at 50% | 8.09 | 7.02 | 7.31 | 7.48 |

FSR and Catmull-Rom are within a few percent of each other everywhere.
FSR is the best of the four on the sharp 2D flame, a little behind
Catmull-Rom on the soft 3D one and on the aliased escape pictures, where
bilinear's blur scores best, as phase 1 found: the measure rewards blur,
so sharpness is the eye's to judge. Not timed: each is one or two passes
at display size.

## The governor

**One governor** (`app/governor.rs`): the target, the context, the
measured costs and each engine's rung. The existing loops become its
inner loops and keep their jobs: the flame batch, the escape chunk pacer
and the terrain pass budgets. The governor picks the rung from how far
the inner loop falls short:
- **escape:** the GPU time to settle a picture against the budget;
- **flames:** the samples per pixel the batch reached;
- **terrain:** the lit pass's GPU time.

`aa_governor`'s supersampling ladder folds in as escape's first rungs.
Its lessons carry over: frames right after a change are not its cost;
shed on several dropped frames, not one; a failed probe up waits twice as
long next time.

**What it measures.** For the GPU, the span from the first pass of the
frame's engine work to the last pass of its tail, as two timestamps on
one query set (`timestamp_writes` on the first and last passes; WebGPU
offers nothing finer). For the CPU, the frame's wall-clock work: egui,
encoding, uploads. When the CPU is the bottleneck, as the motion-blur
flame uploads were, lowering a rung does not help, and the governor must
not shed quality for it.

**Stepping.** Cost goes with pixels (scale squared, times supersampling
squared), so with a measured cost the governor can predict a rung and go
straight to it instead of probing one step at a time. Probing up stays,
with backoff, because predictions are wrong at the edges (escape
interiors, fills).

**Without timestamps** (browsers without `timestamp-query`): frame
intervals and dropped frames, as `aa_governor` does today.

**The target.** A new setting, *Target frame rate*: the display's refresh
by default, or a number. Under vsync at 60 Hz, a 30 fps target gives each
frame two refreshes of work. `target_fps` stays the cap without vsync.
D2 asks what playback targets.

**The readout.** A governed frame is never silent. The viewport corner
already says why escape's supersampling is lower (`aa_governor::readout`:
"Governor" or "Size"). It grows to cover the scale: "67% · 2x AA,
governed".

**Holding a drag** ([slow-frame-slider-hold.md](slow-frame-slider-hold.md))
is the other half. The governor keeps frames fast where a cheaper picture
exists; the hold covers settings with none (an iteration count, a
compile). The hold plan's note that "governed engines hide the cost"
covers more engines once this is built.

## Exports

An export renders at fixed rungs, chosen by the user, and every frame the
same, so the output never depends on load. Options in the video export
(and the CLI: `--render-scale`, `--upscaler`):
- **Render scale + upscaler**: 1080p rendered at 67% and upscaled with
  FSR, for instance.
- **Escape supersampling** below the config's.
- **Flame samples** below the config's `max_iterations`.
- **A simulation's field scale** below a bound grid's. The export panel
  already says which grid an export uses. With this option the export is
  a different run from the still, and the panel says that too.

Whether the export can pick the rung itself from a time per frame,
measured on its first frames and then locked, is D3.

## Phase 1: measurement

1. **The frame's cost.** The GPU frame span and the CPU work, logged per
   frame behind an environment variable (as the motion-blur work's
   temporary `FFLAME_DEBUG_ANIM` hook did, but kept).
2. **The metrics** (`scripts/frame_metrics.py`):
   - **loss**: a governed frame against the full-quality frame of the same
     moment, by block, for what a rung gives up;
   - **flicker**: consecutive frames' block differences, for exports.
3. **Reference scenes** (`tests/perf/` or `output/`): an escape zoom, an
   escape parameter sweep, a mode-D solid, a 2D flame morph, a 3D flame,
   a terrain fly-through, and two simulations: one stepped on a fixed
   grid, and one on a bound grid at 1080p that needs a low scale to play
   in real time.
4. **The calibration table**: for each engine and rung, the ms saved
   and the loss, at 1080p on the GTX 1660 SUPER. This orders the ladder
   and sets the flame's samples-per-pixel thresholds.
5. **Allocation cost** of a scale change per engine, for the
   region-rendering question.
6. **A simulation step's cost against its cells**, per model family,
   to check that the scale prediction holds. A cost proportional to
   cells is the assumption; spectral models (`sim::spectral`) and
   multi-layer couplings may not follow it.

The tools (TESTING-GUIDE.md §5, *Frame cost in the app*): the frame log
(`src/app/frame_log.rs`, `FFLAME_FRAME_LOG`), the calibration
(`renderer::calibration::governor_calibration`), the scenes
(`scripts/governor_scenes.py`), the playback runner
(`scripts/governor_playback.py`) and the metrics.

### Results (2026-10-29, GTX 1660 SUPER)

Items 1-4 and 6 are measured. Item 5, allocation, is not yet. The tables
are in `output/governor/` (`calibration.csv`, `loss.csv`, `table.md`) and
the logs in `output/governor/playback/`; both are gitignored, and the
tools make them again. Loss is in levels of 255 against each scene's
reference (6x supersampling; 1024 samples a pixel for flames): the mean
(mae) and the 95th percentile of 16x16 blocks (p95), after a bilinear
upscale.

**The measure's bias.** Against a supersampled reference, a blurred
picture scores better than an aliased one of the same detail. The numbers
order rungs within an engine. They do not settle sharpness, or which
upscaler looks best: the manual setting (order of work, item 3) is for
judging that by eye.

**Escape-time 2D: drop the scale before the supersampling.** At 1080p:

| scene | ss2 100% | ss1 100% | ss2 75% | ss2 50% | ss1 50% |
|---|---|---|---|---|---|
| seahorse | 91 ms, 4.6 | 52 ms, 9.6 | 62 ms, 4.4 | 36 ms, 4.8 | 20 ms, 8.6 |
| deep zoom (perturbed) | 357 ms, 12.9 | 137 ms, 27.2 | 232 ms, 12.8 | 116 ms, 14.2 | 59 ms, 25.1 |
| newton (smooth) | 50 ms, 0.27 | 34 ms, 0.62 | 32 ms, 0.44 | 23 ms, 0.66 | 18 ms, 0.78 |

2x at 50% costs less than 1x at 100% and loses half as much: aliasing
loses more than resolution does. So the 2D ladder is supersampling down
to 2x, then 75%, then 50%, and 1x last. Above 2x, the size cap decides:
at 1080p the grid stops at 2x, and an export makes the rest up by
accumulation. The calibration's 3x and 4x are the same picture (2x grid,
2x2 accumulated), and the viewport, which does not accumulate, cannot
have them at 1080p at all.

**Mode D solids do not reach 60 fps on any rung.** At 1080p, lit: 1x
225 ms, 1x at 50% 89 ms; 2x and above are one picture (loss 0.0 against
the reference). In the app at 754x786, a picture took about 110 ms of
GPU time. Two faults showed in playback:
- **Half-drawn frames.** A picture renders in about four chunks over
  frames (`escape_dirty` stays set until it settles), and the viewport
  shows each one. The user saw "extremely choppy, with what looked like
  half-drawn frames in between". The pattern repeats every 7 frames:
  one with ~65 ms of CPU time and no GPU work, two nearly idle, four
  chunks of 15-33 ms. About 7 pictures a second.
- **The 65 ms frame.** The next frame waiting for a surface image
  while the GPU works through the chunks queued before it: in the second
  run (below) the long frames were 63-86 ms of `acquire_ms`, with 1-5 ms
  of the frame's own work. Backpressure, not CPU work.

**Flames: the scale wins only below about 2 samples a display pixel.**
At the same sample count, 2D (plastic-sierpinski) at 1080p:

| samples a display pixel | 100% | 75% | 50% |
|---|---|---|---|
| 0.25 | 26.1 | 22.0 | 21.9 |
| 1 | 15.7 | 13.1 | 13.5 |
| 4 | 10.0 | 10.1 | 11.8 |
| 16 | 5.4 | 8.3 | 10.9 |
| 64 | 2.7 | 7.6 | 10.6 |

The 3D flame (bubble-3d) is softer, so its break-even is higher: 50% wins
up to 16 samples a display pixel (7.0 against 8.5) and loses at 64 (6.4
against 5.4). Below 1 sample, most of the loss is brightness: the
export's Log tone map darkens a sparse picture. The app's live-preview
boost corrects some of that, so the in-app break-even may be lower. At
the same samples, a smaller scale is also faster (64 samples: 144, 120
and 74 ms), as the histogram's atomics hit a smaller buffer. The
harness's times include building the renderer and reading the picture
back (about 25 ms of every 1080p flame row), so the per-sample cost is
the difference between rows: about 0.77 ms per million samples at
1080p.

**Upscalers: small differences.** For escape-time, bilinear scores best
by about 5% (blur, as above). For flames at 16 samples and more, bicubic
and Lanczos beat bilinear by 5-10%. Neither result makes the case for
Catmull-Rom as the default; judging it by eye does.

**Simulations: cost goes with cells, as predicted.** ms a step at 1080p:
Lenia 27, McCabe 4.3, SmoothLife 3.5, Physarum 1.1, Cahn-Hilliard 0.61,
DLA 0.54, sandpile 0.39, Gray-Scott 0.31, cyclic CA 0.20. Per million
cell-steps, Lenia holds 13.0-13.6 ms from 100% down to 18.75%, McCabe
2.1-2.4 ms down to 37.5%, SmoothLife 1.6-1.7 ms down to 25%. Below that,
a fixed per-step overhead takes over (~0.1 ms). The cheap models sit at
the measurement's noise floor (one read negative), and need no
governing at these sizes.

**In the app today** (the user's layout: a 754x786 viewport in a
1440x810 window; 60 Hz, vsync, `iterations_per_thread` 3600). Every
scene but the solid holds 60 fps at this size. Two faults:
- **Escape's antialiasing governor hunts.** In the zoom it probes 3x
  about every 4 s and holds it for 8-11 frames at 23-36 ms (dropped
  frames, a visible change of antialiasing), then steps back to 2x.
  The user saw it ("Looked like AA changed in the middle"). 3x costs
  about 2.2x what 2x does, so a governor that predicts from cost would
  never try it.
- **The flame's batch governor alternates.** At 3600 iterations a
  thread the batch is ~29 million samples, ~16 ms of GPU time: the
  budget's edge. The dispatch length flips between 3600 and 3130 almost
  every frame, and frame intervals alternate between ~1 ms and ~32 ms
  (44% over 25 ms). The median interval reads 60 fps; what the display
  shows is uneven, and the animation's clock, which advances by each
  frame's interval (`AnimationController::update`), jumps as unevenly.
  The governor's median of three ratios flips with the intervals: shed,
  grow, shed.

**The second run** (2026-10-30, after order of work item 2's first
fixes; the user's window now 1920x1080 at 120 Hz, a 1079x1056 viewport):
- **The solid holds its pictures.** 1% of frames showed a part-drawn
  picture (the first, before any was held). The user: "Flame attractor
  was slow, but not partially-drawn frames anymore."
- **The antialiasing governor still switches.** The user: "The escape
  time switched between AA settings." At 120 Hz it dropped from 3x to
  1x, then probed 2x twice (8-11 frames at ~20 ms against 1x's 5.3 ms
  and an 8.3 ms budget) before staying at 1x. By the calibration, 2x at
  50% would cost about what 1x at 100% does and lose half as much.
- **A cap on the dispatch did not fix the flame's alternation.** Tried:
  the batch may not grow past where the measured trajectory dispatch
  fills 80% of the budget. The batch settled at about a third (engine
  ~7 ms of an 8.3 ms frame) and frames still alternated (~1 ms and
  ~15 ms; an acquire wait of ~13 ms every other frame), so it was taken
  out. Pacing broke between ~65% (the escape zoom at 5.4 ms paced evenly)
  and ~84% of the budget in GPU time. The dispatch alone is the wrong
  measure: the tail, the UI and the compositor's share vary with the
  size, the effects and the display. The whole frame's GPU span
  (`gpu::timer::FrameTimer`, the frame log's) is the measure.

## Order of work

1. **Measurement** (phase 1). Done but for allocation (its item 5);
   *Results*.
2. **The faults phase 1 found**, each on its own. Done: playback shows
   only settled escape pictures (`EscapeRenderer::present_output`: the
   last finished one stays up while the next renders in chunks). The
   flame batch governor's alternation at the budget's edge goes to the
   governor (item 5), which measures the frame's GPU span anyway
   (*The second run*); decided 2026-10-30. It shows only where a full
   batch is nearly a frame's work (the user's 3,600 iterations a
   thread, a heavy flame), not at the default 256. The antialiasing
   governor's hunting is left to the governor too, which replaces it.
3. **Render scale + Catmull-Rom upscale**, manual, in every engine
   (built 2026-10-30: *As built*); the effects' pixel scaling (built 2026-10-30: *Sizes in pixels*);
   the readout (the menu shows the setting; a governed frame's readout
   is the governor's). For simulations: **the active run's letterbox**,
   which on its own stops a resize or full screen from restarting a live
   run (built 2026-10-30: `app/sim_lock.rs`; it letterboxes only once the
   viewport is not the size the run began at, so starting a run moves
   nothing, and a scale edit while running lets go for a frame so the
   edit takes effect); then **the field resampler**, which makes the
   scale slider do what its docs say, before the governor uses it (built
   2026-10-30: `ModelDef::state`, `SimRenderer::carry_field`; agent models
   reseed, as the plan allowed, rather than carrying their population).
4. **FSR 1** as a second upscaler, judged against bicubic with the loss
   metric, and offered to simulations as a `SimUpscale` choice. Built
   2026-10-30: the viewport's (*FSR 1, as built*) and the simulation's
   (`SimUpscale::Fsr`: the cells coloured at about a cell a pixel, then
   FSR on the linear picture, encoded around its passes; SIMULATION.md).
5. **The governor**: GPU frame timing, the contexts, the target setting,
   the ladders. Escape first (it gains the most), then flames, terrains,
   simulations. The mode-D stride preview is replaced by the 50% rung,
   once that is shown to look as good or better.
6. **Exports**: the fixed rungs in the video export and the CLI.
7. **The web**: the fallback signal; tried in a browser.

## Decisions

| # | Decision | Taken |
|---|---|---|
| D1 | What full resolution means on a high-DPI screen: logical points (today) or physical pixels | Decided: upscale to physical pixels at today's render size (sharper, no cost); a setting to render at physical pixels, off by default |
| D2 | What in-app playback targets: the display's refresh, the setting, or the animation's own fps | Decided: the *Target frame rate* setting, defaulting to the display's refresh; the animation's fps offered as a choice |
| D3 | An export's rung: chosen by the user, or picked automatically from a time per frame and locked | Decided: chosen by the user first; automatic later if wanted |
| D4 | Whether the governor may change the look (effects, DoF, the render tier) or only fidelity | Decided: fidelity only (but see D6) |
| D5 | The lowest render scale | Revised: 50% for escape, flames and terrains; 12.5% for a bound simulation grid's field, its clamp. The user: "Simulation field resolution sometimes needs to be very low for real-time animation." Then 25% for mode-D solids, after phase 1 (2026-10-29). The user: "I'm not entirely sure we'll be able to do good live animations with solid flame attractors period, so we don't need to spend too much time on them." |
| D6 | Whether the governor may lower a bound simulation grid's scale (a different run, not a softer one: the one exception to D4), or only offer *Fit to target* for the user to apply | Decided: the governor, as a cell budget held for the session (re-fitted to a resize), restored on stop; *Fit to target* as well |
| D7 | How a live field crosses a resize that changes its shape: resampled (stretched), extended (new margins from the init), or neither | Decided: neither. The user: "I don't really want to warp the image in any way ... reuse the existing letterbox system when an active simulation's aspect ratio changes." An active run is letterboxed to its grid and keeps its cells; a stopped one follows the window |

## Later

- **Content-adaptive antialiasing** (item 5 before): extra samples only
  where a pixel's samples disagree (boundaries, filaments), as the path
  tracer's adaptive sampling already does. For escape-time it could give
  most of 8x's quality at a fraction of the cost. It would fit this
  ladder as a rung, but it is a renderer change of its own.
- **Motion vectors** (item 3 before): re-projection of the last frame,
  temporal blending, temporal upscaling. The upscale pass and the
  measurement here are what they would build on.
- **Region rendering**, if phase 1 shows reallocation costs.

## Tests to write

- The ladder: rungs ordered, never above the ceiling, the floor held.
- The contexts: interacting, then still, returns to full quality on the
  next frame; an export is deterministic at a fixed rung (same bytes
  twice).
- Render scale: a still at 50% upscaled is within a bound of the full
  frame (the loss metric); pixel-sized effect radii scale.
- The governor's stepping, as `aa_governor`'s tests do: a model of cost
  per rung; sheds under load, holds under jitter, probes up with backoff,
  does not shed when CPU-bound.
- The web fallback without timestamps.
- Simulations: a fixed grid is never governed; a bound grid's scale is
  picked from the predicted cost at the start of playback, holds through
  interaction and jitter, and is the config's again after stop; never
  below 12.5%.
- A resize mid-session: the rung is re-predicted for the new size; a
  window drag picks once the size has held.
- An active simulation through a resize to another shape and to full
  screen: letterboxed to its grid; the field's texture and step index
  unchanged (no reseed, no replay); the letterbox released on stop, and
  the grid then the window's at the config's scale. With the letterbox
  setting on, or a fixed grid, nothing changes.
- The resampler: every registered model has a kind of state; integer
  state comes through bit-exact; a continuous field resampled down and
  back up is within a bound of the original; an agent population keeps
  its count.
