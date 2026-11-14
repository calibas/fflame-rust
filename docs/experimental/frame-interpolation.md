# Frame interpolation for the video export

Parked 2026-11-14, while planning multi-pass effects
([multipass-effects.md](../archive/projects/multipass-effects.md)).

Interpolation makes frames that were never rendered. That is a stage of
the video export (`animation::export`), not an effect: it changes how
many frames there are. The renderer can draw any in-between frame
exactly, and the export already has true motion blur
(`Animation::motion_blur`), so interpolation only buys speed. That
matters for the expensive renders: path-traced terrains, deep zooms.

- **Generic optical flow** guesses badly on fractals: detail appears and
  changes rather than moving.
- **The camera's motion is known.** A 2D view's map between two frames
  is exact; a 3D view has the depth buffer. Reprojecting the neighbours
  along it and blending would serve zooms and pans.
- **It fails on morphs**: a formula parameter, a flame's transforms, a
  palette. Those would need to be rendered, or blended.

Open: whether a render that slow is common enough to be worth it.
