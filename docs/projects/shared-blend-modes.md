# Shared Blend Modes for Color Effects

## Status: ✅ Complete

## Overview

Implement a shared blend mode system for all color effects, providing consistent blending options across the effects pipeline.

## Standard Blend Modes (0-12)

| Index | Mode | Description |
|-------|------|-------------|
| 0 | Normal | Simple alpha/intensity blend |
| 1 | Add | Additive (Linear Dodge) - brightens |
| 2 | Multiply | Darkens, good for shadows |
| 3 | Screen | Lightens, inverse of multiply |
| 4 | Overlay | Combines multiply/screen, increases contrast |
| 5 | Soft Light | Gentler version of overlay |
| 6 | Hard Light | Stronger version of overlay |
| 7 | Color Dodge | Brightens base, high contrast highlights |
| 8 | Color Burn | Darkens base, high contrast shadows |
| 9 | Hue | Takes hue from effect, sat/lum from original |
| 10 | Saturation | Takes saturation from effect |
| 11 | Color | Takes hue+saturation from effect, luminosity from original |
| 12 | Luminosity | Takes luminosity from effect, hue+sat from original |

## Implementation Checklist

### Phase 1: Shared Infrastructure ✅

- [x] Create `shaders/effects/common/blend_modes.wgsl` with:
  - [x] `rgb_to_hsl()` function
  - [x] `hsl_to_rgb()` function
  - [x] `luminance()` function
  - [x] `blend_normal()` function
  - [x] `blend_add()` function
  - [x] `blend_multiply()` function
  - [x] `blend_screen()` function
  - [x] `blend_overlay()` function
  - [x] `blend_soft_light()` function
  - [x] `blend_hard_light()` function
  - [x] `blend_color_dodge()` function
  - [x] `blend_color_burn()` function
  - [x] `blend_hue()` function
  - [x] `blend_saturation()` function
  - [x] `blend_color()` function
  - [x] `blend_luminosity()` function
  - [x] `apply_blend(base, effect, mode, intensity)` unified function

- [x] Modify `src/renderer/effect_chain.rs`:
  - [x] Load blend_modes.wgsl content at startup
  - [x] Implement `// INCLUDE_BLEND_MODES` marker substitution
  - [x] Handle both desktop (file load) and WASM (embedded) paths

### Phase 2: Update Color Effects ✅

Each effect needs:
1. Add `// INCLUDE_BLEND_MODES` marker
2. Remove duplicate blend/color conversion functions
3. Add `blend_mode` parameter (as last parameter)
4. Use `apply_blend()` for final output

#### Effects Checklist

| Effect | Has blend_mode | Uses shared lib | Tested |
|--------|---------------|-----------------|--------|
| plasma | [x] | [x] | [ ] |
| simplex_noise | [x] | [x] | [ ] |
| worley_noise | [x] | [x] | [ ] |
| domain_warp | [x] | [x] | [ ] |
| sobel_edges | [x] | [x] | [ ] |
| kaleidoscope | [x] | [x] | [ ] |
| tunnel | [x] | [x] | [ ] |
| film_grain | [x] | [x] | [ ] |
| chromatic_aberration | [x] | [x] | [ ] |
| vignette | [x] | [x] | [ ] |
| hue_shift | [x] | [x] | [ ] |

### Phase 3: Registration Updates ✅

- [x] Update `src/effects/mod.rs` registrations:
  - [x] plasma - update blend_mode range to 0-12
  - [x] simplex_noise - add blend_mode parameter
  - [x] worley_noise - add blend_mode parameter
  - [x] domain_warp - add blend_mode parameter
  - [x] sobel_edges - add blend_mode parameter
  - [x] kaleidoscope - add intensity + blend_mode parameters
  - [x] tunnel - add intensity + blend_mode parameters
  - [x] film_grain - add blend_mode parameter
  - [x] chromatic_aberration - add intensity + blend_mode parameters
  - [x] vignette - add blend_mode parameter
  - [x] hue_shift - add intensity + blend_mode parameters

### Phase 4: Translations ✅

- [x] Update `locales/en.yml`:
  - [x] Add `blend_mode` translation for each effect that needs it
  - [x] Add `intensity` translation for effects that gained it

## Technical Notes

### Shader Include Pattern

Effects use this pattern:
```wgsl
// Near top of shader, after struct definitions
// INCLUDE_BLEND_MODES

// In fragment shader
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(input_texture, input_sampler, input.uv);

    // ... effect-specific processing to get effect_color ...

    let blend_mode = i32(get_param(LAST_PARAM_INDEX));
    let result = apply_blend(original.rgb, effect_color, blend_mode, intensity);

    return vec4<f32>(result, original.a);
}
```

### Parameter Ordering Convention

Blend mode is always the **last** parameter for consistency:
- effect-specific params first
- `intensity` (blend amount)
- `time` (if animated)
- `blend_mode` last

### WASM Considerations

- Desktop: Load `blend_modes.wgsl` from filesystem
- WASM: Embed at compile time (like other effect shaders)
- Uses same `#[cfg(target_arch = "wasm32")]` pattern as existing embedded shaders

## Testing

- [ ] Verify all 13 blend modes work correctly
- [ ] Test with each effect type
- [ ] Verify WASM build works
- [ ] Check no visual regressions in existing effects
