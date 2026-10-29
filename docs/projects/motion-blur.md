# Motion blur

**Status:** planned 2026-10-29, branch `motion-blur`. Phase 1 (the
model, the shutter, the panel) built 2026-10-29; the panel's setting
changes no picture until phase 2. Phase 2 (escape-time) next.

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
[PROPOSAL-artistic-effects.md](../archive/experimental/PROPOSAL-artistic-effects.md)
(*2. Motion Blur*: per-sample time dithering in the chaos game, deferred
for want of an animation system) and
[escape-new-families.md](../archive/escape-time/escape-new-families.md)
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
  linear within the shutter.

### Simulation: phase 4

The picture is the grid at a step count, and the moments are step
counts within the shutter: the frame averages the coloured field at
those steps, which the grid passes through anyway walking forward. A
held backward leg (`timeline_target_applies`) is not blurred: the
picture is held. A colouring parameter's track takes its value at each
moment.

### Terrains and solids: phase 5

- **A camera move alone:** the sections are the picture's, keyed
  without the view (`picture_key`), so the moments share them; each of
  the lit tier's grid passes, or each path-traced sample, is taken at
  its own moment. Nearly free.
- **A picture that changes within the shutter** (a formula parameter):
  each moment needs its own sections -- `K` times the frame's sections.
  Same in the app and the export (D5), with the samples as the cost to
  choose; revisit if it is too slow to use.
- Mode D solids (lit): `K` renders averaged; path traced: the samples
  spread over the moments.

### Phase 6: the governor and the docs

The in-app cost meets the frame governor (supersampling first); the
docs: ANIMATION.md (the setting, playback), EXPORT.md (the video loop),
the engines' docs where they changed; the API note.

---

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
| D5 | Terrain picture blur in the app | Same as the export; samples are the cost |
| D6 | Online animations | Lost until the API stores it; tell the API repo |

## Tests to write

- The moments: stratified and centred; clamped, wrapped, reflected.
- Evaluating the moments leaves signal smoothing untouched.
- An `.anim` without the field loads with blur off; the default is not
  written.
- Escape: a still animation renders the same blurred as sharp; a pan,
  blurred, is the mean of its moments' renders.
- Flames: a still animation keeps its density and brightness blurred.
