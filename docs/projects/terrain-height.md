# Escape terrain: height quality

**Status:** built on `terrain-height` (2026-10-10): H1 to H4.

The escape terrain's fine detail comes out as noise, its heights warp and
shift as you zoom, and the Height slider did not always redraw. The
pipeline -- an escape render per section, ingested as raw values, mapped
to heights in the walk -- is in `src/escape/footprint.rs` (module docs)
and `src/escape/terrain.rs` (`INGEST_WGSL`, `hf_f`).

## Where things stood

- **The map** (`hf_f`): distance `H·exp(−d/w)` (d the estimate in current
  view widths); escape count `H·log(1 + n − lo)/log(1 + span)`; relief
  linear. `H` and `w` are fractions of the CURRENT view width.
- **`lo`/`span`** are the count range over the ground's sections,
  re-reduced every time the ground is set: at every zoom step, and as
  finer sections land -- so a count terrain rescales while you zoom and
  while it loads.
- **No smoothing.** Every section texel is a point sample of the escape
  render (supersample 1); the boundary's counts are rough at every scale,
  so sub-texel detail aliases into spikes. Distance heights are smooth
  because the estimate is.
- **The Height slider** (H1 below).

## Decisions (the user, 2026-10-10)

1. **Zoom:** a setting that fixes the relief at a chosen zoom -- no warp,
   no shift while zooming: a fixed landscape, defined there.
2. **Smoothing:** both a prefilter (each texel the average over its
   footprint) and a Softness control (a world-scale blur); sections
   rendered with a border so neighbours stay seamless.
3. **Curves:** Invert, a Mesa curve and a Linear curve, beside today's
   logarithmic one.

## Phases

- **H1. The slider, and 0.** *Built.* The ground's slab top was the
  height when the ground was last set, and the height was not in the
  ground's key, so a raised height drew the terrain cut flat at the old
  one until a section landed or the camera moved. The height is in the
  key. Height reaches 0 (a flat plane); a hole's floor is a sixth of the
  relief, at most the 0.01 view widths of the default 6% and above.
- **H2. Curves and invert.** *Built.* A Curve (Auto -- log for a count,
  linear for a relief, as before -- Log, Linear, Mesa) and Invert,
  walk-time uniforms (`fdata[2]`): no section re-renders. The curves
  shape the land's rise over the count range `v` in [0, 1]: Linear `v`,
  Log `log(1 + n − lo)/log(1 + span)`, Mesa `(1 − e^(−6·v_log))/(1 −
  e^(−6))` -- the log curve pushed up into broad tablelands with steep
  shores. The distance source keeps its own `exp(−d/w)` shape, so the
  Curve is hidden for it. Invert flips the land (`H − h`), for every
  source; holes and the interior choice stay where they are (a hole
  floors, a lake is flat at 0, a plateau tops). An inverted map is not
  increasing, so the walk's empty-space bound for it is `H` -- tight,
  as an inverted terrain stands near `H` almost everywhere. The
  distance source's plateau sentinel became `−1e30` (from 0) so the
  inversion can tell it from land at distance 0; the plain picture is
  unchanged (both map to the top).
- **H3. A height fixed at a zoom.** *Built.* "Lock height at this zoom"
  (`terrain.locked`, `lock_zoom`, `lock_range`). Locked, the height, the
  flanks' width, occlusion's reach and a hole's depth are fractions of
  the lock zoom's view width: `EscapeConfig::terrain_scale` is
  `2^(zoom − lock_zoom)` current widths (held within 2^±24, where an f32
  world still resolves the view), applied wherever the walk, the camera
  and the section planner read them. A count's or a relief's range is
  pinned as the view measured it at the lock -- the ground's reduced
  range, read back without waiting (`read_range_back` / `poll_range`)
  and handed to the panel -- so locking changes nothing on screen
  (tested to the bit) and the heights stop following the ground's range
  as it zooms and streams; the panel shows the range for editing. "Lock
  here" moves the lock to the current zoom. A pinned range also makes an
  export's count heights the viewport's: unlocked, the export measures
  the range over its own, finer sections.
  - **The camera** still rests its target on the relief's top -- the
    locked top, in current widths -- so zooming in dollies toward a
    point at the peaks' altitude: the peaks are approached, while a
    valley falls away and, far past the lock, out of reach (Target lift
    brings the camera down). A target resting on the ground under the
    view's centre would follow the land but needs the height there,
    which the GPU holds and which moves as finer sections land; not
    done.
- **H4. Section borders, prefilter, Softness.** *Built.* A section's
  texels are made from its render in three passes (`INGEST_WGSL`: raw
  values, the softness along the rows, the store down the columns), one
  path for every filter -- at its defaults the ingest every section had,
  which the visual suite confirms.
  - **Prefilter** (Off, 2×2, 3×3; `terrain.prefilter`, default Off): the
    section renders at that supersample, and a texel is the mean of its
    samples that escaped (the interior where most did not; a distance
    in texels, from the render's pixels). The supersample's samples sit
    symmetrically about each grid point, so this is a box over the
    texel's footprint and needs no border. It costs its square in
    section render time and in the section renderer's memory -- about
    0.15 GB off, 0.6 GB at 2×2 and 1.4 GB at 3×3 (148 bytes a render
    pixel), within the escape renderer's 1.5 GB budget, which
    clamps the factor where it cannot (a browser's limits may hold it
    at 1). Measured over the seahorse valley's busiest section, 2×2
    takes a fifth off the land's spikiness (mean |Laplacian| of the log
    rise, 0.0052 to 0.0043); on screen a quarter off the step between
    pixels.
  - **Softness** (0 to 4, `terrain.softness`, default 0): a Gaussian of
    that many texels over the land, the interior left as it is and the
    weights the land's own, so a shore is not pulled toward a sentinel.
    The section renders with a border of three sigmas, so the blur at
    its edge reads its neighbour's ground: shared edges agree exactly,
    softened or not. Softness 2 takes the same section's spikiness from
    0.0052 to 0.0004. The colours are not blurred.
  - **Texels, not plane units** -- a change from the plan above. A blur
    in plane units would be a fraction of the view width, which changes
    with every unlocked zoom step: every cached section would re-render
    on each wheel notch, and near the eye, where texels are finest, its
    reach in texels -- the border -- would run to hundreds. A texel is
    about `supersample × detail` of a screen pixel where the ground is
    nearest, so a softness in texels is a softness on screen: the same
    fine bumps rounded off everywhere, the sections cacheable, the
    border at most a dozen texels. Where a coarser section meets a
    finer one, the blur's reach in the plane changes with the
    resolution, as the detail already does.
  - Both are part of what the sections' pictures depend on
    (`picture_key`): changing either re-renders the ground.
