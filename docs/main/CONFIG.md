# Configuration and State Management

`FractalConfig` is everything needed to reproduce one picture: the flame,
the view, the colouring and tone mapping, the effects, and the settings
of the escape-time and simulation engines. It is saved as a `.fflame`
JSON file, embedded in exported PNGs, and stored as the cloud API's
config blob. Every edit the editor makes goes through `ConfigManager`,
which records it for undo and says what the renderers must refresh.
Device preferences that are not part of a picture live separately, in
`SystemSettings`.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — module overview
- [TRANSFORMS.md](TRANSFORMS.md) — `Flame` and `Transform`
- [UI.md](UI.md) — panels, the History panel, render modes in the UI
- [RENDERER.md](RENDERER.md) — how the frame loop consumes update actions
- [EXPORT.md](EXPORT.md) — PNG metadata, which embeds the config
- [ESCAPE.md](ESCAPE.md), [TERRAIN.md](TERRAIN.md) — what `EscapeConfig` drives
- [SIMULATION.md](SIMULATION.md) — what `SimConfig` drives
- [ANIMATION.md](ANIMATION.md) — tracks, which address configs by `ConfigPath`
- [FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) — the Fractal Browser

**Design record** (archived; the code is the authority where they differ):
- [delta-migration/](../archive/delta-migration/README.md) — the move to
  delta-based state management
- [undo-redo-improvements/](../archive/undo-redo-improvements/undo-redo-improvements.md)
  and [bidirectional-snapshots.md](../archive/undo-redo-improvements/bidirectional-snapshots.md)
- [remove-preview-mode.md](../archive/remove-preview-mode.md) — why edits
  apply immediately
- [config-versioning.md](../archive/config-versioning.md) — the original
  versioning plan
- [local-storage-system.md](../archive/projects/local-storage-system.md) —
  `SystemSettings` and the storage backend
- [ui-render-modes.md](../archive/projects/ui-render-modes.md) — the
  render-mode rules

**Code locations:**

| file | holds |
| --- | --- |
| `src/config/fractal_config.rs` | `FractalConfig`, serialization, versioning and migration |
| `src/config/defaults.rs` | default constants, `entry_tone_mapping` |
| `src/config/escape.rs` | `EscapeConfig` and its types |
| `src/config/sim.rs` | `SimConfig` and its types |
| `src/config/delta.rs` | `ConfigPath`, `ConfigValue`, `ConfigDelta`, `ConfigChange`, `SnapshotData`, `UpdateType`, `json_to_config_value` |
| `src/config/manager.rs` | `ConfigManager`, `EditingTarget`, `UpdateAction`, `ConfigError` |
| `src/config/precision.rs` | how an `f32` is written down and quantised |
| `src/config/slider.rs` | `ConfigSlider` and the egui helpers (`web-app` feature only) |
| `src/storage/settings.rs` | `SystemSettings` |
| `src/storage/backend.rs` | the cross-platform storage backend |
| `src/scene/presets.rs`, `src/resources/presets.rs` | the preset library |
| `src/ui/render_mode.rs`, `src/ui/visibility.rs` | the render-mode rules |

---

## FractalConfig

`FractalConfig` (`src/config/fractal_config.rs`) has one required field,
`flame`; every other field has a serde default, so a file that omits a
field loads with its default.

| group | fields |
| --- | --- |
| the flame | `flame: Flame` — transforms in three pools, xaos, solo, post-symmetry, subflames. Variation weights are `HashMap<String, f32>` keyed by variation name; parameters are `HashMap<String, f32>` keyed `"variation.param"`. See [TRANSFORMS.md](TRANSFORMS.md). |
| scene-level render | `render_mode` (`TwoD`, `ThreeD`, `Escape`, `Simulation`; wire form `"2d"`, `"3d"`, `"escape"`, `"simulation"`), `preserve_z`, `perspective_strength`, `depth_density_compensation`, `far_density_fade`, `far_density_fade_start` |
| 2D view | `zoom` (f32), `pan_x`, `pan_y` (f64), `rotation` (radians) |
| 3D camera | `camera_rotation_x` (pitch), `camera_rotation_y` (yaw), `camera_bank`, `camera_x`, `camera_y`, `camera_z` |
| depth effects | `dof_focus_distance`, `dof_blur_strength`, `fog_strength`, `fog_start` |
| solid rendering | `solid_strength`, `surface_thickness`, `solid_shading` (`SolidShadingSettings`: lights, ambient/diffuse/specular, the material's gloss, roughness and glow, SSAO, shadows) |
| accumulation | `density_scale`, `speed_factor`, `max_iterations`, `blend_factor`, `use_dynamic_blend`, `deterministic_rng`, `filter_radius`, `filter_blur_edges` |
| colour | `color_mode` (`Palette`, `Speed`, `PathMap`), `path_map_style`, `path_map_level`, `palette` (the palette itself, always embedded), `palette_rotation`, `palette_size`, `palette_squeeze`, `palette_squeeze_mode`, `palette_squeeze_falloff`, `palette_log_strength`, `palette_reverse`, `background_color` |
| tone mapping | `tonemap_mode`, `highlight_mode`, `tonemap_curve`, `use_curve`, `exposure`, `gamma`, `gamma_threshold`, `brightness`, `vibrancy`, `white_level`, `saturation`, `hue_shift`, `alpha_blend_low`, `alpha_blend_high`, `levels_enabled`, `levels_low`, `levels_high`, `levels_gamma` |
| effects | `density_effects`, `color_effects` (lists of `EffectInstance`; see [EFFECTS.md](EFFECTS.md)) |
| focused rendering | `importance`, `cylinder_targeting`, `cylinder_always`, `cylinder_trim`, `cylinder_trim_levels`, `word_removals`, `auto_exposure` (see [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md)) |
| other engines | `escape: EscapeConfig`, `sim: SimConfig` (below) |
| file | `image_size` — the picture's size: what the app's PNG and video exports render at (the View panel's Output Size; `ImageWidth`/`ImageHeight`, clamped to `MIN_IMAGE_DIM..=MAX_IMAGE_DIM`), and the shape the viewport letterboxes to; JWildfire/Apophysis's `size`, round-tripped through `.flame` XML. The CLI's `export` does not read it (`--width`/`-H`) |

Points worth knowing:

- **The pan is f64**; every other view field is f32. The pan is the only
  absolute position, and an f32 centre stops being addressable past a
  zoom of about 1e8. `zoom` stays f32 because only its relative
  precision matters. A JSON number parses into either, so the change
  needed no migration.
- **The palette is always embedded.** There is no palette index; the
  config carries the palette's stops, so a file renders the same
  whatever the user's palette library holds.
- **`render_mode` is always serialized**, even at its default, because
  the cloud API projects it into a column.
- **`preserve_z` has two defaults.** `FractalConfig::default()` (a new
  flame) is `false`, the Apophysis/JWildfire semantics, and is written
  explicitly; a file that omits the field loads as `true`, which is how
  flames made before the flag existed rendered.
- **`render_mode` and `preserve_z` live on `FractalConfig`**, not on
  `Flame`, since config v3 (see *Versioning*): the whole render has one
  mode, so subflames inherit it.

### Defaults

The constants are in `src/config/defaults.rs`; `impl Default for
FractalConfig` assembles them.

| field | default | field | default |
| --- | --- | --- | --- |
| `render_mode` | `TwoD` | `tonemap_mode` | `Logarithmic` |
| `zoom` | 1.0 | `exposure` | 1.0 |
| `density_scale` | 1.0 | `gamma` | 4.0 |
| `speed_factor` | 0.5 | `gamma_threshold` | 5.0 |
| `max_iterations` | 1,000,000,000 | `brightness` | 4.0 |
| `blend_factor` | 0.1 | `vibrancy` | 1.0 |
| `use_dynamic_blend` | true | `white_level` | 200 |
| `color_mode` | `Palette` | `saturation` | 1.0 |
| `palette` | Fire | `alpha_blend_low` / `high` | 0.3 / 0.8 |
| `palette_size` | 256 | `levels_enabled` | true |
| `palette_squeeze` | 1.0 | `levels_high` / `levels_gamma` | 10 / 0.5 |
| `dof_focus_distance` | 0.0 | `surface_thickness` | 0.02 |
| `image_size` | (1920, 1080) | `path_map_level`, `cylinder_trim_levels` | 2, 2 |

Entering Escape or Simulation from a flame mode with a logarithmic tone
map switches the tone map to Linear (`entry_tone_mapping`): exposure 2.0
and gamma 1.0 for Escape (`ESCAPE_EXPOSURE`, `ESCAPE_GAMMA`), the flame
defaults for Simulation. A Log-calibrated flame tone map renders either
engine's unit-range image black.

Entering a lit 3D view -- a solid formula, an escape or simulation
terrain -- from a view that draws none sets **the 3D look**
(`ui::render_mode::look_3d`) in the same undo step: Linear, exposure 2.0,
gamma 1.0, saturation 1.0, no hue shift, and, when the lighting rig is
untouched, shading strength 1.0 and ambient 0.0. The mode switch, the
Escape panel's formula picker and terrain switch, the Simulation panel's
terrain switch and the Formula Editor's Use go through it
(`apply_with_look`).

**Changing a default changes old files.** Serialization omits values
equal to their default, so a file saved before the change and relying
on the old default renders with the new one. A default change needs a
version bump and a migration arm that restores the old value (see
*Versioning*).

---

## Serialization

### Writing

`FractalConfig::to_json()` is `to_json_value()` printed pretty.
`to_json_value()` produces the canonical form:

- `version` first (`CURRENT_CONFIG_VERSION`), then `flame`, then the
  remaining fields;
- fields equal to their defaults omitted, by two mechanisms: serde's
  `skip_serializing_if` on most fields, and `remove_default_fields` for
  the older ones that have none;
- every number that is exactly an `f32` rewritten to its shortest
  round-trip decimal (`precision::normalize_f32_numbers`), so `0.98` is
  saved as `0.98` and not `0.9799999594688416`. Numbers that need f64
  (the pan) are left alone.

`save_to_file` writes `to_json()`. `to_json_array` and
`save_multi_to_file` write a JSON array of configs (defaults stripped,
version first; this path does not normalize the numbers).
`EscapeConfig::texture` embeds a whole config through `to_json_value` /
`from_json_value`, so a texture recipe is stored and migrated like a
file.

`serde_json::to_string(&config)` — the plain derive — writes **no
`version`** and strips no defaults. The PNG metadata's `Config` chunk is
written this way (`PngMetadata::from_app_state`); see the warning under
*Versioning*.

### Reading

`from_json(json)` → `from_json_value(value)`:

1. Read `version`; refuse a version newer than `CURRENT_CONFIG_VERSION`.
2. `migrate_value` on the raw JSON, before deserializing — so an arm can
   tell an absent field (saved at its old default) from a present one.
3. `lift_path_material`: a shape fix, not a version. An older picture
   kept its material (`gloss`, `roughness`, `emission`) in a path
   tracer's settings (`escape.path` or `sim.terrain.path`); it is moved
   into `solid_shading` (`gloss`, `roughness`, `glow`), without
   overwriting values already there.
4. Deserialize; serde fills every still-absent field with its current
   default.
5. `fixup_ids`: assign session-local IDs to every transform, flame and
   effect that arrived with the zero sentinel (IDs are not serialized;
   animation tracks bind to them).

`from_json_multi` accepts either one object or an array, and
`load_multi_from_file` reads such a file; File > Open uses it, so one
`.fflame` can hold several configs.

---

## Versioning and migration

`CURRENT_CONFIG_VERSION` is **6** (`src/config/fractal_config.rs`).
`migrate_value` upgrades one version per arm:

| arm | what it does |
| --- | --- |
| v0 → v1 | nothing; configs from before versioning |
| v1 → v2 | nothing. v2 is the cloud "opaque blob" format ([api-v2.md](../archive/api-v1-v2/api-v2.md)): the `.fflame` JSON stored as-is |
| v2 → v3 | lifts the scene-level render fields from `flame` to the top level: `render_mode` (absent: `"2d"`), `perspective_strength` (or the legacy `projection` enum, converted), `depth_density_compensation`, `far_density_fade`, `far_density_fade_start`, and `preserve_z` (absent: `true`) |
| v3 → v4 | for a file saved as v3 only: an absent `levels_gamma` becomes 1.0. A v3 save skipped the field at 1.0, the pre-2026-05-31 default, rather than at its 0.5 default; every v3 file postdates that change, so absent meant 1.0. Older files keep loading at 0.5, as that change intended |
| v4 → v5 | the Shadows default rose from 0.7 to 0.98 (2026-10-19): an absent `shadow` becomes 0.7 in `escape.formula_params` when the formula is a solid (a built-in solid's name, or a `custom-` formula whose carried source has `fn distance`), and in `escape.terrain` and `sim.terrain` when the file has them |
| v5 → v6 | a solid's directional lights became the fractal's (2026-10-19): `solid_shading.lights_follow_camera` becomes true when the formula is a solid (as v4 → v5 tells one), so an older solid keeps the camera's lights and renders as it did. The shipped solid presets were converted to the fractal's at their own cameras and saved as v6 (`scripts/presets_lights_to_world.py`) |

A value **without a `version`** is read as v0, with two exceptions. A
top-level `render_mode` with none on `flame`, or one of `"escape"` or
`"simulation"`, dates the file to at least v3: the field moved to the top
level in v3, and both modes came after it. That covers the plain-serde
JSON older PNGs carry in their `Config` chunk, which has no `version`.
And an animation's embedded `base_config` is read as v2
(`from_json_value_with_default_version`).

**The v1 → v2 arm is a no-op, by decision.** Two unit changes landed
while files were stamped v1: Levels values became multiples of the mean
(2026-05-12) and the shader started scaling `dof_blur_strength` by 0.1
(2026-05-29). A v1 file with Levels values set explicitly between
2026-01-15 and 2026-05-12, or a blur strength set between 2026-01-24 and
2026-05-29, renders differently today. A version-1 stamp cannot tell
those files from later v1 saves or from hand-written v1 configs in the
new units, so nothing translates them; they are fixed by hand. The
record is [config-v2-migration.md](../archive/projects/config-v2-migration.md).

`SystemSettings` has its own version (`CURRENT_SETTINGS_VERSION` = 4,
`src/storage/settings.rs`); all four of its migration arms are no-ops.

---

## Precision

`src/config/precision.rs` is the one place that decides how an `f32` is
written and how a slider quantises it:

- `fmt_f32` — the shortest decimal that parses back to the same `f32`
  (Rust's `{}`), not a fixed number of places; `"0"` for non-finite.
- `normalize_f32_numbers` — the serialization pass above.
- `nice_step`, `snap_to_step`, `snap_to_significant`,
  `decimals_for_step` — drag quantisation to 1/2/5 × 10^k, so a drag
  lands on short decimals. Typed values are never snapped.

`ConfigValue::Double` and `ConfigValue::Vec2` carry the pan as f64, with
an f64 epsilon for change detection, so undo never merges two pans that
differ at a deep zoom. There is deliberately no `From<f64>` for
`ConfigValue`: an untyped float literal would become a `Double` and fail
at runtime on every f32 path.

---

## The other engines' configs

`escape: EscapeConfig` and `sim: SimConfig` sit beside the flame. Both
are skip-if-default — a flame file never grows them — and both are
compiled into every build, engine or not: a build without the escape or
simulation engine still parses such a config, so the file round-trips
unchanged, and rendering it returns `RenderError::EngineMissing` rather
than drawing the flame (see [EXPORT.md](EXPORT.md)). Switching modes
leaves the inactive engine's block in place, so a round trip keeps it.

### EscapeConfig

`src/config/escape.rs`. Formula, colouring and lens parameters are keyed
maps (`BTreeMap<String, f32>`, deterministic order), and parameters of
an inactive formula are kept, as `variation_params` keeps them.

| group | fields |
| --- | --- |
| the fractal | `formula` (default `mandelbrot`), `formula_params`, `julia`, `julia_re`, `julia_im`, `max_iter` (256), `bailout`, `damping_re`, `damping_im`, `biomorph`, `reference_period` |
| the plane view | `center_re`, `center_im` (exact decimal strings), `zoom_log2` (f64), `rotation` |
| the lens | `lens` (a variation name; empty is none), `lens_params`, `lens_amount` |
| the solid camera | `cam_target_x`, `cam_target_y`, `cam_target_z` (decimal strings; empty is the attractor's centre), `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, `solid_tier` |
| colour | `coloring` (default `smooth`), `coloring_params`, `layer` (a second colouring blended in), `palette_map`, `contrast`, `shading` (relief) |
| texture | `texture` (a simulation recipe, embedded whole), `texture_overlay` |
| antialiasing | `supersample` (1–`MAX_SUPERSAMPLE` = 8), `downsample` |
| 3D | `terrain` (`TerrainConfig`), `path` (`PathTraceConfig`) |

The centre is a pair of strings because no float the config could hold
reaches a deep-zoom centre; the zoom is a log2 exponent so animating it
is an ordinary float track. `bailout` has two defaults: a new config
gets 1e4, while a file without the key loads as 4, the value it was
saved under. What the fields do is in [ESCAPE.md](ESCAPE.md) and
[TERRAIN.md](TERRAIN.md).

### SimConfig

`src/config/sim.rs`: the model and its parameters, seed, init, steps,
`dt`, boundary, grid, warp, view, resolve, matte, colouring, layers,
couplings, colour layers and the terrain. The table, and what each edit
costs the run, are in [SIMULATION.md](SIMULATION.md). `sim.steps` is a
cap to the panel's transport and a target to the timeline and exporters
(see [UI.md](UI.md)).

---

## Who changes the render mode

The render mode has exactly one writer: `switch_render_mode` in
`src/ui/render_mode.rs`. It applies the tone-map reset on entering a
non-flame mode in the same undo entry as the mode change, and a mode
change never coalesces with another. The test
`nothing_outside_this_module_writes_the_render_mode` scans the source
tree and fails on any mention of `ConfigPath::RenderMode` outside that
module, except the config module itself (`delta.rs`, `manager.rs`) and
the whole-config load paths (`ui_handlers.rs`, `panel_viewer.rs`,
`api/`), which bring a mode in with a file rather than switch one.

What a mode makes available — which panels, which controls, whether fly
mode is offered — is answered only by `src/ui/visibility.rs`
(`visibility::panel`, `visibility::control`, `visibility::fly_mode`,
`WINDOW_MENU`). Add a case there rather than a `matches!(render_mode,
...)` in a panel. See [UI.md](UI.md).

---

## ConfigManager

`ConfigManager` (`src/config/manager.rs`) owns the current
`FractalConfig`, the undo history and the `SystemSettings`, and
accumulates the GPU work edits imply.

| state | meaning |
| --- | --- |
| `current` | the config |
| `history`, `position` | one `Vec<ConfigChange>`; entries before `position` can be undone, entries from it on redone |
| `max_undo_depth` | 500; past it the oldest entry is dropped |
| `pending_actions` | an `UpdateAction`, the union of what every edit since the last clear needs |
| `editing_target` | `Main` or `Subflame { index }` (below) |
| `modify_session` | an open transform edit (below) |
| `animation_mode` | while true, `update_param` records no history |
| `load_generation` | bumped by `load_config`, so panels can reset per-fractal UI state |
| `system_settings` | device preferences (no undo) |
| `preview` | vestigial: nothing sets it any more, so `active_config()` and `config()` both return `current` |

### Writing

| method | use |
| --- | --- |
| `update_param(path, value)` | one value; a history entry, coalesced (below). Returns `Result<UpdateType, ConfigError>`; an unchanged value (`ConfigValue::approx_eq`) returns `UpdateType::None` and records nothing. |
| `update_batch(changes, description)` | several values as one history entry under the given description; each delta's update type is recorded separately, so the actions are their union. The changes apply in order, each one's old value read as the earlier ones left the config, so a later path whose default depends on an earlier one (a colouring and its parameters) is never dropped as a false no-op. A batch holding a setter that changes more than its own field (the simulation's model, a layer's model, the grid mode, the init kind) is recorded as the config before and after, as a single such edit is. A change that fails leaves the config as it was. Coalesces like a single edit when consecutive batches change the same paths, or carry the same gesture description. |
| `update_param_silent`, `update_param_silent_on(target, …)` | no history: animation playback, and tracks aimed at a subflame |
| `apply_value_detached(config, target, path, value)` | apply one value to a detached config with the manager's own setters and clamps, no history or disk; the video exporter's way to apply tracks |
| `remove_variation(xref, name)` | removes a variation and records the whole transform before and after, so undo restores its parameters and order |
| `load_config(config, description)` | a new picture (file, preset, browser): one full-config snapshot, editing target reset to `Main`, `load_generation` bumped, every update action plus `reset_accumulation` and `structural_changed` |
| `apply_config_edit(config, description)` | an edit no single path carries (a simulation's layers, couplings or colour layers added, removed or reordered): a full-config snapshot without the side effects of a load |
| `load_config_silent`, `load_config_with_explicit_before` | restore the base config after animation playback, and record the playback as one undo entry |
| `apply_structural_change(change)` | add, delete or modify a transform; add, delete or move an effect |
| `start_modify_transform` / `commit_modify_transform` / `cancel_modify_transform` | a modify session: edits apply without history, and the commit records one before/after snapshot of the transform — one undo per Triangle Editor drag, in any pool |
| `add_subflame`, `delete_subflame`, `replace_subflame`, `set_editing_target` | subflames (below) |
| `update_system_setting(path, value)` | `System*` paths: no history, saved to disk at once |

Four simulation setters change more than their own field — choosing a
model resets its parameters, `dt` and `steps`; the grid mode its sizes;
the init kind its shape — so `update_param` records `SimModel`,
`SimLayerModel`, `SimGridMode` and `SimInitKind` as full-config
snapshots rather than single deltas.

`System*` paths are not part of `FractalConfig`: passing one to
`update_param`, `get_value` or the setters panics. Use
`update_system_setting` and `system_settings()`.

The App wraps the manager: `App::load_config_with_undo`
(`src/app/config.rs`) loads a config through `load_config` and pushes it
to the renderers; File > Open accepts `.fflame` (one or several configs)
and `.flame` XML; File > Save writes `save_to_file`; the JSON dialog and
the clipboard go through `from_json` and `to_json`
(`src/app/ui_handlers.rs`).

### Undo and redo

`undo()` and `redo()` return `Result<UpdateType, ConfigError>`
(`EmptyUndoStack` / `EmptyRedoStack` at either end). Each moves
`position`, first switching the editing target silently to the one the
entry was made against, then applies the entry: inverse deltas, or the
snapshot's before or after state. `structural_changed` is flagged for a
structural snapshot so animation tracks rebind. `App::undo` and
`App::redo` re-import the resulting config into the renderers; undoing
or redoing a load, a preset or a script run restarts the simulation, so
the field matches the config.

History descriptions are i18n keys (`history.param.*`,
`history.action.*`), translated by the History panel
(`src/ui/undo_history.rs`).

Keyboard: Ctrl+Z undoes and Ctrl+Y redoes (Cmd on macOS; the web build
accepts either), in `src/app/input.rs`.

### Coalescing

A new entry merges into the previous one instead of being pushed when
all of these hold (`should_coalesce`):

- it is a delta change, not a snapshot;
- the history is at its head, and the previous entry was made against
  the same editing target;
- either both changes touch the same paths in the same order and every
  path supports coalescing, or both carry the same description from
  `GESTURE_HISTORY_DESCS` — the fly camera and the viewport's pan, zoom,
  rotate and orbit gestures, whose per-frame path sets vary;
- no more than **500 ms** have passed since the previous entry's last
  update (`COALESCE_INACTIVITY_THRESHOLD`);
- no more than **3 s** have passed since the previous entry began
  (`MAX_COALESCE_SPAN`).

A merge keeps each path's first old value and takes the newest new
value, adding any path the entry did not have yet. So a slider drag is
one undo step, a pause of half a second starts another, and a long drag
is split every three seconds. Discrete choices never coalesce
(`supports_coalescing`): the render mode, and the simulation's seed,
model, colouring, init kind, grid mode, boundary, warp filter, mode,
cull and channel mask, matte channel, direction and edge, layer models
and enabled flags, coupling endpoints, forms and channels, and
`use_transforms`.

### Subflames: EditingTarget

`EditingTarget` is `Main` or `Subflame { index }` — which flame the
editor is working on. The data never moves: `current.flame` is always
the main flame and subflames stay in `current.flame.subflames`. Per-flame
paths (transforms in every pool, xaos, solo, post-symmetry) route to the
target flame through `target_flame_mut` / `active_flame_mut`;
scene-level paths (`RenderMode`, `PreserveZ`, the view, tone mapping)
always write the config. Panels read the target through
`active_flame()`.

Switching target is an undo entry of its own (`SwapTarget`). Every
history entry is stamped with the target it was made against
(`ConfigChange::target`), undo switches back to it before applying, and
entries never coalesce across targets. `load_config` resets the target
to `Main`.

---

## ConfigPath and ConfigValue

`ConfigPath` (`src/config/delta.rs`) names every editable parameter:
372 variants, many carrying an index or a name.

| family | examples | update type |
| --- | --- | --- |
| view | `Zoom`, `Pan`, `PanX`, `PanY`, `Rotation`, `CameraRotationX`/`Y`, `CameraBank`, `CameraX`/`Y`/`Z` | `ViewOnly` |
| depth and filter | `DofFocusDistance`, `DofBlurStrength`, `FogStrength`, `FogStart`, `FilterRadius`, `FilterBlurEdges` | `IterationReset` |
| tone mapping | `Exposure`, `Gamma`, `GammaThreshold`, `Brightness`, `Vibrancy`, `WhiteLevel`, `Saturation`, `HueShift`, `AlphaBlendLow`/`High`, `DensityScale`, `TonemapMode`, `HighlightMode`, `TonemapCurve`, `UseCurve`, `Levels*`, `BackgroundColor`(`R`/`G`/`B`) | `ToneMappingOnly` |
| colour | `ColorMode`, `Palette`, `PaletteIndex`, `PaletteRotation`, `PaletteSize`, `PaletteSqueeze*`, `PaletteLogStrength`, `PaletteReverse`, `SpeedFactor` | `ColorOnly` |
| accumulation | `BlendFactor`, `UseDynamicBlend`, `MaxIterations`, `DeterministicRng`, `PathMapStyle`, `PathMapLevel` | `IterationReset` |
| focused rendering | `AutoExposure`, `CylinderTargeting`, `CylinderAlways`, `CylinderTrim`, `CylinderTrimLevels`, `WordRemovals` | `IterationReset` |
| transforms | `TransformCount`, `TransformWeight`, `TransformColor`, `TransformAffine { index, param }`, `TransformVariation { index, variation }`, `TransformVariationParam { index, variation, param }`, the post-affine, plane-coefficient and origin/rotation/scale paths; the same set as `LinkedTransform*` and `FinalTransform*` | `IterationReset` |
| flame and scene | `RenderMode`, `PerspectiveStrength`, `DepthDensityCompensation`, `FarDensityFade`(`Start`), `PreserveZ`, `Xaos { src, dst }`, `SoloTransform`, `PostSymmetry*`, `SolidStrength`, `SurfaceThickness` | `IterationReset` |
| solid lighting | `ShadingStrength`, `SolidAmbient`, `SolidDiffuse`, `SolidSpecular`, `SolidShininess`, `SolidGloss`, `SolidRoughness`, `SolidGlow`, `Ssao*`, `NormalSmoothing`, `GapFill`, `SolidShadowStrength`, `SolidLight*` | `ShadingOnly` |
| effects | `DensityEffectEnabled`/`Param`, `ColorEffectEnabled`/`Param`, `Add`/`Remove*Effect` | `ToneMappingOnly` |
| escape-time | about a hundred `Escape*` paths: formula, view, solid camera, colouring, relief shading, contrast, palette map, texture overlay, terrain, path tracer, lens | `EscapeRerender` |
| simulation | about eighty `Sim*` paths: model, grid, seed, init, steps, warp, view, matte, resolve, layers, couplings, colour layers, terrain, path tracer | `SimRerender`, `SimResample` or `SimReseed` (see [SIMULATION.md](SIMULATION.md)) |
| system | 17 `System*` paths | see *SystemSettings* |

What a path supports is spread over several exhaustive matches, so the
compiler lists every place a new variant needs an arm:

| function | where | purpose |
| --- | --- | --- |
| `ConfigPath::update_type()` | `delta.rs` | the GPU work the path implies |
| `to_i18n_key()`, `Display` | `delta.rs` | history labels (`history.param.*` in `locales/*.yml`) |
| `to_string_key()`, `from_string_key()` | `delta.rs` | stable string keys for animation files (`"Transform.0.Affine.A"`); legacy un-indexed `FinalTransform.*` keys map to index 0 |
| `json_to_config_value()` | `delta.rs` | converts a track's JSON value; the paths it returns `None` for are not animatable |
| `get_value_from_config()`, `set_value()` | `manager.rs` | read and write, with the setters' clamps and side effects |

`ConfigValue` is the value side: `Unit`, `Float(f32)`, `Int`, `UInt`,
`UInt64`, `Bool`, `String`, `StringList`, `Vec2(f64, f64)`,
`Double(f64)`, `ColorRgb`, `ToneMapMode`, `HighlightMode`, `ColorMode`,
`PathMapStyle`, `RenderMode`, `ToneCurve`, `Palette`, `SqueezeMode`.
`approx_eq` compares floats within 1e-6 (f32) or 1e-15 (f64).

---

## UpdateType and UpdateAction

`UpdateType` (`delta.rs`) classifies a change. It is ordered, and
`merge` takes the maximum, so a change set takes its strongest member:

`None` < `ViewOnly` < `ToneMappingOnly` < `ShadingOnly` < `ColorOnly` <
`IterationReset` < `EscapeRerender` < `SimRerender` < `SimResample` <
`SimReseed`

`UpdateAction` (`manager.rs`) is what the frame loop acts on: a set of
flags, unioned across every edit until the loop consumes them
(`get_pending_actions()`, then `clear_pending_actions()`, in
`process_gpu_updates`, `src/app/gpu_updates.rs`).

| update type | action flags |
| --- | --- |
| `ViewOnly` | `update_view` |
| `ToneMappingOnly` | `update_tone_curve` |
| `ShadingOnly` | `update_shading` |
| `ColorOnly` | `update_palette` |
| `IterationReset` | `update_flame`, `update_palette`, `update_view`, `update_tone_curve`, `update_shading` |
| `EscapeRerender` | `rerender_escape` |
| `SimRerender` / `SimResample` / `SimReseed` | `rerender_sim` / `resample_sim` / `reseed_sim` |

None of these sets `reset_accumulation`. A flame edit is shown through
overwrite mode instead: while edits arrive, and for 100 ms after the
last, each frame replaces the accumulation rather than blending into it,
and when the window closes the iteration count restarts
(`update_overwrite_mode` in `src/app/gpu_updates.rs`; not in Escape
mode). `reset_accumulation` comes from `load_config`, `apply_config_edit`,
`request_reset` and `request_full_resync`. `structural_changed` comes
from structural snapshots and asks the App to rebind animation tracks.
`rebuild_shader` is read by the frame loop but nothing sets it.

---

## UI helpers

`src/config/slider.rs` (built with the `web-app` feature):

- `ConfigSlider::new(manager, path, range, label)` returns
  `Result<ConfigSlider, ConfigError>` (the path must hold an `f32`);
  `.tooltip(text)` and `.show(ui)` render it and call `update_param` on
  change, returning a `ConfigSliderResult { changed, should_capture,
  update_type }`.
- `ConfigSliderUi` on `egui::Ui`: `config_slider(manager, path, range,
  label, tooltip)` and `config_drag_value(manager, path, speed, label)`.
- `LazyUndoUi`: `lazy_slider` and `lazy_drag`, the same calls under the
  names the tone-mapping panel uses.

`.lazy(true)` and the drag-end `force_commit_preview` it triggers are
vestigial: preview mode is gone, and coalescing is what turns a drag
into one undo step.

Most controls use the plain pattern:

```rust
let mut value = config_manager.active_config().exposure;
if ui.add(egui::Slider::new(&mut value, 0.1..=5.0)).changed() {
    let _ = config_manager.update_param(ConfigPath::Exposure, value.into());
}
```

---

## Presets

The shipped presets are one file, `assets/presets.fflame`: a JSON array
of configs (twenty at present). `src/resources/presets.rs` loads it:

- **Desktop** reads the file at runtime (resolved against the working
  directory and the executable's location), so editing it needs no
  rebuild. If the file cannot be read it falls back to the copy embedded
  at compile time (`PRESETS_JSON`). A file that reads but does not parse
  falls through to a single built-in default preset.
- **WASM** uses the embedded copy.

`PresetLibrary` (`src/scene/presets.rs`) holds the result;
`global_preset_library()` is its shared instance. To add a preset, add a
config to the array — for example one saved with File > Save — and keep
it as `to_json` writes it. The preset browser is described in
[FRACTAL-BROWSER.md](FRACTAL-BROWSER.md).

---

## SystemSettings

`SystemSettings` (`src/storage/settings.rs`) holds device preferences:
no undo, saved on every change, the same for every picture.

| group | field | default |
| --- | --- | --- |
| rendering | `vsync_enabled` | true |
| | `target_fps` (used when VSync is off) | 60 |
| | `iterations_per_thread` | 256 |
| | `burn_in` | 20 |
| | `frame_governor` | true |
| | `manual_workgroups` (with the governor off; clamped 1–65535) | 128 |
| | `orbit_cache_mb` (deep-zoom reference orbit cache) | 1024 |
| fly mode | `fly_mouse_sensitivity` (radians per pixel) | 0.005 |
| | `fly_move_speed`, `fly_sprint_multiplier` | 1.0, 3.0 |
| | `fly_invert_y` | false |
| | `fly_camera_mode` (`FreeLook` or `Fps`) | `FreeLook` |
| export | `video_encoding` — the Export Animation panel's codec, encoder, quality (CRF), preset and tune, loaded when the panel opens and saved when one changes (on the pointer's release); read leniently, so an encoder a later build renamed costs only itself | H.265, software, CRF 12, Medium, Animation |
| | `png_export_strip_metadata` | false |
| viewport | `viewport_letterbox` — the viewport in the picture's shape, black bars around it | false |
| | `render_scale` — the viewport's picture renders at this share of its size each side, drawn up to the screen (`app/render_scale.rs`); not in Simulation; exports render at `image_size`, Export Current Image saves the picture as rendered | 1.0 |
| | `upscaler` (`Bilinear` or `CatmullRom`; `renderer::upscale`) | `CatmullRom` |
| | `render_physical_pixels` — render at the screen's pixels rather than egui's points | false |
| UI | `language` | `"en"` |
| | `show_help_on_startup` | true |
| | `compact_mode` | detected on first run |
| online | `online_mode`, `auth_token`, `auth_email`, `saved_credentials` (desktop) | see [ONLINE.md](ONLINE.md) |
| files (desktop) | `recent_files`, `last_opened_file`, `default_save_location` | empty |

`update_system_setting(path, value)` writes one `System*` path, saves
the file, and returns the path's update type:

| paths | update type | side effect |
| --- | --- | --- |
| `SystemIterationsPerThread`, `SystemBurnIn` | `IterationReset` | |
| `SystemFrameGovernor`, `SystemManualWorkgroups` | `None` | read by the frame loop each frame |
| `SystemOrbitCacheMb` | `None` | pushed into the escape engine's orbit store (desktop) |
| `SystemVsyncEnabled`, `SystemTargetFps`, `SystemFly*` | `ViewOnly` | |
| `SystemPngStripMetadata` | `None` | mirrored into the PNG encoder's global switch at once |
| `SystemExportWidth`, `SystemExportHeight`, `SystemLanguage`, `SystemShowHelpOnStartup` | `None` | |

Fields without a path (compact mode, the online and sign-in state) are
written through `system_settings_mut()` by their owners.

Things the code does not do:

- `recent_files`, `last_opened_file` and `default_save_location` are
  declared and serialized but nothing reads or writes them.

### The settings file

`SystemSettings::load()` and `save()` use `settings.json` through the
storage backend. The file is written with a `version` first and some
default-valued fields omitted (`remove_default_fields`); an unreadable
or unparsable file loads the defaults.

---

## Local storage

`src/storage/backend.rs` gives every platform the same small API
(`read_file`, `write_file`, `delete_file`, `file_exists`,
`list_entries`) over relative paths:

| platform | where |
| --- | --- |
| Windows | `%APPDATA%\Fractals for All\Fractal Art Editor\data\` |
| macOS | `~/Library/Application Support/com.Fractals-for-All.Fractal-Art-Editor/` |
| Linux | `$XDG_DATA_HOME/fractalarteditor/` |
| web | browser `localStorage`, keyed by the relative path |

The desktop directory comes from `directories::ProjectDirs::from("com",
"Fractals for All", "Fractal Art Editor")`. Changing those strings moves
everyone's data; there is no migration.

What lives there:

| path | contents |
| --- | --- |
| `settings.json` | `SystemSettings` |
| `custom_palettes.json` | the user's palettes |
| `plugins/variations/`, `plugins/effects/` | locally installed plugins |
| `textures/` | user simulation textures ([SIMULATION.md](SIMULATION.md)) |
| `texture_cache/` | generated texture images (desktop) |
| `orbit_cache/` | deep-zoom reference orbits (desktop) |
| `crash.log` | panic breadcrumbs (desktop) |

---

## Common tasks

### Add a config field

1. Add the field to `FractalConfig` (or `EscapeConfig` / `SimConfig`)
   with a serde default and a `skip_serializing_if`, so old files load
   and unchanged files stay byte-identical; set it in the `Default`
   impl.
2. If the UI edits it, add a `ConfigPath` variant and follow the
   compiler through the exhaustive matches listed above — including a
   `history.param.*` string in `locales/*.yml`, and a
   `json_to_config_value` arm that returns a value if it should be
   animatable.
3. Make the renderer use it: in `FlameRenderer::load_config` if a load
   must carry it, and where its update type's action is handled
   (`src/app/gpu_updates.rs`).

### Change a default

Old files that relied on the old default change with it. Bump
`CURRENT_CONFIG_VERSION` and add a `migrate_value` arm that restores the
old default for files that omitted the field
(`obj.entry("field").or_insert(json!(old))`).

### Change the undo depth

`max_undo_depth` in `ConfigManager::new`. Each full-config snapshot
holds two whole configs.
