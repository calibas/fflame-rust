# Animation

**Overview:** An animation is a list of **tracks**, each driving one
config parameter over time from keyframes or from a named **signal**
(an audio analysis, a live audio input, a procedural waveform, a
`.signal` file). The app plays it by writing the evaluated values into
the active config every frame; the video exporter evaluates the same
tracks at each frame's time and renders that frame's config through the
still-render path. Every render mode animates: flames, escape-time
fractals and simulations, where a `Sim.Steps` track is the run itself.

**See also:**
- [EXPORT.md](EXPORT.md) — video export: the frame loop, FFmpeg,
  engines that outlive a frame, progress and cancellation
- [UI.md](UI.md) — the panels, and *The timeline and the simulation
  grid*
- [SIMULATION.md](SIMULATION.md) — why `sim.steps` is a cap to the
  transport and a target to the timeline
- [SCRIPTING.md](SCRIPTING.md) — the `anim` object: animations written
  by Rhai scripts
- [CONFIG.md](CONFIG.md) — `ConfigPath`, `ConfigManager`, undo
- [ONLINE.md](ONLINE.md) — saving and loading animations online
- [ESCAPE.md](ESCAPE.md), [EFFECTS.md](EFFECTS.md),
  [VARIATIONS.md](VARIATIONS.md) — what the escape, effect and
  variation targets mean

**Code locations:**
- [src/animation/mod.rs](../../src/animation/mod.rs) — `Animation`,
  `Track`, `TrackSource`, `Keyframe`, `LoopMode`, the track binding
- [src/animation/interpolation.rs](../../src/animation/interpolation.rs)
  — `Interpolation`, `EasingFunction`, `geometric_lerp`
- [src/animation/controller.rs](../../src/animation/controller.rs) —
  `AnimationController`: playback clock and track evaluation
- [src/animation/export.rs](../../src/animation/export.rs) — the video
  loop, and the exporter's own apply path (`apply_animation_values`)
- [src/app/animation_update.rs](../../src/app/animation_update.rs) —
  playback in the app: pacing, silent apply, the simulation hand-off
- [src/config/delta.rs](../../src/config/delta.rs) —
  `ConfigPath::to_string_key` / `from_string_key`,
  `json_to_config_value`
- [src/ui/animation_panel.rs](../../src/ui/animation_panel.rs),
  [src/ui/track_editor.rs](../../src/ui/track_editor.rs),
  [src/ui/target_selector.rs](../../src/ui/target_selector.rs) — the
  Animation panel, the Add/Edit Track window, the target picker
- [src/signal/mod.rs](../../src/signal/mod.rs),
  [src/signal/generator.rs](../../src/signal/generator.rs) — `Signal`,
  `SignalManager`, the `.signal` format, generators
- [src/ui/signal_panel.rs](../../src/ui/signal_panel.rs) — the Signals
  panel
- [src/audio/](../../src/audio/) — decode, offline analysis, live
  capture, playback

---

## The model

**`Animation`** is one self-contained timeline:

| field | means |
| --- | --- |
| `name` | user-facing name; the default file name on save |
| `base_config` | optional embedded `FractalConfig` — the flame (or escape picture, or simulation) the tracks were written for |
| `duration` | seconds; the panel allows 0.1–3600 |
| `tracks` | `Vec<Track>`, in order; several tracks may share a target (the last one evaluated wins) |
| `generators` | signal generator configs (below), regenerated into signals on load |
| `loop_mode` | `Once`, `Loop` or `PingPong` |

**`Track`** has a `target` (a `ConfigPath` string key), a
`flame_target` (`Main` or `Subflame { index }`), a `source` and an
`interpolation`. The source is a tagged enum:

- `Keyframes { keyframes }` — each `Keyframe` is `time` (seconds),
  `value` (a JSON value) and `easing` (the curve to the next keyframe).
- `Signal { signal_name, min_output, max_output, smoothing, start_time,
  end_time, fade_in, fade_in_easing, fade_out, fade_out_easing }` — see
  [Signal tracks](#signal-tracks).

The app holds one `AnimationController`, which owns at most one
animation. Opening the Animation panel creates an empty
`"New Animation"` of 10 s if none is loaded, so an animation with no
tracks always exists once the panel has been shown.

---

## The `.anim` file

An animation is saved as pretty-printed JSON with the extension
**`.anim`**; the load and save dialogs also accept `.json`. (The
superseded design's `.ffanim` was never built.)

```json
{
  "name": "Dive",
  "base_config": { "version": 3, "...": "the full FractalConfig" },
  "duration": 10.0,
  "tracks": [
    {
      "target": "Zoom",
      "flame_target": "Main",
      "source": {
        "type": "Keyframes",
        "keyframes": [
          { "time": 0.0,  "value": 1.0,   "easing": "Linear" },
          { "time": 10.0, "value": 100.0, "easing": "EaseInOutSine" }
        ]
      },
      "interpolation": "Exponential"
    },
    {
      "target": "Transform.0.Rotation",
      "flame_target": { "Subflame": { "index": 0 } },
      "source": {
        "type": "Signal", "signal_name": "gen_1",
        "min_output": 0.0, "max_output": 6.2832, "smoothing": 0.0,
        "start_time": 0.0, "end_time": 0.0,
        "fade_in": 1.0, "fade_in_easing": "Linear",
        "fade_out": 0.0, "fade_out_easing": "Linear"
      },
      "interpolation": "Linear"
    }
  ],
  "generators": [
    { "name": "gen_1", "waveform": "Sine", "frequency": 0.5, "phase": 0.0 }
  ],
  "loop_mode": "Loop"
}
```

**Configs do not embed animations; animations embed configs.** Saving
from the Animation panel always writes the active config into
`base_config` (desktop: a file dialog; web: a browser download named
from the animation's name). Loading an `.anim` that carries one loads
that config first, as an ordinary undoable config load, then binds the
tracks against it. An `.anim` without one applies to whatever is open.

**Versioning.** `base_config` is written through the canonical config
serializer, so it carries a `version`, and read through the versioned
migration. An embedded config with no `version` is assumed to be **v2**
(`FractalConfig::from_json_value_with_default_version(.., 2)`), which is
what the pre-versioning serializer wrote.

**Older files.** Tracks stored as a map from target to track (the old
format) load as a list. A track with no `flame_target` targets `Main`.
`generators` and the signal-track timing fields default when absent.

**What the file does not carry:** signal data. Generators are stored as
their configs and regenerated; audio analysis and `.signal` files are
loaded separately, and a signal track refers to them only by name.

---

## Track targets

A target is a `ConfigPath` in its string form
(`ConfigPath::to_string_key` / `from_string_key`):

| target | example key |
| --- | --- |
| view, tone, colour | `Zoom`, `PanX`, `Exposure`, `PaletteRotation` |
| a transform | `Transform.0.Weight`, `Transform.0.Affine.A`, `Transform.0.Rotation` |
| a variation weight / parameter | `Transform.0.Variation.julian`, `Transform.0.VariationParam.julian.power` |
| linked / final pools | `LinkedTransform.1.Scale`, `FinalTransform.0.PostAffine.C` |
| xaos | `Xaos.0.2` |
| effects | `DensityEffect.0.Enabled`, `ColorEffect.1.<param>` |
| escape | `Escape.ZoomLog2`, `Escape.JuliaIm`, `Escape.FormulaParam.<name>` |
| simulation | `Sim.Steps`, `Sim.ModelParam.feed`, `Sim.Layer.1.Param.<name>`, `Sim.Terrain.Height` |

Indices in keys are 0-based; the track list shows `Transform.N` 1-based.
Angles are stored in **radians** (`is_angle_path` in
[track_editor.rs](../../src/ui/track_editor.rs) lists them) and the
editor shows and accepts degrees.

**`flame_target`** says which flame a per-flame path resolves against:
the main flame or a subflame by position. Paths that live on the
`FractalConfig` itself (view, tone, effects, escape, simulation) ignore
it.

### Binding: tracks follow their items

A track's key names an item by **index**, and indices move when a
transform, subflame or effect is added, deleted or reordered. Each
track therefore carries a runtime-only `TrackBinding` (never
serialized): the session id of the subflame it targets and of the list
item its key indexes (`TargetBinding::Transform | Linked | Final |
ColorEffect | DensityEffect`).

- `Animation::bind_to_config` resolves every track's ids. It runs after
  an animation is loaded, after a config is loaded with an animation
  open, and when the exporter starts; `Track::bind` does it for one track
  the editor creates or retargets.
- When `ConfigManager` reports a structural change to one of those
  lists (`UpdateAction::structural_changed`),
  `Animation::rebind_targets` rewrites each track's index and
  `flame_target` to wherever its bound item now sits.
- A track whose bound item is gone keeps its key and its binding, so an
  undo that restores the item restores the track. The track list marks
  it with a warning sign and the header counts them ("N broken").

The apply path never reads the binding: it parses `target` and indexes
directly. The binding's only job is keeping `target` current.

---

## What animates

Three things have to agree for a parameter to animate, and each is a
separate piece of code:

1. **The target picker offers it** —
   [target_selector.rs](../../src/ui/target_selector.rs).
2. **`json_to_config_value` converts it** —
   [delta.rs](../../src/config/delta.rs). This decides animatability:
   it returns `None` for a path that cannot be animated, and its match
   over `ConfigPath` is exhaustive with no wildcard, so a new path must
   be classified there.
3. **Something applies it.** In the app, `ConfigManager`'s own setter,
   silently. In video export, the exporter's `apply_config_value`, which
   handles the common paths directly and hands everything else to
   `ConfigManager::apply_value_detached` — the manager's setter run on a
   detached copy, same clamps and side effects.

Two tests in [export.rs](../../src/animation/export.rs) hold the third
to the first:
`every_offered_target_renders_in_the_video_as_in_the_app` applies every
offered target, in every render mode, through both the exporter and the
manager and requires identical configs; the escape and simulation
variants require every offered target to move the config at all.

### Value conversion

Keyframe values are JSON, and interpolation produces floats. Per path:

| path kind | accepts |
| --- | --- |
| float paths (most) | any number, cast to `f32` |
| pan (`PanX`/`PanY`) | float or double |
| integer paths (`Sim.Steps`, `Escape.MaxIter`, `MaxIterations`, `PaletteIndex`, …) | any number, **rounded** (negative and NaN become 0) — an interpolated `1000.0` must not be dropped |
| signed index paths (`SoloTransform`, post-symmetry type/order) | number, rounded |
| booleans | a JSON `true`/`false` only |
| enums (`TonemapMode`, `ColorMode`, `RenderMode`, …) | the serde name |
| `Pan`, `BackgroundColor`, escape shading colours | `[x, y]` / `[r, g, b]` arrays |
| `Escape.CamTargetX/Y/Z` | a finite number, stored as the decimal **string** the config uses |

### Not animatable

`json_to_config_value` returns `None`, deliberately, for:

- **Choices with no value in between:** formula, colouring, lens name,
  sim model, boundary rule, grid mode, seed, init kind, layer models and
  enables, coupling endpoints, warp and matte modes, shading model,
  transfer curve, terrain source and tier, path-tracer sample and bounce
  counts, and the like.
- **Exact or structural state:** the escape centre strings
  (`Escape.CenterRe/Im`), the reference-period hint, the terrain lock,
  the palette and tone curve, add/remove effect, variation phase
  priority and order, word removals.
- **Settings that re-render everything** on each change, such as the
  terrain prefilter and smoothing.

### What the picker offers

The picker groups targets by category and filters by the current mode:

| category | offered when | contents |
| --- | --- | --- |
| View | flame modes; 3D adds the camera, DoF, solid; fog in 3D and escape solids; lighting in 3D or any solid | zoom, pan, rotation, camera angles and position, DoF, fog, lights and material |
| Color | always | palette rotation and squeeze, speed factor, background R/G/B |
| Tone Mapping | always | exposure, gamma, threshold, brightness, vibrancy, saturation, hue shift, density scale, levels |
| Rendering | flame modes; 3D adds projection and depth weighting | blend factor, solo transform, perspective, depth-density compensation, far-density fade |
| Effects | when the config has effects | each effect's Enabled and parameters |
| Escape | Escape mode | zoom (log2), rotation, iterations, bailout, Julia seed, damping, the active formula's and colouring's parameters; then, as they apply, the solid or terrain camera, terrain and path-tracer controls, palette-map pivot, contrast, colouring layer, texture overlay, relief, and the lens amount and parameters |
| Simulation | Simulation mode | steps, dt (when a layer has one), view and warp rates, matte, each layer's model parameters, each colour layer's parameters, opacity and matte, coupling strengths, the terrain camera and ground |
| Xaos, Transform *n*, Linked *n*, Final *n* | wherever the flame is drawn: flame modes, an escape IFS formula, a simulation with `use_transforms` | weights, colour, opacity, affine and post-affine (raw and as origin/rotation/scale), active variations and their parameters |

Subflames appear as their own groups with only the per-flame
categories. Escape and Simulation targets are offered only in their
mode, but a track keeps applying if the mode later changes.

---

## Interpolation and easing

A keyframe track holds its first value before its first keyframe and
its last value after its last; a single keyframe is a constant. Between
two keyframes the **segment's start keyframe's easing** reshapes
normalized time, then the **track's interpolation** maps it to a value:

| `Interpolation` | between keyframes |
| --- | --- |
| `Step` | the first value until halfway, then the second (easing not applied) |
| `Linear` (default) | linear in the eased time |
| `Smooth` | smoothstep, `3t² − 2t³`, of the eased time |
| `Sinusoidal` | `(1 − cos πt) / 2` of the eased time |
| `Exponential` | equal **ratio** per unit time: `a · (b/a)^t` (`geometric_lerp`). The zoom curve: every doubling takes the same time, in either direction. Falls back to linear when the endpoints are zero or differ in sign. |

`EasingFunction` is one of `Linear`, `EaseIn`/`EaseOut`/`EaseInOut`
(quadratic, identical to the `*Quad` variants), the `*Cubic` and the
`*Sine` families.

Booleans switch at the halfway point; non-numeric values step there.
Evaluation assumes keyframes are in time order: adding a keyframe at the
playhead sorts the list, editing a time does not, and the editor has a
**Sort by time** button.

---

## Playback in the app

**The clock.** `AnimationController::update` advances
`current_time += dt × speed × direction`. Speed is chosen from 0.1×,
0.25×, 0.5×, 1×, 2×, 4×.

| `LoopMode` | at the end |
| --- | --- |
| `Once` | stops and returns to t = 0 |
| `Loop` | wraps to the start |
| `PingPong` | reverses; reverses again at t = 0 |

**Applying values.** Each frame the app evaluates every track at the
current time and writes the values with
`ConfigManager::update_param_silent_on(flame_target, path, value)`:
no undo entry, routed to the track's flame regardless of which flame
the editor is focused on.

**Undo.** Starting playback snapshots the config and puts
`ConfigManager` in animation mode, where `update_param` — the path the
panels' sliders use — writes silently too (`update_batch` does not
check the mode). Stopping or pausing leaves animation mode and
records one undo entry, `history.action.animation_exit`, from the
snapshot to the config as it stands. **Stop** then returns to t = 0 and
applies the tracks there; **Pause** leaves the picture at the playhead.

**Rendering while playing.**

- *Flame modes:* every frame is an overwrite frame (one batch, not
  accumulated), so the preview is grainier than a still; it refines
  once playback stops.
- *Escape:* the controller is held until the frame **settles**, then
  advanced by all the wall time that passed (`paced_playback_tick`).
  Playback slows rather than showing partial renders. With the frame
  governor on, the viewport's supersampling is shed and restored to
  keep settled frames on time
  ([aa_governor.rs](../../src/app/aa_governor.rs)); stills and exports
  use the config's factor.
- *Simulation:* paced the same way when the animation has a
  `Sim.Steps` track — see below.

**Scrubbing.** Dragging the timeline slider applies values silently on
every change; releasing it resets accumulation. The frame-step buttons step
1/60 s and clicking a track's bar jumps there; both count as a release.
With audio sync on, scrubbing pauses the audio; the release seeks it to
the playhead and resumes it if playback is running.

**Spacebar** toggles play/pause, but only when the animation has at
least one track (the auto-created empty animation would otherwise put
the renderer into playback mode with nothing moving). In Simulation mode
the spacebar runs the grid instead. The panel's Play button always plays.

---

## Simulation timelines

`Sim.Steps` is the one target that animates the **run**: the picture at
time *t* is the state `round(track(t))` steps from the seed. The
behaviour is described in full in [UI.md](UI.md) (*The timeline and the
simulation grid*); in short:

- The timeline **owns** the step count while an animation with a
  `Sim.Steps` track plays, or while a target it committed is still being
  walked to (`App::timeline_owns_sim`). The transport is inert then, and
  Run is disengaged when playback starts.
- The grid is advanced toward the target on a per-frame budget, and the
  controller is held until it gets there, so what plays is what exports.
- **Backward targets are held.** The rule is not invertible, so a lower
  step count means reseeding. `sim::timeline_target_applies` applies a
  falling target only on a discrete event — a scrubber release, a frame
  step, a `Loop` wrap, a `PingPong` turnaround at t = 0
  (`app::animation_update::playback_motion`). A track written to count
  down therefore plays in-app as a still of the highest state reached;
  export renders every frame of it.
- An animation **without** a step track leaves the simulation's own
  transport alone, so a colouring parameter can be animated over a
  free-running grid.

[SIMULATION.md](SIMULATION.md) explains why the step count is a cap to
the transport and a target to the timeline; [EXPORT.md](EXPORT.md)
(*Simulation video*) covers the cost of reversed tracks in export.

---

## The panels

### Animation panel

[animation_panel.rs](../../src/ui/animation_panel.rs), rendered by
`PanelViewer::render_animation_panel`. The **Animation** workspace
layout docks it under the viewport.

- **Transport:** play, pause, stop, frame step back and forward, speed,
  duration, loop mode, and **Sync audio** (play, pause, stop and seek
  the Signals panel's audio player with the timeline; it resyncs on
  drift over 100 ms).
- **Timeline:** time, duration and percentage, and a full-width
  scrubber whose geometry (`TimelineLayout`) the track rows align to.
- **Tracks:** one row per track — a label (flame prefix and path), a bar
  from first to last keyframe (a signal track spans the whole timeline),
  keyframe dots with a value tooltip, the playhead line, and Edit and
  delete buttons. Clicking a dot opens the track in the editor.
- **File:** name, **Save**, **Load**, **Export Animation** (opens the
  export window), and when signed in, **Save Online** plus the list of
  the online flame's animations with a Load button each (see
  [ONLINE.md](ONLINE.md)).

### Track editor

[track_editor.rs](../../src/ui/track_editor.rs). **Add Track** and
**Edit** open one floating window.

- **Type** (Add only): Keyframes or Signal. A track's type cannot be
  changed afterwards.
- **Target:** the hierarchical picker, with search
  ([target_selector.rs](../../src/ui/target_selector.rs)). Read-only
  when editing.
- **Keyframes:** the interpolation, then each keyframe's time, value
  and easing; **Add at current** (the parameter's current value at the
  playhead), **Add keyframe** (one second after the last, same value),
  **Sort by time**. Edits to an existing track apply as they are made;
  a new track is created by **Confirm**.
- **Signal:** the signal (any name `SignalManager::signal_names`
  returns, live ones included), output min and max, smoothing (0–0.99),
  start and end time (end 0 = the animation's end), fade-in and
  fade-out with their easings.

**Auto-fill.** Picking a target for a new keyframe track fills two
keyframes from the parameter's current value
(`initialize_preview_keyframes`): t = 0 at the value, t = duration at
`get_auto_fill_end_value`:

| target | end value |
| --- | --- |
| any rotation or yaw (transform, post-affine, view, camera, escape, sim view and terrain yaw) | start + 2π |
| `Transform.n.Color` | start + 1 |
| `PaletteRotation` | start + 1 |
| everything else | start (adjust by hand) |

A new keyframe track starts `Exponential` when the target is `Zoom`
and `Linear` otherwise (`initialize_preview_keyframes`); the dropdown
offers every mode.

---

## Signals

A **signal** is a sampled time series that a track maps onto a
parameter. The animation system does not know where one came from.

**`Signal`** ([src/signal/mod.rs](../../src/signal/mod.rs)): `name`,
`sample_rate` (Hz), `signal_type`, `data` (`Vec<f32>`) and optional
`metadata`. `value_at(t)` depends on the type:

| `SignalType` | `value_at(t)` |
| --- | --- |
| `Continuous` | linear between samples; the last sample past the end |
| `Trigger` | the nearest sample, no interpolation |
| `Scalar` | the first sample, whatever `t` is (e.g. `bpm`) |

**`SignalManager`** holds stored signals by name plus a list of live
**producers** (`SignalProducer`). `get_live_value(name, t)` asks each
active producer first, then falls back to the stored signal at `t`.

### Signal tracks

At animation time *t*, a signal track:

1. Is inactive outside `[start_time, end_time]` (`end_time` 0 means the
   animation's duration): it returns nothing, and the parameter keeps
   whatever value it has.
2. Reads the signal's value (expected 0–1).
3. With `smoothing` *s* > 0, applies an exponential moving average per
   track, `alpha = 1 − s^(dt / (1/60 s))` with *s* clamped to 0.99, so
   the response is the same at any frame rate.
4. Multiplies by a fade factor: the eased ramp over `fade_in` after the
   start and over `fade_out` before the end.
5. Outputs `min_output + fade × (max_output − min_output) × value`.

A fade therefore pulls the output toward `min_output`, not toward the
parameter's own value.

### Signal sources

| source | names | where it comes from |
| --- | --- | --- |
| audio analysis | `amplitude`, `energy_low`, `energy_mid`, `energy_high`, `spectral_centroid`, `spectral_flux`, `onset`, and when a tempo is found `bpm`, `beat`, `beat_phase` | loading an audio file in the Signals panel |
| live audio | `live_amplitude`, `live_energy_low/mid/high`, `live_spectral_centroid`, `live_spectral_flux`, `live_onset`, `live_bpm`, `live_beat_phase` | the live capture producer, registered at startup; values only while capturing |
| generators | the generator's name (`gen_1`, …) | the Signals panel; stored in the `.anim` |
| signal files | the name inside the file | **Load Signal…** in the Signals panel (desktop) |

All but `bpm` / `live_bpm` (beats per minute) are normalized to 0–1.

### Generators

`GeneratorConfig` ([generator.rs](../../src/signal/generator.rs)) is a
`name`, a `waveform` (`Sine`, `Triangle`, `Sawtooth`, `Square`,
`Noise`), a `frequency` in Hz (panel range 0.01–100) and a `phase`
(0–1). It is rendered into a `Continuous` signal sampled at **100 Hz**
over the animation's duration, output 0–1. `Noise` draws a new value
per sample from a PRNG seeded by the generator's name, so it is
reproducible and ignores the frequency.

Generators live in the Signals panel's state and are copied into
`Animation::generators` when the animation is saved, and back (then
regenerated) when one is loaded. A generator's signal is regenerated
when its row is edited; it is **not** regenerated when the animation's
duration changes, so past its original length it holds its last sample
until the row is touched.

### The `.signal` file

A little-endian binary, `SIGNAL_VERSION` 1:

| bytes | field |
| --- | --- |
| 4 | magic `FSIG` |
| 2 | version (`u16`) |
| 1 | type: 0 Continuous, 1 Trigger, 2 Scalar |
| 1 | flags (reserved, 0) |
| 8 | sample rate (`f64`) |
| 8 | sample count (`u64`) |
| 2 | name length (`u16`) |
| 4 | metadata length (`u32`) |
| … | name (UTF-8), samples (`f32` each), metadata (JSON, optional) |

A newer version than the reader knows is refused. The Signals panel
lists the signals loaded from `.signal` files, each with a Save and a
remove button; loading and saving are desktop-only.

### The Signals panel

[signal_panel.rs](../../src/ui/signal_panel.rs), titled **Signals**,
available in every mode:

- **Audio File** — load a file (decoded, analysed and imported as
  signals immediately), and a playback section with play/pause, stop
  and position. The **Analyze** button has no effect; analysis already
  ran on load.
- **Live Capture** — device list, start/stop, a level meter. Switching
  device while capturing restarts on the new one.
- **Signal Generators** — add, edit, delete.
- **Signal Files** — load, save, remove.
- **Signal Monitor** — the core audio signals' values at the playhead,
  the live ones while capturing, generator values, and the detected BPM,
  which can be overridden: a new BPM regenerates `beat` and `beat_phase`
  on a grid anchored at the first onset.

---

## Audio

[src/audio/mod.rs](../../src/audio/mod.rs). The audio dependencies are
unconditional; there is no `audio` Cargo feature.

**Decoding** ([decode.rs](../../src/audio/decode.rs)): symphonia, with
the `mp3`, `wav`, `flac`, `ogg` and `pcm` features. The file pickers
offer `.mp3 .wav .flac .ogg`. Note that symphonia's `ogg` feature is the
container: no Vorbis codec crate is in the build.

**Offline analysis** ([analyzer.rs](../../src/audio/analyzer.rs)),
`AnalysisConfig::default()`:

| setting | value |
| --- | --- |
| FFT | 2048-sample Hann window, hop 512 (signal rate = sample rate / 512, about 86 Hz at 44.1 kHz) |
| mel bands | 128, 20 Hz – 20 kHz |
| energy bands | thirds of the mel bands: low, mid, high |
| onsets | spectral flux against a local mean over 10 frames, threshold 0.3, at least 3 frames apart |
| tempo | autocorrelation of `energy_low` over 60–200 BPM; needs at least 4 s of audio |

The audio is mixed to mono first. Each band, the amplitude and the flux
are normalized to their **own maximum over the file**, so a value of 1
is the loudest moment of that file, not an absolute level. Analysis runs
synchronously on the UI thread when the file loads.

**Live capture** ([capture_common.rs](../../src/audio/capture_common.rs)):

- *Desktop* ([capture_native.rs](../../src/audio/capture_native.rs)):
  cpal input devices, plus **System Output (Loopback)** on platforms
  other than macOS (it opens the default output device for capture).
  On macOS a virtual loopback device appears as an ordinary input.
- *Web* ([capture_wasm.rs](../../src/audio/capture_wasm.rs)): the Web
  Audio API through a `ScriptProcessorNode`, from the microphone or from
  **Screen/Tab Audio** (`getDisplayMedia`). The browser's permission
  request and refusal are states of their own.
- `RealtimeAnalyzer` runs on the audio thread: a 1024-sample FFT with
  50% overlap, normalized against a slowly decaying running maximum, a
  tempo estimate from about 8 s of bass energy, and a running beat
  phase. Results cross to the main thread through atomics.

**Playback** ([playback.rs](../../src/audio/playback.rs) on desktop,
cpal output; [playback_wasm.rs](../../src/audio/playback_wasm.rs) on
the web, an `AudioBufferSourceNode`). The player is separate from
analysis; with **Sync audio** on, the timeline drives it.

---

## Video export at a glance

The frame loop, FFmpeg piping, engine state and progress are documented
in [EXPORT.md](EXPORT.md) (*Video Export*). What belongs to animation:

- **Frames.** `ceil(duration × fps)` frames; frame *n* is at
  `n / fps` seconds. Each frame's config is the export config with the
  tracks applied through the exporter's apply path (above).
- **Signals in export.** The app passes a copy of the **stored**
  signals (`SignalManager::clone_signals`): audio analysis, generators,
  `.signal` files. The exporter then renders each of the animation's
  generators that is not among them (`export_signals`), so the CLI,
  which passes none, still drives generator tracks; the app's panel
  version of a generator wins. Live producers are not copied, so a
  track on a `live_*` signal applies nothing in a video — deliberate:
  the export logs a warning naming each signal track whose signal it
  does not have (`tracks_missing_signals`; the CLI prints it).
- **Smoothing in export.** The exporter sets the controller's step to
  one frame (`set_step(1 / fps)`), so signal smoothing comes out the
  same at any frame rate.
- **The base config.** The app exports from the active config, with
  the export window's Max Iterations written over it. The CLI takes the
  config from `-c`, or without it the `.anim`'s embedded `base_config`
  (`cli_base_config`); with both, `-c` wins and the CLI says so.
- **Encoding.** H.264, H.265 (default) or VP9; software or a hardware
  encoder:

  | | H.264 | H.265 | VP9 |
  | --- | --- | --- | --- |
  | Software (CPU) | libx264 | libx265 | libvpx-vp9 |
  | NVIDIA NVENC | h264_nvenc | hevc_nvenc | — |
  | Intel Quick Sync | h264_qsv | hevc_qsv | vp9_qsv |
  | AMD AMF | h264_amf | hevc_amf | — |
  | Apple VideoToolbox | h264_videotoolbox | hevc_videotoolbox | — |

  Quality is a CRF (or the encoder's constant-QP equivalent), with a
  speed preset where the encoder has them and, for CPU encoders, a
  tune (default `animation`). Output is yuv420p; odd frame sizes are rounded down to
  even.
- **Audio.** An audio file chosen in the export window (or `--audio` on
  the CLI) is muxed as AAC with an offset (negative skips into the file,
  positive delays it), fade-in, fade-out and bitrate. It is independent
  of the Signals panel's file. In the app the offset also shifts the
  audio-analysis signals the tracks read (`Signal::shifted`), so the
  picture stays with the sound; generators and `.signal` files are not
  shifted.
- **Where.** Desktop only: the export window says it is unavailable on
  the web. In the app it opens from **Export Animation**; it fills
  iterations per thread and Max Iterations from the current settings,
  and defaults to 1920×1080, 30 fps, H.265, CRF 12, into the platform's
  Videos folder. The CLI:

  ```bash
  FractalArtEditor export-animation -c config.fflame -a anim.anim -o out.mp4 \
    -w 1920 -H 1080 --fps 30 --video-codec h265 --hw-accel none --video-quality 18 \
    --audio track.mp3 --audio-offset=-2.0
  ```

---

## Animations from scripts

A Rhai script that touches the `anim` object emits an animation beside
its flame, with the flame as its `base_config`; the CLI writes it as an
`.anim` next to the `.fflame`, and the app loads it after the script's
config and binds it. The calls are in [SCRIPTING.md](SCRIPTING.md)
(*The `anim` object*, *Per-transform keyframes*, *Animating the
simulation itself*).

---

## Gotchas

- **A boolean target takes numbers.** Auto-fill and **Add at current**
  store the current value as `1.0` or `0.0`, so the offered booleans
  (effect Enabled, PostAffineEnabled) read a number as on from 0.5 up
  (`json_as_bool`): a Linear 0 -> 1 track switches halfway. Booleans
  the picker does not offer still accept only `true`/`false`.
- **A signal track outside its window, or on a missing signal, applies
  nothing** rather than a default: the parameter keeps whatever the
  config or another track set.
- **The exporter's arms must keep up with the picker.** A path the
  exporter handles itself but differently from `ConfigManager` renders
  one way in the preview and another in the video; the parity test
  above is what catches it.
- **Keyframe times are not re-sorted on edit.** Out-of-order keyframes
  evaluate wrongly until **Sort by time**.

---

## Design record

- [animation-overhaul.md](../archive/projects/animation-overhaul.md) —
  subflame targeting, stable track identity, the affine operations
- [animation-system.md](../archive/projects/animation-system.md) — the
  original design (superseded: it planned `.ffanim`; `.anim` was built)
- [embedded-animation-config.md](../archive/projects/embedded-animation-config.md)
  — `base_config`
- [track-editor.md](../archive/projects/track-editor.md),
  [animation-ui-overhaul.md](../archive/animation-ui-overhaul.md),
  [keyframe-auto-fill.md](../archive/keyframe-auto-fill.md),
  [animation-track-refinements.md](../archive/animation-track-refinements.md)
  — the panel, the track editor, auto-fill, signal-track timing and fades
  (and the removal of oscillator and circular tracks)
- [signal-panel.md](../archive/projects/signal-panel.md),
  [audio-system.md](../archive/projects/audio-system.md) — signals,
  generators, `.signal`, audio analysis and capture
- [video-loop-and-sim-timeline.md](../archive/projects/video-loop-and-sim-timeline.md)
  — one video loop, and the simulation timeline
- [animation-export.md](../archive/projects/animation-export.md),
  [animation-export-performance.md](../archive/projects/animation-export-performance.md),
  [animation-export-race-conditions.md](../archive/animation-export-race-conditions.md)
  — earlier export designs

**Research, not built:**
[audio-analysis-dsp-ml.md](../experimental/audio-analysis-dsp-ml.md)
(further audio features and small models),
[osc-input.md](../experimental/osc-input.md) (OSC hardware control as a
signal source).
