# Surface textures: the relief's texture on the 3D surface

**Status 2026-10-12:** step 1 built on branch `escape-expansion`
(TERRAIN.md, "The surface texture"). Decided with the user: the 2D relief and the 3D terrain share
one set of texture settings; step 1 first; the simulation textures
(the Textures panel's recipes) are one of the sources.

## Why

The escape relief's texture (Grain, Paper, a simulation texture) leaves
much to be desired:
- **It is fixed to the screen.** It is defined in display pixels, so the
  fractal slides under it when you pan or zoom.
- **The noise is crude.** It is one octave (Grain) or three stretched
  octaves (Paper) of value noise, with blocky lattice artefacts and no
  filtering against the pixel.
- **It only tilts the 2D relief's normal.**
- **The 3D terrain has none.** It shades the height field's smooth
  normal with one uniform material. The 2D relief, texture included, is
  off under a terrain.
- **The simulation texture has no mipmaps**, so it shimmers when
  minified.

The user's example: rough cliffs, from a texture.

## The plan

One surface-texture system, shared by the 2D relief and the 3D terrain:
- **Mapping:** on the terrain, triplanar (three planar projections
  blended by the normal), anchored to the plane, at a size that stays the
  same on screen as you zoom.
- **Channels:** bump first; roughness, gloss and albedo maps later;
  maybe displacement.
- **Masks:** slope first (steep faces are the cliffs); height and
  crease/curvature later.
- **Filtering:** each octave fades as its wavelength nears the pixel's
  footprint; the simulation texture gets mipmaps.
- **Kinds:** Grain, Paper and the simulation texture, plus new ones
  (Rock first; canvas, sand and hammered metal later).

### Steps

1. **Bump on the terrain, with a slope mask** (this step):
   - the noise library;
   - plane anchoring and zoom-stable octaves;
   - footprint filtering;
   - triplanar bump in both the Lit and path-traced tiers;
   - the Where/angle mask, also in the 2D relief;
   - Rock as a new kind;
   - simulation textures on the terrain, with mipmaps.
2. **Material maps:** roughness, gloss, albedo tint.
3. **The 2D relief anchored to the picture**, with zoom-stable octaves and
   the new noise (the 2D Grain and Paper keep their old look until then).
4. **More kinds:** canvas, sand, hammered metal.
5. **Maybe displacement**, at section ingest, so the walk's bounds stay
   right.

## Step 1 design

### Settings (shared, in `EscapeShading`)

- `texture_kind`, `texture_strength`, `texture_scale`: the existing
  ones. `ShadingTexture` gains **Rock**.
- New: `texture_where` (Everywhere, Steep, Flat), `texture_angle` (the
  slope where Steep begins, degrees, default 45) and `texture_angle_soft`
  (the half-width of the change, default 10). All are skipped while
  default.
- **The terrain** applies the texture whenever the kind is not None. It
  has its own lighting, so it does not depend on the relief's on/off
  switch. The panel enables the texture controls when either is on.
- **Scale** stays in display pixels for the 2D relief (screen-anchored
  until step 3). On the terrain, `texture_scale` is pixels of a
  1080-line view at the target's distance, so the two match at 1080p and
  the terrain's texture does not change with the output size.

### Anchoring and zoom-stable octaves

**The octave ladder:** octave *k* has wavelength 2^*k* in plane units,
fixed in the plane. The texture is a weighted sum over the octaves near
the target wavelength λ = scale × (a 1080-line pixel at the target) ×
(the current view width in plane units):
- *c* = log2 λ;
- *j* = *k* − *c*;
- weight *a*(*j*) = a window over *j* ∈ (−*J*, 1), with fractal falloff
  2^(*H j*) per kind.

As you zoom, *c* slides and the window slides over fixed octaves, so the
texture stays on the land and keeps its size on screen.

| Kind | *J* | *H* | Per octave |
|---|---|---|---|
| Grain | 1 | 0 | gradient noise |
| Paper | 3 | 0.7 | fibres at four orientations, built from integer matrices so the octave stays periodic |
| Rock | 4 | 1 | ridged, (1 − \|n\|)² |
| Simulation | 1 | 0 | the image's luminance, one tile per wavelength |

**The phases:** the terrain's world is in current view widths, with the
origin at the eye's ground point. Octave *k*'s texture coordinate is
*o<sub>k</sub>* + *p*·(*W*/2^*k*), where *W* is the current view width
in plane units and *o<sub>k</sub>* is the eye's plane position over 2^*k*,
mod the lattice period *P* = 1024.
- The CPU computes *o<sub>k</sub>* exactly: the anchor's decimal centre
  as a `FixedPoint`, from which the bits are taken (a window of the
  binary expansion), plus the eye's offset from the anchor in f64.
- Every per-octave function is exactly *P*-periodic, so wrapping costs
  nothing.
- Heights (the triplanar side projections) need no phase: *z* is
  absolute from the plain.

**Filtering:** octave *k*'s weight fades to 0 between λ<sub>k</sub> = 4
and 2 footprints, where the footprint is the relight's and the path
tracer's own `foot`. The simulation image samples its mip chain at the
footprint's level.

### The bump

- **Triplanar:** each projection's 2D gradient is lifted into 3D, and
  the projections are blended by |n|⁴.
- **Applying it:** the surface gradient *g* = ∇*h* − (∇*h*·*n*)*n*,
  and *n'* = normalize(*n* − *A*·mask·*g*). *A* is the strength × λ ×
  a calibration constant, so the tilt does not change with the scale.
- **The mask** reads the true normal: the slope angle against the
  angle ± its softness.
- **Lit tier:** the relight bumps the normal it unpacks. A light the
  true normal faces away from stays dark, so the walk's shadow test (made
  with the true normal) still holds.
- **Path tracer:** `pt_surface` bumps the shading normal. `PtHit` gains
  the true normal `ng`, which offsets the next ray and rejects any
  direction under the true surface.
- **Not in `walk_key`:** a texture edit relights, it never re-walks.
  `picture_key` ignores the texture settings, so no section re-renders.
- **The simulation image** is copied on the GPU from the section
  renderer's texture (which the app and exports already fill) into the
  terrain renderer, with a mip chain. `uses_texture` also counts a
  terrain with the Simulation kind.

### Cost

Nothing with the kind None (a uniform branch). Otherwise per shaded hit:
up to 3 projections × ≤ 5 octaves, minus the projections the blend
weighs at 0. To be measured on the path tracer.

### Tests

- **Anchoring (CPU):** a ground point's texture coordinate is the same
  from two eye positions, and exact past f64's reach.
- **Octave windows (CPU):** cover the scale continuously.
- **Masked bump (GPU):** Steep changes only steep pixels.
- **Path tracer (GPU):** finite, and deterministic.
- **A visual config:** Rock on cliffs.
