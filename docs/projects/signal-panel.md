# Signal Panel

## Overview

Refactor the Audio panel into a broader **Signal panel** that manages all signal sources: audio-derived signals, procedural generators, and loaded signal files.

## Motivation

The Audio panel currently handles audio file loading, playback, live capture, and signal monitoring. As the animation system grows, signals from non-audio sources (procedural waveforms, external data) need first-class support. The Signal panel unifies all signal management in one place.

## Architecture

### Panel Structure

```
Signal Panel
├─ Audio (collapsible)
│  ├─ Audio File (load, analyze)
│  ├─ Playback (play/pause/stop)
│  └─ Live Capture (device, start/stop)
├─ Signal Generators (collapsible)
│  └─ Procedural waveforms (sine, triangle, sawtooth, square, noise)
├─ Signal Files (collapsible)
│  └─ Save/load .signal binary files
└─ Signal Monitor (always visible)
   └─ Real-time meters for all signals at current playback time
```

### Generator Signals

Generator signals are procedural waveforms defined by configuration:

```rust
pub struct GeneratorConfig {
    pub name: String,           // Signal name (e.g., "sine_1hz")
    pub waveform: WaveformType, // Sine, Triangle, Sawtooth, Square, Noise
    pub frequency: f64,         // Hz
    pub phase: f64,             // 0.0-1.0
}
```

- Output range: 0.0 to 1.0 (animation tracks handle amplitude mapping)
- Pre-computed into Signal data arrays and inserted into SignalManager
- Stored as config in .anim files (no binary data)
- Regenerated on parameter change or animation load

### Signal Flow

```
Generator Config ─→ generate_signal() ─→ Signal ─→ SignalManager
Audio File ─→ AudioAnalyzer ─→ Signal ─→ SignalManager
.signal File ─→ load_from_file() ─→ Signal ─→ SignalManager
Live Capture ─→ SignalProducer ─→ SignalManager

SignalManager ─→ AnimationController.evaluate_signal() ─→ Config updates
```

### Persistence

- **Generator configs**: Stored in `Animation.generators` field of .anim files
- **Audio signals**: Saved as .signal binary files (FSIG format), referenced by name
- **No binary data in .anim files**: Animation tracks reference signals by name

### BPM System

- **Offline**: Auto-detected via autocorrelation of bass energy
- **Editable**: User can override detected BPM; beat/beat_phase signals regenerate
- **Live**: Running autocorrelation during capture with exponential smoothing
- **Beat confirmation**: Predicted beats compared with actual onsets

## Key Files

| File | Purpose |
|---|---|
| `src/ui/signal_panel.rs` | Panel UI (renamed from audio_panel.rs) |
| `src/signal/generator.rs` | GeneratorConfig, WaveformType, signal generation |
| `src/signal/mod.rs` | SignalManager, Signal, SignalProducer trait |
| `src/audio/capture_common.rs` | Live BPM tracking in RealtimeAnalyzer |
| `src/animation/mod.rs` | Animation.generators field |


# Signal Panel Refactor

## Context

The Audio panel currently handles audio file loading, playback, live capture, and signal monitoring. We want to evolve it into a broader **Signal panel** that encompasses all signal sources: audio-derived, procedural generators, and loaded signal files. This also addresses a bug where offline signal values don't update during playback, adds editable BPM, live BPM detection, and generator signals.

## Key Files

| File | Role |
|---|---|
| `src/ui/audio_panel.rs` → `signal_panel.rs` | Panel UI (rename + restructure) |
| `src/ui/panel_viewer.rs` | PanelContext — needs SignalManager added |
| `src/ui/workspace.rs` | PanelType enum — Audio → Signal |
| `src/ui/mod.rs` | Module declaration, state fields, render_ui params |
| `src/ui/menu_bar.rs` | Window menu item |
| `src/signal/mod.rs` | SignalManager, Signal, SignalProducer trait |
| `src/signal/generator.rs` | **New** — GeneratorConfig, waveform generation |
| `src/animation/mod.rs` | Animation struct — add `generators` field |
| `src/audio/capture_common.rs` | RealtimeAnalyzer — add live BPM tracking |
| `locales/en.yml` | i18n keys (audio: → signal: section) |
| `src/app/mod.rs` | Pass signal_manager to UI |

## Step 1: Rename Audio → Signal panel

Pure rename, no behavior change.

- **Rename file** `src/ui/audio_panel.rs` → `src/ui/signal_panel.rs`
- **Rename struct** `AudioPanelState` → `SignalPanelState`
- **Rename function** `render_audio_panel()` → `render_signal_panel()`
- **Update** `src/ui/mod.rs`: `pub mod signal_panel`, field name `signal_panel_state`
- **Update** `src/ui/workspace.rs`: `PanelType::Audio` → `PanelType::Signal`, Display impl
- **Update** `src/ui/panel_viewer.rs`: match arm, PanelContext field name, method name
- **Update** `src/ui/menu_bar.rs`: `PanelType::Signal`, i18n key
- **Update** `locales/en.yml`: `panels.signal: "Signal"`, `menu.window_signal: "📊 Signal"`

## Step 2: Pass SignalManager + current_time to panel

The panel needs `SignalManager` for signal access/save/load and `current_time` for the monitor bug fix.

- **`src/ui/panel_viewer.rs`**: Add `signal_manager: &'a mut SignalManager` to PanelContext
- **`src/ui/mod.rs`**: Add `signal_manager` param to `render_ui()`, thread to PanelContext
- **`src/app/mod.rs`**: Pass `&mut self.signal_manager` to `render_ui()`
- **`src/ui/signal_panel.rs`**: Add `signal_manager: &mut SignalManager` and `current_time: f64` params to `render_signal_panel()`

## Step 3: Fix Signal Monitor bug

Offline signals show `data[0]` instead of value at current playback time.

- **`src/ui/signal_panel.rs`**: Replace `signal.data.first().copied().unwrap_or(0.0)` with `signal.value_at(current_time).unwrap_or(0.0)` at all offline signal display points (~4 places)
- `current_time` comes from `animation_controller.current_time` (already in PanelContext)

## Step 4: Restructure panel into collapsible sections

Wrap audio sections inside "Audio" collapsing header. Add new top-level sections.

Layout:
```
Signal Panel
├─ Audio (collapsible, default open)
│  ├─ Audio File (load, analyze)
│  ├─ Playback (play/pause/stop)
│  └─ Live Capture (device, start/stop)
├─ Signal Generators (collapsible)
│  └─ [placeholder — Step 7]
├─ Signal Files (collapsible)
│  └─ [placeholder — Step 8]
└─ Signal Monitor (always visible)
   └─ [existing monitor, now with current_time fix]
```

## Step 5: Editable BPM

- Add `user_bpm: Option<f32>` to `SignalPanelState`
- In Signal Monitor, replace BPM label with `DragValue` (range 30-300, speed 0.1)
- When user edits BPM: update the "bpm" Signal in SignalManager, regenerate beat_phase and beat signals using existing `generate_beat_phase()` logic
- "Reset" button to restore auto-detected value

## Step 6: Generator signal infrastructure

New file `src/signal/generator.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub name: String,
    pub waveform: WaveformType,
    pub frequency: f64,        // Hz
    #[serde(default)]
    pub phase: f64,            // 0.0-1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveformType {
    Sine, Triangle, Sawtooth, Square, Noise,
}
```

- `GeneratorConfig::generate_signal(duration, sample_rate) -> Signal` — pre-computes data array
- Reuse waveform math from `AnimationController::evaluate_oscillator_pure()` (controller.rs:319)
- Output range: 0.0 to 1.0 (mapping to parameter ranges done by animation tracks)
- Insert generated Signal into SignalManager by name
- Add `pub mod generator` to `src/signal/mod.rs`

**Animation persistence**: Add `#[serde(default)] pub generators: Vec<GeneratorConfig>` to `Animation` struct. No binary data in .anim — generators are just config.

## Step 7: Generator UI

In "Signal Generators" section:
- List generators with name, waveform, frequency
- "Add Generator" button → creates default sine 1Hz
- Per-generator row: name text field, waveform dropdown (Sine/Triangle/Sawtooth/Square/Noise), frequency DragValue (0.01-100 Hz), phase slider (0-1), delete button
- On any change: regenerate signal, insert into SignalManager
- Generators stored in `SignalPanelState.generators: Vec<GeneratorConfig>`
- Sync to/from `Animation.generators` on load/save

## Step 8: Signal file save/load UI

In "Signal Files" section:
- List loaded .signal files with name and duration
- "Load Signal..." button → `rfd::FileDialog` with `.signal` filter, calls `SignalManager::load_from_file()`
- Per signal: "Save" button → `rfd::FileDialog`, calls `SignalManager::save_to_file(name, path)`
- Uses existing binary format (FSIG, already implemented in `signal/mod.rs`)

## Step 9: Live BPM detection

Add running BPM detection to `RealtimeAnalyzer` in `src/audio/capture_common.rs`:

- Add `bass_energy_buffer: Vec<f32>` ring buffer (~8 seconds at analysis rate)
- Every ~2 seconds of accumulated data, run autocorrelation (reuse algorithm from `analyzer.rs:detect_bpm`)
- Exponentially smooth BPM estimate: `bpm = prev_bpm * 0.7 + new_bpm * 0.3`
- Track beat phase from smoothed BPM
- Confirm beats: compare predicted beat times with actual onset peaks, adjust phase
- Expose via `AtomicSignals`: add `live_bpm: AtomicU32`, `live_beat_phase: AtomicU32`
- Show in Signal Monitor when capturing

## Step 10: Animation .anim compatibility

- Old .anim files without `generators` load fine (`#[serde(default)]`)
- On animation load: regenerate signals from `animation.generators` configs
- On animation save: sync `SignalPanelState.generators` → `animation.generators`
- Signal tracks reference signals by name — .signal binary files loaded separately

## Post-processing

Keep in animation tracks (existing system handles this well):
- `TrackSource::Signal.smoothing` — frame-rate independent EMA
- `TrackSource::Signal.min_output/max_output` — amplitude mapping
- Generator signals produce raw 0-1 waveforms; tracks handle the rest

## Verification

1. `cargo build` — compiles at each step
2. `cargo test` — all pass
3. Panel opens as "Signal" with collapsible Audio section
4. Signal Monitor shows live values at current playback time
5. Generator signals appear in monitor and work with animation tracks
6. BPM editable, beat signals regenerate
7. .signal files save/load correctly
8. .anim files save/load generators, backward compatible
9. Live BPM stabilizes within ~5 seconds during capture
