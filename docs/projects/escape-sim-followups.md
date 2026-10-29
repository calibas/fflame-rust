# Escape-time and simulation: open decisions and follow-ups

What the escape-time / simulation compatibility review (PR #129, merged
2026-10-08) left open: three decisions that need the user, the solid
path tracer's remaining performance work, and the known gaps the review
found and did not fix. Each item names the code it touches.

## Decisions to make

### D1. Simulation tracks while Run is on

**The mismatch.** A simulation parameter track (feed, kill, a coupling's
strength, ...) with **no Steps track**, played back while Run is
engaged: the preview keeps stepping the run, so the pattern morphs as
the parameter moves. The video holds the run at `sim.steps` -- the Steps
track is the only thing that advances a run in the exporter
(`ConfigPath::SimSteps` in [src/animation/export.rs](../../src/animation/export.rs)).
Preview and video are different pictures.

**Options.**
- **(a) The video advances the run each frame,** as the preview does:
  `steps_per_frame` more a video frame when Run was on. Keeps the
  "morph while it grows" effect, which a Steps track cannot reproduce
  (a Steps track re-reaches each step count under that frame's
  parameters). Needs Run -- today app state -- recorded where the
  exporter can see it, and a frame-rate-free definition of the rate, or
  the video depends on how fast the preview ran.
- **(b) Playback holds the run** unless a Steps track drives it: the
  preview pauses Run while the timeline plays, so it shows what the
  video will. Matches the rule that a simulation's picture is the state
  at a step count (`docs/main/SIMULATION.md`), and loses the morph
  effect.

### D2. Script access to terrain and the newer settings

**The gap.** Scripts cannot reach the terrain, path tracing, contrast,
the texture overlay, or a simulation's layers, couplings and colour
stack. `config.set` walks only groups already present in the config's
JSON, and these groups are skip-if-default, so a fresh config has none
of them: "unknown setting group" (`config_set` in
[src/script/api.rs](../../src/script/api.rs)). And `escape.formula`
accepts only the planar `FORMULAS` registry, so the 2D-field (mode B)
and 3D/IFS (mode D) formulas are refused. There is also no
`escape.preset()` and no `sim` getters.

**Options.**
- **(a) `config.set` creates a missing group** from its default and
  validates the key against the type -- reaches every setting at once,
  including ones added later, at the cost of scripts addressing JSON
  paths (`escape.terrain.cam_pitch`).
- **(b) Dedicated functions** (`escape.terrain(...)`, `sim.layer(...)`,
  ...): a curated, documented surface, one function per group to write
  and keep in `docs/main/SCRIPTING.md` (its staleness test enforces
  that).

Either way `escape.formula` should accept the field and IFS registries
(`get_field`, `get_ifs`), as the CLI's `unknown_names` already does.
**Recommended: (a)**, with (b) later for the groups scripts use most.

### D3. Animation tracks on simulation layers

**The gap.** Tracks on a layer's model parameters, a colour layer or a
coupling (`SimLayerParam { layer, .. }`, `SimCouplingStrength { index }`
and kin in [src/config/delta.rs](../../src/config/delta.rs)) address it
by **position**. Adding, removing or reordering layers is not followed:
a track silently drives whichever layer now sits at its index. A track
on a flat parameter also breaks when the model is promoted to layers.

**The fix needs stable layer IDs.** Transforms and effects already have
them: `Track.bound` holds the item's ID and `rebind_targets`
([src/animation/mod.rs](../../src/animation/mod.rs)) finds where it
moved, keeping a track on a deleted item marked broken so an undo can
restore it. Layers, colour layers and couplings would need the same
session IDs (`#[serde(skip)]`, re-issued by `fixup_ids`), and promotion
would need to carry a flat track onto layer 0.

## TODO: the solid path tracer's speed

**Since (2026-10-20):** researched and the agreed order built -- the
denoiser's fixes, adaptive sampling, specular and diffuse denoised
apart, the ray tolerance, the fused march measured slower and parked
([path-tracing-research.md](../archive/projects/path-tracing-research.md), archived; TERRAIN.md
for how it works now). The record below is where that work started.
Left, for the user's decision or later:
- **S2's deeper rewrite** (the march fused with the path's state; parked
  slower as built) and **S3, a wavefront path tracer** (a pass per
  bounce) -- the speed levers left.
- **D6, a learned denoiser** (Intel Open Image Denoise, or a small
  network of our own): a model and a library.
- **V3, blue-noise sampling** (Ahmed and Wonka 2020); **V4, Russian
  roulette on shadow rays past the first surface** (5-10%, some noise),
  and from the first bounce rather than the second -- a crevice's long
  paths are the costly ones.
- **S4, more compiled in or out** (the coat, MIS, the shadow rays) and
  **S5, a coarse distance cache** (speculative).
- **Zirr et al. 2018's outlier re-weighting** for stills (three to six
  vec4 a pixel more).
- **Adaptive sampling's default**: off (as shipped), or on at a
  threshold of 4.

**Where it stands** (GTX 1660 SUPER, the shipped Menger Sponge at
1080p, default settings -- two bounces, two lights, shadows 0.7): a
sample takes **6.2 s** in the viewport, **4.7 s** in an export; the lit
picture of the same view takes **0.6 s**. The default 256 samples is
about 25 minutes. The 2026-10-08 fix (`a73c481d`, see
[heightfield-3d.md](../archive/terrain/heightfield-3d.md), "Losing the device, and
re-banded") made it safe and faster on narrow viewports, but not
faster at full width.

**Where the time goes** (measured at 320x180, one whole-frame dispatch
a sample):

| What the sample does | ms |
|---|---|
| Camera march and its surface only (no bounces, no shadows) | 12 |
| + a shadow ray to each of the 2 lights | 42 |
| + 1 bounce, no shadows | 44 |
| + 2 bounces, no shadows | 59 |
| Defaults: 2 bounces, shadows | 125 |
| Defaults, the surfaces bounces meet skipped (not a valid picture) | 111 |

About nine-tenths of a sample is its secondary marches (shadow and
bounce rays); the normals and colourings at the surfaces they meet cost
little. A secondary march calls a hit at a ten-thousandth of the
bounding ball (`pt_ifs_leave` in `IFS_PATH_WGSL`,
[src/escape/assembler.rs](../../src/escape/assembler.rs)) -- the lit
walk's shadow rule, because at a pixel's width shadows erased the
sponge's sub-squares (`ifs_shadow`'s comment). At 1080p that is about
seven times finer than 0.4 of a pixel.

**Candidates, roughly by expected gain.** All but the first change
pixels; each needs an image comparison against the current tracer on
the Menger and the tetrahedron at 64+ samples, and the
`solid_samples_are_band_invariant` test and the visual suite's
`ifs-solid-tetrahedron-path` and `-sky`.

1. **Bigger viewport calls when idle.** No pixel changes: at 1080p a
   sample takes 6.2 s at the viewport's 40 ms calls, 5.9 s at 100 ms,
   4.7 s at the export's 250 ms (`menger_band_costs`). Longer calls
   once nothing has moved for a few seconds trade UI frame rate for
   about 1.3x.
2. **A looser hit for bounce rays only.** Indirect light alone; shadow
   rays keep the tight hit the sub-squares need. The likely largest
   shader gain -- deeper levels per distance call and more steps to
   converge are both what the tight hit costs.
3. **Over-relaxed sphere tracing** on secondary marches (step by more
   than the distance, back up on overshoot; Keinert et al. 2014). The
   IFS distance is a lower bound, so the backtrack test holds. Fewer
   steps along tunnels and grazing rays.
4. **Fewer shadow rays past the first surface.** `pt_path` sends one
   to every light at every bounce; picking one light by power was
   measured as speckle on the lit faces (the comment in `pt_path`,
   [src/escape/path_core.rs](../../src/escape/path_core.rs)). Every
   light at the first surface and one by power after would keep the
   direct light clean.
5. **A lower default sample count for solids,** leaning on the
   denoiser.

**Tried 2026-10-19** (`path_quality_against_a_reference`: a preset path
traced at 320x180, the time a sample takes as a still renders it, and
the RMSE over the solid's pixels against a 1024-sample reference of the
tracer as it was, rendered on a seed of its own -- the first measurements
shared the reference's stream and flattered the baseline). Repeat runs
move a sample's time about 5%.

| Candidate | Menger: ms a sample, RMSE at 64 | Tetrahedron | Verdict |
|---|---|---|---|
| As it was | 155.5, 1.7% (0.9% at 256) | 57.7, 3.5% (1.8% at 256) | |
| 4. One light by power past the first surface | 147.6, 2.3% | | No: half the samples again for the noise |
| 2. Bounce rays hit at 0.4 of a pixel | 121.1, 2.0%, bias +0.40% | | No: biased |
| 2. ... at ten times the tight hit, capped there | 132.4, 1.7% | 46.4, 4.4%, bias +1.34% | No: light leaks past the tetrahedron's thin walls |
| 3. Over-relaxed (ω 1.5, 1.2) on top | 142.1, 142.7 | | No: slower; the sponge's tunnels fail the overlap often |
| Each distance's precision from the last distance | 152.6, 1.7% (0.9%) | 58.4, 3.7% (2.2% at 256) | No: no faster, and the tetrahedron moves |

**What the time is:** parallelism. A 1080p row of the Menger costs about
11 ms, so a 40 ms call is three or four rows -- a few thousand threads,
too few to fill the card, and each dispatch waits on its slowest pixel.
Measured at 1920 (`menger_band_costs`): a sample takes 6.5 s in 40 ms
calls, 6.2 in 100, 5.8 in 160, 4.9 in 250. **Done: candidate 1** --
`IDLE_FRAME_MS`, the export's 250 ms call, once no input has come for two
seconds and nothing plays (the app's `last_input_time`); any input goes
back to 40 ms after the call in flight. No pixel changes. The lever left
is the integrator's shape: a wavefront path tracer (a pass per bounce,
the paths' state in a buffer) would fill the card within a short call,
at the cost of a rewrite of `pt_path` and both geometries' sides.

Found on the way: the lighting work (lights-in-the-fractal) had made a
directional-only sample a fifth slower -- 130 ms to 158 -- by the
positional light code's registers; fixed by compiling it in only when a
light uses it (`4a8671ae`).

**Measuring.** The ignored tests in
[src/escape/renderer/solid_path.rs](../../src/escape/renderer/solid_path.rs):
`menger_band_costs` (the lit walk for scale, bands, a sample at each
call budget, whole frames at 640 wide or less; `MENGER_W`,
`MENGER_BOUNCES`, `MENGER_SHADOW`, `MENGER_LIGHTS`) and
`the_viewport_pace` (driven as the app drives it; `PACE_MENGER=1`,
`PACE_W`, `PACE_SHIFT`). Measure small: a band near two seconds
resets the display driver for the whole desktop.

## 3D views: not built

From finished plans, archived 2026-10-21:
- **From [3d-look-and-tools.md](../archive/projects/3d-look-and-tools.md):** underlining a
  formula check's errors in the Formula Editor's text (it keeps the
  messages, not their spans; P1); a light's size apart from its shadow
  sharpness -- a broad highlight under a sharp shadow, left for the user
  (P4); Iteration transformations for every solid, which needs a step
  factor (the built-ins' distance estimates cannot know a variation's
  stretch; P5).
- **From [lights-in-the-fractal.md](../archive/projects/lights-in-the-fractal.md):** a point
  light on a terrain (its world moves with the view; it needs a fixed
  frame -- greyed in the panel now); more than four lights (the geometry
  record holds four shadow terms); placing a light by clicking a surface,
  or a fly-mode key to drop one; emissive geometry (Glow lights nothing
  else).
- **From [origami-creases.md](../archive/projects/origami-creases.md):** a Paper preset.
- **From [hdri-environment.md](../archive/projects/hdri-environment.md):** presets that show
  the skies; the web build's skies and skies past 1K are in
  [online-assets.md](online-assets.md).

## Known gaps, not fixed in PR #129

- ~~The video's deep-zoom memory check looks only at the base config, not
  at each frame's zoom.~~ Fixed 2026-10-19: every frame is checked at its
  own zoom, and only the frames that render perturbed (EXPORT.md, *What a
  frame is*).
- No PNG input for `export` (a PNG's embedded config); flame-only
  metadata fields are written for escape and simulation pictures.
- Browser exports don't generate texture layers, for a 2D or a terrain
  picture.
- The Spanish, Japanese and Chinese locales fall back to English for
  most panels.
- ~~The animation picker's View and Rendering categories are not yet
  gated by render mode.~~ They are (`target_selector.rs`).

## The escape engine: not built

Moved here from the escape engine's design record (now
[escape-time-fractals.md](../archive/escape-time/escape-time-fractals.md), archived) when its
main doc, [docs/main/ESCAPE.md](../main/ESCAPE.md), was written. Everything
else in that record is built. These are not:

- **Perturbation tiers for the transcendental formulas** —
  `tetration`, `exponential`, `trig`, `collatz`. The deltas exist
  (`exp(Z+d) = exp(Z)·exp(d)`); each needs its own error analysis.
  Ducks' trig variants 1-3 (Secant among them) sit here too: they
  want big-float `sin`/`cos`/`exp` on the reference side, which is
  comparable groundwork to the `ln`/`atan2` already done.
- **Chebyshev over the two non-unity Newton functions** DECLINES the
  tier, by measurement rather than omission: its `r` is
  asymptotically constant, so the quotient rule loses the delta, and
  the symbolic cancellation that fixes it exists only for `z^p - 1`.
  Recorded so it is not re-attempted blind.
- **Series approximation** — absent, and still unmeasured against
  BLA. Worth a Phase-0 measurement before any building.
- **Row banding of the perturbed path** — would let a frame exceed a
  device's buffer limit by sizing the per-pixel state to a band, as
  the direct path already does via `tile_y0`. Only needed where a
  device's real `max_buffer_size` is under the frame's state (398 MB
  at 4K); accumulation covers the antialiasing case. Not built.
- **Temporal antialiasing / spectral rendering** — motion blur and
  wavelength-correlated fringing on a zoom. Planned, never scheduled;
  it is an export-loop feature, not a shader one, and wants its own
  plan. Full costing in the archived new-families research (§7).
  Motion blur now has its plan, for every engine and in the app too:
  [motion-blur.md](motion-blur.md); the spectral part is not in it.
- **GPU reference-orbit computation (the NTT direction)** —
  measurement-gated go/no-go, never run. See
  [escape-ntt-reference.md](../archive/escape-time/escape-ntt-reference.md).
- **Mode C, the escape-time IFS buffer** -- closed for affine maps by
  decision D9 (2026-09-10; ESCAPE.md, the modes' table): mode D gives
  its level in one pass. Folds and non-invertible maps would be its only
  ground, should it be revived.
- **Mode D ignores opacity.** It draws the attractor of every map, where
  the chaos game plots only the points whose last map plots: any flame
  with a hidden transform (opacity 0) draws more in mode D. Found
  2026-10-23 with level flames, whose hidden levels mode D drew over the
  picture
  ([level-dependent-flames.md](../archive/projects/level-dependent-flames.md)).
  The fix: the walk's first inverse -- the CPU's handover prefix
  (`seed_beam`) and the shader's level 0 -- admits only transforms with
  opacity above 0; xaos admits the rest. Test: a two-map flame with one
  map hidden, mode D against the chaos game's coverage.
- **`timeline_of_a_cached_revisit`** fails (148 vs 197 limbs) and has
  since before 2026-09-02; verified pre-existing against a stashed
  tree. Unrelated to the tiers.
