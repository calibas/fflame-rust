# Terrain speed-ups

**Status:** a rough outline (2026-10-31), nothing built. Split out of the
quality governor ([quality-governor.md](../archive/projects/quality-governor.md)), whose
render scale cannot reach the cost that matters here.

A deep escape terrain plays and moves slowly, and the time goes to
rendering its sections, not to drawing the ground. Measured with the
governor's scene `escape-terrain-deep` (terrain-deep-60, a yaw track) at
60 Hz on a 754x424 viewport: 34 fps, the GPU's median frame 3.9 ms, the
lit pass 11-14 ms, and single section steps of 100-360 ms.

## What the quality governor does

`App::govern_terrain` (`src/app/governor.rs`) lowers an escape terrain's
render scale to 75% or 50% while an animation plays or the view moves,
in the Lit and Auto tiers. It learns its cost only from frames that drew
a moving view's lit pass with no sections arriving
(`TerrainTiers::drew_fresh_lit`, `EscapeTerrain::sections_missing`): a
section's render is not a cost the render scale moves, and counting it
shed the deep terrain for nothing. Path traced is not governed. A
simulation terrain follows the simulation's grid scale. Without a GPU
timer (some browsers) terrains are not governed at all.

So the governor keeps the drawn ground at the target frame rate and
leaves the sections alone.

## Where the time goes

**Section steps.** Missing sections render in chunks
(`EscapeTerrain::update`): `steps` a frame, as many as fit about 8 ms at
the measured section time (`section_ms`), between 1 and 4
(`src/app/mod.rs`). The floor is one step, and on the deep terrain one
step took 100-360 ms, so those frames were that long whatever the budget
said. Why a step is that large there (the escape renderer's chunking of
a deep, perturbed section; the prefilter's square; the hidden roots at
`COARSE_SAMPLES`) is the first thing to find out.

**The lit tier while sections arrive** (suspected by the user, the
mechanism read in the code, its cost not measured). `TierInputs::filling`
says the viewport "draws one picture a ground and leaves the GPU to the
fill", and the path tracer does: one sample a ground. The lit tier does
not. In `TerrainTiers::viewport` a landing section changes the ground's
version, which restarts the accumulation (one pass), and on every frame
until the next landing it draws more antialiasing passes, as many as fit
`LIT_FRAME_MS` (12 ms), which the next landing throws away. `filling`
only stops those passes being timed. On the deep terrain a lit pass is
11-14 ms, so each frame of the fill carries about one discarded pass
beside its section step.

## Candidates, roughly in order

1. **Measure.** Frame-log columns for sections missing, section steps run
   and their time, and lit passes drawn a frame; the deep scene and a
   shallow one; the fill's wall time, the frames' pacing.
2. **One lit picture a ground while filling**, as `filling` says: no
   further passes until the sections are in, or until the next landing.
   Measure the fill's time and the frames before and after.
3. **Smaller steps.** Split a section's render so a step fits the
   frame's share (cut the chunk, not only the count of chunks), so a
   frame with a step is a frame on time.
4. **A section budget the governor sets.** The section work a frame
   from the Target Frame Rate and what the drawn ground leaves (the
   governor's GPU span), in place of the fixed 8 ms: tight while the
   picture moves or plays, generous at rest so a still fills fast.
5. **Section detail while moving** (the governor plan's ladder, *The
   ladder*: "section detail while moving"): fewer, coarser sections
   while the view moves (`terrain.detail` lower), the full detail once it
   rests. Check what that does to the planner's cap, the cache and the
   picture key before relying on it.

## Open questions

- Is the long step GPU time (the section's render) or CPU (the planner,
  a readback wait)?
- How much of the fill is the roots, which cost the most a sample at
  depth?
- Does the web path differ: a browser's export loop and its section
  pacing (TERRAIN.md, *The section planner*, Settling)?

Read [TERRAIN.md](../main/TERRAIN.md) (*The escape terrain*, *Render
tiers*) for how sections and the tiers work now.
