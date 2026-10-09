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

**Where it stands** (GTX 1660 SUPER, the shipped Menger Sponge at
1080p, default settings -- two bounces, two lights, shadows 0.7): a
sample takes **6.2 s** in the viewport, **4.7 s** in an export; the lit
picture of the same view takes **0.6 s**. The default 256 samples is
about 25 minutes. The 2026-10-08 fix (`a73c481d`, see
[heightfield-3d.md](heightfield-3d.md), "Losing the device, and
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

**Measuring.** The ignored tests in
[src/escape/renderer/solid_path.rs](../../src/escape/renderer/solid_path.rs):
`menger_band_costs` (the lit walk for scale, bands, a sample at each
call budget, whole frames at 640 wide or less; `MENGER_W`,
`MENGER_BOUNCES`, `MENGER_SHADOW`, `MENGER_LIGHTS`) and
`the_viewport_pace` (driven as the app drives it; `PACE_MENGER=1`,
`PACE_W`, `PACE_SHIFT`). Measure small: a band near two seconds
resets the display driver for the whole desktop.

## Known gaps, not fixed in PR #129

- The video's deep-zoom memory check looks only at the base config, not
  at each frame's zoom.
- The escape-time and simulation engines still allocate a flame
  histogram they never read, which is what sets their export size
  ceiling.
- No PNG input for `export` (a PNG's embedded config); flame-only
  metadata fields are written for escape and simulation pictures.
- Browser exports don't generate texture layers, for a 2D or a terrain
  picture.
- The Spanish, Japanese and Chinese locales fall back to English for
  most panels.
- The animation picker's View and Rendering categories are not yet
  gated by render mode -- deferred to the camera/View panel work.
