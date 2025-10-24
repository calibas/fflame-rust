# Variation Index Issue (2025-10-24)

## Problem

3D fractals crash with shader error:
```
Accessing index 24 is out of [51] bounds
```

## Root Cause

- Variation registry contains 26 variations (JuliaN and Blob added later)
- MAX_VARIATIONS hardcoded to 24
- GPU struct `variations: [f32; 24]` can't hold 26 values
- Shader tries to access `variations[24]` and `variations[25]`

## Why Simple Fix Doesn't Work

Increasing MAX_VARIATIONS to 26 breaks ALL existing presets because:
1. Variation order in registry determines array indices
2. JuliaN and Blob were inserted in **middle** of list
3. This shifts all subsequent variations' indices
4. Old preset index 16 (Zcone) now maps to wrong variation

## Current Approach Issues

Using **array indices** for variations has fundamental problems:
- Adding variations breaks backward compatibility
- Order matters but isn't explicitly controlled
- UI order vs storage order vs shader order confusion
- No way to safely insert variations

## Proper Solution

Refactor to use **variation names** (HashMap) instead of indices:
1. Store variations by name in Transform: `HashMap<String, f32>`
2. GPU buffer: Convert to fixed array at upload time using registry
3. Shaders: Keep array-based (GPU can't use HashMap)
4. Mapping: Use registry to convert name → index for GPU

Benefits:
- Adding variations doesn't break compatibility
- Order doesn't matter in storage
- Clear separation: names in Rust, indices only in GPU/shaders
- Can reorder registry without affecting saved presets

## Files Affected

Would need to modify:
- `src/scene/transforms.rs` - Use HashMap for variations
- `src/scene/transforms_legacy.rs` - Keep for backward compat (16/24 arrays)
- `src/gpu/buffers.rs` - Convert HashMap → array at GPU upload
- `src/config.rs` - Serialize as HashMap
- All existing `.flame` files - Need migration or compat layer

## Temporary Workaround

For now: Don't use JuliaN or Blob variations. Limit to first 24.
