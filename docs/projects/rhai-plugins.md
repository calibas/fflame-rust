# Everything scriptable in Rhai: the direction

Recorded 2026-10-18 from the user's statement of where the program is
going. It is the frame the scripting work is designed in -- the KIFS
plan's C items ([escape-kifs-and-variations.md](escape-kifs-and-variations.md)),
the formula language ([escape-formula-language.md](escape-formula-language.md))
and whatever follows -- so each step complements it rather than
building something that has to be undone. Not a plan with steps: the
plans that build it say what is next.

## The goal, in the user's words

> Using them as plugins is perfect. That's actually my plan, to switch
> all plugins over to Rhai scripts. Everything scriptable: flame
> variations, escape-time formulas, simulation formulas, colorings,
> they can be used as plugin, saved within .fflame files, and
> downloaded from an API.
>
> They can declare parameters, the parameters can be targeted for
> animations and the undo/redo history.
>
> One more goal for Rhai scripts, they should be usable as animation
> tracks. The script would get the current animation time (delta?) and
> the config state of the fractal, and then be able to modify the state
> however you want.

## Where each piece stands (2026-10-18)

| Kind | Written in Rhai | In the `.fflame` | As a plug-in | From the API |
|---|---|---|---|---|
| Flame variations | yes (C1, 2D; 3D needs the vector type, B) | yes (`Flame::custom_variations`) | as the `plugin` category, `Provenance::Custom`; local plug-in *files* are still WGSL JSON | no (downloads are WGSL JSON) |
| Escape formulas | yes | yes (`EscapeConfig::custom_formula`, one) | the Formula Editor's library (`formula_store`) | no |
| Escape colourings | yes | yes (`custom_coloring`, one) | the library | no |
| Simulation models and colourings | no (31 built-in models, 13 colourings) | -- | -- | -- |
| Effects | no (WGSL JSON plug-ins and downloads) | -- | WGSL JSON | WGSL JSON |
| Animation tracks | no (keyframes and signals: `TrackSource`) | -- | -- | -- |
| Generator scripts | yes (the scripting API, `src/script/`) | -- | the script library | yes (`ScriptResponse`) |

Carrying: an entry that calls a variation (`vary`) carries its source
in `CustomSource::variations`, so a config holds everything its
entries need, whatever the registry holds where it is opened.

Parameters: a custom formula's and colouring's `PARAMS` are
`ConfigPath::EscapeFormulaParam` / `EscapeColoringParam` -- panel
controls, animation targets, undo steps, script parameters. A custom
variation's are ordinary variation parameters on each transform, so
the same holds.

## Two Rhai worlds

The word covers two engines, and the design keeps them apart:
- **The GPU dialect** (`escape::lang`): a typed subset of Rhai lowered
  to the formula language's IR and compiled to WGSL, run per sample or
  per pixel. Variations, formulas, colourings, and later simulation
  rules and effects. It has no maps of the config, no I/O, no unbounded
  loops the GPU cannot run; what it cannot express it refuses at check
  time with a reason.
- **The scripting API** (`src/script/`): full Rhai on the CPU, building
  configs, with a pinned PCG stream for `script + seed`. Generator
  scripts today; **animation tracks** belong here -- a track runs once
  a frame on the CPU, not per sample.

## Design rules that follow

1. **One compiler, one parameter model.** Every GPU-side kind is a Rhai
   source with `PARAMS` and `INFO`, checked and compiled by
   `escape::lang`. A new kind (a simulation rule, an effect) is a new
   entry kind of the same language, not a new language.
2. **The source is the artefact.** A config carries the sources it
   uses and registers them on load (`escape::custom::ensure`,
   `variations::custom::ensure`), so a `.fflame` renders anywhere. A
   plug-in file and an API download are other places the *same text*
   comes from; compiled WGSL is at most a cache, never the thing
   stored. (Today's WGSL JSON plug-ins and downloads are the old form,
   to be moved over.)
3. **Parameters are `ConfigPath`s.** Whatever a script declares in
   `PARAMS` is addressed through `ConfigPath`, so it is animatable,
   undoable and scriptable by construction -- never a side channel the
   history or the timeline cannot see.
4. **Names and provenance.** A registered script has a name of its own
   and a provenance (built-in, download, plug-in, the user's own); a
   name is never shadowed, and re-registering the same provenance's
   source replaces it and bumps the registry's version.
5. **Determinism.** A script's output depends on its source, its
   parameters, the config and its seed alone -- the same on desktop,
   web and CLI, in the viewport and in an export -- so a still or a
   video is reproducible from the file.
6. **Hooks are the built-ins' features.** A Rhai variation reaches the
   flame shader's machinery through the same `Feature`s a built-in
   declares (random numbers, the accumulator, per-thread state, colour
   writes: the KIFS plan's C2), so a script can do what a ported
   variation does and the shader builder has one code path.

## Animation tracks as scripts (not started)

A `TrackSource::Script` beside `Keyframes` and `Signal`: a CPU Rhai
function given the time (and the step since the last frame) and the
config, returning changes as `(ConfigPath, value)` pairs that go
through `ConfigManager` like any edit -- so what it touches is what a
keyframe could touch, and the history sees it. To decide when it is
built: whether a track sees the config before or after the keyframe
tracks, how it keeps state between frames and stays deterministic when
an export renders frames out of order or skips them (a pure function of
time, or state re-run from zero), and how a script declares the targets
it writes (for the timeline's display and for conflicts with keyframe
tracks on the same target).
