# Flame Shaders

**Overview:** The flame engine's WGSL and how it is built. A flame's
compute shader is never a file on disk: it is assembled per flame from
templates in `shaders/core/` and the inline WGSL of the variations the
flame actually uses, specialised by template flags, then compiled,
cached and bound with a layout of exactly the bindings it names.

> **One of three assemblers.** This document covers the flame's
> `shader_builder_v2`. The escape engine and the simulation engine each
> have their own, on the same principle: a template with markers, and
> only the active definitions spliced in (see
> [Other assemblers](#other-assemblers)). All three are held to the same
> `shader_lint` rules.

**See also:**
- [RENDERER.md](RENDERER.md) — the render pipeline these shaders run in
- [BUFFERS.md](BUFFERS.md) — the bindings' buffers and the Rust structs
  that mirror `header.wgsl`
- [VARIATIONS.md](VARIATIONS.md) — `VariationDef`, features, parameters
- [TRANSFORMS.md](TRANSFORMS.md) — the flame algorithm the kernel runs
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the deep-zoom planner
  whose shaders are summarised [below](#deep-zoom-planner-shaders)
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md) — the other
  two assemblers
- [EFFECTS.md](EFFECTS.md) — the post-effect shaders

**Code locations:**
- [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) —
  `ShaderBuilder`, `ShaderConstants`, `TemplateProcessor`
- [src/shader_cache.rs](../../src/shader_cache.rs) — `ShaderCache`, the
  pipeline LRU, the init pipeline
- [src/renderer/sticky.rs](../../src/renderer/sticky.rs) — the sticky
  variation superset
- [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) — `FlamePipelines`,
  `used_bindings`, the compute layout
- [src/shader_dumps.rs](../../src/shader_dumps.rs) — canonical WGSL dumps
- [src/probe/](../../src/probe/), [src/census/](../../src/census/),
  [src/bin/variation_probe.rs](../../src/bin/variation_probe.rs) — the
  variation math probe and the reachability census

---

## The shader files

### The flame kernel's parts (`shaders/core/`)

| file | what it holds | spliced in |
| --- | --- | --- |
| `header.wgsl` | `Transform`, `Params`, `PostSymmetry`, `VariationParams`, `AttachmentList`, `Sample`/`SampleCounter`, `SubflameMeta`, `BlurConvolveParams`, every group-0 binding, and the flag-gated helpers that live beside their bindings (`bias_*`, `ct_*`, `pathmap_origin`, `shadow_map_splat`) | always |
| `rng.wgsl` | `RngState`, `pcg_hash`, `rng_init`, `rng_next`, `rng_nextf` | always |
| `affine.wgsl` / `affine_3d.wgsl` | `apply_affine`, `apply_post_affine` (3D also `rotate_x`, `rotate_y` and the JWF plane affines) | one, by `RENDER_3D` |
| `complex.wgsl` | complex arithmetic and the 2×2 complex matrix `CMat2` | always (main and init shader) |
| `utilities.wgsl` | transform selection, palette and speed colour, the camera and projection, `world_to_pixel`, `post_symmetry_copy`, `ff_atan2` | always |
| `main_template.wgsl` | the entry point `main` (and `probe_main` under `PROBE`) | always, last |
| `subflame.wgsl` | `subflame_iterate` | `subflame_wf` or `pre_subflame_wf` active |
| `fractwf.wgsl` | the shared escape-time loop of the `fract_*_wf` family | any `fract_*_wf` active |
| `noise.wgsl` | simplex and Perlin noise tables (from JWildfire's `NoiseTools.java`) | `dc_perlin`, `crackle`, `dc_crackle_wf`, or whenever `voronoi.wgsl` is |
| `voronoi.wgsl` | Voronoi cells for the crackle family | `crackle`, `dc_crackle_wf`, `crackle_fast` |
| `polyhedra.wgsl` | polyhedron radial-support tables | `polyhedron`, `polyhedron_volume` |
| `polychora.wgsl` | regular 4-polytope vertex tables, the Menger tesseract set | `polychoron`, `menger` |
| `su_mobius.wgsl` | SL(2,C) Möbius-group helpers and SU(n) tables | any variation with `Feature::NeedsMobiusLib` |
| `replay.wgsl` | `ct_apply_symbol`, `ct_apply_finals`, the forced-arm hooks | `cylinder_replay` |
| `replay_delta.wgsl` | the forward-difference forms the offset replay runs (`ct_offsets`, `ct_fwd_diff`) | `cylinder_replay` and `cylinder_offsets` |
| `accumulate_samples.wgsl` | the scatter pass for the sample-emit export path | not spliced; its own pipeline |
| `plan_eval.wgsl`, `plan_gather.wgsl`, `plan_offsets.wgsl` | the deep-zoom planner's GPU kernels | not part of a render; see [below](#deep-zoom-planner-shaders) |

Variation WGSL is not in a shader file. Each variation carries its 2D
and 3D bodies inline in its `VariationDef` in `src/variations/defs/*.rs`;
647 are registered (`ALL_VARIATIONS` in
[src/variations/defs/mod.rs](../../src/variations/defs/mod.rs), with
the `engine-flame` feature; without it, `linear` alone).

### The other passes (`shaders/`)

Hand-written shaders with fixed layouts. None is assembled from the
flame's variation set.

| file | pass | created by |
| --- | --- | --- |
| `accumulate.wgsl` | histogram → `Rgba32Float` accumulator | `FlamePipelines` ([src/gpu/pipelines.rs](../../src/gpu/pipelines.rs)) |
| `histogram_blur.wgsl` | separable Gaussian on the batch histogram (Apophysis `filter`), two dispatches when the radius is above 0 | `FlamePipelines` |
| `blur_convolve.wgsl`, `blur_upscale.wgsl` | analytic blur, stages 2 and 3: low-resolution convolution, then B-spline upscale-and-add into the histogram | `FlamePipelines` |
| `tonemap.wgsl` | the display tone map; a render pipeline (`vs_main`/`fs_main`), not compute | `FlamePipelines`, and the high-res exporter ([src/export/high_res.rs](../../src/export/high_res.rs)) |
| `normals.wgsl`, `atrous.wgsl`, `shade.wgsl` | solid rendering: normal estimation, à-trous smoothing, deferred shading | [src/renderer/shade_pass.rs](../../src/renderer/shade_pass.rs) |
| `dof.wgsl` | solid rendering's post-process depth of field (`coc_main`, `max_main`, `dof_main`) | [src/renderer/dof_pass.rs](../../src/renderer/dof_pass.rs) |
| `core/accumulate_samples.wgsl` | scatters the sample stream into a histogram region | [src/export/accumulate.rs](../../src/export/accumulate.rs) |
| `effects/color/*`, `effects/density/*`, `effects/common/blend_modes.wgsl` | the post-effect chain; an effect that needs blend modes carries the line `// INCLUDE_BLEND_MODES`, which is replaced with `blend_modes.wgsl` | [src/effects/mod.rs](../../src/effects/mod.rs) — see [EFFECTS.md](EFFECTS.md) |

`downsample.wgsl` and `accumulate_tiled.wgsl` are still in the folder,
but no Rust code includes either.

The hard-coded histogram colour scale, `100.0`, appears as
`HISTOGRAM_COLOR_SCALE` in `accumulate.wgsl` and as `color_scale` at
the deposit in `main_template.wgsl`. The two must match.

---

## How a flame's shader is assembled

### The builder's entry points

`ShaderBuilder::new(registry)` takes a clone of the variation registry.
Everything else is a pure function of the flame, the flags and a
`ShaderConstants`: the builder never sees the platform or the backend.

| method | returns | called by |
| --- | --- | --- |
| `build_definitions(flame, render_3d, path_tracking, xaos, output_histogram_direct, &constants)` | `Definitions { source, processor, active, has_dc, has_rgb }`: everything before the kernel | `build_from_template`, `build_layer_map_at`, `build_plan_eval` |
| `build_from_template(flame, _active, render_3d, path_tracking, xaos, output_histogram_direct, &constants)` | the complete flame kernel: the definitions plus the processed `main_template.wgsl` | `ShaderCache`, the high-res exporter, the probe, the shader dumps, `variations::bound` tests |
| `build_init_shader(flame, _active)` | `Option<String>`: the init kernel, or `None` when no active variation has `wgsl_init` | `ShaderCache`, the high-res exporter, the planner |
| `build_layer_map(flame)` / `build_layer_map_at(flame, group)` | the definitions plus `flame_map(xform_id, u, seed)`, every binding moved to `group` and `params` renamed `flame_params` | the simulation (group 1), the escape lens (group 2, `LENS_GROUP`) |
| `build_plan_eval(flame)` | the definitions, built with the replay on, plus `plan_eval.wgsl` | [src/scene/plan_gpu.rs](../../src/scene/plan_gpu.rs) |

The `_active` map that `build_from_template` and `build_init_shader`
take is unused: the active set and its indices come from the flame.

### The active set and the local index map

There are no fixed variation indices. Each flame gets a **local index
map**, `compute_local_index_map(flame.active_variation_names_ordered(..))`
in [src/scene/transforms.rs](../../src/scene/transforms.rs): names are
numbered `0..N` in the order they first appear walking the normal
transforms, then linked, then finals, then subflames, each transform in
its own `variation_order`. More than `MAX_VARIATIONS_PER_FLAME` (100)
are truncated with a warning.

The same map is used by the GPU buffer packer, the init shader, the
state layout and the dispatcher, so slot `idx` of
`Transform.variations` (an `array<f32, 100>`) and the `variation_id`
passed to `get_param` always agree. The map's order is semantics, not
bookkeeping: it fixes the dispatch order of `Replace`, `NeedsAccum` and
chained pre/post variations (see the sticky superset's
[fallback rule](#the-sticky-variation-superset)).

`active_with_local_indices` pairs each active name with its index,
iterating the registry's order so the emitted bodies are deterministic.
In 2D it drops only `VariationCategory::Only3D`; a `Full3D` variation
with a real 2D body is kept.

### Assembly order

`build_definitions` concatenates, in this order:

1. `ShaderConstants::to_wgsl()`: `NUM_TRANSFORMS`, `COLOR_MODE`,
   `HAS_POST_AFFINE`, `USE_INLINED_WEIGHTS`, `USE_INLINED_TRANSFORMS`,
   and the inlined tables when there are any
   ([inlined constants](#inlined-constants-cli-export)).
2. `header.wgsl`, with `{{ATTACHMENT_CAP}}` replaced by
   `constants.attachment_cap`, then run through the template processor.
3. `rng.wgsl`.
4. `affine_3d.wgsl` or `affine.wgsl`.
5. The active variations' bodies (`generate_variation_code`).
6. The census helpers, when `constants.census` is set.
7. `apply_variations`.
8. When a subflame variation is active, `apply_subflame_variations`
   and the processed `subflame.wgsl`.
9. The helper libraries: `fractwf`, `noise`, `voronoi`, `polyhedra`,
   `polychora`, `su_mobius`, each only when the table above says so.
10. The per-flame `get_param`.
11. The per-thread state: `thread_state`, `get_state`, `set_state`;
    `const ORBIT_SLOTS` when orbits persist. `HAS_THREAD_STATE` is set
    here, after the header was processed, because only the template
    reads it.
12. `point_w` and its summing globals (`HAS_W`), the volume
    side-emission globals (`HAS_VOLUME_SIDE`), the multi-emit
    collector (`HAS_PLOT_EMIT`).
13. `complex.wgsl`.
14. `utilities.wgsl`, through the processor.
15. `replay.wgsl` (processed) and `replay_delta.wgsl`, under the
    replay flags.

`build_from_template` then appends `main_template.wgsl`, processed with
the same `TemplateProcessor` so a flag cannot pick one side in the
header and the other in the template, and replaces the line
`//__STATE_INIT_BLOCK__` with the generated state-init block.

WGSL resolves module-scope names regardless of order, which is why the
header can declare `ORBIT_WORDS = 16u + ORBIT_SLOTS` before step 11
defines `ORBIT_SLOTS`.

### Variation bodies

`generate_variation_code` emits each active variation's source:

- **2D or 3D body.** `wgsl_source` in 2D, `wgsl_source_3d` in 3D. A
  variation with no 3D body (only possible for a downloaded one) is
  dropped from a 3D flame with a `log::warn!`, never silently replaced
  by its 2D body.
- **Per-flame specialisation.** `synth` and `mondrianomies` re-emit a
  body containing only the modes or tables the flame uses
  (`specialize_wgsl_2d`/`_3d`). `ShaderCache::compute_specialization_key`
  makes a change to those inputs force a rebuild.
- **Forced arms.** Under a replay build (`constants.cylinder_replay`),
  `variations::bound::force_arms` wraps a many-valued variation's
  random draw so it reads `ct_forced_arm`. Any other build emits the
  body untouched.
- **Helper deduplication.** Bodies are split into top-level blocks by
  `split_wgsl_top_level_fns`: a `fn ` or `const ` at column 0, a `fn`
  ending at its matching brace (line comments skipped), a `const` at
  its first `;`. A block whose name was already emitted with
  byte-identical text is skipped; a conflicting body panics, naming
  both variations. Module-scope `struct` and `var` declarations inside
  a variation body are not supported.

### `apply_variations`

The dispatcher is generated (`build_apply_variations_2d` /
`build_apply_variations_3d`). Its signature, where `T` is `vec2<f32>` or
`vec3<f32>`:

```wgsl
fn apply_variations(xform: Transform, xform_id: u32, p: T,
                    rng: ptr<function, RngState>
                    [, vc: ptr<function, f32>]                   // HAS_DC
                    [, vrc: ptr<function, vec3<f32>>]            // HAS_RGB
                    , hide: ptr<function, bool>
                    [, blur_contribution: ptr<function, T>])     // HAS_ANALYTIC_BLUR
                    -> T
```

The call sites in `main_template.wgsl` (the selected transform, the
linked chain, the final chain, `probe_main`) and in `replay.wgsl` and
`flame_map` pass exactly these, which is why the template spells out
each `HAS_DC` × `HAS_RGB` combination as its own call.

`resolve_phase_buckets` sorts the active variations into three phases,
the Apophysis order (XForm.pas):

| phase | emitted as | order |
| --- | --- | --- |
| Pre | `temp = variation_<name>(temp, …)`: the body applies its own weight | local index |
| Normal | `result += w * f(temp, …)`, or `result = w * f(…)` for `Feature::Replace` | local index, `Replace` variations last |
| Post | `result = f(result, …)`: the body applies its own weight | local index |

Every emission is gated on the variation's weight being non-zero. A
`VariationPhase::Any` variation lands in normal unless a transform's
`fx_priority` override moves it (negative: pre; positive: post). When
it spans several phases across transforms each placement carries an
`xform_id` gate, and normal is the catch-all so finals and linked
transforms still run it. A moved variation applies its weight in the
emission. The overrides come from `collect_phase_overrides` and are
part of `ShaderConstants`, so editing one rebuilds the shader; see
[jwf-features.md](../projects/jwf-features.md).

Arguments after the point follow the variation's declaration:

| condition | argument |
| --- | --- |
| `Feature::NeedsAccum` (normal and post phases) | `result`, the running sum |
| has parameters, or `Feature::NeedsTransform` | `xform_id, <local index>u` |
| `Feature::NeedsRng` | `rng` |
| `Feature::WritesColor` | `vc` |
| `Feature::WritesRgb` | `vrc` (normal phase, and the 2D pre phase) |
| `Feature::CanHide` | `hide` |

Further rules in the generated code:

- **Z gating (3D).** A normal-phase contribution has its z zeroed when
  the variation is `Feature::NeverZ`, or when
  `constants.flatten_z_per_iter` (3D with `preserve_z` false) is set
  and the variation is not `Feature::AlwaysZ`. See
  [preserve-z-semantics.md](../archive/projects/preserve-z-semantics.md).
- **The fourth coordinate (3D).** `Feature::NeedsW` variations write
  `point_w_out`; the normal phase sums it into `point_w_acc` with the
  same weight as xyz and commits it to `point_w` if any wrote.
- **Analytic blur.** With `HAS_ANALYTIC_BLUR`, a `Feature::AnalyticBlur`
  variation also stores `w * offset` in `*blur_contribution`, so the
  plot can recover the deterministic mean.
- **Inlined weights.** When `constants.inlined_transforms` is set, the
  normal phase reads `get_inlined_var_weight(xform_id, idx)` instead of
  `xform.variations[idx]`.
- **Census.** With `constants.census`, the dispatcher counts input and
  output classes (`census_sel`, `census_in2/3`, `census_out2/3`,
  `census_pp2/3`). Off, nothing is emitted.

`apply_subflame_variations` is the same function without
`subflame_wf`/`pre_subflame_wf` (breaking the call cycle), without the
`hide` parameter, inlining, analytic blur, census or w-summing, for
`subflame.wgsl` to call. Subflame transforms use synthetic ids from
`SUBFLAME_XFORM_ID_BASE` (128).

### `get_param` and the packed parameter buffer

`VariationParams` is one flat `array<f32, 1600>` per transform
(`MAX_VARIATION_PARAM_SLOTS`). `compute_packed_layout` gives each active
variation a contiguous run of `slot_count()` slots (its user parameters,
then its `init_param_count` derived slots), in local-index order, at the
same offset in every transform's block. `build_packed_get_param` bakes
those offsets into the shader:

```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    var offset: u32 = 0u;
    switch (variation_id) {
        case 0u: { offset = 0u; }  // linear: 0 slots
        case 1u: { offset = 0u; }  // julian: 3 slots
        ...
    }
    return variation_params[xform_id].params[offset + param_slot];
}
```

So a reordering of the active set moves offsets, and the shader cache
compares the variation order, not just the set, before deciding
nothing changed.

### The template processor

`TemplateProcessor` understands one construct:

```
{{#if FLAG}} ... {{/if}}
{{#if FLAG}} ... {{else}} ... {{/if}}
```

Blocks nest and may sit inside a line (the template's
`{{#if HAS_ANALYTIC_BLUR}}, &blur_contribution{{/if}}` is a trailing
argument). A flag nobody set reads as false. Any other `{{…}}` tag is
passed through unchanged, which is why `{{ATTACHMENT_CAP}}` is replaced
before processing. There is no `unless`; the negative form is
`{{#if FLAG}}{{else}}…{{/if}}`.

### Template flags

Set in `build_definitions` (and `HAS_THREAD_STATE` after the state
block is built). Each is processed in `header.wgsl`,
`main_template.wgsl`, `utilities.wgsl`, `subflame.wgsl` or
`replay.wgsl`.

| flag | set from | gates |
| --- | --- | --- |
| `RENDER_3D` | the `render_3d` argument: the render mode, except that the high-res exporter builds the 3D kernel for a 2D flame too unless analytic blur, targeting or a `NeedsW` variation forces 2D | `vec3` vs `vec2` points, the 3D projection, the z rail |
| `OUTPUT_HISTOGRAM_DIRECT` | the `output_histogram_direct` argument | binding 2 is the atomic `histogram` (true) or the `samples` stream with `sample_counter` at 6 (false) |
| `PATH_TRACKING` | the `path_tracking` argument (PathMap) | `ct_word`, the `path_ids` write, `pathmap_origin` |
| `XAOS_ENABLED` | the `xaos` argument (`flame.has_xaos()`) | `prev_xform_idx` and `select_transform_xaos` |
| `HAS_DC` | an active `Feature::WritesColor` variation | the `vc` register and the three-step colour flow |
| `HAS_RGB` | an active `Feature::WritesRgb` variation | the `vrc` register and the plot-time RGB blend |
| `HAS_W` | an active `Feature::NeedsW` variation, 3D only | `point_w`, its seeding, rail and respawn reset |
| `HAS_VOLUME_SIDE` | an active `Feature::VolumeSideEmit` variation | the volume-only side point |
| `HAS_PLOT_EMIT` | an active `Feature::PlotEmits(n)` variation (cap is the max `n`, at most 16) | the multi-emit source loop |
| `HAS_ATTACHMENTS` | `constants.has_attachments` | the `attachments` load, the linked and final chains |
| `HAS_POST_SYMMETRY` | `constants.has_post_symmetry` | the post-symmetry copy loop |
| `HAS_ANALYTIC_BLUR` | `constants.has_analytic_blur` | mean-splat routing into `blur_histograms` |
| `HAS_THREAD_STATE` | an active variation declares state (`state_count > 0`) | the state words of a persistent orbit |
| `PERSISTENT_ORBITS` | `constants.persistent_orbits` | the `orbits` binding (8), resume and write-back |
| `IMPORTANCE_SAMPLING` | `constants.importance_sampling` | `bias_table` (11), biased selection, the likelihood-ratio window |
| `CYLINDER_TARGETING` | `constants.cylinder_targeting` | `cylinders` (15), the word draw, the forced prefix |
| `CYLINDER_REPLAY` | `constants.cylinder_replay` | the replay arm instead of the composed one |
| `CYLINDER_RELATIVE` | `constants.cylinder_relative` | view-relative plot; `world_to_pixel` skips the pan |
| `CYLINDER_OFFSETS` | `cylinder_offsets && cylinder_replay` | the replay's steps in offsets from reference orbits |
| `FRAME_COVERAGE` | `constants.frame_coverage` | the `coverage` counters (16) |
| `SOLID` | `solid_enabled && render_3d` | nearest-depth `atomicMax`, occlusion, shadow-map splats |
| `PROBE` | `constants.probe` | the `probe_main` entry point |
| `FLATTEN_Z_PER_ITER` | always `false` | an end-of-iteration z flatten kept in the template but never enabled; per-variation z gating replaced it |

`HAS_POST_AFFINE` is not a processor flag. It is the WGSL `const` from
`to_wgsl`, tested as `if (HAS_POST_AFFINE)`, which the compiler can
fold. The one `{{#if HAS_POST_AFFINE}}` block in
`main_template.wgsl` (post affine on the volume side point) is
therefore always stripped.

### `ShaderConstants`

Apart from the variation set and the builder's four boolean arguments,
everything compiled into a flame's shader rather than read from a buffer
is a field of `ShaderConstants`. It derives `PartialEq`, and the shader
cache rebuilds when it changes.

| field | becomes |
| --- | --- |
| `num_transforms` | `const NUM_TRANSFORMS` (the selection loops' bound) |
| `color_mode` | `const COLOR_MODE` (0 Palette, 1 Speed, 2 PathMap) |
| `has_post_affine` | `const HAS_POST_AFFINE` |
| `has_attachments`, `has_post_symmetry`, `has_analytic_blur` | the flags of the same names |
| `attachment_cap` | `{{ATTACHMENT_CAP}}`: the length of `AttachmentList`'s arrays, and the host's packing stride |
| `flatten_z_per_iter` | per-variation z gating in the 3D dispatcher |
| `solid_enabled`, `importance_sampling`, `frame_coverage`, `persistent_orbits`, `cylinder_targeting`, `cylinder_replay`, `cylinder_relative`, `cylinder_offsets`, `probe` | the flags above |
| `census` | the dispatcher's census instrumentation |
| `variation_priorities` | `fx_priority` placements of `Any` variations |
| `inlined_transforms`, `cumulative_weights` | the inlined tables (CLI export only) |

`ShaderCache::constants_from_config` derives everything a config
determines. What depends on the view or on the renderer is set after:
`FlamePipelines::ensure_shaders_current_with_config` takes the census,
targeting arm (`cylinder_*`), leak probe (which turns on
`frame_coverage`) and `persistent_orbits` as arguments, because a
`.fflame` cannot ask for them.

### Off means byte-identical

Every optional feature is required to leave the WGSL untouched when it
is off, so a feature costs nothing and moves no pixel for flames that do
not use it. Tests hold this per feature: `solid_off_is_byte_identical`,
`importance_sampling_off_is_byte_identical`,
`cylinder_targeting_off_is_byte_identical`,
`frame_coverage_off_is_byte_identical` and
`census_off_no_trace_census_on_validates` in
[src/shader_builder_v2.rs](../../src/shader_builder_v2.rs),
`probe_off_is_byte_identical` in
[src/probe/shader.rs](../../src/probe/shader.rs) (it splits on the
`// PROBE-BLOCK-BEGIN` marker, which must not be reworded), and the
[canonical dumps](#canonical-shader-dumps) for everything at once.

---

## The kernel

### `header.wgsl`

The structs mirror Rust structs in
[src/gpu/buffers.rs](../../src/gpu/buffers.rs) (`GpuTransform`,
`GpuParams`, `GpuPostSymmetry`, `GpuVariationParams`,
`GpuAttachmentList`, `SubflameMeta`, `BlurConvolveParams`). A field
added to one must be added to the other, padding included: `Params`
keeps `post_symmetry` on a 16-byte boundary with explicit pad fields.
[BUFFERS.md](BUFFERS.md) has the layouts.

`Transform` is flat: `a`–`f`, `g` (z offset), `weight`, the post affine
`post_a`–`post_g` and `post_enabled`, `variations: array<f32, 100>`,
`color`, `color_speed`, `opacity`, `direct_color`, the JWildfire plane
affines (`yz_coefs`, `zx_coefs` and their post forms) with
`plane_flags`, and the analytic-blur routing fields.

The group-0 bindings:

| binding | name | declared |
| --- | --- | --- |
| 0 | `transforms` | always |
| 1 | `params` (uniform) | always |
| 2 | `histogram` (`array<atomic<u32>>`) or `samples` | by `OUTPUT_HISTOGRAM_DIRECT` |
| 3, 4 | `palette_texture` (`texture_2d<f32>`), `palette_sampler` | always |
| 5 | `variation_params` | always |
| 6 | `sample_counter` | sample-emit only |
| 7 | `path_ids` | always |
| 8 | `orbits` | `PERSISTENT_ORBITS` |
| 9 | `xaos_weights` | always |
| 10 | `attachments` | always |
| 11 | `bias_table` | `IMPORTANCE_SAMPLING` |
| 12 | `subflame_metadata` | always |
| 13 | `blur_histograms` | always |
| 14 | `blur_params` (uniform) | always |
| 15 | `cylinders` | `CYLINDER_TARGETING` |
| 16 | `coverage` | `FRAME_COVERAGE` |

Declared is not bound: the pipeline's layout holds only the bindings
the shader actually names ([below](#per-shader-bind-group-layouts)).

### `rng.wgsl`

PCG, 32-bit state. `pcg_hash(x)` is the stateless permutation;
`rng_init(thread_id, seed)` seeds a stream with
`pcg_hash(thread_id ^ seed)`; `rng_next` steps it; `rng_nextf` returns
the top 24 bits as an f32 in `[0, 1)`, never exactly 1.0.

`main` seeds its stream with `rng_init(thread_id, params.seed)`.
Features that need randomness of their own take a separate stream from
the same thread id and a constant-xored seed (the word draw:
`seed ^ 0x85EBCA6B`; the importance-sampling deposit:
`seed ^ 0x9E3779B9`), so switching them on does not shift the chaos
game's own sequence.

### `utilities.wgsl`

- **Transform selection.** `select_transform_const` (cumulative weights
  over `NUM_TRANSFORMS`), `select_transform_xaos` (weights times the
  `xaos_weights` row of the previous transform), and under
  `IMPORTANCE_SAMPLING` the biased forms.
- **2D view.** `world_to_pixel` composes pan → rotate → zoom, then maps
  to pixels at `min(width, height) * 0.25` pixels per unit about the
  image centre. Under `CYLINDER_RELATIVE` the point arrives already
  relative to the view centre and the pan is not subtracted again.
- **3D view.** `project_3d_full` builds the camera matrix with
  `build_camera_matrix(yaw, pitch, bank, roll)`, applies it to
  `p - camera_pos` (`camera_transform`), projects with
  `apply_perspective` (Apophysis `zr = 1 - persp * z`, clipping
  `zr < 1e-3`), then pans, rotates by `params.rotation` (the screen-plane
  roll) and zooms exactly as `world_to_pixel` does. It returns the pixel
  and the camera-space point every depth consumer reads. The call
  passes `(0, -pitch, -bank, yaw)` into the matrix's slots: JWildfire
  applies its matrix transposed, and the comments there explain the
  mapping. Read them before touching it.
- **Colour.** `srgb_to_linear`, `speed_to_color`.
- **`post_symmetry_copy`.** The k-th copy for the axis and point
  symmetries.
- **`ff_atan2`.** See [Metal fast-math](#metal-fast-math).

### `main_template.wgsl`

One entry point, `@compute @workgroup_size(64, 1, 1) fn main`. One
thread is one orbit, `thread_id = global_id.x`. In order:

1. **A fresh start.** A random point in `[-1, 1]²` (or `³`), white
   speed colour, `color_index = 0`, `fuse = params.burn_in`, each
   feature's registers, and under xaos an unconditioned first pick. Then
   the state-init block spliced at `//__STATE_INIT_BLOCK__`.
2. **Resume** (`PERSISTENT_ORBITS`). A thread whose stored generation
   word equals `params.orbit_generation`, and which this dispatch's
   refresh does not select, restores its point, `point_w`, colours,
   fuse, previous transform, importance window, analytic-blur residual
   and variation state. Under xaos a starting thread's first pick is
   stratified across threads instead. The layout is in `header.wgsl`:
   `ORBIT_WORDS = 16 + ORBIT_SLOTS` u32 a thread. See
   [persistent-orbits.md](../archive/projects/persistent-orbits.md).
3. **The loop**, `params.iterations_per_thread` times:
   - select a transform; test its opacity (stochastic transparency: it
     decides the plot, not the dynamics);
   - affine, `apply_variations`, post affine; then the linked chain
     (`HAS_ATTACHMENTS`);
   - if a `CanHide` variation hid the point, revert to the previous
     point and skip the plot;
   - **bad-value recovery**: an x or y outside `±1e32` (tested as
     `!(abs(x) <= 1e32)`) respawns the point and resets the fuse; in 3D,
     z saturates at `±1e32` (non-finite z becomes 0) and a railed point
     respawns with probability 1/256 per iteration;
   - the colour update (palette, speed, or the `HAS_DC` three-step flow);
   - while the fuse runs, count it down; otherwise **plot**: the forced
     prefix under targeting, the final chain, post-symmetry copies, the
     multi-emit sources, projection, depth effects, and the deposit,
     `atomicAdd` of four u32 (R, G, B, density, scaled by 100) into the
     histogram or one `Sample` into the stream. A targeted sample's
     detour is undone after the plot so the free orbit continues.
4. **Write-back** of the orbit (`PERSISTENT_ORBITS`) and one flush of
   the frame-coverage tallies per thread (`FRAME_COVERAGE`).
5. **`probe_main`** (`PROBE` only): evaluates `apply_variations` at the
   probe's input points with no affine, reading and writing through the
   histogram binding.

[RENDERER.md](RENDERER.md) covers what the pipeline does with the
histogram and the sample stream.

---

## Per-variation init and state

### The init shader

A variation can declare derived parameters, values computed once from
its user parameters (`wgsl_init`, `init_param_count`).
`build_init_shader` emits a separate kernel:

- binding 0, `variation_params` as `read_write`, and `complex.wgsl`;
- each init-bearing variation's `init_<name>(user: array<f32, N>)`,
  once;
- `@compute @workgroup_size(64) fn init_main`, a `switch` with one
  `case` per (transform, variation) pair: read the user slots, call the
  init, write the result into the slots after them.

Pairs cover every transform the main shader reads, in global id order:
normals, linked, finals, then subflame transforms from
`SUBFLAME_XFORM_ID_BASE`. The init source is part of the pipeline
cache's key, and `ShaderCache` compares an init signature (the
`(xform id, variation)` pairs) so that a structural edit which shifts
ids, adding a linked transform for instance, rebuilds it.

`FlameRenderer` runs the init pass, `ceil(pairs / 64)` workgroups,
before the main dispatch whenever parameters have changed (`init_dirty`
in [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs));
`run_init_pass` runs it standalone for callers that dispatch their own
entry point, such as the probe.

### Variation state

A variation can declare `state_count` per-thread mutable slots.
`build_state_accessors` emits:

- `var<private> thread_state: array<f32, N>`, zero-initialised by the
  WGSL spec;
- `get_state(xform_id, variation_id, slot)` and `set_state(…)`, a
  `switch` on `xform_id * 100 + variation_id` with each pair's offset
  baked in.

Slots go only to non-zero-weight pairs (`compute_state_layout`), but
the accessors are emitted whenever any active variation declares state,
because its body is compiled in even at weight 0.

`wgsl_state_init` is a fragment of statements, not a function. The
builder wraps one copy per pair in a block with `xform_id` and
`variation_id` as constants and splices them at
`//__STATE_INIT_BLOCK__`, where they may draw from `rng`. With
persistent orbits the state is saved in the orbit and restored on
resume, so the fragment runs for a starting thread only.

Design records: [variation-init-dispatch.md](../archive/variation-init-dispatch.md),
[packed-variation-params.md](../archive/packed-variation-params.md),
[intra-iteration-state-and-accum.md](../archive/intra-iteration-state-and-accum.md).

---

## Caching and compilation

### The shader cache

`ShaderCache` ([src/shader_cache.rs](../../src/shader_cache.rs)), owned
by `FlamePipelines`, holds the current pipeline. `ensure_current_full`
returns early unless one of these changed:

- the active variation **set**, or its **order**;
- path tracking, xaos, the render mode;
- `ShaderConstants`;
- the registry version (a variation fetched at run time);
- the specialisation key (`synth`, `mondrianomies`);
- the init signature.

Otherwise it builds the main and init WGSL and looks the pair up in an
LRU of `PIPELINE_CACHE_CAP` (8) compiled pipelines. The key is a hash of
both sources, confirmed by comparing the full text, so a collision is a
miss rather than a wrong pipeline. A hit installs the cached pipeline,
layout and init pipeline with no compile; a miss compiles and pushes to
the front. Only the active render mode's shader is built; the 2D and 3D
slots hold the same pipeline. `rebuild_stats()` reports (rebuilds,
hits, compile ms).

Before compiling, on the desktop, `validate_wgsl` parses the source with
naga. A parse failure is logged with the generated source's error and,
when downloaded variations are registered, their names, rather than
arriving as wgpu's panic. The web build has no naga; the browser
validates.

### The sticky variation superset

Compiling is the expensive part of switching flames: a batch of random
flames each with its own variation set would compile a shader per
flame. `StickyVariations`
([src/renderer/sticky.rs](../../src/renderer/sticky.rs)) is renderer
state, never serialised:

- It remembers which variations recent flames used and compiles each
  flame against a **canonical map**: the sorted union of the flame's
  variations and the retained extras, up to `STICKY_CAP` (32) and
  within the hard caps of 100 variations and 1600 parameter slots.
- The extras ride at weight 0, so they are branch-skipped; the map
  depends only on the set, so flames drawing from one pool converge on
  one pipeline.
- `adopt` returns the flame rewritten onto the map: every map entry
  transform 0 lacks is added to it at weight 0, and its
  `variation_order` becomes the map, which pins the local index map.
  Extras are evicted by how recently they were live.

**The fallback.** Reordering is only safe where order is not semantics,
so a flame is compiled specialised (unaugmented) when any transform
(normal, linked, final, or in a subflame) has an `fx_priority`
override, two or more pre or two or more post variations, two or more
`Replace`, `WritesColor` or `WritesRgb` variations, or a `NeedsAccum`
variation beside another normal one.

**Who must stay wired.** Every path that compiles or packs from a
caller's flame goes through it, or the compiled map and the packed
offsets disagree (a single lit pixel): `load_config` and `update_flame`
call `adopt`; `update_path_features`, `resize` and
`set_variation_params` call `augmented`. One-shot paths opt out with
`set_sticky_enabled(false)`: the unified `render()`, the animation
exporter, the probe and the census.

Design record:
[sticky-shader-compilation.md](../archive/projects/sticky-shader-compilation.md)
(layer A is the pipeline LRU, layer B the superset).

### Per-shader bind group layouts

A browser counts every storage buffer in a layout against
`max_storage_buffers_per_shader_stage`, used or not, and some allow
only 8 or 10. So the flame compute layout holds only the bindings the
generated shader uses:

- `used_bindings(source)` in
  [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) scans the WGSL
  text (comments stripped) for each `@binding(N) var … name` whose
  `name` occurs again. It is a text scan because the web build has no
  WGSL parser. It may include a binding named only in an uncalled
  function; it never leaves one out.
- `compute_layout(device, Some(&bindings))` filters
  `compute_layout_entries()` to those; `storage_bindings` counts the
  storage buffers among them. `ShaderCache` logs an error when the
  count exceeds the device's limit.
- The renderer rebuilds its bind group whenever a rebuilt shader's
  bindings differ.

A new binding goes in `compute_layout_entries` and `compute_entries`,
and costs a storage buffer only where a shader names it. Two tests:
`a_layout_holds_every_binding_its_shader_uses` holds the scan to naga's
own account of `main` over the canonical dumps and the in-app shader
with each binding-bearing feature on; `a_flame_renders_within_a_browsers_storage_limit`
(GPU, `#[ignore]`) renders a plain flame on a device limited to 8
storage buffers, and one with Focused Rendering, auto exposure and
importance sampling on at 10.

### Inlined constants (CLI export)

`export_mode` and `export_animation_mode` in
[src/lib.rs](../../src/lib.rs) call `enable_inlined_constants()`. From
then on `constants_from_config` uses
`ShaderConstants::with_inlined_transforms`, and every flame edit
recompiles. What the generated shader reads from the inlined data is
`get_inlined_var_weight`, in the normal phase of the dispatcher. The
other tables `to_wgsl` emits (`INLINED_AFFINE`, `CUMULATIVE_WEIGHTS`,
`get_inlined_var_param`) are not referenced by any shader code.

### Debug dumps

`FractalArtEditor export … --dump-shader` writes the generated kernel
to `debug_shader_2d.wgsl` or `debug_shader_3d.wgsl` in the working
directory, and the high-res exporter's to `debug_shader_export.wgsl`.

---

## Keeping the WGSL honest

### Canonical shader dumps

A template or variation edit shows in review as a source diff, never as
the WGSL it produces.
[src/shader_dumps.rs](../../src/shader_dumps.rs) closes that gap:
`canonical_shader_dumps` builds eight specs and compares them, line
endings normalised, with the golden files in `tests/shader_dumps/`:

| file | covers |
| --- | --- |
| `in-app-2d.wgsl`, `in-app-3d.wgsl` | the direct-histogram path |
| `in-app-3d-solid.wgsl` | `SOLID` |
| `in-app-2d-multi-emit.wgsl` | `HAS_PLOT_EMIT` (`spray_blur`) |
| `in-app-2d-path-xaos.wgsl` | `PATH_TRACKING` and `XAOS_ENABLED` |
| `export-2d.wgsl`, `export-3d.wgsl`, `export-2d-multi-emit.wgsl` | the sample-emit path |

All are built with persistent orbits on. After an intended change,
regenerate and review the diff with the source change:

```bash
UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps
```

There is one set for every platform because codegen has no platform
forks; `codegen_has_no_platform_forks` fails if
`shader_builder_v2.rs`, `main_template.wgsl` or `header.wgsl` gains a
`target_arch` conditional.

### WGSL lints

`shader_lint` in [src/variations/mod.rs](../../src/variations/mod.rs)
(test-only) scans WGSL with comments stripped:

| lint | rejects | why |
| --- | --- | --- |
| `subnormal_literals` | a nonzero float literal below `f32::MIN_POSITIVE`, such as `1e-40` | GPUs flush subnormals to zero, so a guard built on one is `max(x, 0.0)`; it arrives by copying an f64 epsilon from JWildfire (see `defs/exp_log.rs`) |
| `self_operations` | `x != x`, `x == x`, `x / x`, `x /= x` on one bare operand | they only work under IEEE rules Metal's fast-math does not keep |

They run over every registered variation's 2D, 3D, init and state-init
source and every `.wgsl` under `shaders/`. The escape assembler
([src/escape/assembler.rs](../../src/escape/assembler.rs)), its terrain
shaders, the simulation assembler
([src/sim/assembler.rs](../../src/sim/assembler.rs)) and the effects
([src/effects/mod.rs](../../src/effects/mod.rs)) call the same two
functions on their assembled output.

### Cross-platform rules

#### Metal fast-math

Metal compiles with fast-math on (wgpu never clears
`fastMathEnabled`), so IEEE rules about NaN and infinity do not hold
there. A bad value can pass a guard and survive as a plausible finite
number.

- **Do not detect non-finite values with a self-compare** or by
  expecting `Inf` arithmetic to poison a result. Write
  `!(abs(x) <= 1e32)`, the bad-value test `main_template.wgsl` uses: NaN
  fails the comparison and the negation fires.
- **Do not divide a value by itself.** `x / x` is 1.0 except at `0/0`
  and `Inf/Inf`, and fast-math folds those to 1.0 too. Write the
  intended value and reproduce the reference's zero case (`cut_btree` in
  `defs/cut_simple.rs`). The lint enforces both rules.
- **Use `ff_atan2` where `(0, 0)` is reachable.** Metal's `atan2` gives
  π/4 for same-sign zero pairs and NaN for mixed-sign ones.
  `ff_atan2` in `utilities.wgsl` reproduces IEEE exactly,
  `atan2(±0, +0) = ±0` and `atan2(±0, -0) = ±π`, reading signs through
  `bitcast` because `-0.0 < 0.0` is false and integer operations are
  immune to fast-math. Returning 0 for all four instead moves the
  render on Vulkan. It is not substituted globally: it costs a
  comparison per call, and only call sites that can reach an exact zero
  pair need it.
- **Do not guard trigonometry of huge arguments.** Past about 2²⁴,
  adjacent f32 values differ by more than a period, and every
  implementation, the f64 reference included, gives a different answer;
  there is no correct class to guard toward. `docs/accepted-divergences.txt` records this and the other
  investigated divergences. Do flag a port whose reachable inputs feed
  trig unbounded arguments.

A fix belongs in the shader, and should be bit-identical under IEEE so
platforms that never needed it are unaffected.

#### Texture sampling on the web

`textureSample` is only valid in uniform control flow. Desktop drivers
are lenient; a browser's WebGPU enforces the rule. Sample
unconditionally before branching, or use `textureLoad`, as
`tonemap.wgsl` does (both). The flame compute shader samples its palette with
`textureSampleLevel`, which has no such restriction.

### The variation math probe

[src/probe/](../../src/probe/) evaluates every variation's
`apply_variations` arithmetic directly, through `probe_main`, at a fixed
adversarial grid of inputs (the four signed zeros, the smallest normal,
a subnormal, values near and past the `1e32` threshold) in 2D and 3D.
Each output is reduced to a **class** (finite, NaN or infinite, its
sign, a magnitude bucket) plus a value quantised well above ulp noise:
a vendor's rounding cannot change a class, and π/4 is still told from
zero. A sweep then moves each parameter one at a time: both arms of
every boolean, every enum choice, numeric extremes.

```bash
cargo run --release --bin variation_probe                          # writes docs/generated/variation-probe.txt and -sweep.txt
cargo run --release --bin variation_probe -- --no-sweep
cargo run --release --bin variation_probe -- compare OLD NEW
cargo run --release --bin variation_probe -- lens [grid] [out]     # every variation as a screen lens
```

`compare` exits non-zero on a class difference or a variation present
in only one report; magnitude-only differences are printed but do not
fail. Design record:
[variation-math-probe.md](../archive/projects/variation-math-probe.md).

### The reachability census

The probe says what *can* diverge; [src/census/](../../src/census/)
counts which input classes real renders actually deliver to each
variation. With `constants.census`, the generated dispatcher classifies
each transform's normal-phase input, each pre/post variation's chained
input and every output by bit pattern, and counts into a tail of the
histogram buffer. It needs the direct-histogram path, skips solid
flames, and does not instrument subflame dispatchers.

```bash
cargo run --release --bin variation_probe -- census <flame.fflame> [iterations]
cargo run --release --bin variation_probe -- census --corpus [iterations] [seeds]
cargo run --release --bin variation_probe -- rank <old-probe-report> <new-probe-report> [census-report]
cargo run --release --bin census_dumpseed -- <seed> > seedN.fflame
```

`--corpus` writes `docs/generated/variation-census-<os>.txt`, one file
per platform because a census row measures that GPU. `rank` joins a
probe comparison with the census into a worklist, minus the
dispositions in `docs/accepted-divergences.txt`. `census_dumpseed`
prints the random flame a report's `random:N` names. Design record:
[variation-reachability-census.md](../archive/projects/variation-reachability-census.md);
its usage lines predate these subcommands.

### Trust the compiler

Modern shader compilers do common-subexpression elimination.
Precalculating shared trigonometric values by hand measured about 1%
slower, not faster, so write clear WGSL and do not hand-hoist:
[SHADER_COMPILER_CSE_ANALYSIS.md](../archive/optimization-attempt-2025-11-02/SHADER_COMPILER_CSE_ANALYSIS.md).
The `const` values `to_wgsl` emits (`NUM_TRANSFORMS`, `COLOR_MODE`,
`HAS_POST_AFFINE`) rely on the same thing: they exist so the compiler
can fold the dead branch, rather than the template carrying a flag for
each.

---

## Deep-zoom planner shaders

Focused Rendering's planner asks the GPU which words send which sample
points into the view, using the render's own maps.
[src/scene/plan_gpu.rs](../../src/scene/plan_gpu.rs) builds them:

| shader | built as | entry points |
| --- | --- | --- |
| `plan_eval.wgsl` | `build_plan_eval(flame)`: the flame's definitions with the replay flag on, then this file | `plan_eval`, `plan_eval_gathered` |
| `plan_gather.wgsl` | standalone | `gather_ranges`, `gather_scan`, `gather_fill` |
| `plan_offsets.wgsl` | `build_plan_eval` + `replay_delta.wgsl` + this file, its own bindings at group 2 | `plan_offsets` |

`replay.wgsl`'s `ct_apply_symbol` is the single definition of applying
one symbol of a word, called by both the render's replay arm and
`plan_eval`, so a plan and the picture cannot disagree.
`replay_delta.wgsl` is the shader copy of `scene::forward_delta`,
held to it by `the_shader_forward_forms_are_the_cpu_ones`. See
[FOCUSED-RENDERING.md](FOCUSED-RENDERING.md),
[gpu-cylinder-planning.md](../projects/deep-zoom/gpu-cylinder-planning.md)
and [deep-zoom-precision.md](../projects/deep-zoom/deep-zoom-precision.md).

---

## Other assemblers

**Escape-time.** [src/escape/assembler.rs](../../src/escape/assembler.rs)
splices one formula and one colouring into a fixed template at
whole-line markers (`//__FORMULA__`, `//__COLORING__`,
`//__ESCAPE_TEST__`, …), and splices feature-gated code (the bailout
test, the orbit accumulator) in or out at assembly time. A pipeline writes an `Rgba32Float` image in the flame
accumulator's layout for the shared tail. The escape lens
([src/escape/lens.rs](../../src/escape/lens.rs)) borrows the flame
builder's `build_layer_map_at` at group 2. See [ESCAPE.md](ESCAPE.md)
and [TERRAIN.md](TERRAIN.md).

**Simulation.** [src/sim/assembler.rs](../../src/sim/assembler.rs)
splices one model and one colouring into the seed, step and colour
templates (and the warp, agent, pyramid, reduction and jump-flood
passes),
with fixed function names (`sim_step`, `sim_seed`, `sim_color`), so
there is no index map. The boundary rule is spliced into `sim_read`
rather than branched on. With `use_transforms`, the flame builder's
`build_layer_map` supplies `flame_map` at group 1. See
[SIMULATION.md](SIMULATION.md).

---

## Common changes

| change | where |
| --- | --- |
| Add a variation | its `defs/*.rs` file and the end of `ALL_VARIATIONS`; see [VARIATIONS.md](VARIATIONS.md). Regenerate the dumps only if a dumped shader changes. |
| A variation needs a shared helper library | the file in `shaders/core/`, and its condition in `build_definitions`; a new `Feature` (as `NeedsMobiusLib`) is better than another hard-coded name list |
| Add a template flag | `processor.set` in `build_definitions`; its source in `ShaderConstants` if a config or the renderer decides it, so the cache rebuilds; an off-is-byte-identical test |
| Add a binding | `header.wgsl` (gated if optional), `compute_layout_entries` and `compute_entries` in [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs), the buffer in [BUFFERS.md](BUFFERS.md)'s terms |
| Add a `Params` field | `header.wgsl` and `GpuParams` in [src/gpu/buffers.rs](../../src/gpu/buffers.rs), keeping the 16-byte boundaries |
| Change the plot or the iteration | `main_template.wgsl`, then `UPDATE_SHADER_DUMPS=1` and read the diff |
| Change the view mapping | `world_to_pixel` and `project_3d_full` in `utilities.wgsl`, together |
| Change the tone map or accumulation | `tonemap.wgsl` / `accumulate.wgsl`; see [RENDERER.md](RENDERER.md) |

---

## Design records

- [sticky-shader-compilation.md](../archive/projects/sticky-shader-compilation.md) — the pipeline LRU and the sticky superset
- [persistent-orbits.md](../archive/projects/persistent-orbits.md) — the orbit buffer and resume
- [unified-render-pipeline.md](../archive/projects/unified-render-pipeline.md) — one template for in-app and export, `OUTPUT_HISTOGRAM_DIRECT`
- [shader-consolidation-and-dc-gating.md](../archive/shader-consolidation-and-dc-gating.md) — one template per mode, `HAS_DC`
- [shader-builder-v3.md](../archive/shader-builder-v3.md) — hard-coded constants and feature exclusion
- [packed-variation-params.md](../archive/packed-variation-params.md), [variation-init-dispatch.md](../archive/variation-init-dispatch.md), [intra-iteration-state-and-accum.md](../archive/intra-iteration-state-and-accum.md) — parameters, init, state
- [per-transform-linked-and-final.md](../archive/projects/per-transform-linked-and-final.md) — attachments
- [analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md), [multi-emit-stereograms.md](../archive/projects/multi-emit-stereograms.md), [solid-rendering.md](../archive/projects/solid-rendering.md) — the flag-gated plot features
- [variation-math-probe.md](../archive/projects/variation-math-probe.md), [variation-reachability-census.md](../archive/projects/variation-reachability-census.md) — the probe and the census
