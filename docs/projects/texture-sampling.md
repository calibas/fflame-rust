# Texture Sampling Framework

Project plan for adding texture (user-supplied image) sampling as a variation framework feature. Unblocks the texture/image-sampling family of JWildfire variations and any future hand-written texture-using variations.

This is **blocker #2** from [`variation-port-blockers.md`](variation-port-blockers.md): "Texture / image / SVG sampling."

## Variations unblocked

Direct image sampling — first targets after the framework lands:

| Variation | Source class | Notes |
|---|---|---|
| `colormap_wf` | `AbstractColorMapWFFunc` | Sample image as palette/RGB source; writes color + optional Z displacement from intensity |
| `post_colormap_wf` | `AbstractColorMapWFFunc` | Post-phase form of the same |
| `displacemap_wf` | `AbstractDisplacementMapWFFunc` | Sample image to displace spatial output |
| `post_displacemap_wf` | `AbstractDisplacementMapWFFunc` | Post-phase form |
| `post_bumpmap_wf` | (`PostBumpMapWFFunc`) | Sample image for surface bump displacement |

Plus rasterizer-needing variations that need texture sampling as a prerequisite but also need their own CPU-side rasterizer before they're portable: `text_wf` (font glyph rasterizer), `svg_wf` (SVG path rasterizer), `primitives_wf` (renderable primitive bank). Out of scope for this project — handle each in its own follow-up after the texture framework is in.

## Design

### Storage

User-supplied images live as **files on disk**, referenced by relative path stored in the flame's parameter data. The flame file itself (`.fflame` JSON or `.flame` XML) does not embed image bytes — that's a deliberate choice to keep flame files small (no 10MB embedded images) and to keep the online API's flame storage cheap.

**Path constraint**: paths are **relative to the `.flame` file** and must resolve to a sibling file in the same directory. No `../`, no absolute paths, no paths outside the flame's directory. This is a security choice: a downloaded flame file shouldn't be able to load images from arbitrary filesystem locations.

**File formats**: PNG (primary), JPEG, common raster formats supported by the `image` crate. Decoded to RGBA8 on load. HDR formats (EXR, etc.) deferred until needed.

### Texture pool

Per-flame texture pool with a fixed cap: **8 slots in v1** (can bump to 16 if a real use case demands it; 8 covers every JWildfire flame we have on disk). Each slot holds:

- The original file path (relative, stored on the flame)
- The decoded GPU texture (loaded lazily on first use)
- Width / height metadata for the variation's coordinate-system math

**Per-flame scoping**: slots belong to the flame, not the app session. Loading a new flame allocates a new pool; closing the flame frees the GPU textures. Two flames with the same image file each get their own slot occupancy (no cross-flame slot sharing in v1). A `.fflame` file records its own pool contents.

**Slot assignment**: **implicit**. When a variation references a texture, the app auto-assigns a slot — first free slot, or the existing slot if the same file path is already loaded for this flame. The user doesn't see slot indices in the UI; they pick a file via the file picker on a variation's "Texture" param and the slot bookkeeping happens automatically. The slot index is what gets serialized to the flame file, but it's an implementation detail.

### GPU binding

Eight `texture_2d<f32>` bindings exposed to the shader (group 0, bindings TBD when we wire it up). A flame using only 2 textures still has 8 declared bindings — the unused slots bind to a 1×1 placeholder texture so the WGSL is uniform across flames regardless of how many slots are populated. (Same shape as the `palette_texture` we already have — always bound, sometimes unused.)

One shared `texture_sampler` binding for all slots. Sampler config (address mode, filter mode) is irrelevant for v1 because we use `textureLoad` (integer pixel access) and do manual bilinear in WGSL — see Filtering below.

### WGSL access

Variations read textures via `textureLoad(textures[slot], coord, 0)` — integer pixel coordinates, no mip level. This sidesteps the WGSL spec restriction on `textureSampleLevel` from non-uniform control flow, which the WASM/browser WebGPU implementation enforces strictly. Desktop drivers are lenient but WASM is not, and the variation dispatch is non-uniform control flow by definition (`if xform.variations[idx] != 0.0`).

**Manual bilinear filtering**: each sample reads 4 neighbor texels via `textureLoad`, then blends by `frac(coord)`. JWildfire's CPU code does the same thing (see `AbstractColorMapWFFunc.transform` line 134-170) — so we match their semantics bit-for-bit. ~10 extra ops per sample vs hardware bilinear, which is irrelevant compared to the per-iteration variation work.

**No mip levels**. Single-resolution textures only. Aliasing concerns at extreme zoom-outs are deferred until they show up in real flames.

**Coordinate system** (matches JWildfire `AbstractColorMapWFFunc`):

```
image_x = (input_x - (offset_x + 0.5) + 1.0) / scale_x × (img_width - 2);
image_y = (input_y - (offset_y + 0.5) + 1.0) / scale_y × (img_height - 2);
```

Default `scale=1, offset=0` maps fractal-space `[-0.5, 0.5]²` to the image's `[0, img_width-2] × [0, img_height-2]` pixel range (a centered unit square in fractal coordinates covers the whole image, minus the rightmost / bottommost edge pixel that the bilinear's `+1` neighbor would need). User-controllable `scale_x/y` (zoom into image) and `offset_x/y` (pan image origin in fractal coords).

**Tiling**: per-axis `tile_x`, `tile_y` flags. `1` = wrap-around (modular indexing); `0` = out-of-bounds yields zero color (no contribution). Both modes are a handful of WGSL ops in the variation body, not sampler config.

### Variation system integration

New `Feature` variant: `Feature::ReadsTexture`. A variation declares this when its WGSL body references `textureLoad`. When set:

- The variation gains a `texture_slot: u32` *user parameter* (separate from regular f32 params — this is an integer slot index pointing into the per-flame pool)
- The shader builder gates the texture-binding declarations in the header on `HAS_TEXTURE` (parallel to `HAS_DC` / `HAS_RGB`); flames with no texture-reading variations don't get the texture bindings at all
- Variation params don't change shape — the texture-slot index is a regular f32 param the user touches via a file picker UI

### File path resolution

**Desktop import**:
1. Read the relative path from the flame file.
2. Resolve against the directory of the `.flame` file.
3. Reject if resolved path escapes the flame's directory (canonicalize, check `starts_with` the flame dir).
4. Decode via `image` crate, upload to a free slot, record the slot index on the variation's param.

**Desktop export**: write the relative path. If the user picked a file from outside the flame's directory, prompt to copy it into the flame's directory at save time (or refuse save / suggest a "Save As" that includes the textures).

**WASM**: the relative-path concept doesn't translate to the browser. WASM workflow:
- User uploads images via a file picker per-flame.
- App holds the decoded texture in browser memory for the session.
- Flame XML written from WASM records a synthetic / placeholder filename so the file still parses round-trip.
- Reopening the same flame in WASM requires re-uploading the same images. Worth a one-time onboarding hint.
- Reopening in desktop after a WASM save: works if the user dropped the files into the desktop flame's directory.

### JWildfire import: inlined images

JWildfire `.flame` XML supports two image-supply modes:
- `image_filename="path"` — file reference
- `inlined_image="<base64>"` — embedded base64 bytes

We support file references natively. For **inlined images** on import: decode the base64 into memory, allocate a slot from the per-flame pool, hold the decoded texture in memory (no file written to disk). Synthesize a placeholder filename for the slot's path field. On *re-export* from our app: write the original file path if we have one, or fall back to `inlined_image` for slots that came in inlined. This keeps round-trip lossless without ever writing user image data to disk silently.

(Why not write the inlined image to a temp file? Disk-write side effects on import would be surprising — a user opens a flame to look at it and a file appears in their textures directory. Holding-in-memory is more predictable.)

## Out of scope (v1)

Deliberately deferred:

- **Image sequences** (`is_sequence`, `sequence_start`, `sequence_digits` in JWF) — per-frame image swap for animation, useful for animated colormaps. Adds a frame-aware texture-loading path; defer until animation export tooling needs it.
- **HDR image formats** (EXR, 32-bit float). RGBA8 only in v1.
- **Mip levels** and trilinear filtering. Single resolution.
- **Cross-flame slot sharing**. Each flame's pool is independent. If a user keeps reusing the same image across flames, the decode+upload cost happens per flame load. Acceptable for v1.
- **Bindless / dynamic-slot-count texture arrays**. Fixed 8 slots is simpler and covers known use cases.
- **`with_alpha` → `doHide`** — the JWildfire pattern of dropping a plot when the sampled pixel alpha is below a threshold. We don't have a per-iteration plot-skip mechanism in our framework. Could be added later as a separate `Feature::CanSkipPlot` (or similar). For v1, ignore alpha; treat all-opaque.
- **Z-from-intensity** (the JWildfire colormap_wf side effect that writes Z based on RGB luminance). Our variations already control Z directly via vec3 return values in 3D mode, so the equivalent expression lives in the variation body — `return vec3(x_out, y_out, scale_z × luminance + offset_z)` — and the user gets the same effect by setting `scale_z`. No framework change needed.
- **Palette-index color output mode** (JWildfire's `dc_color > 0`: find the palette entry closest to the sampled RGB and write its index). We already have `Feature::WritesRgb` for direct RGB output, which covers the common case. The palette-index mode would require a per-sample nearest-color search through the palette texture — implementable but defer until a real port needs it. Same compromise we made for `gradient = 1` in the `glsl_*` family.

## Implementation phases

Roughly the order things have to land:

1. **Texture pool infrastructure**
   - `FractalConfig.texture_slots: [Option<TextureSlot>; 8]` field with serde defaults
   - `TextureSlot { path: PathBuf, width: u32, height: u32 }` (decoded bytes held separately at runtime)
   - File-path validation helper (same-dir-only check)
   - PNG/JPEG decode via `image` crate
   - GPU texture upload + binding setup in `shader_builder_v2` / `compute_kernel`

2. **`Feature::ReadsTexture` plumbing**
   - Add the enum variant
   - Detect `has_texture_variation` in the shader builder
   - `HAS_TEXTURE` template flag → conditional texture bindings in header
   - Texture-slot param plumbed as a regular f32 param the variation reads

3. **Reference variation port: `colormap_wf` (RGB mode only)**
   - Coordinate-system math (centered unit square, scale/offset)
   - Manual bilinear via 4× `textureLoad`
   - Per-axis tile / clamp
   - Writes RGB via existing `Feature::WritesRgb`
   - Smoke render

4. **`displacemap_wf` + `post_displacemap_wf`**
   - Same texture-sampling infrastructure
   - Displace spatial output instead of writing color

5. **`post_bumpmap_wf`**
   - Sample image, modulate Z (3D mode only)

6. **`.flame` XML import — inlined image support**
   - Base64 decode into the slot pool, in memory only
   - Round-trip: re-export as `inlined_image` if path field is the synthetic placeholder

7. **WASM workflow**
   - Per-flame file upload dialog
   - Texture cache holds decoded images in browser memory
   - UX polish for the "re-upload on reopen" pattern

## Open questions for later

- **Texture slot UI**: what does the variation parameter look like in the panel? A "Choose File…" button? Drag-and-drop? Preview thumbnail? Defer until step 3 lands and we see one variation working.
- **Same-dir constraint on save**: if a user picks a file from elsewhere, do we silently copy it, prompt to copy, or refuse? Currently leaning prompt-to-copy.
- **Round-trip with JWF when JWF used an absolute path**: many flames in the wild reference images by absolute path. Our same-dir constraint means we'd reject those on import. We could fall back to "prompt user to locate the missing image" UX. Worth thinking about before users hit it.
- **Memory budget**: 8 slots × `~max` (say 4K×4K RGBA8 = 64MB) = 512MB worst case. Almost certainly we'd never hit that in practice (typical JWF colormap images are 1K×1K or smaller), but worth being aware. Could add a per-slot byte cap with friendly error.

## Related docs

- [`variation-port-blockers.md`](variation-port-blockers.md) — blocker #2 is what this project resolves.
- [`jwf-features.md`](jwf-features.md) — companion doc for non-variation JWF features we don't yet support.
- [`jwf-common-variations-port.md`](jwf-common-variations-port.md) — variation port tracking; the texture-dependent variations listed there will move to "shipped" as this project lands them.
