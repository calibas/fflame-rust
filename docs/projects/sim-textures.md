# Simulation textures: generated images for the escape engine

Status: **planned**, branch `escape-textures` (off `escape-coloring`),
2026-10-03. This is the plan being followed, not a summary of it.

## Why

Image textures were skipped (survey R10, 2026-10-02) because every way of
carrying an image file with a picture has a major drawback:
- embedding bloats the `.fflame`;
- a path breaks on other machines, on the web build and in online sync;
- installed builds make `assets/` read-only;
- a shared file's path must be confined, or it could read a private
  local image.

A **simulation still** sidesteps all of it. It is a small recipe,
`(model, params, seed, init, steps)`, that reproduces its field exactly
(`steps_are_batch_invariant`). So a texture is a few numbers in the
config, not a file. Two properties also suit textures:
- the `Periodic` boundary tiles seamlessly;
- the grid is the information, so a fixed grid upscales to any size.

## Decisions (2026-10-03)

1. **Escape first.** Flames later, wherever their image textures go.
2. **Three uses**, from one texture:
   - **Texture overlay (R10)**, as KF2 draws it: the texture in screen
     space, warped by the local slope, mixed into the colour before the
     relief lights it.
   - **Relief bump source**: the texture as micro-relief, beside the
     procedural grain and paper.
   - **Image orbit traps**, as UF's image trap and Visions of Chaos's
     bitmap traps do it: the texture sampled where the orbit lands.
3. **A Texture panel** lists textures: lightweight shipped presets plus
   whatever the user saves. Picking one puts it in the current escape
   config.
4. **Simulation mode gets "Save as texture"**, which adds the current
   simulation to that list.
5. **Textures are config files.** A texture is a full `FractalConfig` in
   Simulation mode, so it renders exactly as Simulation mode draws it:
   palette, colouring, tone map.
6. **A config that uses a texture embeds the texture's config in full**,
   so the parent file is self-contained and needs nothing else to
   reproduce.
7. **The cache is a PNG** of the generated texture, keyed by the
   recipe's hash. It is derived data: when it is missing, it is
   regenerated.

## The texture

- **Recipe:** a `FractalConfig` with `render_mode = Simulation`.
- **Size:** its grid, which must be `Fixed` (cells). "Save as texture"
  fixes a viewport-scaled grid at its current size, capped at 2048 on a
  side.
- **Render:** the existing headless path (`render_with`), at exactly the
  grid size. Letterbox at a matching aspect means no bars, so a Periodic
  run tiles.
- **On the GPU:** one `Rgba8UnormSrgb` texture with a repeating linear
  sampler, sampled in linear light. The relief and the overlay warp read
  its luminance; the overlay and the traps read its colour.

### Where textures live

| | Shipped presets | User textures | PNG cache |
|---|---|---|---|
| Desktop | `assets/textures/*.fflame`, folder scanned at startup (the palette-pack pattern) | data dir `textures/<name>.fflame` | data dir `texture_cache/<hash>.png` |
| Web | embedded in the binary by `build.rs` (small JSON) | browser storage via the storage backend (small JSON, as custom palettes) | in memory, per session |

**Cache key:** SHA-256 of the recipe's canonical JSON, the size, and a
`TEXTURE_CACHE_VERSION` constant, bumped when the simulation engine's
output changes (and with the crate version). A stale or corrupt entry is
regenerated.

### Config

```text
EscapeConfig.texture: Option<EscapeTexture>   // skip if None
EscapeTexture {
    name: String,              // what the panel shows
    config: Box<FractalConfig> // the recipe, in full (decision 6)
}
```

The three uses' settings sit with what they extend:
- overlay: `EscapeConfig.texture_overlay` (KF2's enable, merge, power,
  ratio);
- relief: a `ShadingTexture::Image` kind with its scale;
- traps: a colouring's parameters.

## Generation

Generation happens when a config with a texture loads, or when one is
picked.
1. Look up the cache.
2. If missing, run the recipe through `render_with` (the simulation
   engine's own watchdog-safe batching), write the PNG, and upload.

**Cost** scales with steps × grid. Shipped simulation presets run from
200 to 2,000,000 steps. From the measured ~0.5 ms/step on a 1080p grid,
a 512² texture is estimated at about a second per 10k steps (to be
measured in phase 1). Texture presets are chosen to generate in about a
second. A long user recipe shows progress, and the picture renders
without the texture until it is ready.

**Reproducibility:** exact on one machine. The simulation renders in the
visual suite match on macOS, but chaotic models could amplify GPU
differences. This is the same class of caveat as the escape engine's
macOS divergence.

## The uses

1. **Overlay (R10).** KF2's `KF_TextureWarp` and texture block, read
   from source (`gl/kf.frag.glsl`), recorded in the escape-coloring
   survey's R10 note:
   - the warp comes from the 3×3 neighbourhood of the iteration value
     (`pow(1 + d, power)`, inverted below 1, mapped through
     `(atan(x) − π/4)/(π/4)`, scaled by `ratio/100`);
   - offset `power/64 ± power·that`;
   - `mix(colour, texture, merge)` before the slope shading;
   - with a texture on, the interior is coloured too.

   It runs in the shade pass. Our height texture already holds the
   iteration value KF2 differences.

   **As built (phase 2).** Settings: `TextureOverlay { enabled, merge,
   power, ratio, fit, tile_scale }`. Merge 1, power 200 and ratio 100
   are KF2's defaults, and Stretch is KF2's fit. Where it departs from
   KF2:
   - **The value it differences.** The iterate and recolour passes
     store the smooth count in the height texture's blue channel (flags
     bit 8, `esc_overlay_field`). Two cases fall back to the relief's
     own source (green): field formulas and the IFS walk, which have no
     count, and a lit stored-slope relief (analytic, offset, embossed),
     which already has blue.
   - **Display pixels.** Under supersampling it differences and pushes
     whole output pixels, so antialiasing smooths the picture rather
     than halving the warp (our convention since R9).
   - **Mixing.** The mix is in display values, as KF2's is, through a
     2.2 power, because the accumulator is linear. The interior takes
     the texture with coverage `merge`, composited over the background
     by the tone map. KF2 mixes the texture into its interior colour.
   - **Tile** is ours: the texture repeated at `tile_scale` display
     pixels per texel, so a periodic simulation tiles seamlessly.
   - **Edges** follow KF2. A neighbour off the image is mirrored, as
     `getN3x3` reflects. A Stretch lookup past the edge holds the edge,
     as the CPU path clamps.

   Off, it costs nothing: bit 8 clear writes the height texture exactly
   as before, and the resolve binds a 1×1 stand-in. The config carries
   the recipe. `render_with` obtains the image (cache, or generation)
   before an escape render, so the CLI, thumbnails and video inherit
   it. The app re-checks only on frames that re-render; desktop does
   the check blocking, the web spawns it.

   **Measured** (`the_texture_overlay_warps_as_kalles_fraktaler_does`):
   - The test feeds a smooth synthetic texture through the whole render
     and tone map, and compares against an f32 port of `KF_TextureWarp`
     and a bilinear repeating sampler, with the count read back from the
     GPU. In all three cases every one of the 12,288 pixels is within
     one level: Tile at power 200 / ratio 100, Tile at 37 / 60 with
     tile scale 2.5, and Stretch at 200 / 100. Every exterior lookup is
     pushed more than two pixels.
   - Merge 0 is byte-identical to the overlay off.
   - 2× antialiasing differs from 1× by 1.99 levels on average, against
     57.4 for a texture at the wrong scale.

   Two visual tests: KF2's defaults, and a tiled half-merge under a lit
   relief.
2. **Relief bump.** A new `ShadingTexture` kind next to Grain and Paper:
   the texture's luminance as micro-relief, its gradient added to the
   tilt exactly as grain and paper are. Screen space, with a scale in
   display pixels per texel. This one is ours; no foreign program has it.

   **As built (phase 3).** `ShadingTexture::Simulation` reuses the
   surface texture's strength and scale. It samples the image the
   overlay binds: luminance (Rec. 709, display values) − 0.5, repeating,
   `texture_scale` display pixels per texel. Without a texture it is
   none. The Texture panel offers it as "Relief bump", the same setting
   as the Escape panel's surface texture. `EscapeConfig::uses_texture`
   (the overlay, or the bump under a lit relief) decides whether the
   image is obtained at all.

   **Measured** (`the_relief_bump_is_the_textures_luminance`):
   - Setup: a view wholly outside the set, a flat grey palette, relief
     height 0, so the shading is the bump alone.
   - The shading's departure from flat grey correlates 0.9991 with the
     shader's response formula on the CPU-sampled texture. The same
     formula with the light turned a quarter gives −0.97, and at the
     wrong scale −0.01.
   - 2× antialiasing gives 0.995 against the 1× formula.
   - A flat texture, and a config without a texture, are byte-identical
     to no bump.

   One visual test.
3. **Image orbit traps.** A colouring that samples the texture at orbit
   points mapped into texture space (position, scale, rotation in the
   plane). **Semantics to be read from source before implementing:**
   UF's image trap shape (its Standard/common library) and Visions of
   Chaos's bitmap orbit traps. If the sample is taken inside the loop,
   then like direct traps the texture belongs to the iteration's
   identity (`PaletteInLoop`'s pattern).

## UI

- **Texture panel** (escape mode, `visibility.rs`):
  - the list, presets then user textures, each with a thumbnail from the
    cache;
  - pick, clear, rename and delete (user textures only);
  - generation progress.
- **Simulation panel:** "Save as texture" asks for a name and writes the
  current config to the user textures, where it appears in the list.

## Phases

1. **Library, generation, cache, panel, "Save as texture".** No consumer
   yet; the panel previews. Tests:
   - a recipe round-trips through the config, embedded;
   - generation is deterministic (two runs, identical bytes);
   - the cache hits, and misses after a recipe edit;
   - a Periodic texture tiles (opposite edges continue).
2. **Overlay (R10)**, against a CPU port of `KF_TextureWarp`. Done; see
   the uses above.
3. **Relief bump.** Done; see the uses above.
4. **Image orbit traps**, after reading the sources.

Each phase gets the usual gates, its own tests and a visual test. The API
sees new config fields, so the contract procedure in `docs/RELEASE.md`
applies.

## Open

- WASM storage for the PNG cache is per session for now; IndexedDB if
  generation time makes reloads painful.
- Several textures per config (one per use) if one turns out to be
  limiting; v1 has one slot.
- Gallery modules: an escape-only module that renders textured configs
  has to link `engine-sim`.
