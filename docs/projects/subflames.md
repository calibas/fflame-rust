# Subflame variation support

## Goal

Implement the JWildfire `subflame_wf` variation. A subflame is **not** a
layered render — it's a variation function that owns a complete inner
flame definition, and during each step of the parent flame's chaos game
it advances a *nested* chaos game by one iteration on the subflame's
IFS, then uses the resulting point as the variation's output:

```
FPx += scale * (cos(angle)*q.x - sin(angle)*q.y) + offset_x
FPy += scale * (sin(angle)*q.x + cos(angle)*q.y) + offset_y
FPz += scale * q.z + offset_z + colorscale_z * q.color
```

where `q` is the subflame's current chaos-game point. One render
pipeline, one histogram, one tonemap — but the iteration loop calls
into a nested IFS as a variation function.

Reference: [output/jwildfire-vars/output/subflame_wf.cpp](../../output/jwildfire-vars/output/subflame_wf.cpp)
(C++ port of JWildfire's
[SubFlameWFFunc.java](https://github.com/thargor6/JWildfire/blob/master/src/org/jwildfire/create/tina/variation/SubFlameWFFunc.java)).
Spec details: [JWildfire Sanctuary — subflame](https://www.jwfsanctuary.club/variation-information/subflame/).

## subflame_wf is a "blur" variation

The single most load-bearing detail for implementation: per the
sanctuary spec page, `subflame_wf` is classified as a **blur
variation**. That has three concrete consequences:

1. **The variation ignores its input point `p`.** Like `circleblur`
   or `starblur`, the variation's output is determined entirely by
   its own internal state (the subflame's chaos-game point `q`),
   not by where the parent's current iteration landed.
2. **The parent xform's pre-affine is therefore a no-op for the
   `subflame_wf` contribution.** It still transforms `p` for the
   other variations on the same xform, but `subflame_wf` doesn't
   consume `p`.
3. **The variation `amount` is ignored.** Users in JWildfire scale
   the subflame's contribution via the parent xform's **post-affine**
   instead (the page's example: "shrink the triangle by 300%").

The variation still has `scale` / `angle` / `offset_*` parameters
that act on `q` *before* adding to FP, but the spec page treats
post-affine scaling as the canonical workflow. We support both for
round-trip fidelity.

## Context

`subflame_wf` is a popular variation that appears in many shared
`.flame` files. Without it our app silently drops the variation on
import; the rendered output diverges from what the author intended.
This project closes the compatibility gap.

Layered rendering (independent flames composited as separate
"Photoshop-style" layers) is **out of scope**. Apophysis/flam3/most
of the ecosystem don't have layered rendering as a built-in — users
composite externally via transparent PNGs. The structural changes
this project introduces (top-level `subflames: Vec<Flame>` field)
*could* be extended to support layered rendering later, but that's a
future call.

## Architecture survey findings

The current renderer's shape makes this cheaper than expected:

- **Variation parameters are all scalar f32** with rich types
  (`Float`, `Integer`, `Angle`, `Enum`, etc.). We don't need a new
  `ParamType` — `subflame_id` is a regular `Integer`.
- **State-count mechanism already exists**: variations can declare
  `state_count` slots for per-`(thread, xform, variation)`
  persistence (`src/variations/mod.rs:134-142`). The subflame's
  point + transform-index state fits here for free.
- **Binding slot 11+ is open** (`shaders/core/header.wgsl` uses
  0–10). A second transforms buffer for subflames fits cleanly.
- **Final transforms already supported** in the `Flame` struct's
  `final_transforms: Vec<Transform>` pool — subflames can use them.
- **Inner loop iterates `iterations_per_thread` times per dispatch**
  with per-thread persistent state. The subflame's chaos game runs
  alongside the parent's, advancing one step per parent step.

## Data model

`FractalConfig` gains:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub subflames: Vec<Flame>,
```

Each entry is a complete `Flame` — transforms, palette index, color
mode, render mode, etc. Subflames referenced by index from the
`subflame_wf` variation's `subflame_id` parameter. An empty `Vec`
matches today's behavior; old `.fflame` files deserialize unchanged.

Naming chosen for specificity: `subflames` instead of `flames` or
`layers`. If layered rendering arrives later it goes in a separate
`layers: Vec<Layer>` field; the two concepts don't compete for the
same slot.

## Variation registration

Register `subflame_wf` in the variation registry with these parameters:

| Name | Type | Default | Notes |
|---|---|---|---|
| `subflame_id` | Integer | 0 | Index into `FractalConfig.subflames` |
| `scale` | Float | 1.0 | Post-iteration uniform scale |
| `angle` | Angle | 0.0 | Post-iteration rotation (degrees) |
| `offset_x` | Float | 0.0 | Post-iteration X translation |
| `offset_y` | Float | 0.0 | Post-iteration Y translation |
| `offset_z` | Float | 0.0 | Post-iteration Z translation |
| `colorscale_z` | Float | 0.0 | Z contribution proportional to subflame color |
| `color_mode` | Enum | Off | Off / Direct / Red / Green / Blue / Brightness |

Plus a `state_count`:
- 3 slots for the subflame's current point (`p.x`, `p.y`, `p.z`)
- 1 slot for the subflame's current transform index (cast f32 ↔ u32)
- 1 slot for the subflame's current color scalar
- Total: **5 state slots per variation instance** at nesting depth 1

Per-instance state means using `subflame_wf` in multiple parent
transforms gives each instance its own independent chaos-game state
— correct semantically.

**Nesting-friendly design**: the shader's iteration helper is
parameterized as `subflame_iterate(subflame_id, state_offset)`
rather than `subflame_iterate(subflame_id)`. v1 always passes
`state_offset = 0` (one nesting level), but v2 nesting just adds
more state slots and emits the call with `state_offset = 5 × depth`.
See "Out of scope (v1) → future-proofing" below.

## GPU layout

### Subflame transforms buffer (`@binding(11)`)

A new storage buffer holding the concatenated `GpuTransform` arrays
for every subflame:

```
[subflame_0.transforms..., subflame_0.linked_transforms..., subflame_0.finals...,
 subflame_1.transforms..., subflame_1.linked_transforms..., subflame_1.finals...,
 ...]
```

### Subflame metadata uniform

A small uniform buffer with per-subflame offset + counts:

```rust
struct SubflameMeta {
    transform_offset: u32,
    transform_count: u32,
    linked_offset: u32,
    linked_count: u32,
    final_offset: u32,
    final_count: u32,
    palette_size: u32,    // for color mode lookup
    _pad: u32,
}
// Up to MAX_SUBFLAMES of these in an array<SubflameMeta, MAX_SUBFLAMES>.
```

This lets the shader look up where a given subflame's transforms
live without hardcoding sizes.

### Subflame palettes

For v1, subflames borrow the parent flame's palette texture when
`color_mode` is anything other than Off. The sanctuary spec is
explicit on this point: *"the colors come from the gradient of the
main flame, not the subflame"*.

**v2-friendly design**: the shader function that does the palette
lookup takes a `palette_id` indirection — `sample_palette(palette_id, t)`
— even though v1 only ever passes `palette_id = 0` (parent's
palette). When v2 wants per-subflame palettes, we add a 1D-texture
array binding (one entry per subflame) and the shader function gains
the array-index path. No call-site changes needed.

## Shader changes

### `subflame_iterate(subflame_id, state) -> vec4<f32>`

A new shader helper. Reads the subflame's current `(p, xf)` from
the variation's state slots, picks the next transform from
`subflame_transforms[subflame_meta[subflame_id].transform_offset ..
.transform_offset + .transform_count]` (weight-table or xaos),
applies its affine + variations to `p`, runs any
`final_transforms` after, writes the new `(p, xf)` back to state,
returns `(new_p.xyz, new_color)`.

This is essentially the parent's iteration step extracted into a
function that operates on a subflame-specific transform pool. Most
of the existing shader iteration code is reusable; the shader-builder
generates a parameterized version.

### `variation_subflame_wf(...)` body

```wgsl
let subflame_id = u32(get_param(xform_id, variation_id, 0u));
let scale = get_param(xform_id, variation_id, 1u);
let angle = get_param(xform_id, variation_id, 2u);
let offset_x = get_param(xform_id, variation_id, 3u);
// ... etc

let q = subflame_iterate(subflame_id, ...);
let x = scale * q.x;
let y = scale * q.y;

return vec3<f32>(
    x * cos(angle) - y * sin(angle) + offset_x,
    x * sin(angle) + y * cos(angle) + offset_y,
    scale * q.z + offset_z + colorscale_z * q.color,
);
```

Color mode handling: if `color_mode != Off`, write the subflame's
color (`q.w`) back into the parent's `vc` state. The CM_RED/GREEN/BLUE
modes need access to the subflame's palette lookup result, not just
the scalar — handle via an inline palette sample.

### Prefuse (burn-in)

JWildfire's subflame_wf runs 42 burn-in iterations at init time to
escape from the random starting point. We replicate this **once per
dispatch per thread** at the start of the compute loop (cheap, ~42 ×
N iterations where N is the number of subflame_wf instances active
on the current xform pick).

Per-dispatch instead of per-app-launch because the GPU shader has no
"app launch" — each compute dispatch is a fresh invocation. The cost
is 42 × samples_per_thread = ~1% of the total iteration budget at
default settings.

## Recursion + cycles

JWildfire **does** allow nested subflames (from the sanctuary spec:
*"nesting subflames is allowed"*), but we defer that to v2.

For v1, **a subflame's variations cannot include `subflame_wf`**.
Enforce by failing config validation if a subflame's transform has a
non-zero weight on `subflame_wf` (similarly on Apophysis import —
emit a warning and disable the nested instance).

**Why this is cheap to add later**:
- State allocation is already parameterized per nesting depth (see
  `state_offset` in the variation-registration section). v2 just
  allocates `5 × max_depth` slots per instance instead of 5.
- The `subflame_iterate(subflame_id, state_offset)` helper is
  already nesting-aware. v2's only shader change is generating the
  call with the right state offset at the inner level.
- Maximum nesting depth becomes a compile-time constant in shader
  codegen (likely 2 or 3 — beyond that, state-count explosion isn't
  worth it for a niche feature).

The validation check at config-load time lives in a single function
that v2 can relax to "depth ≤ MAX_NESTING" once the shader supports it.

## Apophysis XML import

Apophysis `.flame` XML stores subflames as a `<flame ...>` XML blob
embedded in the `subflame_wf` variation's resource params. Our
importer needs to:

1. Detect `subflame_wf_flame` resource on a variation.
2. Parse the embedded XML as a `Flame` using existing `apophysis_xml`
   logic.
3. Append the parsed `Flame` to `FractalConfig.subflames`.
4. Set the variation's `subflame_id` param to the new index.

## UI

- New panel (or tab on the Transforms panel): **Subflames**.
  Shows the `subflames: Vec<Flame>` list. Add / remove / rename.
  Selecting a subflame swaps the Transforms panel into "edit this
  subflame" mode (same widgets, different data).
- The `subflame_id` parameter on a variation appears as a dropdown
  populated from `FractalConfig.subflames` instead of a numeric
  slider.

## Phases

1. **Data model.** Add `subflames: Vec<Flame>` to `FractalConfig` +
   serde + `ConfigPath::Subflame*` paths. No GPU changes yet; new
   field is unused by the renderer.
2. **GPU buffer plumbing.** Add `@binding(11)` `subflame_transforms`
   storage buffer + subflame metadata uniform. Upload on flame load
   / subflame edit. Shader still doesn't reference them.
3. **Variation registration.** Register `subflame_wf` in the registry
   with its 8 parameters and 5 state slots. Stub shader function
   returns input unchanged.
4. **Shader iteration.** Implement `subflame_iterate()` and the
   `subflame_wf` body. Wire prefuse logic. This is the main work —
   shader-builder changes to generate the function per active
   subflame count.
5. **Apophysis import.** Extend `apophysis_xml.rs` to extract
   embedded subflame XML and populate `FractalConfig.subflames`.
6. **UI.** Subflames panel + subflame_id dropdown rendering.
7. **Validation + tests.** Visual smoke test on a known JWildfire
   subflame flame file. Manual A/B against JWildfire's render of the
   same `.flame` if available.

## Out of scope (v1) → v2 plan

Each item below is deliberately deferred *and* the v1 design avoids
baking in assumptions that would force a rewrite to add it later.

| Feature | Why deferred | What v2 adds | v1 design hook that makes v2 cheap |
|---|---|---|---|
| **Nested subflames** | State-count + shader codegen complexity; rare in the wild | Support depth ≤ N (likely 2-3) | `subflame_iterate(id, state_offset)` already takes an offset; only state allocation + a validation check change |
| **Per-subflame palettes** | Adds a texture-array binding + per-subflame upload; few JWildfire flames use it | Adds 1D-texture-array binding; shader's `sample_palette(palette_id, t)` gains the array path | `palette_id` indirection in v1's shader; v1 always passes 0 |
| **`pre_subflame_wf` / `post_subflame_wf`** | Variants triggered in different transform phases; trivial follow-up | New variation registrations using the same shader function in a different phase | Variation phase is already a registration parameter; nothing v1-specific blocks this |
| **Flame sequence animation** (`flame_is_sequence`, `flame_sequence_*`) | Niche; requires frame-indexed file loading | Add the sequence parameters + per-frame file loading | Just additional parameters on the variation; no architectural change |
| **Layered rendering** | Different feature entirely (separate render pipelines + composite); confirmed not a JWildfire-compat ask | Adds `layers: Vec<Layer>` parallel to `subflames` | `subflames` field is named specifically so the layer field can sit alongside it without colliding |

## Risks

| Risk | Mitigation |
|---|---|
| Shader-builder complexity to generate per-subflame iteration code | Start with v1 = single subflame; lift to multi only after pipeline works |
| Prefuse cost at default settings | Measured per the project plan; if >5% overhead, move prefuse to a separate small init dispatch |
| Subflame color mode requires palette sample | Handle inline with the parent's palette in v1; second palette texture is a future expansion |
| Binding slot 11 conflicts with anything WASM-only | Audit `cfg(wasm32)` paths in `gpu/buffers.rs`; slot 11 should be safe per the survey |
| Subflames bloat config file size | Each subflame is a full `Flame` (~50-200 lines of JSON). Acceptable for the rare flames that use subflames. |

