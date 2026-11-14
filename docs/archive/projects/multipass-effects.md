# Multi-pass and temporal effects

> **Archived 2026-11-14** -- finished: phases 1-5 built, merged (client PR #165; the API side, ffapi `multipass-effects`, merged). Reference: [EFFECTS.md](../../main/EFFECTS.md), *Multi-pass effects* and *Temporal effects: the clock*; [ANIMATION.md](../../main/ANIMATION.md), *Temporal effects*. Frame interpolation is parked in [frame-interpolation.md](../../experimental/frame-interpolation.md). Open items are in [BACKLOG.md](../../projects/BACKLOG.md).

Branch `multipass-effects`, started 2026-11-14. The user chose the larger
scope -- passes, buffers, pyramids -- and temporal effects in the same
branch. This is the plan being followed; it is updated as the work goes.

## Why

- **`distance_field`** is the one built-in effect still in WGSL. It runs
  a seed pass, ~log2(max dim) jump-flood passes and a composite, and the
  chain special-cases it by name (`effect_chain::JfaResources`,
  `ensure_jfa`, `run_jfa`, `slots_of`). The Rhai effect dialect is one
  function of one pixel reading one image.
- **Pyramids** make bloom-style effects possible: a glow wider than a
  few pixels costs O(r^2) a pixel as one pass, O(1) a pixel over a
  down-and-up pyramid.
- **Temporal effects** read earlier frames: trails, echoes, feedback.
  The same buffer model, with a buffer that outlives the frame.

## The model

An effect declares its passes and buffers in `INFO`. Without `passes` it
is what it is today, one pass, `fn effect`, and compiles to the same
WGSL.

```rhai
const INFO = #{
    title: "Distance Field", id: "distance_field", category: "color",
    buffers: #{ coords: #{} },
    passes: [
        #{ run: "seed", to: "coords" },
        #{ run: "flood", to: "coords", repeat: "log2" },
        #{ run: "effect" },
    ],
};
fn seed(c, p, ctx) { ... }
fn flood(c, p, ctx) { ... read_at("coords", ...) ... }
fn effect(c, p, ctx) { ... }
```

- **A pass** is a function of the same shape as `effect(c, p, ctx)`,
  named by `run` (`fn` is a Rhai keyword, no map key).
  `to` names the buffer it writes; the last pass has none and writes the
  chain's output (it must be `effect`). Only the last pass may blend.
- **`repeat`**: an int, or `"log2"` (the covering power of two of the
  frame's larger side, as the jump flood counts), or `"levels"` over a
  pyramid target (below). `ctx.pass` is the repeat's index from 0,
  `ctx.passes` the count, `ctx.step` `2^(passes - 1 - pass)` (the jump
  flood's halving step).
- **Buffers** are `Rgba32Float`: any four numbers (a jump flood's
  coordinates need f32). `scale` (default 1) sizes one against the frame.
  A pass that reads the buffer it writes reads the previous version (a
  ping-pong pair).
- **Reads**: `buffer(name, uv)` -- filtered, by hand from `textureLoad`
  (`lang::wgsl::texture_wgsl`'s portable read, the same on every vendor
  and without `FLOAT32_FILTERABLE`) -- and `buffer_at(name, x, y)`, the
  texel, clamped (not `read`/`read_at`: those are a simulation model's
  neighbour reads). `input` and `input_at` stay as they are.
- **`ctx`** gains `ctx.xy`, the pixel's position in the grid being
  written (WGSL's `position.xy`, exact: the jump flood measures
  distances from it), and `ctx.width`/`ctx.height` become that grid's
  size (the frame's for a pass into the output, as now).

### Pyramids

- `buffers: #{ down: #{ scale: 0.5, levels: 6 } }`: level `i` is
  `ceil(size / 2^i)`, one texture with mips.
- A pass with `repeat: "levels"` and `order: "down"` writes levels 1..n,
  each reading its target one level finer; `order: "up"` writes levels
  n-1..0, reading its target one level coarser. `ctx.level`,
  `ctx.levels`.
- In such a pass every other pyramid reads at `ctx.level`
  (`buffer(name, uv)`); its own target at the level it came from, zero
  at the first level it writes. Bound as
  single-level views, so writing one level and reading another of the
  same texture is legal in WebGPU.
- A built-in **bloom** (new, Rhai only, appended to the registry) is the
  test case: threshold, down pyramid, up pyramid with tent filter,
  composite.

### Temporal

- `buffers: #{ trail: #{ persist: true } }`: kept from one frame to the
  next. A read before this frame's write sees the last frame's; written
  at most once a frame (by one pass, not repeated).
- `ctx.frame` (0 for the first, and for a still), `ctx.time` (animation
  seconds), `ctx.dt` (seconds since the last frame: `1/fps` in an
  export, the real interval in the viewport's playback).
- `INFO.warmup: k`: the effect promises it forgets past `k` frames (a
  trail that has decayed below 1/255). A frame rendered alone in a
  sequence warms up from frame `n - k`; without `warmup`, from frame 0.

Decisions taken as recommended (the user may amend before phase 4):

- **T1 -- the clock.** History advances once a timeline frame. The
  viewport's re-renders of the same moment (convergence, the governor)
  re-run the effect against the same last frame, so a picture that
  converges does not smear.
- **T2 -- the paused viewport.** History is held; an edit re-renders
  against it. It resets on a seek, a rewind or loop wrap, a resize, a
  change to the chain's effects, and a Reset in the effects panel.
- **T3 -- stills** (PNG, high-res, CLI export of a config) render with
  fresh history, `ctx.frame == 0`: reproducible with no timeline.
- **T4 -- frames rendered alone** (Python's frame source, any future
  ranged export) warm up as `warmup` says.
- **Where the history lives:** `RenderEngines` for headless renders and
  the video export (beside the simulation's field, the state that
  already outlives a frame); the app's effect chain for the viewport.

## Limits

- Buffers an effect: 8 (3 bindings + 8 under the 16 sampled textures a
  stage allows everywhere). Passes declared: 16. Pyramid levels: 12.
- Uniform: `EffectParams` gains the pass fields (`pass_index`,
  `passes`, `step`, `level`, `levels`, `buffer_levels` -- each buffer's
  level to read, 4 bits a buffer) and the time fields (`frame`, `time`,
  `dt`): 256 bytes, the slot stride exactly. A WGSL effect declaring the
  208-byte struct binds as before. One slot a pass run (each repeat its
  own), as the jump flood took.

## Phases

1. **Multi-pass core.** DONE 2026-11-14. Dialect: `INFO.passes`/
   `buffers`, a pass per function (`lang::rhai::effect_pass`, each its
   own program, prefixed `fx<k>_`), `buffer`/`buffer_at` (host loads,
   as a texture's), `ctx.xy` and the pass and time fields. Template:
   `effects::custom::assemble_passes`, an entry point a pass
   (`fs_pass<k>`), the buffer bindings and reads. Plan:
   `effects::passes`. Runner: `renderer::effect_passes`, for any effect
   whose `EffectInfo::passes` is set, in both chains. A one-pass
   effect's WGSL changed (the uniform's new fields, `fx_effect(uv, xy)`,
   `input_at` clamped to the image's own size), its pixels not: the
   twins' tests, and the smoke renders below.
   Tests: `a_multi_pass_effect_runs_its_passes` (ping-pong, repeats,
   `ctx.pass`/`step`/`passes`, a half-size buffer, exact values), the
   compile and refusal tests, `effects::passes`'s plan tests.
2. **`distance_field` as a twin.** DONE 2026-11-14.
   `assets/effects/distance_field.rhai`, held to its WGSL by
   `the_rhai_twins_match_their_wgsl` (five settings, within one 8-bit
   step); the WGSL describes its own passes
   (`effects::distance_field_passes`, the step from the uniform), so the
   reference runs through the same runner; `JfaResources` and the
   chain's name checks are gone. Smoke: eight configs (escape and
   flame, the three modes, and a chain with vignette, chromatic
   aberration and density blur) rendered by `main`'s binary and this
   branch's: pixel-identical.
3. **Pyramids** and the built-in **bloom**. DONE 2026-11-14. A pass
   over levels writes every level (down: finest first; up: coarsest
   first), its target read at the level it came from, zero at the
   first. Bloom (`assets/effects/bloom.rhai`, the first built-in with
   no WGSL: `effects::custom::RHAI_BUILTINS`): a thresholded, soft-kneed
   dual-filter down pyramid, a tent-filtered up pyramid that sums each
   level with the wider ones by the spread (a mix, energy-conserving,
   read too faint in 8-bit colour), added over the picture; defaults
   chosen from renders (threshold 0.45, intensity 1.5, spread 0.7).
   Tests: `a_pyramid_pass_writes_and_reads_its_levels` (exact values),
   `bloom_glows_past_the_bright_parts_alone`.
4. **Temporal.** DONE 2026-11-14. `EffectClock` (still, or epoch and
   frame, with time and dt) set on the chain; each multi-pass effect's
   state remembers the clock it last ran at, so one rule gives T1-T3:
   the same frame re-reads its history, the next carries it on,
   anything else (a jump, a new epoch, a still) starts at zero
   (`effect_passes::tick`). Headless: `RenderEngines::effects` keeps
   the chain, `RenderJob::with_effect_clock`; the video export sets
   `EffectClock::video(frame, fps)` and frees the chain at the end.
   Viewport: `app::effect_timeline` (playback advances, a step over half
   a second or a pause's scrub resets; the Effects panel's Reset History,
   shown with a temporal effect in the chain). T4:
   `effects::warmup_frames` / `temporal_window`, used by Python's
   `Session::frame` for a frame out of order. Built-in **Trails**
   (`assets/effects/trails.rhai`): the persistent trail is
   `max(picture, last trail x persistence^(30 dt))`, frame-rate
   independent; warm-up 240 (persistence at most 0.95, below an 8-bit
   step by then at 60 fps). Tests: `a_persistent_buffer_follows_the_clock`,
   `trails_fade_frame_by_frame`, `a_videos_frames_carry_a_temporal_effects_history`,
   the timeline's and the warm-up's unit tests.
   Tested by hand in the app (2026-11-14): all works.
5. **Docs and the API.** DONE 2026-11-14. EFFECTS.md (the runner, the
   uniform, multi-pass, the clock, the dialect, the shipped list),
   ANIMATION.md (*Temporal effects*), ARCHITECTURE.md. The contract and
   catalog did not move. The effects corpus marks `multi_pass` and
   `temporal`: an app from before this cannot run a multi-pass module
   (no `fs_main`), so the API must serve those only to readers that can
   -- to tell the API repository, with the corpus regenerated.

## Not in this branch

- **Frame interpolation** (frames that were never rendered): a video
  export stage, not an effect. Parked in
  [docs/experimental/frame-interpolation.md](../../experimental/frame-interpolation.md).
- Named history rings (`past(i, uv)`): a persist buffer covers what
  they would.
