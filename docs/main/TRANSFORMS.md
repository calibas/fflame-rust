# Flames and Transforms

**Overview:** The flame model and the chaos game that draws it. A
**flame** is an iterated function system: a set of **transforms**, each
an affine map followed by a weighted blend of nonlinear **variations**.
The renderer runs the chaos game on the GPU — every thread carries one
point, picks a transform at random each iteration, applies it, and
deposits the result into a histogram — and the picture is the density
of where the points land.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — overall system design
- [VARIATIONS.md](VARIATIONS.md) — the variation registry, phases and
  features that `apply_variations` is built from
- [RENDERER.md](RENDERER.md) — dispatch sizing, persistent orbits,
  accumulation and tone mapping
- [SHADERS.md](SHADERS.md) — how the per-flame shader is assembled
- [BUFFERS.md](BUFFERS.md) — `GpuTransform`, `GpuVariationParams`, the
  attachment buffer
- [CAMERA.md](CAMERA.md) — the 3D camera and the view mapping
- [COLOR.md](COLOR.md) — colour modes and the palette
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the forced-prefix plot
  for deep zooms
- [ESCAPE.md](ESCAPE.md) — the escape engine, whose IFS distance mode
  (mode D) renders the flame's transforms
- [CONFIG.md](CONFIG.md) — `FractalConfig`, `ConfigPath`, undo

**Code locations:**
- [src/scene/transforms.rs](../../src/scene/transforms.rs) — `Flame`,
  `Transform`, `PostSymmetry`, `RenderMode`, the per-flame local index
  map and the packed parameter/state layouts
- [src/scene/xaos.rs](../../src/scene/xaos.rs) — xaos matrix
  maintenance, closed-group analysis, xaos-linked pairs
- [shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl)
  — the chaos-game kernel
- [shaders/core/affine.wgsl](../../shaders/core/affine.wgsl),
  [shaders/core/affine_3d.wgsl](../../shaders/core/affine_3d.wgsl) —
  pre- and post-affine
- [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl) —
  transform selection, projection, `world_to_pixel`, post-symmetry
- [shaders/core/subflame.wgsl](../../shaders/core/subflame.wgsl) — the
  nested chaos game behind `subflame_wf`
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) — packing the flame
  into GPU buffers
- [src/flame_xml.rs](../../src/flame_xml.rs) — Apophysis / JWildfire
  `.flame` import and export

---

## The model

`FractalConfig.flame` holds the flame. The whole-render settings that
used to sit on it — `render_mode`, `perspective_strength`, `preserve_z`,
`depth_density_compensation`, `far_density_fade`,
`far_density_fade_start` — moved to `FractalConfig` in config v3 (the
v2→v3 migration lifts them; a nested copy left in an old blob is
ignored on load). The view and the camera — four angles and a
world-space position — are `FractalConfig` fields too
([CAMERA.md](CAMERA.md)).

### `Flame`

| field | means |
| --- | --- |
| `id` | Session-local identity for animation tracks. Never serialized; `FractalConfig::fixup_ids` assigns one after load. |
| `name` | Display name. |
| `transforms` | The **normal** transforms — the ones the chaos game selects by weight. |
| `linked_transforms` | The Linked pool, referenced by index from each normal's `linked_attachments`. |
| `final_transforms` | The Final pool, referenced by index from each normal's `final_attachments`. |
| `xaos` | `Option<Vec<Vec<f32>>>`, `xaos[src][dst]` over the normals. `None` means every entry is 1. |
| `solo_transform` | `Option<usize>`: plot only this normal (see *Opacity and solo*). |
| `subflames` | Nested flames that `subflame_wf` iterates (see *Subflames*). |
| `post_symmetry` | JWildfire's plot-time symmetry (see *Post-symmetry*). |

### `Transform`

The same struct serves all three pools; which fields matter depends on
the pool.

| group | fields | notes |
| --- | --- | --- |
| identity | `id` | Runtime only, like `Flame::id`. |
| affine | `a b c d e f`, `g` | `x' = a·x + b·y + e`, `y' = c·x + d·y + f`; in 3D `z' = z + g`. |
| selection | `weight` | Normals only. Linked and Final members are never selected. |
| variations | `variations`, `variation_params`, `variation_order`, `variation_priorities` | Weight per variation name; parameters as a flat map keyed `"variation.param"`; the dispatch-order hint; sparse JWildfire `fx_priority` overrides. See [VARIATIONS.md](VARIATIONS.md). |
| colour | `color`, `color_speed`, `opacity`, `direct_color` | Palette position 0–1; Apophysis "symmetry" (−1…1); plot probability; direct-colour blend. Read from the **normal** only. |
| post-affine | `post_affine_enabled`, `post_a … post_f`, `post_g` | Applied after the variations. |
| JWildfire planes | `yz_coefs`, `zx_coefs`, `yz_post_coefs`, `zx_post_coefs` | Six floats each in JWF's positional `a c b d e f` order; identity `[1, 0, 0, 1, 0, 0]`. 3D only. |
| attachments | `linked_attachments`, `final_attachments` | Indexes into the flame's pools. Normals only. |

**Variation weights are a map, and zero is a value.** `set_variation`
keeps an entry at weight 0, so a variation can sit on a transform —
and be compiled into the shader — while contributing nothing; the
dispatcher skips it at run time. `remove_variation` scrubs the weight,
its `variation_order` entry, every `"name.*"` parameter and any
priority override. A parameter that is absent reads as the variation
definition's default (`variation_param_or_default`).

**Per-flame local indices.** The GPU never sees variation names. Each
flame gets a local index map (`compute_local_index_map`) from its active
variation names in flame order — normals, then linked, then finals,
then subflames, each transform contributing its `variation_order` and
then any remaining names in registry order. Local indices run `0..N`
with `N ≤ MAX_VARIATIONS_PER_FLAME = 100`; past the cap the overflow is
dropped with a warning. The shader builder and every buffer packer
derive the same map, so `xform.variations[i]` and `variation_id = i`
agree everywhere.

In the app the map is usually not the flame's own. The renderer's
**sticky superset** (`src/renderer/sticky.rs`) compiles the flame
against a canonical map — the sorted union of its variations and
recently used extras at weight 0 — by writing that map into transform
0's `variations` and `variation_order` before compiling and packing.
A flame where order could change the result falls back to its own map:
a transform with two or more pre or post variations, any `fx_priority`
override, two `Replace` or two colour-writing variations, or a
`NeedsAccum` variation beside another. One-shot renders (CLI export,
the visual suite) turn the superset off. See
[SHADERS.md](SHADERS.md#the-sticky-variation-superset).

### `.fflame` serialization

`Transform` has hand-written serde:
- Maps are written as sorted maps, so a config's JSON is byte-stable.
- Optional groups are written only when they differ from the default:
  the post-affine block when `post_affine_enabled`, each plane array
  when it is not identity, attachments, priorities and the order hint
  when non-empty, `direct_color` when non-zero.
- `direct_color` defaults to **1.0** for a new transform, but a file
  that omits it loads as **0.0**, because files from before the default
  changed omitted it at 0.
- Legacy shapes still load: `variations` as an array (indexed by
  registry order), `color` as an RGB triple (averaged to a palette
  position), and a singular `final_transform` on the flame (moved into
  the Final pool and attached to every normal by
  `Flame::migrate_legacy_final`).
- Every name-keyed field is canonicalised through the registry's alias
  table on load, so a config saved under a renamed variation keeps its
  weights and parameters.

### Affine conventions

The six coefficients follow Apophysis. The `.flame` `coefs` attribute
writes them in the order `a c b d e f`. The triangle editor shows a
transform as the image of the unit triangle in Apophysis's y-down
display: `to_triangle_apophysis` gives `O = (e, −f)`,
`X = (e + a, −f − c)`, `Y = (e − b, −f + d)`. The same mapping drives
the post-affine and the plane-affine triangles
(`post_to_triangle_apophysis`, `plane_to_triangle_apophysis`).

---

## The chaos game

One compute dispatch is `workgroups × 64` threads
(`@workgroup_size(64, 1, 1)`), each running `iterations_per_thread`
iterations of one orbit. Threads share nothing but the histogram
atomics. How many workgroups and iterations a frame gets is the
renderer's business ([RENDERER.md](RENDERER.md)).

### A thread starting

- `rng_init(thread_id, params.seed)` seeds a PCG stream.
- The point starts uniform in `[-1, 1]²` (2D) or `[-1, 1]³` (3D);
  `color_index` starts at 0.
- `fuse` is set to the burn-in count (`SystemSettings::burn_in`,
  default 20): iterations that run but do not plot.
- With xaos, `prev_xform_idx` gets an unconditioned weighted pick.
- Variations with a `wgsl_state_init` fragment initialise their
  per-thread state.

**Persistent orbits.** With `PERSISTENT_ORBITS` on (always, in the
app), a thread whose stored generation matches the current one
**resumes** instead: the point, the 4th coordinate, the colour and
colour index, the burn-in left, the previous transform, and every
variation state slot come back from the orbit buffer and are written
back after the loop. Only a change to the dynamics starts a new
generation — the flame, the render mode, `preserve_z`, the colour mode,
the speed factor — not a pan or zoom. A flame carrying variation state
restarts at every restart of the accumulation. So burn-in is paid once
per generation, not once per dispatch. See [RENDERER.md](RENDERER.md)
and the [design record](../archive/projects/persistent-orbits.md).

### One iteration

1. **Select** a normal transform (*Transform selection*).
2. **Opacity draw:** `should_plot = u < opacity`, with the selected
   normal's opacity. Opacity never changes the dynamics.
3. **Affine:** `apply_affine(xform, current)`.
4. **Variations:** `apply_variations` — pre, normal and post phases
   ([VARIATIONS.md](VARIATIONS.md)).
5. **Post-affine**, if the transform has one.
6. **Linked chain:** each attached Linked transform, in list order:
   affine, variations, post-affine. Its output is the new `current`.
7. **Hide:** if a `CanHide` variation in steps 4–6 set the hide flag,
   `current` reverts to the point the iteration started from and the
   iteration ends — no colour update, no plot, no burn-in countdown.
8. **Bad-value recovery** (below). A respawned point ends the iteration.
9. **Colour update** (below).
10. **Burn-in:** while `fuse > 0` it counts down and nothing is plotted.
11. **Plot:**
    - Focused Rendering, when on, carries the point through a planned
      word first and restores the free orbit afterwards
      ([FOCUSED-RENDERING.md](FOCUSED-RENDERING.md)).
    - The **Final chain** runs on a copy of the point.
    - A hide from a Final variation suppresses the splat only.
    - Each **post-symmetry copy** (and, with multi-emit, each emitted
      point) is projected, weighted by the depth effects in 3D, and
      deposited.

The 2D and 3D kernels are the same template compiled with and without
`RENDER_3D`; the point is a `vec2` or a `vec3`.

### Transform selection

`select_transform_const` normalises by the total weight: it draws
`target = u × Σweight` and returns the first transform whose running sum
reaches it, falling through to the last. `NUM_TRANSFORMS` — the normal
count, at least 1 — is compiled in, so changing the number of
transforms rebuilds the shader. A flame whose weights sum to zero always
picks transform 0.

| function | used when |
| --- | --- |
| `select_transform_const(u)` | no xaos |
| `select_transform_xaos(u, prev)` | xaos: weights are `weight[dst] × xaos[prev][dst]` |
| `select_transform_biased(u)`, `select_transform_biased_xaos(u, prev)` | `IMPORTANCE_SAMPLING`: biased weights, with the likelihood ratio folded into the deposit ([flame-deep-zoom.md](../projects/deep-zoom/flame-deep-zoom.md), stage 1) |

`select_transform(u)`, which reads `params.num_transforms` instead of
the constant, is still in `utilities.wgsl` but nothing calls it.

### Colour

The palette index evolves by Apophysis's rule, with
`s = color_speed`:

```
color_index = color_index · (1 + s)/2 + color · (1 − s)/2
```

`s = 1` keeps the incoming index, `s = −1` replaces it with the
transform's `color`, `s = 0` averages them. Only the selected normal's
`color` and `color_speed` take part; Linked and Final members' colour
fields are not read.

**Direct colour** (`HAS_DC`, compiled in when any active variation has
`WritesColor`): the rule above gives `c_base` before the variations
run; `WritesColor` variations on the normal and its Linked chain write
an iteration-local `vc`; afterwards
`color_index = c_base + direct_color · (vc − c_base)`, with the
normal's `direct_color`. Writes from Final variations are discarded.

**Direct RGB** (`HAS_RGB`): `WritesRgb` variations write `vrc`, which
starts at a sentinel; at the plot, when something wrote it,
`final_color = mix(palette_color, vrc, direct_color)`. Not in PathMap
mode. Final writes are discarded.

The colour mode (`FractalConfig::color_mode`) is compiled in as
`COLOR_MODE`: Palette (0) and PathMap (2) run the palette-index flow
above; Speed (1) mixes a speed colour — from the distance the point
moved this iteration, after the Linked chain — into a running RGB by
`speed_factor`. At the plot the palette is sampled at `color_index` and
converted from sRGB to linear. See [COLOR.md](COLOR.md).

### Opacity and solo

`opacity` is the probability that an iteration through this transform
is plotted. A transform at opacity 0 still moves the point — it is an
invisible step in the dynamics, the trick xaos-linked "pre" transforms
rely on. A Linked or Final member's own `opacity` is not read; the
normal's draw covers the whole chain.

**Solo** (`Flame::solo_transform`, the Transforms panel's Solo box,
`.flame` `soloxform`) does not change selection.
`GpuTransform::from_flame` uploads every other normal with an effective
opacity of 0, so the attractor is the same and only the soloed
transform's share of it is drawn.

### Bad-value recovery

flam3 / JWildfire style, in two tiers, after the Linked chain:
- **X or Y** not within ±1e32 (NaN included — the test is
  `!(abs(x) <= 1e32)`, which survives fast-math): the point respawns
  uniform in `[-1, 1]²` with `z = 0`, and `fuse` resets to the burn-in,
  as JWildfire re-fuses a re-randomised point.
- **Z** (3D) saturates at ±1e32 instead, because z compounds
  legitimately through `AlwaysZ` variations under `preserve_z = true`
  and respawning would starve the attractor. A NaN z becomes 0. While z
  sits on the rail, each iteration has a 1/256 chance of respawning,
  standing in for JWildfire's eventual f64 overflow.
- **The 4th coordinate** (`NeedsW` variations) gets the same rail, and
  resets to 0 on respawn.

See [preserve-z-semantics.md](../archive/projects/preserve-z-semantics.md).

### Post-affine

`GpuTransform::post_enabled` is `Transform::has_post_step()`: the XY
post-affine is enabled, **or** a YZ/ZX post plane is not identity.
JWildfire gates the three post planes independently, so a transform
can carry `zxPost` with no `post` attribute at all. In 2D only the XY
part exists.

### 3D affines

`affine_3d.wgsl` has two paths, chosen per transform by the
host-computed `plane_flags` (bit 0 YZ, 1 ZX, 2 YZ post, 3 ZX post):
- **Flat** (no plane flags): the Apophysis affine plus `z' = z + g`.
  Every Apophysis flame takes this path.
- **Full:** JWildfire's `TransformationAffineFullStep`. The XY, YZ and
  ZX linear parts are applied in that order, then all the offsets are
  summed unrotated: `x + e + zx[4]`, `y + f + yz[4]`, `z + yz[5] + zx[5]`.
  `g` (and `post_g` on the post side) has no place in it and is lost —
  a known limitation for a flame that mixes `g` with plane affines.

The plane coefficients are ignored in 2D.

### `preserve_z`

A `FractalConfig` field, compiled into the shader for 3D. With
`preserve_z = false` — the Apophysis/JWildfire default and the default
for new flames — the normal-phase dispatcher zeroes the z of every
variation contribution except `AlwaysZ` ones, so z compounds only
through true-3D variations. `NeverZ` variations contribute no z either
way. A config that omits the field loads as `true`, which keeps flames
authored before it looking as they did.

One consequence for `g`: under `preserve_z = false` only normal-phase
`AlwaysZ` variations carry the affine's z — `g` included — into the
transform's output. A post-phase variation or the post-affine's
`post_g` can still add z of its own. The template still has a
`FLATTEN_Z_PER_ITER` block that zeroes z at the end of every iteration;
the builder never enables it. See [VARIATIONS.md](VARIATIONS.md) for
the z features.

### Projection and the view

- **2D**, `world_to_pixel`: pan → rotate → zoom. The point minus
  `(pan_x, pan_y)` is rotated by `rotation`, scaled by `zoom`, then by
  `min(width, height) / 4`, and offset to the image centre. Under
  `CYLINDER_RELATIVE` the forced point arrives already relative to the
  view, and the pan is not subtracted again.
- **3D**, `project_3d_full`: the point minus the camera position,
  through the camera matrix built from pitch, yaw and bank, then
  Apophysis's perspective `zr = 1 − persp · z` with `perspective_strength`
  (0 = orthographic). A sample with `zr < 1e-3`, behind the camera or at
  the singularity, is clipped. Then the same pan → rotate → zoom, with
  `rotation` as the camera's roll. See [CAMERA.md](CAMERA.md).

In 3D the camera-space depth then feeds depth-density compensation, the
depth-of-field jitter, far-density fade, fog and, with solid rendering,
the depth buffer ([RENDERER.md](RENDERER.md)).

### The deposit

A sample in bounds adds `colour × 100 × density_weight` to each of R, G
and B and `100 × density_weight` to the density word of its pixel, as
`u32` atomics. `density_weight` starts at 1 and carries multi-emit
weights, the Focused Rendering word weight, the importance-sampling
ratio and the 3D depth weights. On the sample-emit export path the
sample is written to a stream instead and scattered per tile later.
The accumulate and tone-map passes are in [RENDERER.md](RENDERER.md).

---

## Linked and Final transforms

Every normal transform can carry two ordered lists:

| | Linked | Final |
| --- | --- | --- |
| pool | `Flame::linked_transforms` | `Flame::final_transforms` |
| attachment list | `linked_attachments` | `final_attachments` |
| runs | after the normal's post-affine | after the Linked chain, at plot time |
| feeds the next iteration | yes | no |
| `WritesColor` / `WritesRgb` writes | kept | discarded |
| hide (`CanHide`) | aborts the iteration | suppresses the splat |
| 4th coordinate | kept | restored after the chain |

Linked is part of the dynamics: deterministic extra steps after the
normal fires. Final is a view filter: it reshapes what is plotted and
nothing else. A pool member can be attached to any number of normals.
A pool member's `weight`, `opacity` and colour fields are not read; it
inherits them from the normal that fired.

**On the GPU** there is one transforms buffer with `MAX_TRANSFORMS = 128`
parent slots: normals at `[0, N)`, Linked at `[N, N + L)`, Finals at
`[N + L, N + L + F)`, zero-padded. Subflame transforms follow from slot
128 (*Subflames*). The attachment buffer has one entry per normal
holding the **global** slot ids of its Linked and Final members. Its
per-side array length is `Flame::attachment_cap()` — the longest list
in the flame, at least 1 — compiled into the shader; a list longer than
`MAX_ATTACHMENTS_PER_TRANSFORM = 32` is truncated with a warning.
`HAS_ATTACHMENTS` strips the per-iteration attachment load and both
chain loops from a flame with empty pools.

**Old flames.** A config with the singular `final_transform`, and every
`<finalxform>` in a `.flame` file, becomes a pool member attached to
every normal — the old "global final" behaviour.

Design record:
[per-transform-linked-and-final.md](../archive/projects/per-transform-linked-and-final.md),
[final-transform-implementation.md](../archive/projects/final-transform-implementation.md),
[post-affines.md](../archive/post-affines.md).

---

## Xaos

`Flame::xaos[src][dst]` multiplies the selection weight of `dst` when
the previous transform was `src`, so
`P(src → dst) ∝ weight[dst] · xaos[src][dst]`. The matrix covers the
normals only; Linked and Final members are outside the chaos game.

- **`has_xaos()`** is true when any entry differs from 1. It compiles
  `XAOS_ENABLED`, and the matrix is uploaded flat, row-major
  (`xaos_flat`). A row whose products are all zero selects transform 0.
- **Matrix upkeep.** `on_transform_added`, `on_transform_cloned` and
  `on_transform_deleted` resize it with the transform list. Adding keeps
  existing xaos-linked pairs intact; deleting clears the matrix if it
  is back to all ones.
- **Closed groups.** Xaos can split the normals into closed groups — sets
  the walk, once inside, never leaves. Under persistent orbits a
  starting thread's first pick is **stratified** across threads (a
  Kronecker sequence in the thread index), so each group gets its share
  of orbits. Where the walk itself, not the first pick, decides the
  group (`xaos_walk_splits_orbits`), every dispatch restarts about one
  thread in `ORBIT_REFRESH_PERIOD = 4` to redraw it.
  See [persistent-orbits.md](../archive/projects/persistent-orbits.md) §3.7.
- **`.flame`**: each `<xform>`'s `chaos` attribute is its row.

**Xaos-linked transforms** are a different thing from the Linked pool,
despite the name. They are an Apophysis convention inferred from the
matrix: `pre` routes only to `post`, and `post` is reachable only from
`pre`. The Xaos editor's Link Transforms dialog applies
`link_transforms_changes`, which returns the xaos edits as one batch
for the ConfigManager; `on_transform_added` uses `detect_linked_pairs`
to keep existing links intact. `detect_linked_chains` and
`unlink_chain_changes` exist, but nothing outside `xaos.rs` calls them
yet. See [linked-transforms.md](../archive/projects/linked-transforms.md)
and [xaos-implementation.md](../archive/xaos/xaos-implementation.md).

---

## Subflames

A **subflame** is a whole `Flame` nested inside a parent and iterated by
a variation: `subflame_wf` (normal phase) or `pre_subflame_wf` (pre
phase), whose `subflame_id` parameter picks the subflame. It is not a
separate render; each call runs the subflame's chaos game one step and
contributes the resulting point (and optionally its colour) to the
parent transform.

- **Limits.** `MAX_SUBFLAMES = 8`, and at most
  `MAX_SUBFLAME_TRANSFORMS_TOTAL = 256` transforms across all of them.
  Only a subflame's normals and Finals are used — its Linked pool,
  attachment lists, xaos, opacity and solo are not — and a subflame
  variation inside a subflame contributes nothing.
- **Its state** lives in the variation's five per-thread state slots,
  keyed by the parent transform and variation: the point, the last
  transform picked, and the colour. So two parent transforms using the
  same subflame walk it independently. The first call on a thread —
  detected by the state still being exactly the origin — runs 20 extra
  steps (`SUBFLAME_PREFUSE_ITERS`) to reach the attractor.
- **One step:** pick a normal by raw weight; blend the colour with the
  `color_speed` rule; affine, variations, post-affine; then the
  subflame's Finals in order; then the direct-colour blend. The point
  after the Finals is what the state keeps, so inside a subflame the
  Finals **do** feed the next step.
- **Variations inside** run through `apply_subflame_variations`, a
  second dispatcher without the subflame variations (which is what
  forbids nesting). Hiding is a no-op there, `NeedsW` does not commit
  the 4th coordinate, and analytic blurs render stochastically.
- **One id space.** Subflame transforms occupy slots
  `[SUBFLAME_XFORM_ID_BASE = 128, …)` in the transforms, parameter and
  state buffers, normals then Finals per subflame. Their variations are
  part of the parent's local index map, so parameters, init dispatch
  and state all work as they do for parent transforms.
- **Editing.** The Subflames panel sets the ConfigManager's
  `EditingTarget`; the Transforms panel and triangle editor then edit
  that subflame in place. The viewport keeps rendering the parent with
  the edits live, unless "View subflame in isolation" is ticked.
- **`.flame`**: JWildfire's `subflame_wf_flame` attribute carries the
  child flame as hex-encoded XML. Import decodes it into
  `Flame::subflames` and points `subflame_id` at it; export re-encodes
  it.

Design record: [subflame-variations-v2.md](../archive/subflame-variations-v2.md),
[subflame-live-editing.md](../archive/subflame-live-editing.md),
[subflames.md](../archive/projects/subflames.md).

---

## Post-symmetry

`Flame::post_symmetry` deposits each sample `K` times — the original
and `K − 1` copies — after the Final chain and before projection. The
dynamics never see the copies.

| `ty` | K | copy |
| --- | --- | --- |
| `None` | 1 | — (`HAS_POST_SYMMETRY` off; the loop compiles out) |
| `XAxis` | 2 | reflected across `y = center_y` |
| `YAxis` | 2 | reflected across `x = center_x` |
| `Point` | `order` (clamped 1–32) | rotated by `k · 2π / order` about the centre |

- In the axis modes the original moves by `+distance/2` and rotates by
  the angle, and the mirror by the opposite. `distance` is therefore
  the total separation, as in JWildfire's `PostAxisSymmetryWFFunc`.
- `rotation_deg` is negated inside the shader so the on-disk value
  means what it means in JWildfire.
- **Axis names are swapped on the wire.** Internally `XAxis` reflects
  across the X axis (flips Y). JWildfire's `X_AXIS` flips X. The XML
  token mapping swaps them, so JWF files round-trip.
- Every copy deposits at full weight, so symmetry multiplies density.
  The palette colour is computed once per sample, not per copy.

---

## `.flame` XML

[src/flame_xml.rs](../../src/flame_xml.rs) reads the Apophysis core,
JWildfire's extensions and `<jwf-flame>` files (only the first
`<layer>`), and writes the Apophysis dialect plus the JWF attributes we
model. JWildfire and Apophysis are the reference: where a flame renders
differently from them, ours is wrong unless the difference is
documented as deliberate.

### Import

**Flame attributes:**

| XML | becomes |
| --- | --- |
| `scale`, `cam_zoom` | `zoom = scale / 200 × cam_zoom` |
| `center` | `pan_x`, `pan_y` |
| `rotate` (degrees) | `rotation` (radians) |
| `cam_pitch`, `cam_yaw` | `camera_rotation_x`, `camera_rotation_y` |
| `cam_roll` | `camera_bank` — JWildfire writes its bank under this name |
| `cam_pos_x/y/z` (`cam_zpos` if `cam_pos_z` is absent) | `camera_x/y/z`, as a world-space position — a deliberate divergence from JWF's camera-space offset |
| `cam_perspective`, `cam_persp` | `perspective_strength` (absolute value) |
| `cam_dof` | `dof_blur_strength` |
| `preserve_z` | `preserve_z` (absent ⇒ `false`) |
| `post_symmetry_*` | `post_symmetry` |
| `soloxform` | `solo_transform` |
| `sld_render_*` | solid rendering settings, where a correspondence exists |

Every imported flame is **3D** (`RenderMode::ThreeD`), deliberately:
Apophysis and JWildfire treat every variation as 3D-capable.

**Transform attributes:**
- `coefs` and `post` in `a c b d e f` order; `post` sets
  `post_affine_enabled`.
- `weight`, `opacity`, `symmetry` / `color_speed` → `color_speed`,
  `pluginColor` → `direct_color`.
- `color`: the palette position, quantised to 1/255 (and only when the
  file has a `<palette>`).
- `yzCoefs`, `zxCoefs`, `yzPost`, `zxPost`; `chaos` (the xaos row).
- **Variations:** any attribute naming a registered variation or alias.
  It is stored under the canonical name — weights for two aliases of
  one variation add — and its position in the attribute list is
  recorded in `variation_order`.
- **Parameters:** `<variation>_<param>`, split at the longest prefix
  that names a variation with that parameter.
- `<variation>_fx_priority`: kept only where it differs from the
  variation's natural phase.
- `subflame_wf_flame`: see *Subflames*.
- `<finalxform>`: same attributes without `weight` and `opacity`; every
  one is attached to every normal.

**Unit conversions** (`variation_param_from_xml` / `_to_xml`):
`radial_blur.angle` is half-turns in the file and degrees here (×180).
The `*plot_wf` family's `preset_id = −1` (a custom formula string we
cannot evaluate) imports as 0.

### Export

`write_flame_xml` writes one `<flame>`:
- The camera and view inverse to the import, with `cam_roll` for bank
  and both `cam_pos_z` and `cam_zpos`.
- `post_symmetry_*`, `preserve_z="1"` when set, `soloxform`.
- Each normal as `<xform>`, its variations in `variation_order` first
  and then sorted, its priority overrides, `coefs`, `post` when enabled,
  non-identity planes, user parameters (converted), and the `chaos` row
  when it is not all ones.
- Each Final as `<finalxform>`.
- `subflame_wf_flame` payloads, and a 256-entry palette.

**What does not survive a `.flame` round trip:**
- The Linked pool, and which normals a Final is attached to (on
  re-import every Final attaches to every normal).
- The render mode: a re-import is always 3D.
- `g` and `post_g`.
- The tone curve (written as identity) and everything that is ours
  alone: effects, Levels, palette squeeze, fog, depth-density
  compensation, far-density fade.

Use `.fflame` for a lossless save.

**Compatibility trackers:**
[jwf-features.md](../projects/jwf-features.md) (non-variation JWildfire
features),
[jwf-common-variations-port.md](../projects/jwf-common-variations-port.md),
[apophysis-remaining-features.md](../archive/projects/apophysis-remaining-features.md).

---

## Where to change what

| task | where |
| --- | --- |
| a transform field | `Transform` and its serde in [transforms.rs](../../src/scene/transforms.rs); `GpuTransform` and the WGSL `Transform` struct ([BUFFERS.md](BUFFERS.md)); a `ConfigPath` ([CONFIG.md](CONFIG.md)); `flame_xml.rs` if it has an XML attribute |
| the iteration | [main_template.wgsl](../../shaders/core/main_template.wgsl); a new compile-time gate goes through `ShaderConstants` ([SHADERS.md](SHADERS.md)) |
| selection | `select_transform_*` in [utilities.wgsl](../../shaders/core/utilities.wgsl) |
| the affine | [affine.wgsl](../../shaders/core/affine.wgsl), [affine_3d.wgsl](../../shaders/core/affine_3d.wgsl) |
| the view mapping | `world_to_pixel`, `project_3d_full` in [utilities.wgsl](../../shaders/core/utilities.wgsl) ([CAMERA.md](CAMERA.md)) |
| xaos | [xaos.rs](../../src/scene/xaos.rs); the Xaos editor in `src/ui/xaos_editor.rs` |
| `.flame` | [flame_xml.rs](../../src/flame_xml.rs) |

## Design record

- [VARIATION-SYSTEM-V2.md](../archive/VARIATION-SYSTEM-V2.md) — named variations
- [transform-color-system-redesign.md](../archive/transform-color/transform-color-system-redesign.md) — colour as a palette position
- [needs-transform-and-direct-color.md](../archive/needs-transform-and-direct-color.md) — direct colour
- [post-affines.md](../archive/post-affines.md)
- [xaos-implementation.md](../archive/xaos/xaos-implementation.md), [linked-transforms.md](../archive/projects/linked-transforms.md)
- [per-transform-linked-and-final.md](../archive/projects/per-transform-linked-and-final.md)
- [subflame-variations-v2.md](../archive/subflame-variations-v2.md), [subflame-live-editing.md](../archive/subflame-live-editing.md)
- [preserve-z-semantics.md](../archive/projects/preserve-z-semantics.md)
- [persistent-orbits.md](../archive/projects/persistent-orbits.md)
- [config-versioning.md](../archive/config-versioning.md)
