# Plugin Variations Directory

This directory contains example plugin variation shaders that can be dynamically loaded into the trajectory shaders.

## How Plugin Variations Work

1. **Variation Indices**: Core variations use indices 0-23. Plugin variations start at index 24.

2. **Naming Convention**: Plugin variation functions must be named `variation_plugin_N` where N is the variation index (24+).

3. **Function Signature**: All variations must accept `(p: vec3<f32>, rng: ptr<function, RngState>)` and return `vec3<f32>`.

4. **Dynamic Loading**: When a transform uses a plugin variation (index >= 24), the shader system:
   - Detects the active variation indices
   - Loads the corresponding `.wgsl` files from this directory
   - Injects them into the compiled shader
   - Rebuilds the compute pipeline

## Adding a New Plugin Variation

1. Create a new `.wgsl` file in this directory
2. Implement your variation function with the correct naming:
   ```wgsl
   fn variation_plugin_N(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
       // Your variation code here
       return modified_p;
   }
   ```
3. Register it in the variation plugin registry (future: src/variations/registry.rs)
4. Use it in a transform by setting `transform.variations[N] = weight`

## Example Variations

- `example_curl_3d.wgsl` - 3D curl/swirl effect (index 24)
- `example_blob.wgsl` - Organic blob shapes (index 25)

## Performance Notes

- Shader compilation occurs when the active variation set changes
- Compilation takes ~100-500ms depending on complexity
- Shaders are cached to avoid unnecessary recompilation
- Only variations actively used in transforms are compiled into the shader
