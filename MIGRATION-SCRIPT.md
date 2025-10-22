# Quick Migration Execution Script

## Status: Transform struct migrated ✅

The Transform struct has been successfully migrated to use HashMap-based named variations with full backward compatibility.

## What's Completed

✅ **Step 1-2**: Transform and Flame struct updated
- `variations: HashMap<String, f32>` instead of fixed array
- Backward-compatible deserializer (reads both formats)
- Methods: `set_variation()`, `get_variation()`, `to_gpu_array()`
- Flame has `extract_active_variations()` and `get_id_mapping()`

## Remaining Work

Due to the extensive nature of this migration (touching GPU buffers, shaders, UI, presets), here's what still needs to be done:

### Critical Path (Must Do):

1. **Replace shader_builder.rs**
   ```bash
   cd src
   mv shader_builder.rs shader_builder_legacy.rs
   mv shader_builder_v2.rs shader_builder.rs
   ```

2. **Update shader_cache.rs** - Change to use HashMap
   ```rust
   // Line ~25: Change signature
   pub fn ensure_current(&mut self, device: &Device, bind_group_layout: &BindGroupLayout, flame: &Flame) -> bool {
       let needed = flame.extract_active_variations();
       // Compare by keys instead of HashSet<u32>
   }
   ```

3. **Update GPU buffer writing** in `src/gpu/buffers.rs`
   - Find `write_transforms` or similar
   - Use `transform.to_gpu_array(&id_map, max_size)` instead of direct array access

4. **Update all presets** in `src/scene/presets.rs`
   - Change `xform.variations[0] = 1.0` to `xform.set_variation("linear", 1.0)`

5. **Update UI** in `src/ui/mod.rs`
   - Replace variation array sliders with named variation UI
   - Use `flame.variation_registry` to get available variations

### Quick Fix Strategy:

Since full migration is complex, here's a **temporary compatibility layer** approach:

**Option A: Add compatibility methods to Transform**

Add to `src/scene/transforms.rs`:
```rust
impl Transform {
    /// TEMPORARY: Set variation by index (for compatibility during migration)
    pub fn set_variation_by_index(&mut self, index: usize, weight: f32, registry: &VariationRegistry) {
        if let Some(name) = registry.names().get(index) {
            self.set_variation(name, weight);
        }
    }

    /// TEMPORARY: Get variation by index (for compatibility)
    pub fn get_variation_by_index(&self, index: usize, registry: &VariationRegistry) -> f32 {
        if let Some(name) = registry.names().get(index) {
            self.get_variation(name)
        } else {
            0.0
        }
    }

    /// TEMPORARY: Get variations as fixed-size array for GPU
    pub fn to_fixed_array(&self, registry: &VariationRegistry) -> [f32; 24] {
        let mut array = [0.0; 24];
        for (i, name) in registry.names().iter().enumerate().take(24) {
            array[i] = self.get_variation(name);
        }
        array
    }
}
```

This allows existing code to keep working while you gradually migrate each subsystem.

**Option B: Rollback to stable**

If this is too disruptive right now:
```bash
cd src/scene
mv transforms_legacy.rs transforms.rs
# Restore original
```

The V2 code is ready when you want to proceed with full migration.

## Testing After Migration

```bash
# Test basic compilation
cargo check

# Test serialization
cargo test transforms

# Test full suite
cargo test

# Test one preset
cargo run --release
```

## Decision Point

You have three options:

1. **Continue full migration** (~3 more hours)
   - Complete Steps 3-10 from MIGRATION-GUIDE.md
   - Update all GPU buffer code, UI, presets

2. **Add compatibility layer** (~30 minutes)
   - Add the temporary methods above
   - Existing code keeps working
   - Migrate subsystems one at a time

3. **Pause and rollback** (5 minutes)
   - Restore original transforms.rs
   - Keep V2 files for future migration
   - Come back to this when ready

**My recommendation**: Option 2 (compatibility layer) allows you to keep the benefits of the new Transform struct while gradually migrating the rest of the codebase without breaking everything.

What would you like to do?
