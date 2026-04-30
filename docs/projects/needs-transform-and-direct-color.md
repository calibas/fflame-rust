# needs_transform + direct color

## Goal

Two related extensions to the variation system:

1. **Rename `needs_affine` → `needs_transform`** and broaden it to mean
   "this variation needs `xform_id`, so it can read anything from
   `transforms[xform_id]`" — affine matrix, weight, color, opacity,
   `direct_color`. Migrate the existing weight-passing hacks
   (`pre_rotate_x/y`, `post_rotate_x/y`, `pre_zscale`, `pre_ztranslate`,
   `pre_sinusoidal`, `pre_disc`) onto it so the by-name special-casing
   in `shader_builder_v2` can go away.

2. **Add direct color support.** Per-transform `direct_color` field
   (Apophysis `pluginColor`) plus a `writes_color: bool` flag on
   `VariationDef` that gives a variation write access to the
   iteration-local color register `vc`. Implement a starter set of
   `dc_*` variations from the JWildfire lineup.

The two are orthogonal but both affect the variation signature, so it's
natural to land them on the same branch.

## Non-goals

- **`dc_image`.** Needs a texture binding; separate follow-up.
- **Xaos.** Different system entirely (transform routing matrix), not
  in this branch.
- **Per-iteration shader rebuild on slider drag.** Already a hard rule;
  `direct_color` is a uniform read, not a build-time constant.

## Design

### Variation flags

```rust
pub struct VariationDef {
    // ... existing fields ...
    pub needs_transform: bool,   // (was: needs_affine)
    pub writes_color: bool,      // NEW
}
```

`needs_transform` is a rename + broadening of `needs_affine`. The
existing 2 users (`waves`, `popcorn`) are unchanged in behavior. Once
renamed, we migrate the weight-passing hacks: each of those variations
sets `needs_transform: true` and reads `transforms[xform_id].weight`
directly. The by-name `pre_rotate_x` etc. branches in
`shader_builder_v2.rs` get deleted.

`writes_color` is the new flag. When a variation sets it, the shader
builder adds `vc: ptr<function, f32>` to its signature, analogous to
how `needs_rng` adds the RNG pointer. The variation body can read
`*vc` and assign `*vc = ...`.

The two flags compose: `julian2dc` will set
`needs_rng + needs_transform + writes_color` (RNG for the integer pick,
xform_id for parameter lookup, vc for the color write).

### Per-iteration color flow

Apophysis 3-step formula, unchanged from upstream:

```
// Step 1: color_speed blend (already implemented)
let symmetry = xform.color_speed;
let c_base = color_index * (1.0 + symmetry) * 0.5
           + xform.color    * (1.0 - symmetry) * 0.5;

// Step 2: vc starts at c_base, DC variations may overwrite
var vc = c_base;
// (variations run; DC ones modify *vc)

// Step 3: lerp
color_index = c_base + xform.direct_color * (vc - c_base);
```

### Build-time gating (the perf win)

The Step 2/3 machinery is **only emitted when at least one active
variation has `writes_color: true`**. The shader builder already
inspects the active set; we just add a flag computation:

```rust
let has_dc = active_vars.iter().any(|v| v.writes_color);
```

When `has_dc == false`, the generated code reverts to today's
`color_index = c_base` and skips the `vc` register and the lerp
entirely. Flames with no DC variations are bit-identical to current
output and pay zero extra cost.

When `has_dc == true`, all transforms get the `vc` plumbing — even
those whose own variations don't write color, because the lerp formula
is uniform across xforms (just with `direct_color = 0` for those
xforms, which makes Step 3 a no-op at the math level but still costs a
few ops per iter). This is intentional: per-transform shader variants
would multiply build cost without meaningful savings.

### Transform field

```rust
pub struct Transform {
    // ... existing ...
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub direct_color: f32,   // NEW, default 0.0
}
```

Mirrored on `GpuTransform`. The current `_padding` slot at the end of
the color quad takes it (no struct size change, no realignment work).

XML import reads `pluginColor` (and the `plugin_color` alternate
spelling, both seen in the wild). Export writes `pluginColor` only
when non-zero.

UI slider in the per-transform color section, range 0.0-1.0. New
`ConfigPath::TransformDirectColor { index }` with `UpdateType::Flame`
for ConfigManager integration. Standard config_slider pattern, no new
ground.

## JWildfire/Chaotica compatibility

The `dc_*` plugins follow a consistent convention: set
`pVarTP.color = ...` to a palette position derived from geometry. Our
`vc` pointer is the direct equivalent.

Notes pulled from the upstream sources:

- **Position + color.** Many DC variations also transform position
  (e.g. `julian2dc` is a real Julian; the DC suffix means "and writes
  color"). Our signature `fn(p, ..., vc) -> vec2<f32>` supports this
  cleanly: returned vec2 is the new position, `*vc` is the new color.

- **Color offset parameter.** Common pattern: a per-variation
  `color_offset` user parameter added to the computed color before
  writing. Just a parameter, no architectural change.

- **3D DC variations.** `dc_ztransl` reads `p.z`. Our existing 2D/3D
  variation split handles this; the 3D shader gets the `vec3` overload
  with `vc` pointer same as 2D.

- **`dc_image`.** Reads from a colormap texture. Needs an extra
  binding. Punt.

### Initial DC variation set (Phase 3 below)

| Name         | Effect                                | RNG | Notes                              |
|--------------|---------------------------------------|-----|-----------------------------------|
| `dc_linear`  | Color from rotated linear position    | no  | Position unchanged.               |
| `dc_bubble`  | Color from radial distance            | no  | Position unchanged.               |
| `julian2dc`  | Real Julian + color from r/θ blend    | yes | Sets `needs_transform` for params.|
| `dc_ztransl` | 3D color from z position              | no  | 3D-only.                          |

Enough to validate the architecture against actual JWF flames; more
ports follow in subsequent PRs.

## Phases

Each phase ends with `cargo build --release` clean and a stash/render
bit-diff against pre-phase baseline. Phases 1 and 2 must produce
**zero differing pixels**.

### Phase 1: rename `needs_affine` → `needs_transform`

Pure mechanical rename. The flag still means the same thing; just the
name broadens to anticipate weight/color reads. Touches ~177 literals
plus the shader-builder code that branches on it.

Bit-diff target: 0/N pixels on `init-migration-smoke.fflame` and the
existing `misc-variations.fflame` configs.

### Phase 2: migrate weight-passing hacks onto `needs_transform`

`pre_rotate_x`, `pre_rotate_y`, `post_rotate_x`, `post_rotate_y`,
`pre_zscale`, `pre_ztranslate`, `pre_sinusoidal`, `pre_disc` —
each sets `needs_transform: true` and reads `transforms[xform_id].weight`
in its body. Then the by-name branches in `shader_builder_v2.rs`
get deleted.

Bit-diff target: 0/N pixels on a config exercising each of those 8
variations.

### Phase 3: add `direct_color` field (no shader changes affecting output)

- `Transform.direct_color: f32` (default 0.0)
- `GpuTransform` mirror, replacing `_padding`
- `header.wgsl` Transform struct add
- ConfigPath + UI slider
- XML `pluginColor` import/export round-trip
- Apply_transform / main loop unchanged this phase

Bit-diff target: 0/N pixels on existing test configs (default 0.0
means no behavior change yet).

### Phase 4: add `writes_color` flag + main-loop `vc` plumbing

- `VariationDef.writes_color: bool` + `VariationInfo` mirror
- API JSON field with `serde(default)` for back-compat
- Shader builder: detect `has_dc`, emit `vc` register + Step 3 lerp
  when true
- `apply_transform` signature gains `vc: ptr<function, f32>` when
  `has_dc`
- Variation signature picks up `vc` pointer when `writes_color: true`
- No DC variations exist yet, so `has_dc` is always false, so behavior
  unchanged

Bit-diff target: 0/N pixels (still no DC variations registered).

### Phase 5: implement starter DC variations

- `dc_linear` (extended.rs or new dc.rs)
- `dc_bubble`
- `julian2dc`
- `dc_ztransl` (3D, in depth3d.rs or full3d.rs)

Each has a smoke-test config under
`tests/visual/configs/variations/`. No bit-diff baseline (these are
new); validate visually against JWF reference renders.

## Performance

| Scenario                                     | Cost vs current                        |
|---------------------------------------------|---------------------------------------|
| `needs_transform` rename, no users          | 0                                     |
| Variation that sets `needs_transform`       | 1 storage load when accessed (today)  |
| Flame with no DC variations active          | 0 (build-time gate)                   |
| Flame with DC variation, `direct_color = 0` | ~3-5 ops/iter (Step 3 evaluates to 0) |
| Flame with DC variation, `direct_color > 0` | ~3-5 ops/iter + DC variation's color  |
|                                             | formula                                |

The build-time gate is the key performance commitment: existing flames
that don't opt into DC pay nothing.

The "DC active but `direct_color = 0`" case is a user pitfall — the DC
variation still runs because removing it would require a rebuild.
Document in the UI tooltip.

## Verification

Same methodology as `variation-init-dispatch`:

```bash
# Phase N pre-baseline
git stash
cargo run --release -- export -i tests/visual/configs/.../foo.fflame \
    -o /tmp/pre.png --width 600 --height 600
git stash pop
cargo run --release -- export -i tests/visual/configs/.../foo.fflame \
    -o /tmp/post.png --width 600 --height 600

python -c "
from PIL import Image; import numpy as np
a = np.array(Image.open('/tmp/pre.png').convert('RGBA'))
b = np.array(Image.open('/tmp/post.png').convert('RGBA'))
diff = np.count_nonzero(np.any(a.astype(int) - b.astype(int), axis=-1))
print(f'differing pixels: {diff}')
"
```

Phases 1, 2, 3, 4 must report `differing pixels: 0`. Phase 5
introduces new variations so there's no pre-baseline; visual sanity
check + comparison against JWF reference.

## Open questions

- **Zero-weight variations.** Does the shader builder currently filter
  variations whose weight is 0 in a particular transform? If yes, a DC
  variation present at weight 0 won't fire `has_dc` for that transform
  — but `has_dc` is global across the active set, so this works out.
  If no, no issue. Confirm during Phase 4.

- **Final transform color handling.** Final xforms ignore `color`,
  `color_speed`, `opacity` per existing comments in
  `transforms.rs:93-107`. Should they also ignore `direct_color`?
  Apophysis applies the final-xform Step 3 blend the same as any other
  xform — so probably yes, *don't* ignore. Confirm against an
  Apophysis flame with a DC final xform.

- **`color_offset` convention.** JWF DC variations frequently take a
  `color_offset` parameter added to the computed color. We can either
  bake it into each variation's WGSL or expose it as a uniform
  variation parameter. Latter is more flexible; pick during Phase 5
  based on what JWF's parameter schemas look like.
