# Quick Fixes for Compilation Errors

## Status
✅ Transform struct migrated
✅ Variation Registry has Debug trait
⚠️ Need to fix compilation errors in other modules

## Fix 1: Add Compatibility Methods to Transform

Add to end of `impl Transform` in `src/scene/transforms.rs`:

```rust
    /// COMPATIBILITY: Set variation by index (for old code)
    pub fn set_variation_by_index(&mut self, index: usize, weight: f32, registry: &VariationRegistry) {
        if let Some(name) = registry.names().get(index) {
            self.set_variation(name, weight);
        }
    }

    /// COMPATIBILITY: Get variation by index
    pub fn get_variation_by_index(&self, index: usize, registry: &VariationRegistry) -> f32 {
        if let Some(name) = registry.names().get(index) {
            self.get_variation(name)
        } else {
            0.0
        }
    }

    /// COMPATIBILITY: Convert to fixed 24-element array for GPU
    pub fn to_fixed_array(&self, registry: &VariationRegistry) -> [f32; 24] {
        let mut array = [0.0; 24];
        for (i, name) in registry.names().iter().enumerate().take(24) {
            array[i] = self.get_variation(name);
        }
        array
    }

    /// COMPATIBILITY: Set from fixed array
    pub fn from_fixed_array(&mut self, array: [f32; 24], registry: &VariationRegistry) {
        self.variations.clear();
        for (i, &weight) in array.iter().enumerate() {
            if weight.abs() > 1e-6 {
                if let Some(name) = registry.names().get(i) {
                    self.set_variation(name, weight);
                }
            }
        }
    }
```

## Fix 2: Update src/app.rs (line 543)

Replace:
```rust
variations: [0.5, 0.0, 0.0, ...],
```

With:
```rust
variations: {
    let mut v = HashMap::new();
    v.insert("linear".to_string(), 0.5);
    v
},
```

## Fix 3: Update src/gpu/buffers.rs (line 42)

Replace:
```rust
variations: xform.variations,
```

With:
```rust
variations: xform.to_fixed_array(&flame.variation_registry),
```

(Make sure `flame` is passed to this function)

## Fix 4: Update src/ui/transforms.rs (lines 116, 134)

Replace:
```rust
transform.variations[idx]
```

With:
```rust
{
    let mut weight = transform.get_variation_by_index(idx, &flame.variation_registry);
    // ... slider code ...
    transform.set_variation_by_index(idx, weight, &flame.variation_registry);
}
```

## Fix 5: Update src/scene/presets.rs

Remove import:
```rust
use super::transforms::{Flame, Transform, VariationType};  // Remove VariationType
```

Add:
```rust
use super::transforms::{Flame, Transform};
```

Update all preset functions. Change:
```rust
xform.variations[0] = 1.0;
```

To:
```rust
xform.set_variation("linear", 1.0);
xform.set_variation("sinusoidal", 0.5);
// etc.
```

Variation name mappings (index -> name):
- 0 = "linear"
- 1 = "sinusoidal"
- 2 = "spherical"
- 3 = "swirl"
- 4 = "horseshoe"
- 5 = "polar"
- 6 = "handkerchief"
- 7 = "heart"
- 8 = "disc"
- 9 = "spiral"
- 10 = "hyperbolic"
- 11 = "diamond"
- 12 = "ex"
- 13 = "julia"
- 14 = "bent"
- 15 = "waves"
- 16 = "zcone"
- 17 = "flatten"
- 18 = "hemisphere"
- 19 = "pre_rotate_x"
- 20 = "pre_rotate_y"
- 21 = "post_rotate_x"
- 22 = "post_rotate_y"
- 23 = "zscale"

## After These Fixes

The project should compile with the new Transform struct while maintaining backward compatibility.

Then you can gradually:
1. Update UI to use named variations
2. Update shader system to use shader_builder_v2.rs
3. Remove compatibility methods once everything is migrated

##  Alternative: Rollback

If fixes are too time-consuming right now:
```bash
cd src/scene
cp transforms_legacy.rs transforms.rs
```

The V2 system is ready when you want it.
