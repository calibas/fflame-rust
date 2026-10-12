# GPU Buffers and Layouts

**Overview:** Every buffer, texture, bind group and host-side struct the
flame renderer gives the GPU: what each binding holds, the byte layout
of each struct the Rust and WGSL sides share, and the per-pixel storage
the passes read and write. How the passes use them is in
[RENDERER.md](RENDERER.md).

**See also:**
- [RENDERER.md](RENDERER.md) — the passes, the frame, the counters
- [SHADERS.md](SHADERS.md) — the template flags that decide which bindings a shader declares
- [COLOR.md](COLOR.md) — the histogram's encoding and the accumulator's semantics
- [TONE-MAPPING.md](TONE-MAPPING.md) — what the `TonemapParams` fields mean
- [VARIATIONS.md](VARIATIONS.md) — variation parameters and their registry
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the cylinder table's contents
- [EXPORT.md](EXPORT.md) — the tiled and CPU-histogram export paths

**Code locations:**
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) — `FlameBuffers` and every host struct below
- [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) — `FlamePipelines`, the bind-group layouts, `used_bindings`
- [shaders/core/header.wgsl](../../shaders/core/header.wgsl) — the WGSL side of the compute bindings and structs
- [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl), [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl), [shaders/histogram_blur.wgsl](../../shaders/histogram_blur.wgsl), `shaders/blur_convolve.wgsl`, `shaders/blur_upscale.wgsl` — the other flame passes
- [src/shader_cache.rs](../../src/shader_cache.rs) — the compute layout of the current shader

> **Flame buffers.** The escape-time and simulation engines own their
> own GPU resources and bind groups — the simulation's field is an
> `Rgba32Float` texture array, one slice per layer (see
> [SIMULATION.md](SIMULATION.md)). Both hand their result over as an
> `Rgba32Float` image in the accumulator's layout, which is where this
> document applies to them again. The shade and DoF passes own their
> layouts too (`src/renderer/shade_pass.rs`,
> `src/renderer/dof_pass.rs`).

---

## Layout rules

The code says std140 for uniform buffers and std430 for storage
buffers. In WGSL terms:

- **Scalars** (`f32`, `u32`, `i32`) align to 4. **`vec3` and `vec4`
  align to 16**, in both address spaces.
- **Uniform buffers**: a struct member that is itself a struct or an
  array is aligned to 16, and an array's element stride must be a
  multiple of 16. `GpuParams` therefore keeps `post_symmetry` at byte
  160, and its arrays are `array<vec4<…>, N>`.
- **Storage buffers**: arrays of scalars pack tightly; an array of
  structs has a stride of the struct's size rounded up to its alignment.
- **Every Rust struct the GPU reads** is `#[repr(C)]` and
  `bytemuck::Pod + Zeroable`. Small ones derive both; those with arrays
  longer than bytemuck derives for (`GpuTransform`,
  `GpuVariationParams`, `GpuAttachmentList`, `SubflameMeta`) implement
  them with `unsafe impl`.
- **Sizes are pinned where a drift would be silent.** A const assertion
  holds `TonemapParams` at 144 bytes, and
  `gpu_transform_std430_size_invariant` holds `GpuTransform` at 592. A
  field added on the WGSL side alone is absorbed by the struct's
  trailing padding and validates, while every later field then reads its
  neighbour's bytes — so change both sides together.
- **Uploads** go through `Queue::write_buffer` with
  `bytemuck::cast_slice`; nothing is mapped for writing.

---

## The compute bind group (group 0)

The flame compute shader (`main_template.wgsl` assembled by the shader
builder) declares its bindings in `header.wgsl`. Binding 6 is a
historical gap in the direct-histogram shader.

| binding | WGSL | type | host buffer | real when, else |
| --- | --- | --- | --- | --- |
| 0 | `transforms` | `storage, read` `array<Transform>` | `transform_buffer` | always |
| 1 | `params` | `uniform` `Params` | `params_buffer` | always |
| 2 | `histogram` | `storage, read_write` `array<atomic<u32>>` | `histogram_buffer` | always (`samples` in the sample-emit shader) |
| 3 | `palette_texture` | `texture_2d<f32>` | `palette_view` | always |
| 4 | `palette_sampler` | `sampler` (filtering binding) | `FlameBuffers::sampler` (nearest) | always |
| 5 | `variation_params` | `storage, read` `array<VariationParams>` | `variation_params_buffer` | always |
| 6 | — | — | — | `sample_counter` in the sample-emit shader only |
| 7 | `path_ids` | `storage, read_write` `array<u32>` | `path_buffer` | PathMap colour mode; else a 4-byte dummy |
| 8 | `orbits` | `storage, read_write` `array<u32>` | `orbit_buffer` | `PERSISTENT_ORBITS`; else a 64-byte dummy |
| 9 | `xaos_weights` | `storage, read` `array<f32>` | `xaos_buffer` | the flame has xaos; else a 4-byte dummy |
| 10 | `attachments` | `storage, read` `array<AttachmentList>` | `attachments_buffer` | always |
| 11 | `bias_table` | `storage, read` `array<f32>` | `bias_buffer` | `IMPORTANCE_SAMPLING`; else a 4-byte dummy |
| 12 | `subflame_metadata` | `storage, read` `array<SubflameMeta>` | `subflame_metadata_buffer` | always |
| 13 | `blur_histograms` | `storage, read_write` `array<atomic<u32>>` | `blur_splat_buffer` | analytic blur active; else a 16-byte dummy |
| 14 | `blur_params` | `uniform` `BlurConvolveParams` | `blur_convolve_params_buffer` | always |
| 15 | `cylinders` | `storage, read` `array<f32>` | `cylinder_buffer` | `CYLINDER_TARGETING`; else a 48-byte dummy |
| 16 | `coverage` | `storage, read_write` `array<atomic<u32>>` | `coverage_buffer` (32 bytes) | always allocated; declared under `FRAME_COVERAGE` (auto exposure or the leak probe) |

Bindings 8, 11, 15 and 16 are declared only under their template flags,
which keeps the generated WGSL byte-identical when those features are
off. The others are always declared, and the layout leaves out any the
shader never references (below) — the path ids without
`PATH_TRACKING`, the xaos weights without xaos, the blur bindings
without analytic blur.

### A layout holds only what its shader uses

A browser counts every storage buffer in a bind-group **layout**
against `max_storage_buffers_per_shader_stage`, whether the shader reads
it or not; a fixed layout of all eleven storage buffers exceeded the ten
a laptop's Chrome allows, and no flame rendered there. So:

- `compute_layout_entries` and `compute_entries`
  (`src/gpu/pipelines.rs`) list every possible binding.
- `used_bindings(source)` scans the generated WGSL: a declared
  `@binding(N) var … name` counts if `name` appears again outside
  comments. It can over-count (a name in a function the entry point
  never calls) but never under-count. It is a text scan because the web
  build has no WGSL parser of its own.
- The `ShaderCache` builds each pipeline with a layout of exactly those
  bindings (`compute_layout`, `compute_bindings`), and
  `create_compute_bind_group` filters the entries to match, returning a
  `ComputeBindGroup { group, bindings }`. `compute_pass` rebuilds the
  group whenever its `bindings` differ from the current shader's.
- `create_full_compute_bind_group` builds a layout of every binding, on
  request, for the numerical probe's own shaders.
- `storage_bindings` lists which of a set are storage buffers.

Tests: `a_layout_holds_every_binding_its_shader_uses` holds the scan to
naga's account of the entry point on the committed shader dumps and on
shaders built with each binding-bearing feature; the GPU test
`a_flame_renders_within_a_browsers_storage_limit` (ignored by default)
renders a plain flame on a device limited to 8 storage buffers and a
flame with Focused Rendering, auto exposure and importance sampling on
at 10.

### Other flame bind groups

| group | bindings | built by |
| --- | --- | --- |
| variation init | 0: `variation_params_buffer`, `read_write` | `create_init_bind_group`; layout `ShaderCache::init_bind_group_layout` |
| accumulate | 0: previous accumulator (`texture_2d<f32>`, non-filterable) · 1: histogram (`storage, read`) · 2: output accumulator (`texture_storage_2d<rgba32float, write>`) · 3: `AccumulateParams` (uniform) · 4: `accum_depth_buffer` (`read_write`; the 4-byte xaos dummy when absent) | `create_accumulate_bind_group`, rebuilt after every swap |
| histogram blur H / V | 0: input (`read`) · 1: output (`read_write`) · 2: `HistogramBlurParams` (uniform) | H reads the histogram and writes the scratch; V reads the scratch and writes the histogram. Two params buffers, because queued writes land before any dispatch runs |
| analytic-blur convolve | 0: low-res splat (`read`) · 1: low-res convolved (`read_write`) · 2: `BlurConvolveParams` · 3: kernel weights (`read`) | `create_blur_convolve_bind_group` |
| analytic-blur upscale | 0: low-res convolved (`read`) · 1: histogram (`read_write`) · 2: `BlurConvolveParams` | `create_blur_upscale_bind_group` |
| tonemap | 0: input (`texture_2d<f32>`, non-filterable, read with `textureLoad`) · 1: sampler (non-filtering, unused) · 2: `TonemapParams` (uniform) · 3: curve LUT · 4: curve sampler (linear) · 5: path buffer · 6: palette · 7: sampler | `create_tonemap_bind_group`, or per call in `tonemap_pass_with_input`. The WGSL no longer declares 5–7; the layout still carries them |

The tonemap pipeline renders into `Rgba8Unorm` with no blending.

---

## Structures

### `GpuTransform` (592 bytes)

One per transform slot of binding 0, mirrored by `Transform` in
`header.wgsl`.

| bytes | fields |
| --- | --- |
| 0–31 | `a, b, c, d, e, f` (pre-affine), `g` (z offset), `weight` |
| 32–63 | `post_a … post_f`, `post_g`, `post_enabled` (1.0 when the transform has any post step) |
| 64–463 | `variations: [f32; 100]` — weights indexed by the flame's local variation index |
| 464–479 | `color` (palette position, 0–1), `color_speed`, `opacity`, `direct_color` |
| 480–575 | `yz_coefs`, `zx_coefs`, `yz_post_coefs`, `zx_post_coefs` — JWildfire plane affines, `[f32; 6]` each |
| 576–591 | `plane_flags` (`PLANE_FLAG_YZ`, `_ZX`, `_YZ_POST`, `_ZX_POST`: which planes are not identity), `analytic_blur_slot` (`i32`, −1 = none), `analytic_blur_strength`, `analytic_blur_residual` |

The colour is a palette index, not RGB: a point's colour coordinate
moves toward `color` at a rate set by `color_speed`, and the palette is
looked up at plot time (see [COLOR.md](COLOR.md)). `opacity` is the
probability a point is plotted. Solo mode writes 0 for the opacity of
every normal transform but the solo one.

**The transform buffer** is `MAX_TRANSFORMS_UNIFIED = 384` slots
(227,328 bytes), indexed by a global `xform_id`
(`pack_gpu_transforms`):

```
[0, N)                       normal transforms
[N, N+L)                     linked transforms
[N+L, N+L+F)                 final transforms        N+L+F ≤ MAX_TRANSFORMS = 128
… zero to 128
[128, 128 + S)               subflame transforms, per subflame normals then finals
… zero to 384                S ≤ MAX_SUBFLAME_TRANSFORMS_TOTAL = 256
```

A flame over 128 parent slots panics in `update_transforms`; subflames
over their total fail validation.

### `GpuParams` (304 bytes, uniform)

One per dispatch at binding 1, mirrored by `Params` in `header.wgsl`.
Written whole by `compute_pass` every dispatch and by `load_config`,
`update_flame`, `update_iterations` and `set_color_mode`.

| bytes | fields |
| --- | --- |
| 0–31 | `num_transforms`, `iterations_per_thread`, `burn_in`, `width`, `height`, `seed`, `color_mode` (0 palette, 1 speed, 2 PathMap), `render_mode` |
| 32–71 | `splat_size`, `zoom`, `pan_x`, `pan_y`, `rotation`, `speed_factor`, `perspective_strength`, `depth_density_compensation`, `far_density_fade`, `far_density_fade_start` |
| 72–95 | `camera_rotation_x` (pitch), `camera_rotation_y` (yaw), `camera_bank`, `camera_x`, `camera_y`, `camera_z` |
| 96–111 | `dof_focus_distance`, `dof_blur_strength`, `fog_strength`, `fog_start` |
| 112–135 | `path_map_style`, `path_origin: [f32; 3]` (PathMap's Origin frame: centre and radius), `_pad_path_filters: [u32; 2]` |
| 136–159 | `background_r`, `background_g`, `background_b`, `solid_strength`, `surface_thickness`, `depth_prime` |
| 160–191 | `post_symmetry: GpuPostSymmetry` |
| 192–223 | `shadow_center_x/y/z`, `shadow_radius`, `shadow_count`, `importance_window`, `orbit_generation`, `orbit_refresh_period` |
| 224–287 | `shadow_dirs: [[f32; 4]; 4]` — world-space direction to each light, `w` = enabled |
| 288–303 | `leak_probe: [f32; 4]` — `[cx, cy, r, _]`, off when `r ≤ 0` |

**The pad before `post_symmetry`.** `post_symmetry` is a struct, so in
a uniform buffer it must start on a 16-byte boundary: byte 160. The 40
words before it are the 37 original scalars and the three solid fields
(`solid_strength`, `surface_thickness`, `depth_prime`), which took the
12 bytes of padding that once sat there. `_pad_path_filters` are two of
the 37: the Path Editor's filter fields, kept as padding when word
editing replaced it. Removing any field before byte 160, on either
side, moves `post_symmetry` off its boundary. Later fields were carved
from shadow padding (`importance_window`, `orbit_generation`,
`orbit_refresh_period`) or appended after a 16-aligned field
(`leak_probe`) so no offset moved.

`splat_size` (always 1.0) and `render_mode` are uploaded but read by no
shader — the render mode is compiled in (`RENDER_3D`). A config in
escape or simulation mode writes `render_mode` 0.

### `GpuPostSymmetry` (32 bytes)

`kind` (0 none, 1 x-axis, 2 y-axis, 3 point), `order` (clamped to
1–32), `center_x`, `center_y`, `distance`, `rotation` (radians,
converted from the config's degrees), two pads. Built from
`scene::transforms::PostSymmetry` with `From`. Read only under
`HAS_POST_SYMMETRY`.

### Variation parameters

`GpuVariationParams { params: [f32; 1600] }` — 6,400 bytes per
`xform_id`, `MAX_TRANSFORMS_UNIFIED` of them (2,457,600 bytes), at
binding 5, laid out in the same slot order as the transform buffer
(`pack_gpu_variation_params`).

- **Packed per flame.** Each active variation gets
  `parameters.len() + init_param_count` consecutive slots
  (`VariationDef::slot_count`) at an offset from
  `scene::transforms::compute_packed_layout`, walking the flame's
  local index map in order. The offsets are the same for every
  transform; a transform without a variation leaves its run at zero.
  `MAX_VARIATION_PARAM_SLOTS = 1600` caps the sum over the flame's
  active variations, and `update_variation_params` logs an error past it.
- **The shader reads** through a generated `get_param(xform_id,
  variation_id, slot)`: a `switch` over the local index baked with each
  offset, returning `variation_params[xform_id].params[offset + slot]`
  (`build_packed_get_param` in `src/shader_builder_v2.rs`).
- **Init-derived slots** follow a variation's user parameters. The
  host writes them as zero; the variation init pass (its own pipeline
  and bind group, `read_write` on this buffer) computes them whenever
  parameters were written (`init_dirty`).
- **Per-thread variation state is not here.** It lives in a
  `var<private> thread_state` array behind generated `get_state` /
  `set_state` accessors, and persists between dispatches in the orbit
  buffer.

Design record: [packed-variation-params.md](../archive/packed-variation-params.md),
[variation-init-dispatch.md](../archive/variation-init-dispatch.md),
[intra-iteration-state-and-accum.md](../archive/intra-iteration-state-and-accum.md).

### Attachments: `GpuAttachmentList`

Binding 10 holds one entry per normal transform: up to `cap` linked and
`cap` final transforms, as global `xform_id`s, each list followed by its
count. The WGSL struct is `{ linked: array<u32, CAP>, linked_count,
final_: array<u32, CAP>, final_count }` with `CAP` substituted from
`Flame::attachment_cap()`, so its stride is `(cap × 4 + 4) × 2` bytes
(`attachment_stride_bytes`). The buffer is allocated at the worst case,
`MAX_TRANSFORMS × 264` bytes (cap `MAX_ATTACHMENTS_PER_TRANSFORM = 32`),
and `update_attachments` packs at the flame's stride with
`pack_attachment_entry`, dropping out-of-pool indices. Design record:
[per-transform-linked-and-final.md](../archive/projects/per-transform-linked-and-final.md).

### `SubflameMeta` (32 bytes)

Binding 12, `MAX_SUBFLAMES = 8` entries indexed by a `subflame_wf`
variation's `subflame_id`: `normals_offset`, `normals_count`,
`finals_offset`, `finals_count`, a reserved word (once the render mode),
`xform_id_base` (128 — the subflame transforms' start in the unified
buffers), two pads. Built by `build_subflame_metas`. Design record:
[subflame-variations-v2.md](../archive/subflame-variations-v2.md).

### `TonemapParams` (144 bytes, uniform)

Binding 2 of the tonemap group, mirrored in `shaders/tonemap.wgsl`. What
each field does is in [TONE-MAPPING.md](TONE-MAPPING.md).

| bytes | fields |
| --- | --- |
| 0–15 | `exposure`, `gamma`, `density_scale`, `tonemap_mode` (0 linear, 1 logarithmic, 2 density visualisation) |
| 16–31 | `background_color: [f32; 3]` (a `vec3`, so 16-aligned), `_pad_bg` |
| 32–63 | `use_curve`, `vibrancy`, `brightness`, `white_level`, `prefilter_white`, `bright_adjust`, `area`, `sample_density` |
| 64–95 | `saturation`, `hue_shift`, `gamma_threshold`, `alpha_blend_low`, `alpha_blend_high`, `transparent_mode` (0–3), `color_mode`, `width` |
| 96–127 | `height`, `path_map_style`, `burn_in`, `num_transforms`, `palette_size`, `levels_low`, `levels_high`, `levels_gamma` |
| 128–143 | `highlight_mode`, `levels_enabled`, `levels_density`, `_pad_levels` |

`width` and `height` are the frame's, which the shader turns into the
texel it loads. `path_map_style`, `burn_in` and `num_transforms` are
read by nothing now and are kept so the fields after them stay put.
`sample_density` and `levels_density` are rewritten at their offsets
before every tone map (`refresh_sample_density`; see
[RENDERER.md](RENDERER.md)).

### `AccumulateParams` (48 bytes, uniform)

`width`, `height`, `blend_factor`, `use_fixed_ema` (0 cumulative mean,
1 fixed EMA), `background_r/g/b` (unused by the shader), `_pad1`,
`surface_thickness`, `has_depth` (1 when the histogram carries the
depth region and `accum_depth_buffer` exists), `_pad2: [u32; 2]`. A
`blend_factor ≥ 0.99` is the overwrite signal.

### `HistogramBlurParams` (32 bytes) and `BlurConvolveParams` (96 bytes)

- `HistogramBlurParams`: `width`, `height`, `radius_pixels` (the
  Gaussian sigma), `direction` (0 horizontal, 1 vertical — fixed per
  buffer at construction), `density_sigma` (the bilateral term), three
  pads.
- `BlurConvolveParams`: `full_width`, `full_height`, `lowres_width`,
  `lowres_height`, `downscale` (D), `count` (slices actually
  allocated), `frame_seed` (byte 24, rewritten each frame for the
  upscale's dither), a pad, then `meta: [[u32; 4]; MAX_BLUR_BUFFERS]` —
  per slot, the kernel's low-res half-extent and its first weight in
  the kernel-weights buffer.

---

## Per-pixel storage

### The histogram

`histogram_buffer`, `u32` words, row-major. Its regions, in order:

```
word 0                          RGBD: 4 words a pixel, W×H×4 words
  ─ with the solid depth region ─
W×H×4                           depth: 1 word a pixel (inverted ordered-float, 0 = none)
W×H×5                           8-word tail: bounding box (words 0–5), occlusion survival (6–7)
W×H×5 + 8                       4 shadow maps, 1024 × 1024 words each
  ─ or, with the census ─
W×H×4                           census::TOTAL_WORDS counters (about 126 KB)
```

- **RGBD.** `(r·s, g·s, b·s, s)` per plot, `s = 100 × weight`, added
  with `atomicAdd`; colour is recovered as `Σcolour / Σdensity`. A
  batch is far from `u32` overflow. See [COLOR.md](COLOR.md) and
  [histogram/README.md](../archive/histogram/README.md).
- **The solid depth region** exists while solid rendering or lighting
  is on in 3D (`set_solid_depth_region`). Turning it on or off
  recreates the buffer, and every bind group that holds the histogram
  (compute, accumulate, histogram blur H and V) is recreated with it.
  Its encoding makes larger values nearer, so `atomicMax` keeps the
  nearest depth and a zero clear is a reset.
- **The 8-word tail**: the world bounding box as ordered-float
  `atomicMax` pairs (max and negated min per axis, from 1 thread in 16)
  and the occlusion-survival sums (1 thread in 1024). Read back 32
  bytes at a time by `BoundsTracker` (`src/renderer/density_stats.rs`).
- **Shadow maps**: four `1024²` light-space depth maps
  (`FlameBuffers::SHADOW_MAP_RES`, 16 MiB), ordered-float `atomicMax`
  of the distance toward each light; `shadow_map_word_offset()` gives
  their start for the shade pass.
- **The census tail** is mutually exclusive with the depth region and
  survives every clear; it is renderer state for `src/census/`, never a
  render.

**Clears.** `clear_histogram` (a batch's first dispatch) clears RGBD
only while the depth region or census tail must survive the batch
boundary, and the whole buffer otherwise — including in overwrite mode,
where depth resets every batch. It also clears the coverage counters
and the low-res blur splat buffer. `clear_all` (a reset) clears both
accumulators and the whole histogram except a census tail.

`histogram_buffer_scratch` is always RGBD-sized: the spatial filter's
horizontal pass writes it and its vertical pass reads it.

### The accumulator pair

`accumulation_texture_a` and `_b`, `Rgba32Float`, the frame's size:
`rgb` the density-weighted mean colour, `a` the raw cumulative hit
count. `current_is_a` says which is current: the accumulate pass reads
the current (`previous_accumulation_view`) and writes the other
(`output_accumulation_view`), then `swap_textures` makes it current; the
tone map and the shade pass read `current_accumulation_view`. `f32`
rather than `f16` because each batch's contribution to a long render
falls below `f16`'s resolution. Both are cleared with `clear_texture`
on the desktop and with an empty render pass on the web.

`accum_depth_buffer`, one `u32` a pixel, exists with the depth region:
the nearest depth the accumulated colour belongs to, for the
accumulate pass's depth-tightening reset.

### The output, palette and curve

- **`fractal_texture`** (owned by `FlameRenderer`): `Rgba8Unorm`, the
  frame's size, `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING` — the
  tone map's target, egui's image, and the readback source.
- **Palette textures**: `PALETTE_FORMAT = Rgba16Float`, 2D, `palette_size
  × 1` with `palette_size` clamped to `256–4096`
  (`DEFAULT_PALETTE_SIZE`, `MAX_PALETTE_SIZE`). `update_palette` writes
  the smooth table and a stepped one (every stop a flat band, for the
  escape engine's stepped palettes) through the same rotation and
  squeeze (`render_palette_lookup`). Half floats keep dark entries'
  hues that 8 bits truncated. `resize_palette` recreates both.
- **Curve LUT**: `Rgba16Float`, 256 × 1, sampled linearly.

### Feature buffers

| buffer | binding | layout | lifetime |
| --- | --- | --- | --- |
| `orbit_buffer` | 8 | `16 + ORBIT_SLOTS` `u32` a thread: `0–3` point (x, y, z, w), `4–7` speed colour and colour index, `8` generation, `9` burn-in left, `10` xaos previous transform, `11–12` importance window and weight, `13–14` analytic-blur residual, `15` unused, `16…` variation state; floats via `bitcast` | made zeroed (generation 0, which no dispatch carries) when a shader first uses it, for at least `128 × 64` threads; grown by `ensure_orbit_capacity`, which loses the orbits |
| `path_buffer` | 7 | one `u32` a pixel: the 1-based plan word a pixel was last drawn through, 0 = none | PathMap colour mode, in a flame mode |
| `xaos_buffer` | 9 | `N × N` `f32`, `xaos_weights[src × N + dst]` | the flame has xaos; recreated when the transform count changes |
| `bias_buffer` | 11 | `N` biased weights, then the `N × N` likelihood-ratio matrix (`scene::importance::build_table`) | importance sampling on |
| `cylinder_buffer` | 15 | the packed plan: composed words of 12 floats, or the replay layout with an 8-float header (`scene::cylinder::pack`, `pack_words`) | a plan is drawn; reused while large enough, so the table carries its own word count |
| `coverage_buffer` | 16 | `[0]` plot attempts in frame, `[1]` attempts, `[2]` attempts outside the leak probe's disc | always (32 bytes, the size `BoundsTracker` reads); cleared with the histogram |
| `blur_splat_buffer`, `blur_convolved_buffer` | 13 / convolve stage | per slot, `ceil(W/D) × ceil(H/D)` pixels of RGBD `u32`, slots concatenated | analytic blur active; reallocated when D or the slot count changes; capped so both fit one binding |
| `blur_kernel_weights_buffer` | convolve stage | the slots' kernels, concatenated | always, at the worst case: `(2 × 64 + 1)²` floats a slot × 4 slots |

The cylinder table's contents are in
[FOCUSED-RENDERING.md](FOCUSED-RENDERING.md); the orbit buffer's
semantics in [RENDERER.md](RENDERER.md);
[analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md)
has the blur's.

---

## Sample-emit output (tiled exports)

Above one storage-buffer binding the export uses `HighResExporter`
(`src/export/high_res.rs`), whose shader is built with
`OUTPUT_HISTOGRAM_DIRECT` off. Then binding 2 is a sample stream and
binding 6 its write cursor:

```wgsl
struct Sample {          // 32 bytes
    x: f32, y: f32,      // pixel coordinates
    r: f32, g: f32, b: f32,
    weight: f32,         // the density weight
    _pad2: f32,          // camera-space depth under SOLID (the Rust side calls it `depth`)
    _pad3: f32,
}
struct SampleCounter { count: atomic<u32> }
```

The shader appends one `Sample` per plot while there is room; a scatter
pass (`shaders/core/accumulate_samples.wgsl`, host side
`src/export/accumulate.rs`) adds them into per-tile histograms of the
same RGBD layout, with a fifth depth word under solid rendering, or the
CPU accumulates them in `f64`. The exporter builds its compute layout
with the same `used_bindings` rule. See [EXPORT.md](EXPORT.md) and
[unified-render-pipeline.md](../archive/projects/unified-render-pipeline.md).

---

## Sizes

Fixed buffers, allocated by `FlameBuffers::with_palette_size`:

| buffer | bytes |
| --- | --- |
| transforms | 227,328 (384 × 592) |
| variation parameters | 2,457,600 (384 × 6,400) |
| attachments | 33,792 (128 × 264) |
| subflame metadata | 256 (8 × 32) |
| `GpuParams` / `TonemapParams` / `AccumulateParams` | 304 / 144 / 48 |
| histogram-blur params, ×2 | 32 each |
| `BlurConvolveParams` | 96 |
| analytic-blur kernel weights | 266,256 (16,641 floats × 4 slots) |
| palette, smooth and stepped | `palette_size × 8` each |
| curve LUT | 2,048 |
| coverage counters | 32 |

Per pixel, at the size the buffers were built for:

| resource | bytes a pixel |
| --- | --- |
| histogram (RGBD) | 16 (20 with the depth region, plus 16 MiB of shadow maps and 32 bytes) |
| scratch histogram | 16 |
| accumulator pair | 32 |
| accumulator depth | 4, with the depth region |
| path ids | 4, PathMap only |
| `fractal_texture` | 4 |

At 1920 × 1080 the always-present 64 bytes a pixel are about 127 MiB.
The shade and DoF passes add 64 bytes a pixel each when they first run
([RENDERER.md](RENDERER.md), *GPU memory*).

### A renderer for the tail

`FlameRenderer::for_tail` builds `FlameBuffers` at 1 × 1: the histogram,
its scratch and the accumulator pair are a single pixel each, since an
escape or simulation picture is never accumulated. The uniform buffers,
palette, curve LUT and transform buffers are full-size as always, and
`fractal_texture` and the `width`/`height` the tone map indexes with
are the frame's.

---

## Who writes what

| data | written by | when |
| --- | --- | --- |
| `GpuParams` | `compute_pass` (whole struct) | every dispatch; also `load_config`, `update_flame`, `update_iterations`, `set_color_mode` |
| transforms, variation parameters, attachments, subflame metadata | `update_transforms`, `update_variation_params`, `update_attachments`, `update_subflames` | `load_config`, `update_flame`, construction, `resize`; parameters alone by `set_variation_params` |
| init-derived parameter slots | the variation init pass | after any parameter write, at the next `compute_pass` (or `run_init_pass`) |
| xaos, bias, cylinder tables | `update_xaos`, `update_bias`, `update_cylinders` | flame loads and edits; plans as they land |
| palette textures | `update_palette` | palette edits, `load_config`, after a resize |
| curve LUT | `update_curve_lut` | curve edits, `load_config`, after a resize |
| `TonemapParams` | `update_tonemap`, `set_transparent_mode`, `update_density_scale`, `update_background_color` (whole struct); `refresh_sample_density` (two fields) | every app frame; every tone map |
| `AccumulateParams` | `accumulate_pass` | every accumulate |
| histogram-blur params | `update_histogram_blur_params` | every accumulate with the filter on |
| `BlurConvolveParams`, kernel weights | `maybe_rebuild_blur_kernels`; `frame_seed` by `accumulate_pass` | when the view or flame changes; every accumulate |
| histogram, orbits, coverage, path ids, blur splat | the compute shader | every dispatch |

`FlameBuffers::destroy` releases the buffers and textures explicitly
(on WebGPU a dropped buffer waits for the JS garbage collector), and
`free_iteration_buffers` releases only the large per-pixel ones once a
render's tone map has run.

A buffer or texture replaced mid-update is **retired**, not dropped:
the histogram when solid or the census changes its size, the palette
textures, path ids, xaos, the analytic-blur pair, and a growing orbit,
cylinder or bias table. It cannot be destroyed at once, because an
encoder not yet submitted may still use it, and a submit that uses a
destroyed buffer fails validation. Every caller submits a
`compute_pass`'s encoder before recording the next one, so each
`compute_pass` starts by destroying what was retired before the
previous one (`release_retired`); `destroy` takes the rest
(`a_replaced_buffer_is_destroyed_once_no_encoder_can_use_it`).
`resize` replaces the whole set and destroys the old one at once, which
is safe only because the app gives it an encoder of its own.

---

## Common tasks

| task | files |
| --- | --- |
| Add a field to `GpuParams` | `GpuParams` in `buffers.rs` and `Params` in `header.wgsl` together; every `GpuParams { … }` literal (`compute_kernel.rs`, `buffers.rs`, `export/high_res.rs`); keep `post_symmetry` at byte 160 — carve from padding or append after a 16-aligned field |
| Add a field to `GpuTransform` | the struct, `Transform` in `header.wgsl`, `GpuTransform::from_transform_with_opacity`; update the 592-byte test, keep the size a multiple of 16 |
| Add a `TonemapParams` field | both sides, the size assertion, every whole-struct writer in `compute_kernel.rs` |
| Add a compute binding | `compute_layout_entries` and `compute_entries` in `pipelines.rs`, the declaration in `header.wgsl` behind its template flag, a dummy in `FlameBuffers` if the buffer is optional; check the storage count in `a_layout_holds_every_binding_its_shader_uses` |
| Change the histogram layout | `buffers.rs` (sizes, clears, regions), `main_template.wgsl`, `accumulate.wgsl`, `accumulate_samples.wgsl`, `export::histogram_size_bytes` |

---

## Design record

- [per-transform-linked-and-final.md](../archive/projects/per-transform-linked-and-final.md) — the unified transform buffer and attachment lists
- [packed-variation-params.md](../archive/packed-variation-params.md) — the packed parameter layout
- [accumulator-unification.md](../archive/projects/accumulator-unification.md) — `Rgba32Float` and the accumulator's units
- [solid-rendering.md](../archive/projects/solid-rendering.md) — the depth region, tail and shadow maps
- [persistent-orbits.md](../archive/projects/persistent-orbits.md) — the orbit buffer
- [analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md) — the low-res blur buffers
- [unified-render-pipeline.md](../archive/projects/unified-render-pipeline.md) — direct histogram and sample emit
- [histogram/README.md](../archive/histogram/README.md) — how the `u32` RGBD histogram was arrived at
