# Code findings from the October 2026 doc review

**Status 2026-10-11:** fixed on branch `bugfixes`, except the items under
*Still open* below. The last column of each table says which. Every
"confirmed" item, and every other item after a verification pass, was
checked against the code first; the fixes carry regression tests where
one could be written.

## Still open

From this list:
- **R12** -- `fx_priority` on a subflame's own transforms is never
  collected, and a parent's ungated move also applies inside the subflame
  dispatcher. A moved pre/post variation in 3D does not get the normal
  phase's z gating.
- **A13** -- the effect compile cache is keyed by name; harmless until
  something can update an installed effect.
- **B6** -- the deep-zoom gates read gitignored fixtures.

Found while fixing:
- `density_blur.wgsl` compared the raw hit count with a 0-1 threshold, so
  a pixel with a hit or more never blurred, and its intended unit was
  recorded nowhere. Fixed: threshold and falloff are multiples of the
  frame's mean density, the unit Levels uses, passed to density effects
  as `density_mean`.
- Escape fog decodes the background with `pow(2.2)`, which matches the
  tonemap exactly only at exposure 1 and gamma 1.
- With online features off, web deep links (`?flame=`) are still
  followed.
- `FlameRenderer::resize` replaces its output texture without destroying
  the old one: 4 bytes a pixel per resize waits for the garbage collector
  on WebGPU. egui keeps its own registration of that texture, so it needs
  a deferral that outlives the frame's paint, not the buffers' retire list.
- The web build has warnings the desktop build no longer has:
  deprecated web-sys calls and desktop-only code compiled for wasm32.

The docs/main refresh (branch `docs-housekeeping`) read most of the code
again to describe it. These are the things that looked wrong along the
way: likely bugs, gaps between the code and its own UI or comments, dead
code and stale comments. The docs describe the code as it is, bugs
included, and several of them name the relevant item under their own
"Gotchas" or "Known gaps".

**Confirmed** means the code was read and the defect is there. Everything
else was found by reading the code, not by reproducing it, and needs a
check before it is fixed.

## Rendering

| # | What | Where | Status |
|---|---|---|---|
| R1 | **Mode D solids rank the beam by the material's gloss.** `pack_globals3` puts the beam's ranking-key flag in `fdata[3].w`. The renderer then writes gloss into the same slot, because `ifs_gloss()` reads it. The solid's beam (`ifs_weighted_key()`) therefore ranks unweighted at the default gloss of 0, even for an all-inversion flame the CPU seeds with the weighted key. Any gloss above 0.5 turns the weighted key on for affine flames. | `src/escape/ifs.rs` `pack_globals3`; `src/escape/renderer.rs` ~7876; `assembler.rs` `ifs_weighted_key`, `ifs_gloss` | fixed |
| R2 | **Edge pixels collect two pixels' worth.** `world_to_pixel` and `project_3d_full` convert with `i32()`, which truncates toward zero, so samples at pixel coordinates in (-1, 0) land in column/row 0. flam3 rejects them before truncating. `floor` is the fix; the DoF jitter already uses `round()` for the same reason. | `shaders/core/utilities.wgsl` | fixed |
| R3 | **Density effects can overflow.** The density ping-pong is `Rgba16Float`, but it carries the accumulator layout, whose alpha is a raw hit count; past 65504 hits a pixel f16 is infinity. `high_res.rs` records the same failure ("black holes") for its old accumulator. | `src/renderer/effect_chain.rs` `ensure_textures` | fixed |
| R4 | **Fixed Blend Rate 0.99-1.0 fades the image.** `accumulate.wgsl` treats `blend_factor >= 0.99` as overwrite, while the host keeps adding to `samples_in_buffer`, so `sample_density` grows over a one-batch accumulation. The slider runs to 1.0. | `shaders/accumulate.wgsl`; `compute_kernel.rs` ~1394; `src/ui/settings.rs:189` | fixed |
| R5 | **Depth fog blends toward the sRGB background.** Fog mixes toward `params.background_r/g/b` undecoded, while the tonemap decodes the background to linear, so a grey or coloured background fogs toward a lighter colour than it shows. | `shaders/core/main_template.wgsl` ~1231 | fixed |
| R6 | **`Palette::sample_color` extrapolates below the first stop** instead of holding its colour (the stepped version holds). The upload clamps the result. | `src/scene/palette.rs:303-340` | fixed |
| R7 | **The 33rd effect pass in a frame reads past its uniform buffer.** `allocate_slot` clamps the counter to 32 but returns `32 * 256`, the buffer's size. `distance_field` uses about `log2(max dimension) + 2` slots. | `src/renderer/effect_chain.rs` | fixed |
| R8 | **`density_blur.wgsl` calls `textureSample` after a data-dependent early return**, which browsers reject (WASM rule in CLAUDE.md). Not tried in a browser. | `shaders/effects/density/density_blur.wgsl` | fixed |
| R9 | **Turning on post-symmetry keeps a Focused Rendering plan.** Neither `flame_key` nor `enumeration_key` hashes `post_symmetry`, so the old plan stays instead of declining with `NoCylinders::PostSymmetry`. | `src/renderer/compute_kernel.rs` | fixed |
| R10 | **More than 128 transforms panics.** `update_transforms` panics past 128 normals + linked + finals. Nothing earlier on import or load validates the count. | `src/gpu/buffers.rs` | fixed |
| R11 | **u32 overflow in buffer sizes**: the histogram and path-buffer sizes are `width * height * 16` in u32, which wraps at 16384². | `FlameBuffers::with_palette_size`, `create_path_buffers` | fixed |
| R12 | **Phase overrides.** These look like problems:<br>- The pre-phase call hard-codes `variation_{name}` instead of `info.wgsl_function`, so `julia` moved to pre emits a call to a function that does not exist.<br>- `vrc` is missing in the post phase (2D, 3D) and the 3D pre phase.<br>- `fx_priority` on a Linked or Final transform never reaches the shader: `collect_phase_overrides` reads only the normal transforms. | `src/shader_builder_v2.rs` | fixed (subflame transforms' fx_priority: open) |
| R13 | **Subflame prefuse is 20 iterations**, though the header says JWildfire's 42 and "we do the same". Inside a subflame the post-Final point is what the state keeps, so subflame Finals feed the next step, unlike the parent's. Check against JWF's `SubFlameWFFunc`. | `shaders/core/subflame.wgsl` | fixed: finals, prefuse, NaN re-prefuse; then opacity, the weight, the finals' colour and `pre_subflame_wf`'s placement |
| R14 | **`render_with` counts dispatched iterations** (burn-in included) where the app counts plotted ones (`total_iterations()`). | `src/renderer/render.rs` | fixed |
| R15 | **`load_config` checks the render mode before updating it.** It tests `current_render_mode.is_non_flame()` for the path buffers before setting the mode from the config. | `src/renderer/compute_kernel.rs` | fixed |

## Config, files and settings

| # | What | Where | Status |
|---|---|---|---|
| C1 | **Levels gamma 1.0 does not survive a save.** `is_default_levels_gamma` compares against 1.0, but the default is 0.5. A config at 1.0 is saved without the field and reloads at 0.5. This hits the High Contrast, Saturated Dim and Pop presets' values. | `src/config/fractal_config.rs:995` | fixed |
| C2 | **A PNG's embedded config reloads as v0.** The `Config` chunk is plain serde with no `version`. `FractalConfig::from_json` treats it as v0, and the v2->v3 arm forces a 3D flame to 2D, perspective 0, `preserve_z` true. This affects `scripts/png_metadata.py -o`. (`from_json_value_with_default_version` exists for exactly this case.) | `src/png_metadata.rs` `from_app_state` | fixed |
| C3 | The v1->v2 migration never applied its two translations (in CLAUDE.md's Known Issues). | [config-v2-migration.md](../archive/projects/config-v2-migration.md) | closed: no value migration, by decision |
| C4 | The chosen language is not persisted: `SystemSettings.language` is never written or read (in CLAUDE.md's Known Issues). | `src/ui/menu_bar.rs` | fixed |
| C5 | **`config.set` cannot reach a settings group that is still at its defaults.** The path is walked through the saved JSON, which leaves default groups out. For example, `config.set("sim.use_transforms", true)` after `sim.model(...)` fails with "unknown setting group `sim`". So a script can't set up sim terrain, view, warp, layers or escape textures. | `src/script/api.rs` `set_config_field` | fixed |
| C6 | **`--category` does nothing to the file**: `to_text_chunks` never writes `test_category`. | `src/png_metadata.rs` | fixed |
| C7 | **XML import quantises transform colour** to 1/255 (`(v*255) as usize / 255`), but only when the file has a `<palette>`. Export writes full precision, so values drift on a round trip. | `src/flame_xml.rs` | fixed |
| C8 | **Desktop Load .palette writes the deprecated `ConfigPath::PaletteIndex`**, which ConfigManager ignores: the renderer shows the palette, but a save or export carries the old one. | `src/app/ui_handlers.rs` `handle_palette_operations` | fixed |
| C9 | **A malformed `assets/presets.fflame`** on desktop gives the single "Default" preset instead of falling back to the embedded copy. | presets loading | fixed |

## Online, animation and audio

| # | What | Where | Status |
|---|---|---|---|
| A1 | **Simulations are not refused on Save Online.** The `ApiRenderMode::Simulation` doc says `sync` refuses them, but nothing does. They upload as `"simulation"`, which the committed `openapi.json` `RenderMode` enum lacks. | `src/api/types.rs`, `src/api/sync.rs` | fixed |
| A2 | **Unchecking "Make public" on a flame update leaves it public.** No visibility is sent and `update_flame` re-sends the current one, while local state records the flame as private. | `src/app/ui_handlers.rs` `handle_save_online`; `src/api/mod.rs` | fixed |
| A3 | **Boolean tracks never apply.** The track editor gives Effect Enabled and PostAffineEnabled numeric keyframes, but `json_to_config_value` accepts only JSON bools for them. The parity tests skip these targets. | `src/ui/track_editor.rs`; `src/config/delta.rs` | fixed |
| A4 | **New Zoom tracks start Linear**: `initialize_preview_keyframes` resets the picker's Exponential. | `src/ui/track_editor.rs` | fixed |
| A5 | **Signals in video export.** Several problems:<br>- The CLI passes no signals (a TODO), so every signal track is skipped, generator signals included.<br>- The CLI also ignores the `.anim`'s `base_config`.<br>- Live signals are never exported.<br>- Smoothing assumes 1/60 s steps whatever the fps.<br>- The audio offset shifts only the muxed audio, not the signals. | `src/lib.rs` `export_animation_async`; `src/animation/export.rs` | fixed |
| A6 | **Generator signals go stale.** They are not regenerated when the duration changes, renaming one leaves the old signal, and "Analyze" does nothing. | `src/ui/signal_panel.rs` | fixed |
| A7 | **Ogg Vorbis can't be decoded**: symphonia has the `ogg` container but no Vorbis codec feature, while the pickers offer `.ogg`. | `Cargo.toml` | fixed |
| A8 | **A 403 in the Fractal Browser's Online tab signs the user out** (`is_auth_error` treats "Access denied" as an expired session). The delete X has no confirmation. | `src/ui/fractal_browser.rs` | fixed |
| A9 | **Private animations from the server list fail on desktop**: `load_from_url` sends an empty Bearer token. An animation's stale `base_config` also wins over the server's current flame. | `src/app/ui_handlers.rs` | fixed |
| A10 | **A broken track can write to the wrong item.** The apply path ignores the binding, so a stale in-range index writes to whatever item now sits there, against the "won't apply until rebound" tooltip. | animation apply path | fixed |
| A11 | **AES-GCM nonce reuse**: the email and password are encrypted under the same key and nonce. The key is hard-coded anyway. | `src/storage/credentials.rs` | fixed |
| A12 | **Plugins.**<br>- `plugins::install` accepts a built-in variation's name (`probe.has` is true for built-ins), writes the file, and it is refused only at the next start.<br>- `remove_by_name` leaves the name in `ordered_names`. | `src/storage/plugins.rs`; `src/variations/mod.rs` | fixed |
| A13 | **The effect compile cache is keyed by name only**, so an updated downloaded effect keeps its old pipeline. | `src/renderer/effect_chain.rs` `compiled_effects` | open (latent) |

## UI

| # | What | Where | Status |
|---|---|---|---|
| U1 | **The thumbnail cache never evicts**: `enforce_cache_limit` looks for `.png`, the cache writes `.jpg`. `clear()` also deletes `.png` names. | `src/storage/thumbnail_cache.rs` | fixed |
| U2 | **A long non-ASCII name can panic** the gallery card: `&name[..n]` slices by bytes. | `src/ui/fractal_gallery.rs` `render_grid_card` | fixed |
| U3 | **The Fractal Browser predates the escape and simulation engines.**<br>- The online filter has only All/2D/3D.<br>- A Simulation row reads "0T \|".<br>- The list view shows flame-only details for escape and sim configs. | `src/ui/fractal_browser.rs`, `fractal_gallery.rs` | fixed |
| U4 | **A multi-flame `.flame` from File > Open lands in the Random tab**, though the comments say Files. | `src/app/ui_handlers.rs` (`load_batch_into_fractal_browser` calls) | fixed |
| U5 | **On the web, enabled palette packs are fetched only once the Palette Library panel is drawn.** Until then their palettes are missing from the Colors dropdown and random picks. | `src/ui/palette_library.rs` | fixed |
| U6 | **The tone curve's "Ctrl+click to remove" is not implemented** (`ToneCurve::remove_point` has no caller). | `src/ui/tone_mapping.rs`; `locales/en.yml` | fixed |
| U7 | **The gamma slider allows -1 to 10.** At 0 alpha is 1 everywhere; below 0 the curve inverts. | `src/ui/tone_mapping.rs:262` | fixed |
| U8 | **The Colors panel.**<br>- Reset to Defaults skips `white_level`, `highlight_mode`, `levels_enabled` and the squeeze settings.<br>- PathMap hides the palette controls every PathMap style reads.<br>- The Transforms panel's colour swatch ignores the palette transforms. | `src/ui/mod.rs:2702` | fixed |
| U9 | **The Effects panel.**<br>- Rows show raw keys (`sobel_edges`), not the curated labels.<br>- Boolean, Integer and Angle parameters render as float sliders.<br>- Focused Rendering refusals number transforms from 0 while the Paths panel says T1. | `src/ui/effects_panel.rs` | fixed |
| U10 | **Wrong tooltips.**<br>- `color_speed`: "0 = no change"; it is 1 that leaves the colour unchanged.<br>- `tooltip_speed_factor`: "0 = pure transform colors"; at 0 the colour stays white.<br>- `tooltip_brightness`: says the default is 1.0; it is 4.0.<br>- The PathMap tooltip describes the removed path history.<br>- `terrain_tier_tip`: "exports are path traced"; a Lit-tier export is lit.<br>- The greyed Solid & Lighting reason says "3D flame mode only". | `locales/en.yml` | fixed |
| U11 | **The translations are stale.** es, ja and zh-CN still carry `menu.fractal` and lack `menu.mode`, plus 52-120 keys English no longer has. | `locales/*.yml` | fixed |
| U12 | Reset Workspace always applies the Standard layout, even in Escape or Simulation. `online_mode` has no UI toggle, and the variation fetches ignore it. | | fixed |

## Exports and GPU memory

| # | What | Where | Status |
|---|---|---|---|
| E1 | **Escape and simulation exports are refused on a stale premise.** `TooLarge` and `export_headless` still refuse above one storage binding, as if the tail allocated a histogram. `render_with` now gives them `for_tail`. | `src/export/mod.rs` `route_custom_size_export`; `src/app/export.rs` | fixed |
| E2 | **The video loop builds the full `FlameRenderer::with_palette_size` for every engine**, at 64 bytes a pixel of unused flame state for escape, sim and terrain videos. | `src/animation/export.rs:1920` | fixed |
| E3 | **`histogram_size_bytes` undercounts solids.** It ignores the 16 MiB shadow-map tail, and the routing tests only `solid_strength > 0`. | `src/export/mod.rs` | fixed |
| E4 | **The web custom-size export is its own copy of the render loop.** It skips the shade pass, DoF, density effects and the exact density fraction. It also leaves sticky shaders on, hard-codes burn-in to 20 and never frees its `SimRenderer`. | `src/app/mod.rs` ~2330-2780 | fixed: it calls `render()`; `render_with`'s exact reads are awaited, escape chunks are fixed on the web, and `render()` frees its iteration buffers before colour effects. Not yet run in a browser |
| E5 | **GPU memory leaks on WebGPU**, where dropping a buffer frees nothing.<br>- `FlameBuffers::destroy` skips the orbit buffers.<br>- `FlameRenderer::destroy` skips the shade and DoF textures.<br>- `resize`, `set_solid_depth_region`, `set_census_region`, `resize_palette`, `drop_xaos_buffer` and `drop_path_buffers` replace buffers without destroying them.<br>- `SimRenderer` has no `destroy`.<br>- `render_escape`'s cancel paths skip the destroy. | `src/gpu/buffers.rs`, `src/renderer/` | fixed; the buffers replaced mid-update go through a retire list, destroyed two dispatches later |
| E6 | `WasmApi.set_target_iterations` has no effect on `export_png`, which has no premultiplied option. `get_current_iterations()` and `get_progress()` are never advanced. | `src/wasm_api.rs` | fixed |
| E7 | **Sim grid sizing under 2x AA**: `supersample_plan` sizes a bound sim grid without `SimGrid::cells_for`'s 0.125-4 clamp. `export-animation` says an unknown codec falls back to H.264, though the default is H.265. | `src/export/supersample.rs`; `src/lib.rs` | fixed |

## Build, release and tests

| # | What | Where | Status |
|---|---|---|---|
| B1 | **`wasm/render` enables only `engine-flame` and `engine-escape`**, so simulations return `EngineMissing`. Its README says "any render mode" and that trajectories don't survive dispatches. | `wasm/render/Cargo.toml`, `wasm/README.md` | fixed |
| B2 | **`release.py version` skips several crates** (`wasm/flame`, `wasm/escape`, `wasm/sim` and `python/Cargo.toml`, still 0.1.0 against `pyproject.toml`'s 0.5.0). It writes files one at a time. `release.py build` builds only render and script, and no gate runs the other gallery crates' smoke tests. | `scripts/release.py` | fixed |
| B3 | **The WASM palette packs may 404.** `load_pack` fetches `assets/palettes/packs/<file>` relative to the page, but the build copies packs to `pkg/assets/...`. Unverified on the live site. | `src/resources/`, build scripts | fixed |
| B4 | **Build and config mismatches.**<br>- `build-wasm --symbols` hands `wasm-locate.py` a file it doesn't read.<br>- `.cargo/config.toml` says 16 MiB of stack but sets 64 MiB.<br>- `Cargo.toml` duplicates the `[profile.dist-symbols]` comment. | | fixed |
| B5 | **The test scripts.**<br>- `test_wasm.py` is Windows-only, names tests without their category (the `affine3d-smoke` collision) and skips sim and solid.<br>- `run_benchmarks.py` compares against 160x120 thumbnails, so it can never match.<br>- `run_tests.py --update-baseline` considers stale PNGs from other categories, and its comments still say 148 tests. | `tests/visual/` | fixed |
| B6 | **The deep-zoom gates read `output/flame-zoom/*.fflame`**, which is gitignored, so they can't run from a fresh clone. | deep-zoom tests | open |
| B7 | **`test_load_presets` always passes.** | | fixed |
| B8 | **The contract's hand-written lists.** `categories` and `param_types` are written out by hand, so a new one won't appear at all. The helper-library table omits `polyhedra.wgsl` and `polychora.wgsl`, and `crackle_fast`'s use of `noise.wgsl`. | `src/contract.rs` | fixed |
| B9 | `python/README` and `src/script/cli.rs` name the binary `fractal_flame_wgpu`; it is `FractalArtEditor`. | | fixed |
| B10 | `tests/visual/` has stray tracked files: `diff.png`, `new_spherical_256.png`, `test_desktop.png`, and old `current/` subfolders. | | fixed |

## Dead code

Status: removed, or compiled only for tests or the web, in the warnings
pass (the desktop build is warning-free); `src/undo.rs` deleted. Items
here that the compiler cannot see (public functions nothing calls) may
remain.

- `src/undo.rs` (not in `lib.rs`); `camera::Viewport`; `GpuProfiler`
  (`src/profiler.rs`); `effective_iterations()`; `select_transform` in
  `utilities.wgsl`.
- `capture_pixels`, `capture_from_accumulation_buffer`,
  `capture_from_tonemap_render` (`compute_kernel.rs`).
- `UpdateAction.rebuild_shader`: never set, and `process_gpu_updates`
  ignores it anyway.
- `ConfigManager.preview`: never `Some`, so `ConfigSlider::lazy` and
  `force_commit_preview` do nothing.
- `SystemSettings.recent_files`, `last_opened_file` and
  `default_save_location` are never used.
- `RenderModeFSM`'s brightness boost: it reaches `update_tonemap` as
  `is_live_preview`, which never reads it (five of its parameters are
  unused).
- `detect_linked_chains` and `unlink_chain_changes` have no callers
  outside `xaos.rs`.
- The `HAS_POST_AFFINE` template block never compiles in, and the
  inlined-constants output is mostly unused.
- `shaders/downsample.wgsl` and `shaders/accumulate_tiled.wgsl` are
  referenced by nothing.
- Tonemap bindings 5-7 are unused.
- Several API client methods are never called.

## Stale comments

Status: some were corrected alongside the fixes above; most remain.
The docs no longer repeat these; the comments still do.

- **Features that don't exist.**
  - `src/audio` mentions an `audio` feature; `src/api` and `fractal_browser.rs` an `api` feature.
  - `index.html` mentions the `api` feature too.
  - The storage module mentions IndexedDB and auto-backup.
  - The `fractal_gallery.rs` module doc names User/Backup Library panels.
  - `font_loader.rs` names unshipped zh-TW, ko, ar and he fonts.
- **Engines and modes that have moved on.**
  - `src/app/input.rs:88` and the `menu_bar.rs` View menu say a simulation has no view.
  - `render_escape_terrain`'s doc and `app/mod.rs` ~3089 describe one footprint tile.
  - `PATH_WGSL` calls the environment uniform (there is a gradient sky).
  - `terrain.rs` says the surface is only bilinear.
  - The `FlyCameraMode` doc predates the roll hold.
  - `mobius.rs` and `cylinder.rs` say family M is gated off; `ENABLED` is `true` since cc1f283c.
  - `cylinder.rs` says "affine transforms only".
  - `bound.rs` says `plan` turns every nonlinear flame away.
- **The flame pipeline.**
  - `header.wgsl`: binding 8 "unused" (it is the orbit buffer); the path-storage note sits above `AttachmentList`; replay float 6 marked unused.
  - `buffers.rs`:
    - `histogram_color_scale` (also in `export/accumulate.rs`);
    - the path buffer as "7x u32";
    - the old `path_map_style` values;
    - the "unused" accumulation sampler;
    - `MAX_SUBFLAME_TRANSFORMS_TOTAL` and `MAX_TRANSFORMS` sizes;
    - a separate `subflame_transforms` buffer;
    - the coverage buffer as two u32s.
  - `build_state_accessors` and the `state_count` docs say state re-initialises each dispatch; persistent orbits restore it.
  - `MAX_VARIATIONS_PER_FLAME` says 1200 slots, not 1600.
  - `compute_local_index_map` says registry order; it is arrival order.
- **Colour and tone.**
  - `accumulate.wgsl`'s header gives `max(new/total, blend_factor)`; the code picks one or the other.
  - "Off by default (Apo-matching)" for Levels, which defaults on: `histogram.rs:413`, `tonemap.wgsl:487`, `FractalConfig::levels_enabled`.
  - `tonemap.wgsl:467` gives the old Levels defaults.
  - `white_level`'s doc has the direction backwards.
  - The `SqueezeMode` doc gives the wrong widths.
  - "Clamped exponential" for dynamic blend.
  - The `TonemapParams` field comments are out of date.
- **The escape engine.**
  - `assembler.rs`'s header calls `a` density; it is coverage.
  - Interior "painted black"; it is left uncovered.
  - `DownsampleMode::Vivid` and the downsample shader comment.
  - `BiomorphMode`'s "in addition to".
  - `ShadingField::Analytic`'s "direct path only".
  - Origami's per-pixel cache.
  - "No colouring writes RGB" (`DirectColor` exists).
  - `orbit_store.rs`'s `FFORBIT1` (now 7); `reference.rs`'s f32 pairs.
  - `wants_perturbation`, `perturb_tier`, `shade_blend`, and `render_escape`'s per-call renderer.
  - Misattached doc comments in `escape/mod.rs`, `renderer.rs` (five), `assembler.rs`, `fixedpoint.rs`, `ifs.rs` and `colorings.rs`.
- **Variations and transforms.**
  - `Flame::solo_transform` says weight 0; it sets opacity 0.
  - The `Transform` doc says only Finals ignore colour fields; Linked members do too.
  - `Feature::Replace` and `Feature::AnalyticBlur` (wrong names).
  - `VariationDef::aliases` (`linear3D`), and the `needs_rng` example.
  - `build_init_shader` (`local_idx * 16`).
  - `Point::phi`.
  - Misattached docs in `variations/mod.rs`.
  - `write_flame_xml`'s doc attached to `solid_shading_from_sld`, saying child subflames aren't written.
- **Config.**
  - The `manager.rs` module doc shows the old lazy-undo API.
  - `update_batch` says batches skip coalescing; they coalesce.
  - `max_view_zoom` says the pan is f32, and carries `effective_levels_enabled`'s doc.
  - The `high_res.rs` header describes only the CPU path.
  - The `SimColoringDef` example signature.
  - The sim "ping-pong pair".
- **Online.**
  - `api/auth.rs` says "never shows a login form".
  - `api/client.rs` says "to be implemented" and calls thumbnails PNG (they are JPEG).
  - `credentials.rs` names `system_settings.json`.
  - The `Smooth` interpolation is called Catmull-Rom; it is smoothstep.
- **Effects.**
  - `export_effects_json.rs` and `effect_chain.rs` count 15 effects and 76 parameters; there are 16 and 81.
- **Build.**
  - `src/gpu/device.rs` gives storage-buffer counts of 4 and 8; the test says 8 and 10.
