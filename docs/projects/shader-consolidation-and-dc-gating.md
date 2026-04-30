# Shader consolidation + HAS_DC gating

## Goal

Two related changes:

1. **Make the direct-color machinery truly zero-cost** when no DC
   variation is active in a flame's variation set, recovering the
   ~3% regression introduced in the `transform-and-dc` PR.

2. **Reduce duplication** across the 5 main compute shaders by
   collapsing the 2D/3D file pairs into one source file each.

The two are coupled: the cleanest way to gate `has_dc` is via the
existing `TemplateProcessor` infrastructure, which we already use in
`main_template.wgsl`. The 4 static shaders (`main_2d_export`,
`main_2d_tiled`, `main_3d_export`, `main_3d_tiled`) currently bypass
the template processor entirely and are included as inert WGSL. To
gate them, we'd add template processing anyway — and once we have
template processing on every main shader, the 2D/3D split becomes a
trivial `{{#if RENDER_3D}}` block, so the pairs can merge.

## Non-goals

- Full 5→1 unification. The trajectory shader has features (xaos, path
  tracking, DOF, fog) the static export/tiled shaders don't have, and
  forcing every line of those into `{{#if EXPORT_MODE}}` blocks makes
  the source unreadable. Out of scope for this branch.
- New DC variations. Phase 5 of the parent PR shipped `dc_linear` and
  `dc_bubble`; more ports come later as their own work.

## Why the regression exists

After the parent PR, every iteration does this regardless of whether
any DC variation is registered:

```wgsl
var c_base: f32 = color_index;
if (COLOR_MODE == 0u) {
    c_base = color_index * (1.0 + symmetry) * 0.5
           + xform.color    * (1.0 - symmetry) * 0.5;
}
var vc: f32 = c_base;
current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);
if (COLOR_MODE == 0u) {
    color_index = c_base + xform.direct_color * (vc - c_base);
}
```

The compiler *can* see that nothing reads or writes `vc` (no DC
variation in the active set), but it can't eliminate the local
because `&vc` is passed to `apply_variations`. The pointer arg
forces the local to occupy a register. The Step 3 lerp also stays —
even though `vc - c_base = 0`, the compiler can't prove it because
`vc` has been escaped via the pointer.

Net cost per iteration: 1 extra `xform.direct_color` storage-buffer
load, 1 fsub, 1 fma. Measured: ~3% slowdown on `simple3` benchmark
(commit `25d9997` vs prior).

For zero cost, `apply_variations` must not take the `vc` pointer
when `has_dc == false`. That requires the call site (in each main
shader) to know `has_dc` and emit the right call.

## Design

### Compute `has_dc` once per shader build

```rust
let has_dc = active_variations
    .iter()
    .any(|(name, _)| registry.get(name).is_some_and(|v| v.writes_color));
```

Pass it into both:
- `build_apply_variations_2d/3d` to control the function signature
  (with or without `vc` parameter).
- `TemplateProcessor` as a `HAS_DC` condition for the main shader.

### Shader-builder change

`build_apply_variations_2d/3d` already extends call args based on
`info.writes_color`. Adjust the function-header line:

```rust
let signature = if has_dc {
    "fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, \
     rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32>"
} else {
    "fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, \
     rng: ptr<function, RngState>) -> vec2<f32>"
};
```

The body is unchanged — when `has_dc == false` no inner variation
has `writes_color: true` so no inner call references `vc`, and the
body's reference to `vc` is purely the function parameter (which we
elided). Symmetric for 3D.

### Main shader change (template-gated)

Each of the 5 main shaders gets:

```wgsl
{{#if HAS_DC}}
        var c_base: f32 = color_index;
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            c_base = color_index * (1.0 + symmetry) * 0.5
                   + xform.color    * (1.0 - symmetry) * 0.5;
        }
        var vc: f32 = c_base;
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);
{{else}}
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);
{{/if}}

        // ... post-affine, speed ...

{{#if HAS_DC}}
        if (COLOR_MODE == 0u) {
            color_index = c_base + xform.direct_color * (vc - c_base);
        } else if (COLOR_MODE == 1u) {
            // speed mode — unchanged
        }
{{else}}
        // Old simple Step 1 — bit-identical to pre-Phase-4
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            color_index = color_index * (1.0 + symmetry) * 0.5
                        + xform.color   * (1.0 - symmetry) * 0.5;
        } else if (COLOR_MODE == 1u) {
            // speed mode
        }
{{/if}}
```

When `HAS_DC == false`, the emitted code is *exactly* what we had
before Phase 4 — no `vc`, no `c_base`, no Step 3 lerp, no
`xform.direct_color` load. Zero cost.

The same gating applies at the final-transform call site (currently
declares a `final_vc` we'd elide).

### Consolidation: 5 → 3 shader files

Merge:

- `main_2d_export.wgsl` + `main_3d_export.wgsl` → `main_export.wgsl`
- `main_2d_tiled.wgsl`  + `main_3d_tiled.wgsl`  → `main_tiled.wgsl`
- `main_template.wgsl` (already universal) — unchanged

The 2D/3D differences in each pair are mechanical:

| Site                | 2D                      | 3D                              |
|---------------------|-------------------------|--------------------------------|
| `current` declaration | `vec2<f32>(rx, ry)`     | `vec3<f32>(rx, ry, rng())`      |
| Pixel projection    | `world_to_pixel(p)`     | `world_to_pixel_3d(p)`          |
| 3D-specific blocks  | (none)                  | DOF, fog (where applicable)     |

All three resolved via `{{#if RENDER_3D}}` blocks — same pattern
already used in `main_template.wgsl`.

The `RENDER_3D` condition is set when the shader builder picks 3D
mode. The static-shader-include sites in
`shader_builder_v2.rs::build_trajectory_2d_tiled` etc. need to
switch from `include_str!("main_2d_tiled.wgsl")` to running the
unified `main_tiled.wgsl` through the template processor with
`RENDER_3D=false`.

After consolidation:

```
shaders/core/
├── main_template.wgsl    (interactive trajectory; existing)
├── main_export.wgsl      (CLI/headless export; new merged)
├── main_tiled.wgsl       (high-res tiled rendering; new merged)
```

## Phases

Each phase verifies via stash/render/unstash/render bit-diff against
the pre-phase baseline. The benchmark harness should also confirm the
~3% recovers in the final phase.

### Phase 1: route static shaders through `TemplateProcessor`

No content changes — wrap each existing static shader in the
processor with no conditions set. Just verifies the include path
works through the processor.

Bit-diff target: 0/N pixels on all existing benchmark configs +
`weight-hack-smoke` + `dc-smoke`.

### Phase 2: add `HAS_DC` gating to all 5 shaders

Wrap the c_base/vc/Step3 emission in `{{#if HAS_DC}}...{{else}}...{{/if}}`
blocks with the pre-Phase-4 code in the `{{else}}` branch.

Adjust `build_apply_variations_2d/3d` to omit the `vc` parameter
from the function signature when `has_dc == false`.

Bit-diff target: 0/N pixels on configs without DC variations.
`dc-smoke.fflame` should still render correctly (visual sanity check).

Benchmark target: regression on `simple3` (no DC variations) recovers
to within ±0.5% of pre-Phase-4.

### Phase 3: merge 2D/3D export pair

Delete `main_2d_export.wgsl` and `main_3d_export.wgsl`, create
`main_export.wgsl` with `{{#if RENDER_3D}}` blocks. Update
`shader_builder_v2.rs::build_trajectory_*_export` (or wherever the
includes happen) to pass the new file through the processor.

Bit-diff target: 0/N pixels on existing visual regression configs
that exercise both 2D and 3D paths.

### Phase 4: merge 2D/3D tiled pair

Same pattern for the tiled shaders. Tiled rendering exercises pixel
routing into per-tile buffers, so verify against any existing
high-res tile tests + a manual 4K render of an existing config.

Bit-diff target: 0/N pixels.

## Verification

Standard methodology from the parent PR:

```bash
# Phase N pre-baseline
git stash
cargo run --release -- export -i tests/visual/configs/.../foo.fflame \
    -o /tmp/pre.png --width 600 --height 600
git stash pop
cargo run --release -- export -i tests/visual/configs/.../foo.fflame \
    -o /tmp/post.png --width 600 --height 600
python -c "..."  # bit-diff
```

Plus benchmark suite for Phase 2:
```bash
python scripts/run_benchmarks.py --quick
```

Comparing against the 2026-04-30 entry (commit `25d9997`) for
`simple3`, expect the new commit to land back near the
2026-04-30 *prior* baseline (~16,440 µs/iter rather than the post-DC
~16,920).

## Risks

- **WGSL function signature divergence.** When `has_dc == true`,
  apply_variations takes `vc`. When false, it doesn't. The shader
  builder has to keep these in lockstep with each main shader's call
  site. The `HAS_DC` template variable is the single source of truth
  on the Rust side; the WGSL just consumes it.

- **Tiled shader complexity.** The tiled shaders write to per-tile
  histogram buffers via a non-trivial pixel-to-tile lookup. Merging
  2D/3D shouldn't touch that logic, but the diff is the largest among
  the static shaders. Plan extra time for Phase 4.

- **Trajectory shader stays a special case.** `main_template.wgsl`
  has xaos, path tracking, DOF, fog — none of which exist in
  export/tiled. We're not unifying it into the merged shaders. If
  we change the consolidated `main_export.wgsl`'s structure later,
  drift between it and `main_template.wgsl` will require care.

## Open question

Is there a benefit to also adding `HAS_DC = false` for the export and
tiled paths even when DC variations *are* registered? In other words:
should DC be supported at all in headless export and high-res tiled
modes, or is it interactive-trajectory-only?

Argument for keeping DC in export/tiled: a user makes a flame with DC
variations interactively, then exports it; they expect the export to
match.

Argument against: export and tiled are CLI/special paths where the
extra cost might matter more than in interactive use.

Default: keep DC in all three paths, gated normally. Reconsider only
if benchmarks show it materially affects export throughput.
