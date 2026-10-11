# Escape terrain: height quality

> **Archived 2026-10-10** -- finished (H1-H6). Read [TERRAIN.md](../../main/TERRAIN.md) for how it works now; this is the record of what was planned and measured when it was written.

**Status:** built on `terrain-height` (2026-10-10): H1 to H6, and the
export-memory work alongside (docs/main/EXPORT.md).

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
- **H5. A smooth surface.** *Built.* "Smooth surface"
  (`terrain.smooth`, default off): the ground the cubic B-spline of its
  mapped heights, as a simulation's is, rather than a bilinear patch a
  texel -- whose crease at every grid line drew fine streaks down the
  flanks and stepped shadow edges wherever the ground is magnified (an
  8K export, 0.74 texels a pixel). Its colour is its colours' spline
  where a ray's share of a pixel is under a texel. Three things make it
  an escape terrain's:
  - **Seams.** A section's control points past its edge continue in a
    straight line (`hf_extends`: 2 P_edge - P_next) where a simulation's
    single section holds its edge. Neighbours share their edge samples,
    and so continued each side's spline passes through them: along the
    seam both are the same curve of the same samples -- no step, as a
    bilinear ground has none (held, they stood apart by a sixth of the
    step at the edge). The colour is continued alike, in sixteen loads,
    which also keeps a coarse section -- smaller than its layer -- from
    reading the texels past its edge.
  - **Bounds.** The spline is of HEIGHTS, the raw values through a map
    that is not linear, so the raw values' Bezier bound a simulation uses
    bounds nothing here. The map is monotone and the spline a weighting
    of its control points, never negative and summing to one -- past the
    edge too, continued as above: in the first cell the real samples'
    weights are 2B0 + B1, B2 - B0 and B3, in the last B0, B1 - B3 and
    B2 + 2B3, each non-negative over the cell -- so the extreme of the
    4x4 real samples bounds it (`BUILD_WGSL`'s spline mode 2).
  - **No re-render.** Turning it on or off re-bounds every cached
    section (`reshape_section`) and renders none again.
  Tested against the CPU's spline on a two-section ground with a rough
  field: every hit within 5e-6 of the CPU's root, none slipping through,
  the seam's step under 1e-9, the colour within 6e-7. The cost is the
  walk's: path traced at 1080p, 340 ms a sample against 133 (2.6x); the
  lit tier at 8K, 142 s against 43 s with the sections. Hence off by
  default -- a still's choice, and the viewport's where it is affordable.
- **H6. More sections for an export.** *Built.* The atlas held at most
  64 sections, and the planner coarsens every section alike until the
  view fits: at 4K a terrain's nearest ground was 0.74 texels a screen
  pixel (magnified; 0.49 at 4K with 2x AA), while the viewport at
  1600x900 had 2.5. An export now fits its cap to the card
  (`fit_export_sections`): 256, then 128, then 64 -- the first whose
  atlas (~21 MB a section) and the rest of the export's peak
  (`export_headroom`, estimated high) the card will give, asked for in
  256 MB pieces and given back at once, since wgpu cannot say what is
  free. On a 6 GB card a 4K export holds 128 (1.66 texels a pixel, 108
  sections, peak 4.25 GB) -- visibly crisper. The viewport keeps 64. An
  export whose view wants no more than 64 is unchanged; one that wants
  more is as fine as the card it is made on allows. Only a discrete card
  is asked: a unified memory (Apple silicon, an integrated GPU) can grant
  far past what is comfortable and pay in paging, a browser may not
  allocate until a buffer is used, and a software renderer is the
  reference -- they keep 64.
