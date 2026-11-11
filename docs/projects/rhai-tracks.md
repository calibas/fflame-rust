# Rhai animation tracks

Status: **steps 1-6 built 2026-11-10**, not merged; the reference is
ANIMATION.md, *Script tracks*. Branch `rhai-tracks`. P5 of
[rhai-plugins.md](rhai-plugins.md): a Rhai script as an animation track.

## Decided (the user, 2026-11-10)

1. **A script can optionally adjust the value under it**, not only
   replace it: each target is written in one of three modes, `"set"`,
   `"add"` or `"scale"`.
2. **One track, many targets.**
3. **Written in the Formula Editor**, saved in the library, usable as a
   plug-in.
4. **State, with a time step.** D4 ("a track is a pure function of time")
   becomes: *the same time always gives the same values*. A track may
   keep state, but only by stepping at a fixed rate from time zero:
   - The step `dt` is a constant, not the frame time.
   - Showing time `t` replays the steps from 0 to `t`, from the nearest
     checkpoint.
   - So scrubbing, playing, a missed frame, a 30 or 60 fps export and
     motion-blur moments all see the same state.
   - A missed frame in live playback runs the steps it missed, then
     draws.
5. **No catch-up cap for now.** An expensive script is a poor fit for
   live animation; address it if it becomes a problem.
6. **Live inputs stay live.** A recorded signal (an audio file's
   analysis) is a function of time and replays exactly. A microphone or
   MIDI value is whatever arrives; nothing records it, and a backward
   seek replays with the current live value.

## What a track script looks like

```rhai
// A slow breath on the zoom, on top of whatever the keyframes say,
// and the rotation taken over outright.
const PARAMS = #{
    depth: #{ value: 0.1, min: 0.0, max: 1.0, caption: "Depth" },
    rate: #{ value: 0.5, min: 0.01, max: 10.0, caption: "Rate (Hz)" },
};

const INFO = #{ title: "Breathing" };

// What it writes, and how: "set" replaces the value under it, "add"
// adds to it, "scale" multiplies it.
const TARGETS = #{ zoom: "add", rotation: "set" };

fn value(t, p, ctx) {
    #{ zoom: p.depth * sin(6.2831853 * p.rate * t), rotation: 0.1 * t }
}
```

With state, as a spring that follows the keyframed zoom with lag and
overshoot:

```rhai
const PARAMS = #{
    stiffness: #{ value: 40.0, min: 1.0, max: 400.0, caption: "Stiffness" },
    damping: #{ value: 6.0, min: 0.0, max: 50.0, caption: "Damping" },
};
const INFO = #{ title: "Springy Zoom" };
const TARGETS = #{ zoom: "set" };

fn start(p, ctx) { #{ x: ctx.get("zoom"), v: 0.0 } }

// Fixed dt; ctx reads the values under the scripts at the step's time.
fn step(s, dt, p, ctx) {
    let a = p.stiffness * (ctx.get("zoom") - s.x) - p.damping * s.v;
    s.v += a * dt;
    s.x += s.v * dt;
    s
}

fn value(t, s, p, ctx) { #{ zoom: s.x } }
```

- **`PARAMS` and `INFO`** are the Formula Editor kinds' own (design
  rules 1 and 8): captions, hints, ranges and choices, drawn by
  `ui::params`. The values live in the track, as a signal track's range
  does.
- **`TARGETS`** is a fixed list (decision 1 below), read without running
  the script, so the timeline shows what a track writes before it plays.
  - Names are the ones `config.set` and the generator `anim` API take
    (`config::by_name::path_named`: `"zoom"`, `"exposure"`,
    `"escape.zoom_log10"`, `"sim.steps"`).
  - Per-transform values use `"transform.N.weight"`,
    `"transform.N.julian.power"` and `"transform.N.a"`, through
    `anim::resolve_transform_target`, so one spelling serves every
    script.
- **`value(t, p, ctx)`**, or `value(t, s, p, ctx)` with state, returns a
  map of target to number. A target left out is not written that frame.
- **`start(p, ctx)` and `step(s, dt, p, ctx)`** are optional. Without
  them the track is stateless and costs one call a frame.
- **`ctx`**:
  - `ctx.get(name)` reads the value under the scripts (below) at the
    time being evaluated.
  - `ctx.signal(name)` reads a signal, 0 to 1, as a signal track does.
  - `ctx.duration` and `ctx.dt`.
  - `noise(x)` (smooth 1D value noise) and `hash(n)`: deterministic,
    for jitter, since nothing may be random.

## How a frame is evaluated

1. **The keyframe and signal tracks**, as today, applied in track order.
   This gives **the value under the scripts**:
   - in an export, the base config with those tracks applied;
   - live, the config at entry to animation (the render mode's
     `exit_animation` snapshot) with those tracks applied.
   A script reads that, never the live config it wrote last frame, so an
   `"add"` does not compound frame over frame and live matches export.
2. **The script tracks, in track order.** Each one's outputs combine
   with the value under it by its mode: set, add or scale. A later
   track writing the same target replaces an earlier one, and the
   timeline marks the clash.
3. **Everything goes out as the same `(flame, path, value)` triples**, to
   the live applier (`update_param_silent_on`) and the export's
   (`apply_animation_values`), so one evaluation serves both.

**State:**
- Its time is quantized to steps: the state at `t` is the state after
  `floor(t / dt)` steps from `start`.
- `value` gets that state and the exact `t`, so output is smooth between
  steps.
- The controller keeps a checkpoint every second of animation time.
  - Forward playback steps on from the last state.
  - A backward seek, a loop's wrap or ping-pong's reverse replays from
    the nearest checkpoint at or before `t`, at most one second of steps.
  - A motion-blur moment does the same, leaving the frame's state as it
    was.
- An edit to a track's source or parameters, or to a keyframe track
  under it, drops its checkpoints.

**Budget:**
- One Rhai engine for every track (sandboxed with the generator host's
  limits).
- Each track's AST is compiled once, cached by its source's hash.
- A per-call operation limit far below a generator's. A track over it
  is stopped and shows the error, and the frame goes on without it.

**Determinism:**
- The output depends on the source, the parameters, the values under it
  and recorded signals, nothing else.
- Rhai's maths is f64 through the platform's `libm`, so desktop and web
  may differ in the last bit of a `sin`. Values land in f32 config
  fields; a test pins a track's values so a change shows.

## Where it goes

- **`TrackSource::Script`** (`src/animation/mod.rs`), saved in the
  `.anim`:
  - the source (design rule 2), its id and version (rule 7), and the
    parameter values;
  - not bound (`Track::bind`): a script names a transform by its position
    in its own text (`"transform.2.weight"`), so rewriting the track's
    `target` through a reorder, as a keyframe track's is, would part it
    from what the script writes. A target the config no longer has is
    skipped. (Planned as a binding a target; changed when built.)
  - `Track::target` holds the first target, so code reading one target
    still works. The script's list is the truth.
- **`src/animation/script.rs`** (new): the engine, compile and cache, the
  `TARGETS` and `PARAMS` reading, the stepper and its checkpoints, and
  `ctx`.
- **`AnimationController`**: the evaluation order above in
  `evaluate_at_time_with_signals`, `evaluate_at_time` and
  `evaluate_moment`; the snapshot of the values under the scripts.
- **The Formula Editor**: a track is recognised by `fn value` together
  with `TARGETS`.
  - It is the CPU world (rhai-plugins.md, *Two Rhai worlds*), so it is an
    editor-level kind beside `lang::EntryKind`, not a new `EntryKind`
    every GPU match would have to carry.
  - Check compiles it and runs `value` (and `start` and a step) at t = 0.
  - **Use as track** adds the track to the loaded animation, or replaces
    the one it came from, through a `UiResponse`; decision 3 covers no
    animation being loaded.
  - With Live on, an edit re-applies.
- **The library**: `library::Kind::Track`, so a track can be a plug-in,
  offered in the track editor's add menu beside keyframe and signal
  tracks.
- **The track editor** (`src/ui/track_editor.rs`): a script track's row
  with its targets beneath it, each with its mode, clashes marked, its
  parameters, its error, and Edit in the Formula Editor.

## Steps

1. **A stateless script track, end to end.**
   - `TrackSource::Script`, `animation::script`, `TARGETS` and the three
     modes, the evaluation order, the value under the scripts, both
     appliers.
   - Tests:
     - live and export give the same values;
     - `"add"` does not compound across frames;
     - a clash resolves to the later track;
     - a `.anim` round-trips;
     - a broken target is skipped.
2. **State.**
   - `start` and `step` at a fixed `dt`, checkpoints, seek, loop,
     ping-pong, motion-blur moments.
   - Tests:
     - scrubbing to `t` equals playing to it;
     - a 30 fps export equals a 60 fps one on the shared frames;
     - a dropped frame lands on the same state;
     - a moment leaves the frame's state as it was.
3. **`ctx`.** `get`, `signal` (a recorded signal replays exactly; a live
   one stays live), `noise` and `hash`, the operation budget and its
   error.
4. **The Formula Editor and the library.** The track kind, check, Use
   as track, Live, `Kind::Track`, plug-ins in the add menu, ids minted on
   Use as the other kinds' are.
5. **The track editor.** The row, its targets and modes, clashes, its
   parameters, errors, Edit.
6. **Examples and docs.**
   - Examples in `assets/formulas/`: Breathing, Springy Zoom, and a
     smoothed audio signal (the case that needs `dt`).
   - Docs: ANIMATION.md, SCRIPTING-GUIDE.md, and this plan's record.
7. **Gates**: the lib tests, a video export of the examples compared
   with the live frames, `release.py check`.

## What was built

| Step | Done | Held by |
|---|---|---|
| 1. A stateless track | `TrackSource::Script`, `animation::script`, the modes, the value under the scripts and the rests | `an_added_value_does_not_compound_live_or_exported`, `a_script_adjusts_the_keyframes_and_the_later_script_wins`, `a_script_track_round_trips_and_a_broken_one_is_skipped` |
| 2. State | `start` / `step` at 1/240 s, checkpoints each second, keyed by what the state depends on | `a_state_is_the_same_however_its_time_is_reached` (60 fps, 30 fps, dropped frames, scrubbed back, a cold jump, motion-blur moments: bit for bit), `an_edit_a_state_depends_on_replays_it` |
| 3. `ctx` | `get`, `signal`, `time`, `dt`, `duration`, `noise`, `hash`, the budget | `signals_hash_and_noise`, `a_broken_track_says_why` |
| 4. Formula Editor, library | the track row, Use as track, Live, `Kind::Track` | `a_track_file_is_used_as_a_track_and_followed_live`, `a_track_is_a_track` |
| 5. Track editor | the row, the window, Add Script Track | `a_script_track_shows_its_targets_and_parameters` |
| 6. Examples, docs | Breathing, Springy Zoom, Smoothed Jitter; ANIMATION.md, SCRIPTING-GUIDE.md | `the_track_examples_run`, `a_video_of_script_tracks_gives_the_frames_playback_gave` (the export's `FrameSource` equals playback, config for config) |

Where it differs from the plan:
- **Not bound** (*Where it goes*): the plan gave each target a binding.
- **The value under the scripts, live**: the plan took it from the
  render mode's snapshot at entry to animation. The video export's base
  is the app's config after playback, holding what the tracks last
  wrote, so a snapshot only the app holds would not reach it: each
  target's rest travels in the animation instead (`script::Rest`).
- **Decision 3 needed nothing**: the panels keep an animation present,
  so Use as track always has one (a fresh one is made if not).
- **The check reads a missing signal as 0.5**, so a track reading the
  user's signals can be checked and used in the editor; `ctx.time` was
  added for a step's input (Smoothed Jitter).

Left: a track's own parameters are not targets other tracks can
animate (they live in the track, not the config); a script cannot
`import` a library yet (the plan's "P0's libraries can be imported").

## Decisions for the user

**All four decided 2026-11-10 as recommended** (the user): a fixed
`TARGETS` list, 1/240 s for every track, Use as track creates an
animation, and signal smoothing left for later.

1. **Targets: a fixed list, or computed?** A fixed `TARGETS` (recommended
   for now) lets the timeline show what a track writes before it runs and
   bind each target. Computed targets, such as "every transform's
   weight", would need the script to run first and its targets to be
   re-bound whenever it returns a different set. A fixed list can grow
   into patterns (`"transform.*.weight"`) later.
2. **The step rate:** 1/240 s for every track (recommended: fine enough
   for springs at any frame rate an export uses, and one second of
   replay is 240 steps), or a script's own `INFO.step`.
3. **Use as track with no animation loaded:** create one with the
   current fractal as its base and the default duration (recommended), or
   refuse and point to the animation panel.
4. **Not in this branch, recorded:** a signal track's smoothing keeps
   state frame to frame by the frame's time today (`signal_smoothed`), so
   it has the problems decision 4 avoids: a scrub and a play differ. It
   could move onto the same fixed steps later.
