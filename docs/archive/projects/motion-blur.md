# Motion blur

> **Archived 2026-10-29** -- finished: every engine blurred in the export and the viewport, the web app's playback tried by the user, the playback speed fixed where it was ours to fix. What is left -- the API's field, the viewport's path-traced solid, per-thread time for flames, a terrain's lit-pass timing -- is in [escape-sim-followups.md](../../projects/escape-sim-followups.md) (*Temporal antialiasing*) and, for the API, [v0.6.0-release.md](../../projects/v0.6.0-release.md). Read [ANIMATION.md](../../main/ANIMATION.md) (*Motion blur*) for how it works now; this is the record of what was planned, decided and measured.

**Status:** planned 2026-10-29, branch `motion-blur`. Built 2026-10-29:
phase 1 (the model, the shutter, the panel) and phase 2 (escape-time
2D, export and viewport; checked in the CLI and, with a startup hook
and a window capture, in the app), phase 3 (flames), phase 4
(simulation), phase 5 (terrains and solids; the viewport's path-traced
solid is not blurred), each likewise checked, and phase 6 (the
governor, which needed no change, and the docs); then the playback's
speed (*Performance in the app*). The user tried the web app's
playback the same day. Open: the API does not store the setting;
per-thread time for flames (no copies at few samples) is not built.

Motion blur for animations: each frame is the average of the picture
over a short **shutter** around its time, as a camera's exposure is. It
is a setting of the animation (the Animation panel), so the same blur
plays in the app -- on the web too -- and renders in the video export.

Why now: a frame of an animation is a correct but under-sampled picture
of detail finer than a pixel, and as things move the detail a pixel
lands on changes, so it shimmers. Sampling the shutter is the correct
temporal antialiasing for that: what does not move within the shutter
stays sharp, and only what moves blurs, by an amount the shutter sets.
It is also a look users of a camera expect.

Prior thinking this builds on:
[PROPOSAL-artistic-effects.md](../experimental/PROPOSAL-artistic-effects.md)
(*2. Motion Blur*: per-sample time dithering in the chaos game, deferred
for want of an animation system) and
[escape-new-families.md](../escape-time/escape-new-families.md)
§7 (escape renders are deterministic, so time sampling is pure
supersampling; a frame's sub-frames share the deep-zoom reference orbit
and BLA table; per-pixel jitter breaks finite-difference shading).

---

## The model

`Animation::motion_blur: MotionBlur`, skipped in the `.anim` while it
is the default, so old files load unchanged with it off:

| Field | Default | Meaning |
|---|---|---|
| `enabled` | false | |
| `shutter` | 1/60 s | How long the shutter is open, in seconds of **animation time** |
| `samples` | 8 | Moments sampled within it, 2-64 |

**The shutter is a duration, not an angle (D1).** In-app playback has
no frame rate, and a duration looks the same in the app, at any export
frame rate, and at any playback speed (0.25x shows the frames an export
would, slowly). The panel shows the equivalent angle at 30 fps (1/60 s
is 180 degrees).

**Centred on the frame's time (D2).** Sample *k* of *K* is at
`t + ((k + 0.5) / K - 0.5) × shutter`: stratified, deterministic (a
video renders the same twice), the frame's own time in the middle so
the blur does not lag the motion. Past the ends: `Once` clamps to the
animation, `Loop` wraps (a loop's seam blurs like any other moment),
`PingPong` reflects.

**A box filter (D3)**, every moment weighted alike. flam3's
`temporal_filter_type` (box, gaussian, exp) is the precedent if another
shape is wanted.

**The frame's own config is the centre.** Each moment is the base
config with the tracks applied at that moment's time; the frame's
tone mapping, effects and background are the centre's, applied once to
the averaged image.

**Signals.** A signal track with smoothing keeps state, advanced once a
frame. The moments are evaluated without touching it: a smoothed track
holds its frame value across the shutter, an unsmoothed one is read at
each moment.

**In-app, only while playing (D4).** Paused or scrubbing, the picture
is the sharp frame at the playhead, which is what editing wants.
(Previewing a paused frame blurred, as the export will render it, is a
possible later toggle.)

**Online (D6).** The API stores an animation's name, duration, tracks,
generators, base config and loop mode; motion blur is none of those, so
an animation saved online loses it until the API stores it too. Tell the
API repository (RELEASE.md §3) when this ships.

---

## The render

`RenderJob::with_motion(&[FractalConfig])`: the frame's moments, beside
`job.config`, the centre. Every engine averages the moments in its
linear accumulation, before the shared tail (tonemap, effects, read
back), and dispatch stays in `render_with`, so the CLI, the app's video
export and anything else that renders a frame inherit it. The viewport
drives the engines itself, so each engine gets a viewport path too.

### Escape-time (2D): phase 2

A moment is a whole render, so neighbouring pixels stay at neighbouring
positions and finite-difference shading is unaffected (§7's jitter
warning applies to per-pixel time, which this is not).

- **Export:** `K` renders, one per moment, folded by the existing
  accumulation (`begin_accumulation` / `accumulate_sample`), counted in
  renders rather than per axis. Where the grid already falls short of
  the antialiasing and accumulates `extra²` displaced renders, each
  displacement is taken at every moment: `K × extra²` renders.
- **Viewport:** the frame's moments rendered in turn, each chunked
  until settled as now, folded into the accumulation; the frame settles
  after the last, so paced playback holds the clock meanwhile. The
  accumulation is complete only after its last moment (coverage builds
  up), so the viewport shows the last finished frame until the next is
  done. The antialiasing governor sheds the supersampling first;
  `K` is the look and is not governed.
- **Deep zoom:** the moments of a camera move are a small displacement
  of one view; they should share the reference orbit (§7). To verify,
  not assume.
- **Built:** `render_escape` renders every displacement at every
  moment, each prepared for its own config (a mode-D flame, a texture),
  into `begin_accumulation_of(renders)`. In the viewport
  (`app/motion_blur.rs`) a dirty frame while playing starts its moments;
  each settled moment folds in; the last presents the accumulation
  (`present_accumulation`), which `escape_view` shows until the next
  frame's is done. Stopped or paused, the presented frame is dropped
  and the sharp frame drawn. A mode-D flame takes each moment's flame in
  the viewport too. Measured: a 640x360 spin, 8 moments, 0.4 s a frame
  against 0.1 s sharp.

### Flames: phase 3

The cheapest engine in the app: a playback frame is one batch whose
histogram is folded once, so splitting it into `K` dispatches, each with
its moment's flame, blurs the frame for the same work.

- **Viewport:** the frame's batch split into `K` dispatches; each
  writes its moment's flame and camera, all plot into the one histogram,
  one fold. The orbits continue from moment to moment -- the moments
  are close, and a contractive orbit settles onto the next attractor in
  a few iterations -- so the moments do not each pay a burn-in. To
  measure: the burn-in it saves against any bias it leaves.
- **Export:** the frame's batches go round the moments, so each moment
  gets the same share of the samples, into one accumulation.
- **Inlined constants:** the CLI compiles the flame into the shader
  (`enable_inlined_constants`); `K` moments would be `K` compiles a
  frame. A blurred frame reads its flame from the buffers instead.
- Later, if the dispatch per moment costs too much: each thread draws
  its own moment and uses parameters interpolated between the shutter's
  ends (the proposal's time dithering) -- one dispatch, a shader change,
  linear within the shutter. It would also end the stepping that a fast
  motion shows with few samples (distinct copies of a spinning flame's
  outer rings at 8), at no cost in samples.
- **Built:** `FlameRenderer::set_moment` uploads a moment's flame and
  view; `set_continuing_frame` marks a frame's later moments, whose
  samples add to the frame's (overwrite mode too) and whose flame takes
  over the orbits without a restart (`compute_pass`). The export's
  dispatches go round the moments and end on a whole round; the
  viewport shares the governed dispatch among the moments (each a K-th
  of its iterations, its own submission after what the frame recorded)
  and puts the frame's own flame back after (since the performance work,
  only when a sharp frame follows: `restore_frame`). A blurred video turns the
  CLI's inlined constants off. Not under Focused Rendering (its plans
  are one view's), nor for a subflame viewed in isolation. Measured: a
  640x360 spin, 0.4 s a frame blurred and sharp alike; the orbits going
  on across moments leave no bias at 0.15% of a block's density (32M
  samples a moment), one restart a frame.

### Simulation: phase 4

The picture is the grid at a step count, and the moments are step
counts within the shutter: the frame averages the coloured field at
those steps, which the grid passes through anyway walking forward. A
held backward leg (`timeline_target_applies`) is not blurred: the
picture is held. A colouring parameter's track takes its value at each
moment.

- **Built:** the averaging is shared now
  (`renderer::average::ImageAverage`, the escape engine's accumulation
  moved out, with a first add's params and every other's made at
  `begin`, so several adds can share a submission); the simulation
  renderer keeps one (`begin_moments`, `add_moment`, `present_moments`).
  The export walks the field to each moment's step count in turn,
  colours it as that moment colours it, and folds it in; one behind the
  field reseeds and runs, as a backward frame does. The viewport walks a
  committed step count's moments the same way, over display frames,
  holding playback until the last (`sim_motion`); one behind the field
  is held where the field is, except a frame the timeline committed
  backward (a loop's wrap, a turnaround), whose first moment runs back.
  A grid the timeline does not step (no step track, a held leg) is
  recoloured at each moment where they differ in more than their step
  count.
- **A loop's seam:** a simulation's moments clamp there instead of
  wrapping (`Animation::moments(t, wrap)`, from `moment_configs`): the
  run starts again at the seam, and a moment from the end carried the
  field ahead of the timeline, which then held it there -- found in the
  app, where playback stuck at the last step.
- Checked: a Cyclic Cellular Automaton stepped 0 to 600 in 2 s, blurred
  over 1/30 s, in the CLI and in the app (startup hook, removed):
  the moving waves wash toward their average colour, the steps following
  the timeline.

### Terrains and solids: phase 5

- **A camera move alone:** the sections are the picture's, keyed
  without the view (`picture_key`), so the moments share them; each of
  the lit tier's grid passes, or each path-traced sample, is taken at
  its own moment. Nearly free.
- **A picture that changes within the shutter** (a formula parameter):
  each moment needs its own sections -- `K` times the frame's sections.
  The export draws them; the viewport cannot afford it and blurs the
  camera alone over the frame's ground (D5, revised).
- Mode D solids (lit): `K` renders averaged; path traced: the samples
  spread over the moments.
- **Built:** the tiers take the moments (`TierInputs::moments`, the view
  a function of the jitter and the moment): the lit grid's passes go
  round them as a Latin square over the jitter grid (row by row, each
  moment took a column -- every sample to one side of the pixel, and a
  sideways turn blurred wider than its moments), a pass for every moment
  at least; the path tracer's samples are the moments' in turn, the
  noise threshold ending only the last share. The escape terrain's
  export (`render_escape_terrain`) draws moments that share the frame's
  picture (`EscapeTerrain::same_picture`) on one ground at the frame's
  cost, and moments whose pictures differ each on its own ground, each
  still with its share of the antialiasing (`moment_share`: the
  supersampling also sets the sections' detail, so the ground is drawn
  from the moment itself), averaged. A simulation's terrain the same way:
  moments differing in the terrain's own settings share the field's
  ground; moments whose field differs each walk it, colour it and draw a
  still, averaged. The viewports pass the moments to the tiers. A solid
  path traced in an export traces each moment with its share of the
  samples, averaged; the viewport's traced solid is not blurred.
- Checked: a terrain turning a radian in 2 s, blurred over 1/30 s, in
  the app (startup hook, removed): the fine dark filaments smear along
  the turn. Tests: still moments are the frame exactly; a camera's move
  on one ground is 0.15 of the sharp frame's distance from the mean of
  the moments' own stills on that ground; a Julia constant moving, a
  ground a moment, 0.08; a simulation terrain blurs over a turn and over
  its steps; a traced solid keeps its light.

### Phase 6: the governor and the docs

The in-app cost meets the frame governor (supersampling first); the
docs: ANIMATION.md (the setting, playback), EXPORT.md (the video loop),
the engines' docs where they changed; the API note.

- **Built:** no governor change was needed. Escape playback's
  antialiasing governor sheds the supersampling as before, measured on
  finished blurred frames (`done`, not each moment's settling); a
  flame's moments share the governed dispatch; the simulation and the
  terrains are paced. The samples are the look and are not governed:
  shedding them would show the copies of a fast motion.
- **Docs:** ANIMATION.md (*Motion blur*, the `.anim` field, the panel
  row), EXPORT.md (the video frame), ESCAPE.md (the shared average),
  RENDERER.md (the moments' orbits), TERRAIN.md (the tiers' moments),
  and the API note and a browser check in
  [v0.6.0-release.md](../../projects/v0.6.0-release.md).

---

## Performance in the app (2026-10-29)

Measured playing, sharp against 8 samples blurred, animation frames a
second (a startup hook, removed):

| Engine | Sharp | Blurred, first | Blurred, after |
|---|---|---|---|
| Flame (a spin) | ~95 | ~59 | ~80 |
| Simulation (a cellular automaton, stepped) | 120 | 15 | ~115 |
| Terrain (a turn) | 5.5 | 2.5 | 2.5 |
| Escape-time (a spin, mode D) | 3.5 | 0.5 | 0.5 |

What was wrong, and what is the work itself:

- **Flames: an upload.** Each moment's `update_flame` sent the whole
  variation-parameter table -- 384 slots of 6.4 KB, 2.5 MB, nearly all
  zeros -- for a flame of four transforms: 1.1 ms of its 1.5, nine times
  a frame, and the governor cut each moment's dispatch to 8 iterations a
  thread to pay for it. Now the tables write the slots used
  (`write_regions`; every edit pays less too), and the frame's own flame
  is put back once, when a sharp frame follows (`restore_frame`), not
  after every blurred one. Left: about 0.5 ms a moment of `update_flame`
  -- the shader check, the transform and parameter packing. A lighter
  path for a frame's later moments could skip the check, but proving a
  moment has its first's shape costs half of what it saves.
- **Simulation: one moment a display frame.** 120 to 15 was 120 / 8,
  each moment cheap. Now a display frame walks as many moments as its
  budget of steps reaches.
- **Terrains: the passes.** A blurred frame draws a pass a moment at
  least (9 at a 2x2 grid, against 4), each about 10 ms at this size; and
  a turning camera's new sections, one a display frame, which a sharp
  frame waits for too (the slower the frame, the further the turn, the
  more sections). The viewport now draws as many passes a frame as fit
  12 ms at the measured cost, a new view's first alone; the measure is
  to the GPU's completion of everything queued, so under load it reads
  high (38 ms) and the passes stay one a frame. GPU timestamps around
  the lit pass would tell; not done.
- **Escape-time: the work.** Every moment is a whole render. Not
  changed: rendering each moment at `supersample / sqrt(samples)`, so a
  blurred frame costs about a sharp one, trades each moment's
  antialiasing for the frame's cost -- a decision, not a fix, and the
  user kept the full supersampling (D7).

## Order of work

1. **The model, the shutter, the panel** -- no picture changes yet.
   `MotionBlur` on `Animation` (serde default, skipped while default);
   the moments' times per loop mode; evaluating the moments without
   touching signal smoothing; the moments' configs (the export's apply
   path, so the app's moment is the export's); the Animation panel's
   row; tests.
2. **Escape-time 2D**, export and viewport.
3. **Flames**, export and viewport.
4. **Simulation.**
5. **Terrains and solids.**
6. **Governor and docs.**

## Decisions

| # | Decision | Default taken |
|---|---|---|
| D1 | Shutter as a duration or an angle | Duration in animation seconds; angle at 30 fps shown |
| D2 | Where the shutter sits | Centred; clamp / wrap / reflect by loop mode |
| D3 | Filter | Box |
| D4 | Blur when paused | No: playing and exports only |
| D5 | Terrain picture blur in the app | Revised: the viewport blurs the camera alone, over the frame's ground; the export draws a ground a moment |
| D6 | Online animations | Lost until the API stores it; tell the API repo |
| D7 | An escape-time moment's supersampling | Full at every moment (the user, 2026-10-29: "escape-time is fine the way it is"); sharing it among the moments was the alternative |

## Tests to write

- The moments: stratified and centred; clamped, wrapped, reflected.
- Evaluating the moments leaves signal smoothing untouched.
- An `.anim` without the field loads with blur off; the default is not
  written.
- Escape: a still animation renders the same blurred as sharp; a pan,
  blurred, is the mean of its moments' renders.
- Flames: a still animation keeps its density and brightness blurred.
