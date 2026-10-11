# The camera lens in every mode

**Status:** planned (2026-10-10). Not started. Three decisions (§5) are
needed first.

The escape engine has a **camera lens**: a variation that warps the
view, not the fractal ([escape-camera-lens.md](../archive/escape-time/escape-camera-lens.md)).
This plan brings it to every mode:
- 2D and 3D flames;
- every escape view: the plane, Flame Attractor, mode D's solid, the
  terrain;
- every simulation, both its 2D picture and its terrain.

The lens only distorts the camera.

## 1. Not an effect

The effect chain runs on the finished image. As a lens there, it would
resample that image:
- **Magnified areas stretch pixels.** The detail was never rendered, so
  they go soft.
- **Compressed areas alias** unless the resample is filtered.
- **The frame's edges are missing.** A lens that brings in what lay
  outside the frame has nothing there to show, so the borders come out
  black or smeared.

Fixing these would mean rendering an oversized, supersampled frame. That
is costly and still approximate.

So the lens goes where each engine decides what a pixel sees. There it
is exact: no pixel is resampled, and whatever the lens brings in from
outside the frame is rendered like everything else.

## 2. Two directions

**Pull-back (the per-pixel engines).** Each pixel's screen offset `n`
goes through the lens, and the engine samples at `L(n)`:
- escape kernels sample the plane there;
- a ray tracer bends the ray through it;
- a simulation reads its grid there.

This is what the escape lens does. It need not be invertible: a
many-to-one lens shows a region twice, and every pixel still gets one
sample.

**Forward (the chaos game).** A flame computes points and plots them, so
there is no per-pixel question to bend. The lens instead moves each
plotted point, after projection and before it is binned: point → screen
offset `q` → `L(q)` → pixel. This is also exact, with no resampling.
Points that fall outside the unlensed frame land inside it if the lens
puts them there.

The two directions give **inverse pictures for the same variation**.
[escape-camera-lens.md](../archive/escape-time/escape-camera-lens.md) §1 already measures this:
`eyefish` as a lens looks like `eyefish`-inverse. See decision 1.

**Conventions.** Every site uses the escape lens's units: half-height
one, with `n.y` in `[-1, 1]` and `n.x` in `[-aspect, aspect]`. Amount
works as it does there, `mix(n, L(n), amount)`, where negative values
are roughly the inverse. A lens also sees its view's own y: the plane is
y up, while the Flame Attractor and mode D are y down. Flames are y down
too, as the Flame Attractor follows them.

## 3. Mode by mode

| Mode | Today | How | Exact? |
|---|---|---|---|
| Escape plane (direct, field, perturbed kernels) | Yes | pull-back, the `//__LENS_APPLY__` sites | yes |
| Flame Attractor, mode D planar and solid | Yes | pull-back; `ifs_ray` carries it to the relight | yes |
| Escape terrain | No | pull-back: bend primary rays at the rig's `//__LENS_APPLY_RAY__` | yes; the planner needs to know (§4.4) |
| Simulation terrain | No | as the escape terrain | yes |
| Simulation 2D | No | pull-back: each output pixel reads the grid at `L(n)` | yes, at the grid's resolution, which is the content's |
| 2D flame | No | forward: in `world_to_pixel` | yes |
| 3D flame | No | forward: in `project_3d_full` / `world_to_pixel_3d` | yes, except solid shading (decision 3) |

## 4. The work

### 4.1 One lens for the camera, not for the escape engine

Today the lens is three `EscapeConfig` fields:
- `lens`, the variation's name;
- `lens_params`, its parameters;
- `lens_amount`, with its limit `LENS_AMOUNT_LIMIT`.

The paths are `EscapeLens`, `EscapeLensAmount` and
`EscapeLensParam { param }`, and the View panel draws the section through
`escape_panel::show_lens_section`.

- **Config.** Move the three fields to a camera-level `CameraLens` on
  `FractalConfig`, skipped when it is the identity. Saved configs that
  carry `escape.lens*` load into it. Escape pictures are then
  byte-identical, which the visual suite checks.
- **Paths.** Use `Lens`, `LensAmount` and `LensParam { param }`. The old
  `Escape.Lens*` keys keep parsing as aliases, so animation tracks and
  scripts keep working.
- **The key strings change.** Regenerate the engine contract and tell
  the API repository ([RELEASE.md](../RELEASE.md) §3).
- **UI.** The View panel's lens section becomes the camera's. Each
  mode's availability goes in `ui/visibility.rs`, never in a `matches!`
  inside the panel. `LENS_RECOMMENDED` and the probe's usable list carry
  over unchanged.

### 4.2 Flames, 2D and 3D

- **The hook.** In `shaders/core/utilities.wgsl`, `world_to_pixel` and
  `project_3d_full` apply pan, rotation and zoom, then
  `center + transformed * scale`. The lens goes between the two, in
  half-height-one units, behind a `LENS` template flag. Off, the WGSL
  stays byte-identical, which the canonical shader dumps check.
- **Before truncation.** It applies to the continuous screen position,
  before it is truncated to a pixel, and before or with the at-splat
  depth of field.
- **The variation code.** The lens becomes one extra transform in the
  flame's own packed variation set, a pseudo-transform after the final.
  That keeps its parameters in the existing parameter buffer, so slider
  drags need no recompile, and it adds no binding. Browsers count a
  layout's storage buffers against
  `max_storage_buffers_per_shader_stage`; see CLAUDE.md, GPU code.
- **Not through `apply_variations`.** It is evaluated by a dedicated
  single-variation call. Drivers inline `apply_variations` at every call
  site, so a fourth site would add its compile cost to every flame.
- **Brightness (decision 2).** Forward mapping spreads points where the
  lens magnifies: a region of area `A` lands on `|det J|·A`, so its
  density falls by `|det J|`. Weighting each sample by `|det J_L(q)|`
  keeps the unlensed brightness. `J` is found by finite differences,
  two extra lens evaluations a plotted point. Every channel is weighted
  alike, as the depth-density weights are, so colour is unaffected.
- **Focused Rendering** plans which words reach the unlensed view. A lens
  changes what reaches it, so the first version turns the lens off under
  targeting and says so in the UI. Planning against the lens's preimage
  can come later.
- **Solid 3D flames (decision 3).** The shade pass rebuilds normals and
  occlusion from the per-pixel depth image through the unlensed
  projection. Under a lens, a pixel's neighbours are not the points
  that projection assumes, so the normals tilt where the magnification
  varies. Shadow maps are in light space and are unaffected.

### 4.3 Simulation 2D

- **The read.** Each output pixel reads the field at its lensed
  position.
- **Colourings that read neighbours** (gradients, the SDF) work per
  cell, so the clean cut is to lens the coloured picture: one bilinear
  read per pixel, at the grid's own resolution. The grid is the content,
  so a magnifying lens shows its cells larger, exactly as zooming a
  simulation does. A per-pixel colour pass at fractional cell positions
  would rewrite every colouring and buy nothing the grid has.
- **Past the grid's edge:** a periodic simulation wraps; any other shows
  the background.

### 4.4 Terrains, escape and simulation

- **The hook.** The terrain passes splice the shared rig through
  `ifs_rig_jittered`, which puts the sub-pixel jitter at
  `//__LENS_APPLY_RAY__`. The lens goes after the jitter, so the walk,
  the relight and the path tracer all build the same rays.
  `PT_CORE_WGSL` empties its own marker for the terrain; check that its
  primary rays come from the rig.
- **The section planner.** `wanted_sections` plans from the unlensed
  frustum, its corner rays widened by 1.2×. A lens that widens the view
  would meet ground the planner never asked for: coarse roots, or
  nothing past the reach. The fix:
  - bound the lensed screen offsets, with a small GPU probe of `L` over
    a grid of the screen, run when the lens changes and cached;
  - plan with that widened frustum;
  - scale `detail` by the lens's largest magnification, so the magnified
    ground is fine enough.
- **Bindings.** The lens's parameter buffer joins the terrain's layouts.
  Count them against the browser limit.

## 5. Decisions before starting

1. **Which way a flame's lens bends.**
   - (a) The exact forward map: the variation applied to the picture.
     Strong lenses are exact, but the look is the inverse of the same
     lens in an escape view.
   - (b) Flip the default amount so mild lenses look the same in both.
     This is first-order only; strong lenses fold.
   - Recommended: (a), saying so in the UI. The amount's negative side
     is there either way.
2. **A flame's brightness under a lens.**
   - (a) Keep it, with the `|det J|` weight.
   - (b) Let magnified regions dim.
   - Recommended: (a), so the lens stays a purely geometric change, as it
     is in every pull-back mode.
3. **Solid-shaded 3D flames.**
   - (a) Turn the lens off for solid shading at first.
   - (b) Accept approximate normals.
   - (c) Store each pixel's pre-lens screen position, and have the shade
     pass unproject through it: one more channel in the depth region.
   - Recommended: (a) now, (c) as a follow-up.

## 6. Order of work

Each step is a commit, gated by the release check, the visual suite and
the shader dumps.

1. **Config, paths, migration and UI** (§4.1). Escape behaviour is
   unchanged; the visual suite and an old config's round trip prove it.
2. **Flames 2D and 3D** (§4.2). Tests:
   - the lens off is byte-identical;
   - `linear` at any amount is the identity;
   - each plotted point is the CPU's `L(q)`;
   - the `|det J|` weight keeps a flat field's brightness;
   - targeting and solid shading turn the lens off.
3. **Simulation 2D** (§4.3). Tests: each pixel reads the cell its lensed
   position names; past the edge it wraps or shows the background.
4. **Terrains** (§4.4). Tests:
   - a lensed terrain's primary ray is mode D's lensed ray for the same
     camera;
   - the planner covers the lensed frustum, with no pixel falling back to
     a coarse root in a widening lens.

## 7. Risks

- **Compile cost** of the lens variation in flame shaders, if the
  single-variation call is not kept separate from `apply_variations`.
- **Browser binding limits** in the terrain layouts.
- **API key change:** `Escape.Lens*` becomes `Lens*` (§4.1). The aliases
  cover old files and tracks; the API needs telling.
