# Simulation Mode — integration checklist

**Status:** Planning, 2026-09-01. No code. Companion to
[simulation-fractals.md](simulation-fractals.md) (master plan),
[simulation-pipeline.md](simulation-pipeline.md) (GPU design) and
[simulation-catalog.md](simulation-catalog.md) (models and sources).

This is the file-by-file list of everything a third render mode has to
touch, derived by mapping every place `RenderMode::Escape` reaches
today (surveyed 2026-09-01; line numbers are as of commit `331160e8`
and will drift — the identifiers will not). Each row says what escape
does and what Simulation adds. Where escape's integration has a known
gap, it is marked **gap** so the new mode does not copy it.

Naming used throughout (decided in the master plan §2): enum variant
`RenderMode::Simulation`, wire string `"simulation"`, module
`src/sim/`, config `FractalConfig.sim: SimConfig`, paths `ConfigPath::Sim*`,
panel `PanelType::Simulation`, layout `WorkspaceLayout::Simulation`,
feature flag `engine-sim`, script global `sim`.

---

## 1. The enum and every match on it

`src/scene/transforms.rs:1834` — add `Simulation` with
`#[serde(rename = "simulation")]` (no alias), extend `RenderMode::ALL`
(`:1867`). The doc comment must say the server's Postgres
`render_mode` enum does not know it yet (see §9).

**Exhaustive matches that will fail to compile until given an arm** —
this is the safety net; do not add `_ =>` arms to any of them:

| file:line | arm to add |
|---|---|
| `src/api/sync.rs:18-22`, `:28-32` | `Simulation ↔ ApiRenderMode::Simulation` |
| `src/png_metadata.rs:75-87` | `Simulation ⇒ ("Simulation", "N/A")` |
| `src/contract.rs:390-392` | the compile-time gate; also `RenderMode::ALL.len() == 4` at `:394-399` |
| `src/gpu/pipelines.rs:706-711` | `Simulation ⇒ pipeline_2d()` (the flame trajectory pipeline is never used, but the match must be total) |
| `src/renderer/compute_kernel.rs:671-677, 1813-1819, 2022-2028, 2330-2336, 2696-2702` | GPU uniform `render_mode` u32: `Simulation ⇒ 0` (5 sites) |
| `src/export/high_res.rs:1714-1718` | `Simulation ⇒ 0`, unreachable (see §7) |
| `src/ui/fractal_gallery.rs:568-570` | `t!("fractal_gallery.render_mode_simulation")` |
| `src/ui/fractal_browser.rs:615-617` | `"Simulation"` label |

**Branches that test `Escape` and must also test `Simulation`** (the
mode is "not a chaos game" in every one of these):

| file:line | why |
|---|---|
| `src/app/mod.rs:1109` | `AboutToWait`: exclude from `is_rendering` — **and** add the `sim_running` redraw term (§3) |
| `src/app/mod.rs:1506`, `:2554` | force `levels_enabled = false` in `update_tonemap` (Levels are density-calibrated) |
| `src/app/mod.rs:1584`, `:1680`, `:1762`, `:2023` | export re-tonemap from `sim_renderer.output_view()` |
| `src/app/mod.rs:2275` | the per-frame branch (§3) |
| `src/app/mod.rs:2326`, `:2563-2565`, `:2569`, `:2586` | idle the chaos game, density stats, shade pass, DoF |
| `src/app/mod.rs:2593-2597`, `:2620-2626` | `pre_tonemap_view` and `tonemap_pass_with_input` selection |
| `src/app/config.rs:321` | export routing guard |
| `src/app/gpu_updates.rs:445`, `:554` | disable Overwrite live-preview |
| `src/app/input.rs:96` | keyboard routing (sim: none of the escape keys apply; Space = run/pause, `.` = step are the natural bindings) |
| `src/app/animation_update.rs:46` | playback pacing — sim does **not** use settle-then-jump; it steps `steps_per_frame` per keyframe frame (§6) |
| `src/animation/export.rs:1992` | video export branch (§6) |
| `src/ui/panel_viewer.rs:467`, `:513`, `:1455` | pan/zoom/pinch: sim has display-only pan/zoom in a later phase; phase 1 = no-op |
| `src/ui/panel_viewer.rs:759` | hide flame-only panels — extend the list; **also hide `PanelType::Escape` in sim mode and `PanelType::Simulation` in escape mode** |
| `src/ui/panel_viewer.rs:1292` | viewport overlay: sim shows step count / steps-per-second instead of the orbit-build progress |
| `src/ui/target_selector.rs:258` | offer sim animation targets (§6) |
| `src/ui/tone_mapping.rs:440` | hide Levels, with a sim-specific hint key |
| `src/script/api.rs:1434` | `sim.enter()` mirrors `escape.enter()` |
| `src/ui/escape_panel.rs:33`, `:1405` | the escape toggle must read as OFF in sim mode and its `switch_render_mode` must handle leaving *to* sim (both toggles share one helper — see §5) |

Sites that only test `ThreeD`/`TwoD` (listed in the survey: `flame_xml.rs:1285`, `app/config.rs:312`, `app/export.rs:183`, `fly_camera.rs:316`, `shader_cache.rs:*`, `shader_builder_v2.rs:618`, `ui/solid_panel.rs:16`, `ui/transforms.rs:*`, `ui/export_panel.rs:171`, `ui/triangle_editor.rs:236`, `ui/view.rs:*`, `export/high_res.rs:*`, `compute_kernel.rs` many, `transforms.rs:2302`) treat everything else as 2D and are correct for Simulation as they are for Escape. No change, but they are why the flame-only panels must be hidden rather than trusted.

---

## 2. Config

### `SimConfig` — new file `src/config/sim.rs`, registered in `src/config/mod.rs`

Mirror `EscapeConfig`'s serde discipline (every field `default`, `skip_serializing_if` at its default, so a flame config serialises nothing for it):

| field | type | default | notes |
|---|---|---|---|
| `model` | `String` | `"gray_scott"` | registry name |
| `coloring` | `String` | `"channel"` | sim colouring registry name |
| `grid` | `SimGrid` enum | `Viewport { scale: 1.0 }` | `Fixed { width, height }` or `Viewport { scale }` — pipeline §7; serde as `{"fixed": [w, h]}` / `{"viewport": scale}` |
| `seed` | `u64` | 1 | init RNG |
| `init` | `SimInit` enum | `Noise` | `Noise{amplitude}`, `Blob{radius}`, `Blobs{count,radius}`, `Ring`, `Line`, `Center` (growth seeds) |
| `steps` | `u32` | 2000 | export/settle contract: exact step count from seed |
| `steps_per_frame` | `u32` | 4 | interactive stepping and video export |
| `dt` | `f32` | 1.0 | model time step where the model has one |
| `boundary` | `SimBoundary` enum | model default | `Periodic`, `Clamp`, `Zero`, `Mirror` |
| `warp` | `SimWarp` struct | identity | `zoom`, `rotation`, `pan`, `flow`, `filter` (pipeline §4.1); `is_default` skip. **Built 2026-09-05** |
| `matte` | `SimMatte` struct | off | `channel`, `cutoff`, `softness`, `invert` — which cells are figure and which take the background colour, multiplied into the colouring's coverage in `sim_shade`. **Built 2026-09-05**, not in the original plan |
| `model_params` | `BTreeMap<String, f32>` | empty | packed by name into slots, exactly `pack_params` |
| `coloring_params` | `BTreeMap<String, f32>` | empty | same |
| `agents` | `u32` | 0 | agent count for agent models (model default) |
| `upscale` | `SimUpscale` enum | `Nearest` | resolve filter when grid < output: Nearest, Bilinear, Bicubic (pipeline §4.6) |
| `downscale` | `SimDownscale` enum | `Box` | resolve filter when grid > output: Box, Pyramid |

`is_default()` for the `FractalConfig` field skip; `FractalConfig.sim` next to `.escape` at `src/config/fractal_config.rs:181`, default at `:721`.

### `ConfigPath::Sim*` — `src/config/delta.rs`

Variants (all → a new `UpdateType::SimRerender`; a second `UpdateType::SimReseed` for the fields that restart the run — `SimGridMode`, `SimGridWidth`, `SimGridHeight`, seed, init, model, boundary; and a third, `UpdateType::SimResample`, for `SimGridScale`, which resamples the running field into the new grid instead of restarting — pipeline §7):

`SimModel`, `SimColoring`, `SimGridMode`, `SimGridWidth`, `SimGridHeight`, `SimGridScale`, `SimSeed`, `SimInitKind`, `SimInitAmplitude`, `SimInitRadius`, `SimInitCount`, `SimSteps`, `SimStepsPerFrame`, `SimDt`, `SimBoundary`, `SimWarpZoom`, `SimWarpRotation`, `SimWarpPanX`, `SimWarpPanY`, `SimWarpFlow`, `SimAgents`, `SimUpscale`, `SimDownscale`, `SimModelParam { param }`, `SimColoringParam { param }`.

**As built, 2026-09-05:** the warp shipped with a sixth path,
`SimWarpFilter` (bilinear or nearest — the spec had no filter, and a
bilinear resample every step erases a pattern over thousands of them),
and the matte added four more, `SimMatteChannel`, `SimMatteCutoff`,
`SimMatteSoftness` and `SimMatteInvert`. `SimAgents` is not built: the
agent count comes from the model's own parameters.

For each, the five tables escape fills: Display (`delta.rs:834-867`), i18n key (`:1072-1105`, `history.param.sim_*` — **and put the keys in `locales/en.yml`; escape's 14 shading keys and `escape_downsample` are missing there today, gap**), string key (`:2659-2692`, `Sim.Model` … `Sim.ModelParam.{param}`), parse (`:2856-2895`), and `json_to_config_value` (`:3452-3504`) for the animatable ones: every `f32`/`u32` field is animatable (`Float`/`UInt`); `SimModel`, `SimColoring`, `SimBoundary`, `SimInitKind`, `SimGridMode`, `SimUpscale`, `SimDownscale` are not.

### `ConfigManager` — `src/config/manager.rs`

- `UpdateAction` gains `rerender_sim`, `reseed_sim` and `resample_sim` (`:221`, built `:272-275`, merged `:289`).
- Read arms next to escape's (`:1722-1800`); write arms next to `:2602-2730` with the clamps: fixed grid `clamp(64, max_texture_dimension_2d)` (the device limit is not known here — clamp to 8192 and let the renderer's `allocation_error` refuse), bound `scale.clamp(0.125, 4.0)`, `steps_per_frame.clamp(1, 64)`, `agents.clamp(0, 8_000_000)`, `dt.clamp(1e-4, 10.0)`.
- Coalescing: model params coalesce (sliders); `SimSeed`/`SimModel` should **not** coalesce — first entry in the `supports_coalescing` exclusion list at `:140-147`, which is empty today.
- Tests to mirror from `src/config/escape.rs:727-930`: default serialises to nothing; string-key round trip; animation value conversion; manager + undo flow; full-config round trip.

---

## 3. App layer — `src/app/`

| item | escape today | Simulation |
|---|---|---|
| fields | `escape_renderer: Option<EscapeRenderer>` `mod.rs:390`, `escape_dirty` `:394`, `escape_anim_pending` `:400` | `sim_renderer: Option<crate::sim::SimRenderer>`, `sim_running: bool` (panel Run state, **not** persisted in config), `sim_reseed: bool`, `sim_resample: bool` |
| per-frame branch | `mod.rs:2274-2316`: lazy create, `resize`, `if escape_dirty { render }`, `escape_dirty = !settled`, `request_redraw()` when unsettled | lazy create; grid size = `Fixed` size or `round(viewport × scale)`; `if sim_reseed { reset(seed, init) }` else `if sim_resample \|\| (bound && grid size changed) { resample_into(new grid) }` (`Fixed` ignores viewport resizes — only the resolve ratio changes; pipeline §7); `if sim_running \|\| step_requested { step_batch() }`; **always** `color()` (one pass) so parameter edits that do not step still recolour; `request_redraw()` while running |
| `AboutToWait` | `mod.rs:1108-1112` excludes escape from `is_rendering`; `:1167` lists the continuous-redraw conditions | add `\|\| app.sim_running` to the `:1167` disjunction so VSync-off `target_fps` pacing (`:1173-1186`) applies; keep it out of `is_rendering` (which drives the flame "rendering complete" UI state) |
| GPU updates | `gpu_updates.rs:396-402` sets `escape_dirty` on a superset of triggers | `rerender_sim` ⇒ recolour; `reseed_sim` ⇒ `sim_reseed = true`; `resample_sim` ⇒ `sim_resample = true`; palette change ⇒ recolour only |
| overwrite mode | `gpu_updates.rs:444-447`, `:553-557` | same exclusions |
| device loss | `mod.rs:1007`, `:1038` | drop `sim_renderer`, set `sim_reseed` |
| export from app | `config.rs:263-370` (`export_custom_size`): escape refuses over-binding sizes, frees the viewport renderer, builds a `RenderJob` | sim: `Fixed` renders the configured grid and resolves to the requested size — no size refusal needed; `Viewport` runs a fresh grid of `round(W×H × scale)` and needs the same over-size refusal escape has (through `allocation_error`); the export panel states which will happen (pipeline §7); free the viewport renderer the same way; `steps` from config |
| CLI export routing | `export.rs:186-192` routes by the *flame* histogram size — **gap**: an escape export can be misrouted to the flame-only HighRes path | route on `render_mode` first: `Simulation ⇒ export_headless_gpu` always |
| WASM in-app export | `mod.rs:1761-1905` has its own escape settle loop | add the sim branch: run `steps` in watchdog-sized batches, then `tonemap_pass_with_input(sim.output_view())` |

---

## 4. Headless renderer — `src/renderer/render.rs`

- `render()` `:178`: the OOM precheck at `:189-208` gains a `Simulation` arm calling `SimRenderer::allocation_error(device, &job.config.sim)` (grid textures + pyramid + deposit buffer + agents against `max_texture_dimension_2d`, `max_buffer_size`, and the 128 MB storage-binding limit for the deposit buffer).
- `render_with()` `:260`: `Simulation ⇒ render_sim(...)` under `#[cfg(feature = "engine-sim")]`, else `EngineMissing("simulation")`.
- `render_sim()` mirrors `render_escape()` `:623-866` minus the antialias loop: `load_config` for palette/tonemap parity; error scope; `SimRenderer::new`; `reset(seed, init)`; run exactly `job.config.sim.steps` in batches; `color()`; `resolve` to `job.width × job.height`; density effects → `tonemap_pass_with_input` → colour effects; `oom_scope.pop()`; readback; destroy. `total_iterations = steps` for the PNG metadata.
- `RenderJob` needs no new field: width/height are the *output* size; the grid is the config's `Fixed` size or `round(output × scale)` for `Viewport` (pipeline §7). `render_sim` reports the grid actually used so the PNG metadata can record it (§7).

---

## 5. UI

### Panel — new `src/ui/sim_panel.rs`

`pub fn render_sim_content(ui, config_manager, workspace_request, sim_state: &mut SimUiState)` — the extra argument is the run/pause/step state and the live step counter, which live on the App (§3), passed through `PanelContext` like `workspace_layout_requested` is (`panel_viewer.rs:374`, wired `:868`, `response.rs:175`).

Sections, top to bottom: mode toggle (reuse the escape toggle's shape — `escape_panel.rs:33-50` — and factor the shared `switch_render_mode` so entering Simulation from Escape and vice versa goes through one helper that resets tonemap to Linear exactly once); Run / Pause / Step / Reset with the step counter and measured steps/s; Model dropdown (registry order, presets submenu as escape's `apply_preset` at `escape_panel.rs:1301`); model params via the same `param_control` (dropdowns for `choices`, sliders otherwise — factor `param_control` out of `escape_panel.rs:1034` into a shared `ui/param_control.rs` since both panels need it); Grid (a Bind-to-viewport checkbox; `scale` when bound, width/height when fixed; a readout of the grid in use and the resolve ratio); Upscale / Downscale filters; Seed / Init / Boundary; Steps / Steps per frame / dt; Warp; Coloring dropdown + params + Auto-scale; Agents (only when the model declares agents).

### `PanelType` — `src/ui/workspace.rs`

`PanelType::Simulation` appended (`:11-69`); title `t!("panels.simulation")` in `Display` (`:73`); `default_size_for_panel` (`:204-235`, `vec2(350.0, 560.0)`); `PanelViewer::render_panel` arm (`panel_viewer.rs:864-869` pattern).

### `WorkspaceLayout::Simulation` — `src/ui/workspace.rs:107-125`

`create_simulation_layout(preserve_help)` after `create_escape_layout` `:483`: left dock 0.28 = `Simulation`, right 0.72 = `[Colors, History]`. Requested from the panel's mode toggle exactly as escape does (`escape_panel.rs:49` → `app/mod.rs:1379-1383`). Add the row to `preset_layouts_contain_their_panels` (`:590`), a `simulation_layout_omits_the_flame_only_editors` twin of `:630`, and add `Simulation` to the help-preservation loop `:645-648`.

**gap to close for both modes:** loading a `.fflame` whose `render_mode` is escape or simulation does not switch the layout or open the panel (only the toggle and the menu do). Phase 1 should make `import_config` request the mode's layout when the mode changes — one line in `src/app/config.rs` where the escape flag is already read (`:321`).

### Menus

- `src/ui/menu_bar.rs:221-224` — copy the hand-written Window-menu block for `PanelType::Simulation` (`menu.window_simulation`); `:355-356` — `WorkspaceLayout::Simulation` (`menu.layout_simulation`).
- `src/ui/compact_menu.rs:99` — one row `(PanelType::Simulation, "menu.window_simulation")`.
- **gap (pre-existing):** the Render Mode entries at `menu_bar.rs:144-151` and the fly-mode gate `:490` use a `render_mode_2d: bool`, so "3D" reads selected in escape mode. Replace the bool with the enum when adding the third mode (`src/ui/mod.rs:1451` computes it).

### Viewport — `src/ui/panel_viewer.rs`

Phase 1: sim ignores drag/wheel/pinch (`:460-513`, `:1442`). The overlay at `:1290-1305` shows `step N · S steps/s · running/paused`. Later phase: display-only pan/zoom into the grid (a view transform applied in the resolve pass, no re-simulation).

### Other UI

- `src/ui/tone_mapping.rs:439-448` — Levels hint keyed `tonemap.levels_sim_hint`.
- `src/ui/fractal_browser.rs:559-567`, `:710-712` — **gap (pre-existing):** the online render-mode filter has no Escape option; add both Escape and Simulation when the API knows the value (§9).
- `src/ui/settings.rs` — nothing (no disk cache for sim).

---

## 6. Animation

`src/animation/` has no mode-specific code; tracks address `ConfigPath` string keys, so `Sim.*` keys work through `to_string_key`/`from_string_key` unchanged.

- `src/ui/target_selector.rs`: `TargetCategory::Simulation` (`:44`, label `:59`, id `:74`, gate `:258-264`) and `get_sim_items` next to `get_escape_items` `:310-348`: **`SimStepCount` first — it is the one that animates the simulation itself** — then `SimDt`, `SimWarpZoom`, `SimWarpRotation`, `SimWarpPanX/Y`, `SimWarpFlow`, plus the active model's and colouring's params. **Do not** offer `SimSeed`, `SimGridMode`, `SimGridWidth`/`Height` (each keyframe would reseed), `SimGridScale` (each keyframe would resample), or `SimStepsPerFrame` (it is the interactive Run speed; the timeline uses `SimStepCount`).
- `src/animation/export.rs:751-786` — `apply_config_value` arms for every animatable `Sim*` path (with the same NaN/clamp discipline as `EscapeZoomLog2` at `:767-772`).
- `src/animation/export.rs:1990-2145` — the escape per-frame settle loop is the template, and **the structure already supports a stateful generator**: `escape_renderer` is declared outside the frame loop and created once with `get_or_insert_with`, so its orbit cache carries frame to frame. A `sim_renderer` sits in exactly the same place. Verified against the code 2026-09-04, not just assumed.

  Per frame the order is: evaluate the tracks → `apply_animation_values` into a fresh `frame_config` → `renderer.load_config` for the shared palette/tonemap tail → advance and colour the sim. Keyframed model parameters therefore take effect **before** the step that frame runs, which is what makes "animate F and k while the pattern evolves" work rather than just cross-fading two stills.

### The step count must come from animation TIME, not from frame count

**Decided 2026-09-04.** The obvious design — advance `steps_per_frame`
on every rendered frame — makes the simulation frame-rate dependent,
and that breaks three things the rest of the animation system
guarantees:

- The same project exported at 30 fps and 60 fps gives **different
  pictures at the same timestamp**: twice the frames means twice the
  steps. Every other animatable quantity in this app is a function of
  time and does not care about fps.
- In-app playback advances by wall-clock `delta_time`
  (`app/animation_update.rs:59`) while export advances by
  `frame / fps` (`export.rs:472`). A frame-counted simulation would
  make the preview and the export diverge, and the preview would differ
  again on a slower machine.
- Seeking is undefined. A track evaluated at time *t* has one value; a
  frame-counted simulation has whatever history the playhead happened
  to take to get there.

So the timeline drives a **cumulative step count**, `Sim.StepCount`,
which is an ordinary animatable float: the simulation state at time *t*
is `round(track(t))` steps from the seed, full stop. That restores the
property the animation system assumes everywhere else — a frame is a
function of its time — and it is strictly more expressive than a rate,
because easing the track gives slow-in/slow-out on the *simulation*
and a hold gives a freeze-frame that keeps animating colour and warp.

- Default track for a new animation: a linear ramp `0 → sim.steps` over
  the duration, i.e. constant speed, which is what a rate would have
  given.
- Advancing is incremental and cheap: the renderer already holds
  `step_index`, so a frame runs `target − step_index` steps.
- **Going backwards costs a reseed and a re-run**, because the rule is
  not invertible. That is the documented price of scrubbing back, the
  timeline shows a "re-simulating" state, and it is why the track is
  the right place for it: the exporter can see a decrease coming
  instead of discovering it.
- `steps_per_frame` stays in the config, but it means only what it says
  for the interactive **Run** button — free-running speed when no
  timeline is driving. It is not an animation target.

The exporter keeps one `SimRenderer` for the whole export and reseeds
only when a reseed-class path changes or the step count moves
backwards.
- `src/app/animation_update.rs:44-59` — playback in the app: same stateful rule; no settle-then-jump.
- Built-in script `assets/scripts/modifiers/zoom_dive.rhai` style: a `sim_sweep.rhai` that keyframes F/k across a Pearson row is the obvious shipped example.

---

## 7. Export paths

| path | change |
|---|---|
| CLI (`src/main.rs:368` → `lib.rs:137` → `app/export.rs:156`) | route on mode before the histogram-size decision (`export.rs:186-192`) |
| `src/export/high_res.rs` | never reached; total-match arm only (`:1714-1718`) |
| thumbnails `src/renderer/thumbnail.rs:26-30` | inherit via `render()` — **but** cap `steps` for thumbnails (a 20,000-step still is too slow for a gallery tile): `RenderJob` gains nothing; the thumbnail path overrides `config.sim.steps = min(steps, THUMBNAIL_SIM_STEPS)` on a cloned config, the way it already fixes iterations for flames |
| video `src/animation/export.rs` | §6 |
| export panel `src/ui/export_panel.rs` | show the grid the export will use: `Fixed` ⇒ "512×512 grid, upscaled ×7.5"; `Viewport` ⇒ "re-simulates on a 3840×2160 grid" (pipeline §7) |
| PNG metadata `src/png_metadata.rs:74-87`, `:162`, `:314` | `("Simulation", "N/A")`; write `SimSteps`, `SimSeed` and `SimGrid` (the size actually used) as extra tEXt keys next to `Iterations` so a PNG is reproducible |
| `tests/visual/run_tests.py:433-457` | the escape rule (`RenderMode == "Escape"` ⇒ exclude from the iterations/s floor) gains `"Simulation"` |

---

## 8. Scripting — `src/script/api.rs`

- `pub struct SimHandle { cfg: Rc<RefCell<FractalConfig>> }` under `#[cfg(feature = "engine-sim")]` next to `EscapeHandle` (`:94-98`); `scope.push("sim", …)` at `:349`; `register_type_with_name::<SimHandle>("Sim")` at `:364`; `register_sim(engine)` at `:373`.
- Methods: `enter()` (mode + Linear tonemap, mirroring `:1433-1444`), `model(name)`, `models()`, `coloring(name)`, `colorings()`, `param(name, v)`, `params()`, `coloring_param`, `coloring_params`, `grid(w, h)`, `seed(n)`, `init(kind, …)`, `steps(n)`, `steps_per_frame(n)`, `dt(x)`, `boundary(name)`, `warp(zoom, rotation)`, `agents(n)`, `preset(name)`.
- Document every name in `docs/main/SCRIPTING.md` (a `sim.*` table after the `escape.*` one at `:225-247`) **before** running tests: `every_script_api_name_is_documented` (`api.rs:3127-3222`) fails otherwise; `"Sim"` joins `NOT_VOCABULARY` (`:3153-3159`) only if the type name is not documented.
- `src/script/tests.rs` — a test that a script sets `RenderMode::Simulation` (twin of `:4069`); `src/script/library.rs:533` — the shipped-scripts test gate gains `engine-sim` if a sim script ships.

---

## 9. Online API and the contract

- `src/api/types.rs:50-68` — `ApiRenderMode::Simulation` with `#[serde(rename = "simulation")]` and the same doc note escape carries (`:56-65`): stored as a flame with zero transforms.
- `src/api/sync.rs:16-34` — both conversion matches.
- **The server's Postgres `render_mode` enum will not know `"simulation"`.** Escape went through this in 2026-08-29 (`sync.rs:341-345`, `:362-403`): the client refused until the server enum gained the value. Reinstate the same client-side refusal for Simulation (`FetchError::Unsupported` still exists at `src/resources/error.rs:26` for exactly this), file the server change with the API repository **first**, and remove the refusal when the value lands. Also `docs/main/openapi.json` (`:6207` has escape).
- `src/contract.rs:256-274` — nothing to write by hand: `render_modes.known` is generated from `RenderMode::ALL`. Regenerate with `UPDATE_CONTRACT=1 cargo test --lib contract_is_current`; `every_render_mode_is_published_in_the_contract` (`:386-410`) must be updated to `len() == 4`. Per `docs/RELEASE.md:121-127`, a new render mode **does not move the shape fingerprint** — tell the API repository directly (the escape narrative at `RELEASE.md:141-153` is the precedent).
- **gap (pre-existing):** `src/scene/transforms.rs:1847-1849` and `CLAUDE.md:63` still say Save Online refuses escape client-side; it no longer does. Fix both when the sim note is added.

---

## 10. i18n — `locales/en.yml`

New keys: `menu.window_simulation`, `menu.layout_simulation`, `panels.simulation`, `fractal_gallery.render_mode_simulation`, `history.param.sim_*` (one per `ConfigPath::Sim*`), `history.action.sim_preset`, `tonemap.levels_sim_hint`, and a `sim_panel:` section (labels, tooltips, `flame_only_hint` twin). The escape panel's `escape_panel.flame_only_hint` wording applies to both hidden-panel directions.

**gap (pre-existing):** `es.yml`, `ja.yml`, `zh-CN.yml` carry no escape keys at all; the new mode should at least ship its `panels.*` and `menu.*` keys in all four so the menus are not half-translated.

---

## 11. Tests

- `tests/visual/configs/sim/` — one config per model at a small grid (256²) and a fixed `steps`, `deterministic_rng: true`; `run_tests.py:614` `choices` list gains `"sim"` (it gained `escape`/`solid` only on 2026-09-01). Discovery is `rglob`, so the directory is picked up automatically.
- `src/sim/app_repro_test.rs` — twin of `src/escape/app_repro_test.rs` (declared like `src/escape/mod.rs:25-26`): the app's exact frame sequence against a fresh `SimRenderer`; a "two runs from the same seed are byte-identical" test; an "export at N steps equals N single steps" test (batching must not change the sequence); a `render()` OOM-refusal test at an absurd grid.
- `src/sim/assembler.rs` tests — naga-validate every model × colouring (the escape `every_field_coloring_combination_validates` shape at `src/escape/assembler.rs:3604`); `shader_lint` covers any new `shaders/*.wgsl` automatically.
- CPU reference tests per model where the rule is cheap to mirror (Gray–Scott one step on a 16² grid, cyclic CA, sandpile toppling): GPU one step vs CPU, class-compared like the variation probe.
- `src/contract.rs` tests (§9), `src/ui/workspace.rs` layout tests (§5), `src/config/sim.rs` round-trip tests (§2), script doc coverage (§8).

---

## 12. Docs to update when the code lands

`CLAUDE.md` (the `src/sim/` summary next to `src/escape/`, the render-mode list, the build/test commands), `docs/ARCHITECTURE.md` (**gap:** it has zero escape mentions today — add both modes), `docs/main/RENDERER.md`, `CONFIG.md`, `UI.md`, `SCRIPTING.md`, `EXPORT.md`, `docs/RELEASE.md` (the render-mode rule already covers it), `docs/WASM.md` (add the storage-format and uniformity rules the survey found only in CLAUDE.md and code comments).

---

## 13. Feature gating

`engine-sim` in `Cargo.toml` next to `engine-escape` (`:210`, in `default` at `:190`), gating: `src/lib.rs` (module), `src/renderer/render.rs` (branch + precheck), `src/script/api.rs` (handle), `src/animation/export.rs` (branch), `src/contract.rs` (the two cfg-gated tests), `src/script/library.rs` (shipped-script gate). The gallery WASM crates (`wasm/render`, `wasm/script`) default-features = false and opt in per engine; a `wasm/sim` gallery module follows `wasm/escape` when the mode ships.
