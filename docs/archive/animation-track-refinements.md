# Animation Track Refinements

**Status: Complete**

## Context

Signal generators (added in the Signal Panel refactor) make Oscillator and Circular track types redundant. Oscillators are just sine/triangle/sawtooth/square waveforms — Signal generators produce the exact same waveforms as named signals. Circular motion can be achieved with two Signal tracks using sine/cosine generators with phase offsets. Removing these simplifies the codebase and the UI.

Additionally, Signal tracks need **start/end time** support (active only within a time window) and **separate fade in/out** with configurable easing (smooth amplitude transitions at boundaries).

## Key Files

| File | Change |
|---|---|
| `src/animation/mod.rs` | Remove `Oscillator` variant, `OscillatorType`, `CircularTrack`, `circular_tracks` field; add Signal timing/fade fields |
| `src/animation/controller.rs` | Remove oscillator/circular evaluation; add timing/fade logic to `evaluate_signal()` |
| `src/animation/interpolation.rs` | No changes (reuse existing `EasingFunction`) |
| `src/ui/track_editor.rs` | Remove Oscillator/Circular UI; add Signal timing/fade controls |
| `locales/en.yml` | Remove oscillator/circular i18n keys; add signal timing/fade keys |

## Step 1: Remove Oscillator from data model (`src/animation/mod.rs`)

- Delete `TrackSource::Oscillator { .. }` variant (lines 133-145)
- Delete `OscillatorType` enum (lines 161-172)
- Delete `Track::oscillator()` factory method (lines 513-532)
- Delete `Track::oscillator_with_phase()` factory method (lines 534-554)
- Update module doc comment (line 4): remove "oscillators, circular motion"
- Update tests:
  - Delete `test_oscillator_track_json` (lines 740-762)
  - Update `test_full_animation_json` (lines 789-830): remove oscillator track, update assertions

## Step 2: Remove CircularTrack from data model (`src/animation/mod.rs`)

- Delete `CircularTrack` struct (lines 174-192)
- Delete `CircularTrack` impl block (lines 609-643)
- Delete `circular_tracks` field from `Animation` struct (lines 39-41)
- Delete `add_circular_track()` method (lines 274-279)
- Delete `remove_circular_track()` method (lines 290-297)
- Remove `circular_tracks: Vec::new()` from `Animation::new()` and `with_config()` (lines 238, 251)
- Remove all `circular_tracks` handling from `on_transform_removed()`, `on_color_effect_removed()`, `on_density_effect_removed()`, `on_color_effect_reordered()`, `on_density_effect_reordered()` (the blocks that iterate `self.circular_tracks`)
- Delete `test_circular_track_json` test (lines 764-787)
- Update `test_full_animation_json`: remove circular track, update assertions

## Step 3: Add timing and fade fields to Signal variant (`src/animation/mod.rs`)

Enhance `TrackSource::Signal` with:

```rust
Signal {
    signal_name: String,
    min_output: f64,
    max_output: f64,
    #[serde(default)]
    smoothing: f64,
    /// Start time in seconds (0.0 = start of animation)
    #[serde(default)]
    start_time: f64,
    /// End time in seconds (0.0 = end of animation, meaning "use full duration")
    #[serde(default)]
    end_time: f64,
    /// Fade in duration in seconds
    #[serde(default)]
    fade_in: f64,
    /// Fade in easing curve
    #[serde(default)]
    fade_in_easing: EasingFunction,
    /// Fade out duration in seconds
    #[serde(default)]
    fade_out: f64,
    /// Fade out easing curve
    #[serde(default)]
    fade_out_easing: EasingFunction,
}
```

All new fields default to `0.0`/`EasingFunction::Linear` via `#[serde(default)]` for backward compatibility.

- `end_time = 0.0` means "use animation duration" (sentinel value, avoids `Option` in serde)
- Update `Track::signal()` and `Track::signal_with_smoothing()` to initialize new fields to defaults

**Import**: `EasingFunction` is already re-exported via `pub use interpolation::{EasingFunction, Interpolation};` in `mod.rs`. No additional import needed — do NOT add a separate `use interpolation::EasingFunction;` (it causes E0252 duplicate definition).

## Step 4: Update controller evaluation (`src/animation/controller.rs`)

### Remove oscillator evaluation
- Delete `evaluate_oscillator_pure()` method (lines 318-352)
- Remove `TrackSource::Oscillator` match arms from `evaluate_track_pure()` (lines 240-242) and `evaluate_source()` (lines 262-263)
- Remove `OscillatorType` from imports (line 3)

### Remove circular evaluation
- Remove circular track evaluation from `evaluate_at_time()` (lines 173-178)
- Remove `circular_info` collection and loop from `evaluate_at_time_with_signals()` (lines 203-228)
- Remove `CircularTrack` from imports (line 478 in test module)

### Add timing/fade to `evaluate_signal()`

Update `evaluate_signal()` signature to accept new fields and add logic:

```rust
fn evaluate_signal(
    &mut self,
    signal_name: &str,
    min_output: f64,
    max_output: f64,
    smoothing: f64,
    start_time: f64,
    end_time: f64,
    fade_in: f64,
    fade_in_easing: EasingFunction,
    fade_out: f64,
    fade_out_easing: EasingFunction,
    time: f64,
    duration: f64,  // animation duration for end_time=0 sentinel
    signal_manager: Option<&SignalManager>,
    track_idx: Option<usize>,
) -> Option<serde_json::Value> {
    // Resolve end_time sentinel
    let effective_end = if end_time <= 0.0 { duration } else { end_time };

    // Outside active window → return None (track inactive)
    if time < start_time || time > effective_end {
        return None;
    }

    // ... existing signal lookup and smoothing ...

    // Calculate fade factor (0.0 = fully faded, 1.0 = fully active)
    let mut fade_factor = 1.0;

    // Fade in
    if fade_in > 0.0 && time < start_time + fade_in {
        let t = ((time - start_time) / fade_in).clamp(0.0, 1.0);
        fade_factor *= fade_in_easing.apply(t);
    }

    // Fade out
    if fade_out > 0.0 && time > effective_end - fade_out {
        let t = ((effective_end - time) / fade_out).clamp(0.0, 1.0);
        fade_factor *= fade_out_easing.apply(t);
    }

    // Apply fade to signal amplitude
    let mapped = min_output + fade_factor * (max_output - min_output) * value as f64;
    Some(serde_json::json!(mapped))
}
```

Update `evaluate_source()` to pass the new fields through when matching `TrackSource::Signal`.

Update `evaluate_at_time_with_signals()` to pass animation duration to `evaluate_source()`.

### Update tests
- Delete `test_oscillator_sine` (line 577), `test_oscillator_square` (line 607)
- Delete `test_circular_track` (line 664), `test_evaluate_frame_with_circular` (line 690)

## Step 5: Update track editor UI (`src/ui/track_editor.rs`)

### Remove Oscillator/Circular UI
- Remove `OscillatorParams` struct and Default impl (lines 48-68)
- Remove `CircularParams` struct and Default impl (lines 70-90)
- Remove `NewTrackType::Oscillator` and `NewTrackType::Circular` variants (lines 124-125)
- Remove imports: `CircularTrack`, `OscillatorType` (lines 11-12)
- Remove `oscillator_params` and `circular_params` fields from `TrackEditorState` (lines 36-39)
- Remove `target_selector_state_y` and `new_track_target_y` fields (lines 27, 35)
- Delete `render_oscillator_subpanel()` function (lines 944-981)
- Delete `render_circular_subpanel()` function (lines 983-1011)
- Delete `oscillator_type_name()` function (lines 1258-1266)
- Delete `initialize_oscillator_center()` function (lines 1472-1482)
- Delete `initialize_circular_centers()` function (lines 1484-1499)
- Remove `NewTrackType::Oscillator` and `NewTrackType::Circular` from type selector combo box (lines 551-558)
- Remove their match arms from target initialization (lines 612-617)
- Remove Y target selector section for Circular tracks (lines 627-672)
- Remove their match arms from subpanel dispatch (lines 683-688)
- Remove Oscillator/Circular cases from `update_or_create_track()` (lines 1108-1168)
- Remove Oscillator handling from `open_edit_track_panel()` (lines 1235-1243)
- Remove circular track visual rendering section (lines 448-468)
- Remove `circular_tracks.len()` from track count (line 161)
- Remove `oscillator_params`/`circular_params` resets from `open_add_track_panel()` (lines 1220-1221)

### Add Signal timing/fade controls to `SignalParams`

Update `SignalParams` struct:
```rust
pub struct SignalParams {
    pub signal_name: String,
    pub min_output: f64,
    pub max_output: f64,
    pub smoothing: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub fade_in: f64,
    pub fade_in_easing: EasingFunction,
    pub fade_out: f64,
    pub fade_out_easing: EasingFunction,
}
```

### Enhance `render_signal_subpanel()`

Add after existing smoothing control:
- **Start Time**: `DragValue` with range `0.0..=duration`, suffix "s"
- **End Time**: `DragValue` with range `0.0..=duration`, suffix "s" (0 = "End")
- **Fade In**: `DragValue` (seconds, `0.0..=30.0`), + `EasingFunction` combo box
- **Fade Out**: `DragValue` (seconds, `0.0..=30.0`), + `EasingFunction` combo box

Pass `duration` into `render_signal_subpanel()` (needs new param from caller).

### Update Signal track creation/editing

Update `update_or_create_track()` Signal case to include new fields.
Update `open_edit_track_panel()` Signal case to populate new fields.

## Step 6: Update i18n (`locales/en.yml`)

### Remove
- `type_oscillator`, `type_circular`
- `circular_label`
- Oscillator control keys: `waveform`, `waveform_sine`, `waveform_triangle`, `waveform_sawtooth`, `waveform_square`
- Oscillator parameter keys: `osc_center`, `osc_amplitude`, `osc_frequency`, `osc_phase`
- `oscillator_section`, `circular_section`
- Circular parameter keys: `circ_center_x`, `circ_center_y`, `circ_radius`, `circ_speed`, `circ_phase`
- `target_y`, `select_target_y`, `change_target_y`

### Add
- `signal_start_time: "Start Time"`
- `signal_end_time: "End Time"`
- `signal_fade_in: "Fade In"`
- `signal_fade_in_easing: "Fade In Easing"`
- `signal_fade_out: "Fade Out"`
- `signal_fade_out_easing: "Fade Out Easing"`

## Backward Compatibility

- Old `.anim` files with `Oscillator` tracks: **will fail to deserialize** (variant removed). This is acceptable since:
  - Oscillator tracks are a brand-new feature from the current branch
  - No production .anim files use them yet
  - Signal generators provide the same functionality
- Old `.anim` files with `circular_tracks`: **will load fine** — `#[serde(default)]` means missing field defaults to empty vec. Since we remove the field, old files with it will have it silently ignored by serde.
- New Signal fields (`start_time`, `end_time`, `fade_in`, etc.): all `#[serde(default)]` → old Signal tracks load with defaults (0.0 = full duration, no fade).

## Verification

1. [x] `cargo build` — compiles (warnings only, no errors)
2. [x] `cargo test` — 174 passed, 0 failed
3. [x] Track editor shows only Keyframe and Signal track types
4. [x] Signal tracks can be created with start/end times and fade in/out
5. [x] During playback, Signal track respects start/end window
6. [x] Fade in/out smoothly transitions amplitude at boundaries
7. [x] Existing .anim files with Signal tracks load correctly (timing fields default via `#[serde(default)]`)
8. [x] No references to Oscillator or Circular remain in codebase (grep verified)
