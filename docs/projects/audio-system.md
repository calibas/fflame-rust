# Audio System Design

## Overview

Audio integration for the fractal flame renderer, enabling:
1. **Offline audio analysis** - Pre-analyze audio files to extract signals for animation
2. **Live audio input** - Real-time audio analysis during playback (limited features)
3. **Audio playback** - Play audio synced to animation timeline
4. **Audio export** - Include audio in exported MP4 files (future)

Audio dependencies are always included (no feature flag). The audio system has zero impact on performance when not in use.

---

## Architecture

### Module Structure

```
src/signal/
  mod.rs              - Signal struct, SignalType, SignalManager, SignalProducer trait

src/audio/
  mod.rs              - Public API, AudioManager (implements SignalProducer)
  analyzer.rs         - STFT, mel spectrogram, onset detection
  playback.rs         - Desktop audio playback via cpal
  playback_wasm.rs    - WASM audio playback via Web Audio API
  capture.rs          - Live audio capture trait
  capture_native.rs   - Desktop capture via cpal
  capture_wasm.rs     - WASM capture via web-sys ScriptProcessorNode
  decode.rs           - Audio file decoding (symphonia)
```

### Dependencies

All audio dependencies are always included (no feature flag):

```toml
[dependencies]
cpal = "0.15"                        # Audio I/O (desktop)
symphonia = { version = "0.5",       # Audio decoding (MP3, WAV, FLAC, OGG)
  features = ["mp3", "wav", "flac", "ogg", "pcm"] }
rustfft = "6.2"                      # FFT analysis
ringbuf = "0.4"                      # Thread-safe ring buffers
```

WASM uses `web-sys` features for Web Audio API (playback, capture) instead of cpal.

---

## Generalized Signal System

Signals are the **universal intermediate representation** for time-varying data that drives animation.
The animation system doesn't care where signals come from - it just consumes them.

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Signal Sources                          │
│                                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │  Audio   │  │  .signal │  │  MIDI    │  │  External  │  │
│  │ Analysis │  │   File   │  │ (future) │  │  (future)  │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └─────┬──────┘  │
│       │             │             │              │          │
└───────┼─────────────┼─────────────┼──────────────┼──────────┘
        │             │             │              │
        ▼             ▼             ▼              ▼
┌─────────────────────────────────────────────────────────────┐
│                         Signal                               │
│                                                             │
│  • name: String                                             │
│  • sample_rate: f64                                         │
│  • signal_type: SignalType                                  │
│  • data: Vec<f32>                                           │
│  • metadata: Option<SignalMetadata>                         │
│                                                             │
└──────────────────────────────┬──────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   TrackSource::Signal                        │
│                                                             │
│  • signal_name: String                                      │
│  • min_output / max_output, smoothing                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Signal Struct

```rust
/// A time-indexed signal that can drive animation parameters.
/// Source-agnostic - could come from audio, MIDI, sensors, files, etc.
pub struct Signal {
    pub name: String,
    pub sample_rate: f64,
    pub signal_type: SignalType,
    pub data: Vec<f32>,
    pub metadata: Option<SignalMetadata>,
}

pub enum SignalType {
    Continuous,              // Value 0.0-1.0 (band energy, amplitude, etc.)
    Trigger,                 // Binary (1.0 = triggered, 0.0 = not)
    Scalar { unit: String }, // Value with units (BPM, Hz)
}

pub struct SignalMetadata {
    pub source: Option<String>,              // e.g., "audio_file: track.mp3"
    pub params: HashMap<String, Value>,      // Analysis parameters
    pub unit: Option<String>,                // For Scalar type
    pub created_at: Option<String>,          // ISO 8601 timestamp
    pub extra: HashMap<String, Value>,       // Extension point
}
```

Key methods on `Signal`:
- `value_at(time) -> Option<f32>` — Interpolates for Continuous, nearest-sample for Trigger, constant for Scalar
- `duration() -> f64`
- `load_from_file(path) / save_to_file(path)` — Binary `.signal` format

### Signal File Format (`.signal`)

Binary format for storing/exchanging signals:

```
Header (fixed size):
  magic: [u8; 4] = "FSIG"           // 4 bytes
  version: u16                       // 2 bytes (currently 1)
  signal_type: u8                    // 1 byte (0=Continuous, 1=Trigger, 2=Scalar)
  flags: u8                          // 1 byte (reserved)
  sample_rate: f64                   // 8 bytes
  data_len: u64                      // 8 bytes (number of f32 values)
  name_len: u16                      // 2 bytes
  metadata_len: u32                  // 4 bytes (0 if no metadata)
                                     // Total header: 30 bytes

Variable sections:
  name: [u8; name_len]               // UTF-8 encoded name
  data: [f32; data_len]              // Little-endian f32 values
  metadata: [u8; metadata_len]       // Optional JSON blob (UTF-8)
```

**File size estimate:**
- 3 minute signal @ 100 Hz = 18,000 samples = ~72 KB per signal
- 10 signals = ~720 KB total (very compact)

### Signal Manager

```rust
pub struct SignalManager {
    signals: HashMap<String, Signal>,
    live_producers: Vec<Box<dyn SignalProducer>>,
}

impl SignalManager {
    // Basic CRUD
    pub fn insert(&mut self, signal: Signal);
    pub fn get(&self, name: &str) -> Option<&Signal>;
    pub fn remove(&mut self, name: &str) -> Option<Signal>;
    pub fn signal_names(&self) -> Vec<&str>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Signal)>;
    pub fn clear(&mut self);

    // File I/O
    pub fn load_from_file(&mut self, path: &Path) -> io::Result<&Signal>;
    pub fn save_to_file(&self, name: &str, path: &Path) -> io::Result<()>;

    // Signal access
    pub fn get_value_at(&self, name: &str, time: f64) -> Option<f32>;   // Offline/export
    pub fn get_live_value(&self, name: &str, time: f64) -> Option<f32>; // Live (producers first, then stored)

    // Producer management
    pub fn add_producer(&mut self, producer: Box<dyn SignalProducer>);
    pub fn import_from_producer(&mut self, producer: &dyn SignalProducer); // Batch import
}
```

### Signal Producer Trait

```rust
pub trait SignalProducer: Send {
    fn signal_names(&self) -> Vec<String>;
    fn get_live_value(&self, name: &str) -> Option<f32>;
    fn get_signal(&self, name: &str) -> Option<Signal>;
    fn is_active(&self) -> bool;
}
```

`AudioManager` implements `SignalProducer`, making audio analysis signals available to the animation system.

### Signal Sources

| Source | Status | Description |
|--------|--------|-------------|
| Audio file analysis | **Implemented** | STFT, onset detection, 7 signals |
| Audio live capture | **Implemented** | Real-time mic/loopback (desktop + WASM) |
| .signal file | **Implemented** | Binary format, load/save |
| MIDI file | Future | Note events → triggers |
| MIDI live | Future | Real-time MIDI input |
| CSV/JSON import | Future | External data sources |
| OSC network | Future | Live data from other apps |
| Procedural | Future | Math expressions (sin, noise) |

---

## Audio Signals

Audio analysis is one **signal producer**. It generates signals from audio files or live input.

### Signal Types

Audio analysis produces standard `Signal` structs (same type used by all signal sources).
Analysis results use the generic signal system - there is no separate `AudioSignal` type.

### Implemented Signals

The `AudioAnalyzer` (STFT with 2048-sample FFT, 512-sample hop, Hann window) produces 7 signals:

| Signal Name | Type | Description |
|-------------|------|-------------|
| `amplitude` | Continuous | Overall RMS amplitude (0-1, normalized) |
| `energy_low` | Continuous | Low band energy (20-150 Hz) |
| `energy_mid` | Continuous | Mid band energy (150-2000 Hz) |
| `energy_high` | Continuous | High band energy (2000-20000 Hz) |
| `spectral_centroid` | Continuous | "Brightness" of sound (normalized) |
| `spectral_flux` | Continuous | Rate of spectral change |
| `onset` | Trigger | Beat/transient detection via spectral flux + adaptive threshold |

Signal sample rate = `audio_sample_rate / hop_size` (e.g., 44100 / 512 ≈ 86 Hz).

### Live vs Offline Availability

| Signal | Live (capture) | Offline (file) |
|--------|------|---------|
| amplitude | Yes | Yes |
| energy_low/mid/high | Yes | Yes |
| spectral_centroid | Yes | Yes |
| spectral_flux | Yes | Yes |
| onset | Yes | Yes |

### Future Signals (Not Yet Implemented)

| Signal Name | Type | Description |
|-------------|------|-------------|
| `amplitude_peak` | Continuous | Peak amplitude with decay |
| `onset_low/mid/high` | Trigger | Per-band onset detection |
| `bpm` | Scalar | Detected tempo |
| `beat_phase` | Continuous | 0-1 sawtooth synced to beat grid |
| `mel_bin_N` | Continuous | Individual mel spectrogram bins |

### Extended Signals (Future)

Additional DSP features that could be added later (see [audio-analysis-dsp-ml.md](audio-analysis-dsp-ml.md)):

| Signal | Type | Latency | Description |
|--------|------|---------|-------------|
| `spectral_spread` | Continuous | ~12ms | Bandwidth around centroid |
| `spectral_rolloff` | Continuous | ~12ms | Frequency containing 85-95% of energy |
| `spectral_flatness` | Continuous | ~12ms | Noise-like (1) vs tonal (0) |
| `spectral_crest` | Continuous | ~12ms | "Peakiness" of spectrum |
| `zero_crossing_rate` | Continuous | ~3ms | Roughness/noise indicator |
| `pitch` | Continuous | ~25ms | Monophonic pitch detection (YIN/MPM) |
| `chroma_N` | Continuous | ~50ms | 12-bin chromagram (C, C#, D, ..., B) |
| `harmonic_ratio` | Continuous | ~50ms | HPSS harmonic vs percussive balance |

**Additional extended signals:**

| Signal | Type | Latency | Description |
|--------|------|---------|-------------|
| `mfcc_N` | Continuous | ~12ms | Mel-frequency cepstral coefficients (N=0-12), captures timbre |
| `harmonic_energy` | Continuous | ~50ms | HPSS harmonic component energy |
| `percussive_energy` | Continuous | ~50ms | HPSS percussive component energy |

**HPSS (Harmonic-Percussive Source Separation):**
- Simpler than full stem separation, runs in real-time
- Median filtering: horizontal (harmonics) vs vertical (transients) on spectrogram
- Outputs two masks applied to STFT
- Benefits:
  - Feed harmonic part to chromagram/pitch (cleaner results)
  - Feed percussive part to onset detection (sharper transients)
- ~100 lines to implement with `rustfft` + median filter

**MFCCs (Mel-Frequency Cepstral Coefficients):**
- Pipeline: FFT → mel filterbank → log → DCT
- First 13 coefficients capture vocal tract shape, discard pitch
- Good for speaker/instrument identification
- ~0.1ms per frame

**Constant-Q Transform (CQT):**
- Logarithmic frequency spacing matching musical octaves
- Harmonics form fixed visual pattern regardless of pitch
- Better for tonal music than FFT
- More expensive, implement via octave-by-octave FFT with resampling
- No mature Rust crate - would need custom implementation

**Crate options for extended features:**
- `spectrograms` - **Primary choice**: STFT, mel, MFCC, chromagram with streaming support (2025)
- `aubio-rs` - Spectral descriptors, pitch detection, onset
- `pitch-detection` - YIN, McLeod (MPM) pitch detection algorithms

---

## ML-Assisted Analysis (Optional)

Machine learning can provide higher-level semantic signals that pure DSP cannot reliably extract.
This would be an optional addition (not yet implemented).

### Potential Dependencies

```toml
[dependencies]
# ONNX inference - choose one:
tract-onnx = { version = "0.21", optional = true }  # Pure Rust, WASM-compatible
ort = { version = "2.0", optional = true }           # ONNX Runtime, faster, desktop only
```

**Runtime comparison:**

| Feature | tract | ort |
|---------|-------|-----|
| Pure Rust | Yes | No (C++ bindings) |
| WASM support | Yes | No |
| Speed | ~1x | ~2-3x faster |
| Model compatibility | Good | Excellent |
| Binary size | Smaller | Larger (+native libs) |

Recommend: `tract` for portability/WASM, `ort` for desktop performance.

### Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                         DSP Thread (real-time)                        │
│                                                                      │
│  Audio samples → FFT → Mel Spectrogram → Ring Buffer (1-2 sec)       │
│                              │                                        │
│                              └──────────────────┐                     │
└─────────────────────────────────────────────────│─────────────────────┘
                                                  │ (async, non-blocking)
                                                  ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         ML Thread (background)                        │
│                                                                      │
│  Mel frames (1 sec window) → tract ONNX model → classification       │
│                                    │                                  │
│                                    ▼                                  │
│                           AtomicSignals (ML results)                  │
│                                                                      │
│  Latency: 50-100ms (inference) + window size                         │
│  Update rate: ~10 Hz (every 100ms)                                   │
└──────────────────────────────────────────────────────────────────────┘
```

**Key principle:** ML runs on a separate thread and never blocks the DSP pipeline.
If ML is slow, we just get stale results - DSP signals remain real-time.

### ML-Derived Signals

| Signal | Type | Latency | Description |
|--------|------|---------|-------------|
| `genre` | Categorical | ~200ms | Music genre classification (electronic, rock, classical, etc.) |
| `mood` | Continuous | ~200ms | Valence/arousal mood space (happy/sad, calm/energetic) |
| `instrument_N` | Continuous | ~150ms | Instrument presence (drums, bass, vocals, guitar, synth) |
| `vocal_presence` | Continuous | ~100ms | Voice activity detection (0 = instrumental, 1 = singing) |
| `section` | Categorical | ~300ms | Song structure (intro, verse, chorus, bridge, outro) |
| `key` | Categorical | ~200ms | Musical key detection (C major, A minor, etc.) |
| `energy_ml` | Continuous | ~100ms | Learned energy (often better than RMS for "perceived" energy) |

### Recommended Models

**Tier 1: Lightweight (<1MB, ~20ms inference)**
- Keyword spotting architectures (DS-CNN, TC-ResNet)
- Good for: vocal detection, simple event triggers
- INT8 quantized, runs comfortably at 10Hz

**Tier 2: Medium (1-5MB, ~50ms inference)**
- MobileNet-v2 on mel spectrograms
- Good for: genre, mood, instrument detection
- Pre-trained: AudioSet subsets, GTZAN, FMA

**Tier 3: Larger (5-20MB, ~100ms inference)**
- EfficientNet-B0/B1 variants
- Good for: section detection, key estimation
- Only for offline analysis or powerful hardware

### Model Sources

Pre-trained models that can be converted to ONNX:

| Task | Model | Size | Source |
|------|-------|------|--------|
| Genre | MobileNet-GTZAN | ~3MB | TensorFlow Hub → ONNX |
| Mood | Musicnn (compact) | ~5MB | GitHub musicnn → ONNX |
| Instrument | OpenL3 (music) | ~5MB | openl3 → ONNX |
| Vocals | Silero VAD | ~1MB | Already ONNX |
| General | YAMNet-lite | ~3MB | TFLite → ONNX |

**Custom training option:** Fine-tune on specific use cases using:
- `tch-rs` (PyTorch bindings) for training
- Export to ONNX for inference via `tract`

### Live vs Offline ML

| Mode | Behavior |
|------|----------|
| **Offline** | Run all ML models on full track, store results as signals |
| **Live** | Run only Tier 1 models in real-time, skip heavy models |

For live, we'd use a sliding window with the most recent 1-2 seconds of mel frames.
Heavier models (section detection, key) only run offline.

### WASM Considerations for ML

`tract` supports WASM, but:
- Inference is ~2-3x slower than native
- Larger models may cause memory pressure
- Recommend Tier 1 models only for live WASM

### Integration Example

```json
{
  "target": "Saturation",
  "source": {
    "Signal": {
      "signal_name": "vocal_presence",
      "min_output": 0.5,
      "max_output": 1.0,
      "smoothing": 0.7
    }
  }
}
```

### Implementation Notes

1. **Lazy loading:** Don't load ML models until first use
2. **Graceful degradation:** If ML fails, signals return 0.5 (neutral)
3. **Progress indication:** Show "Analyzing with ML..." during offline analysis
4. **Model caching:** Store downloaded models in user data directory
5. **Bundled vs downloaded:** Ship Tier 1 models, download larger on demand

---

## Integration with Animation System

### Signal to Parameter Mapping Guide

Suggested mappings from implemented audio signals to fractal parameters:

| Audio Signal | Good For Driving | Why |
|--------------|------------------|-----|
| `amplitude` | Zoom, Scale, Brightness | Overall energy, smooth |
| `energy_low` | Zoom, Transform scale | Bass = "weight", slow movement |
| `energy_mid` | Rotation speed, Pan | Melodic content, medium energy |
| `energy_high` | Saturation, Fine detail params | Hi-hats, sparkle |
| `onset` | Trigger zoom pulse, Color shift | Beat/transient hits |
| `spectral_centroid` | Color warmth, Hue shift | "Brightness" of sound |
| `spectral_flux` | Variation weights, Chaos params | Rate of change |

**Future signals (when implemented):**

| Audio Signal | Good For Driving | Why |
|--------------|------------------|-----|
| `beat_phase` | Cyclic parameters (rotation, pan) | Syncs to tempo |
| `pitch` | Hue (map pitch to color wheel) | Musical pitch |
| `vocal_presence` | Saturation, Foreground emphasis | Voice = focus |

### Smoothing

Each signal track has a `smoothing` parameter (0.0 = no smoothing, up to ~0.99 = very heavy).
Smoothing is frame-rate independent exponential smoothing applied in `AnimationController`:

```
alpha = 1 - smoothing^(dt / reference_dt)   // reference_dt = 1/60s
smoothed = prev + alpha * (raw - prev)
```

**Recommended smoothing values:**

| Use Case | Smoothing | Effect |
|----------|-----------|--------|
| Kick drum response | 0.0-0.3 | Fast, punchy |
| Melodic/vocal | 0.5-0.7 | Medium, natural |
| Bass swell | 0.8-0.9 | Smooth, weighty |
| Background ambient | 0.95+ | Very slow, atmospheric |

**Future:** Separate attack/decay rates for asymmetric smoothing (fast attack, slow decay for percussion).

### TrackSource::Signal Variant

```rust
// In src/animation/mod.rs

pub enum TrackSource {
    Keyframes { keyframes: Vec<Keyframe> },
    Oscillator { /* ... */ },

    /// Signal-driven track - value comes from any signal source
    Signal {
        signal_name: String,  // Name of signal in SignalManager (e.g., "energy_low", "onset")
        min_output: f64,      // Value when signal is at 0.0
        max_output: f64,      // Value when signal is at 1.0
        smoothing: f64,       // 0.0 = none, up to ~0.99 = heavy (frame-rate independent)
    },
}
```

Helper constructors: `Track::signal(target, signal_name, min, max)` and `Track::signal_with_smoothing(..., smoothing)`.

### Example Animation with Signal Track

```json
{
  "name": "Audio Reactive Zoom",
  "duration": 30.0,
  "tracks": [
    {
      "target": "Zoom",
      "source": {
        "Signal": {
          "signal_name": "energy_low",
          "min_output": 1.0,
          "max_output": 2.0,
          "smoothing": 0.3
        }
      },
      "interpolation": "Linear"
    },
    {
      "target": "Exposure",
      "source": {
        "Signal": {
          "signal_name": "onset",
          "min_output": 1.0,
          "max_output": 1.5,
          "smoothing": 0.0
        }
      },
      "interpolation": "Linear"
    }
  ]
}
```

### Signal Track Evaluation

Signal evaluation is handled by `AnimationController::evaluate_signal()`:

```rust
fn evaluate_signal(&mut self, signal_name, min_output, max_output, smoothing, time,
                   signal_manager, track_idx) -> Option<Value> {
    let manager = signal_manager?;
    let raw_value = manager.get_value_at(signal_name, time)?;

    // Frame-rate independent smoothing (if enabled)
    let value = if smoothing > 0.0 {
        let alpha = (1.0 - smoothing.powf(dt / reference_dt)) as f32;
        prev + alpha * (raw_value - prev)
    } else {
        raw_value
    };

    // Map signal value (0-1) to output range
    Some(min_output + value as f64 * (max_output - min_output))
}
```

The `SignalManager` is passed through the call chain:
`App::advance_animation()` → `AnimationController::evaluate_frame(Some(&signal_manager))`

---

## AudioManager & AudioPlayer

Audio functionality is split into two main components:
- **AudioManager** — File loading, decoding, analysis, signal production (implements `SignalProducer`)
- **AudioPlayer** — Playback with timeline sync (separate struct, platform-specific implementations)

### AudioManager

```rust
pub struct AudioManager {
    audio_data: Option<AudioData>,
    analyzer: AudioAnalyzer,
    signals: HashMap<String, Signal>,
    // ...
}

impl AudioManager {
    pub fn load_file(&mut self, path: &Path) -> Result<()>;    // Desktop
    pub fn load_bytes(&mut self, bytes: &[u8]) -> Result<()>;  // WASM
    pub fn analyze(&mut self);                                   // Run offline analysis
    pub fn analyze_with_config(&mut self, config: AnalysisConfig);
    pub fn has_audio(&self) -> bool;
    pub fn duration(&self) -> Option<f64>;
    pub fn sample_rate(&self) -> Option<u32>;
    pub fn channels(&self) -> Option<u16>;
    pub fn available_signals(&self) -> Vec<String>;
    pub fn clear(&mut self);
}

// AudioManager implements SignalProducer, so signals can be imported into SignalManager:
// signal_manager.import_from_producer(&audio_manager);
```

### AudioPlayer

Separate platform-specific implementations for audio playback:

```rust
// Desktop (src/audio/playback.rs) - uses cpal
// WASM (src/audio/playback_wasm.rs) - uses Web Audio API AudioBufferSourceNode
pub struct AudioPlayer {
    // Platform-specific internals
}

impl AudioPlayer {
    pub fn new() -> Self;
    pub fn load(&mut self, audio_data: AudioData);
    pub fn has_audio(&self) -> bool;
    pub fn state(&mut self) -> PlaybackState;  // Stopped | Playing | Paused
    pub fn position_seconds(&self) -> f64;
    pub fn duration(&self) -> Option<f64>;
    pub fn play(&mut self) -> Result<(), PlaybackError>;
    pub fn pause(&mut self);
    pub fn stop(&mut self);
    pub fn seek(&mut self, time: f64);
    pub fn sync_to_time(&mut self, animation_time: f64);  // Drift correction (100ms threshold)
}
```

**Desktop implementation details:**
- Uses cpal output stream with configurable sample format (F32/I16/U16)
- Tracks position in output frames via `Arc<AtomicU64>`
- Divides by `output_sample_rate` (not source rate) for accurate position
- Simple nearest-neighbor resampling when source rate != output rate

**WASM implementation details:**
- Uses `AudioContext` + `AudioBufferSourceNode` (one-shot, recreated per play)
- Tracks position via `context.current_time()` offset arithmetic
- Detaches `onended` callback before stopping to prevent stale async callbacks
- Deinterleaves samples into per-channel `AudioBuffer` data

### Animation Audio Config (Future)

Stored in the animation file (not yet implemented):

```rust
pub struct AnimationAudioConfig {
    pub file: PathBuf,     // Path to audio file
    pub offset: f64,       // Time offset (negative = skip into audio)
    pub fade_in: f64,      // Fade in duration (for export)
    pub fade_out: f64,     // Fade out duration (for export)
}
```

---

## Offline Analysis Pipeline

### Flow

```
Audio File (MP3/WAV/FLAC/OGG)
    │
    ▼
┌─────────────────────┐
│  symphonia decode   │  → PCM samples (f32, interleaved)
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  to_mono()          │  → Mono f32 samples
└─────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│  AudioAnalyzer::analyze()                                │
│  STFT (2048 FFT, 512 hop, Hann window, 128 mel bands)   │
│                                                          │
│  Per frame:                                              │
│    → amplitude (RMS)                                     │
│    → energy_low/mid/high (band energy from magnitudes)   │
│    → spectral_centroid (normalized)                       │
│    → spectral_flux (frame-to-frame difference)           │
│  After all frames:                                       │
│    → onset (adaptive threshold on spectral flux)         │
└─────────────────────────────────────────────────────────┘
    │
    ▼
HashMap<String, Signal>  (7 signals at ~86 Hz sample rate)
```

### Current Limitations

- Analysis runs **synchronously** (blocks UI during analysis)
- Future: Background thread with progress callback
- Full analysis of a 3-minute track completes quickly but freezes the UI

---

## Live Capture Pipeline

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Audio Thread                         │
│  ┌─────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │  cpal   │───▶│ Ring Buffer  │───▶│ Tier 1 + 2    │  │
│  │ callback│    │ (256 samples)│    │ Analysis      │  │
│  └─────────┘    └──────────────┘    └───────────────┘  │
│                                            │            │
│                                            ▼            │
│                                    ┌───────────────┐   │
│                                    │ AtomicSignals │   │
│                                    │ (lock-free)   │   │
│                                    └───────────────┘   │
└─────────────────────────────────────────────────────────┘
                                            │
                                            ▼ (atomic read)
┌─────────────────────────────────────────────────────────┐
│                     Main Thread                          │
│                                                         │
│  AnimationController::evaluate()                        │
│       │                                                 │
│       ▼                                                 │
│  audio_manager.get_signal_live("energy_low")           │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Lock-Free Signal Transfer

Live capture uses `AtomicSignals` for lock-free audio thread → main thread transfer:

```rust
struct AtomicSignals {
    amplitude: AtomicU32,      // f32 bits stored as u32
    energy_low: AtomicU32,
    energy_mid: AtomicU32,
    energy_high: AtomicU32,
    spectral_centroid: AtomicU32,
    spectral_flux: AtomicU32,
    onset: AtomicU32,
}
```

Real-time analysis runs in the cpal input callback (desktop) or ScriptProcessorNode callback (WASM).
Results are read atomically from the main thread for display in the Signal Monitor.

---

## Audio Playback & Animation Sync

### Sync Architecture

Animation drives timing, audio follows. Sync is controlled by `AnimationController.sync_audio` (toggled via UI checkbox in Animation Panel).

**State transitions** (in `src/app/animation_update.rs`):

1. **Animation starts** → `audio_player.seek(current_time)` + `audio_player.play()`
2. **Each frame** → `audio_player.sync_to_time(animation_time)` (seeks if >100ms drift)
3. **Animation pauses** → `audio_player.pause()`
4. **Animation stops** → `audio_player.stop()`
5. **Animation auto-stops** (LoopMode::Once end) → `audio_player.stop()`

### Timeline Scrubbing

During timeline scrubbing (drag), audio is paused to avoid scratch artifacts.
On drag release, audio seeks to the final position and resumes if animation is playing.

```
// In src/app/ui_handlers.rs:
// During drag: audio_player.pause()
// On drag release: audio_player.seek(time) + audio_player.play()
```

### Drift Correction

`sync_to_time()` compares `position_seconds()` with the animation time. If the difference exceeds 100ms, it performs a seek to resync. This handles minor drift without constant seeking.

### Signal Smoothing

Signal tracks support exponential smoothing that is **frame-rate independent**:

```rust
// alpha = 1 - smoothing^(dt / reference_dt)
// At 60fps (reference), smoothing=0.9 gives alpha=0.1
// At 30fps, alpha increases proportionally so results match
let reference_dt = 1.0 / 60.0;
let alpha = (1.0 - smoothing.powf(dt / reference_dt)) as f32;
let smoothed = prev + alpha * (raw_value - prev);
```

Smoothed values are cached per-track in `AnimationController.signal_smoothed`.

---

## Export with Audio (Future)

### MP4 Muxing

Existing export pipeline:
```
Fractal frames → FFmpeg stdin (raw RGBA) → MP4 (video only)
```

With audio:
```
Fractal frames → FFmpeg stdin (raw RGBA) ─┐
                                          ├─▶ MP4 (video + audio)
Audio file ────────────────────────────────┘
```

### FFmpeg Command

**Basic (no offset, no fades):**
```bash
ffmpeg -y \
  -f rawvideo -pix_fmt rgba -s {width}x{height} -r {fps} -i - \
  -i "{audio_file}" \
  -c:v libx264 -preset {preset} -crf {crf} \
  -c:a aac -b:a 192k \
  -shortest \
  "{output}.mp4"
```

**With audio offset (skip into audio):**
```bash
ffmpeg -y \
  -f rawvideo -pix_fmt rgba -s {width}x{height} -r {fps} -i - \
  -ss {offset} -i "{audio_file}" \    # -ss before -i seeks in audio
  -c:v libx264 -preset {preset} -crf {crf} \
  -c:a aac -b:a 192k \
  -shortest \
  "{output}.mp4"
```

**With audio offset (delay audio start):**
```bash
ffmpeg -y \
  -f rawvideo -pix_fmt rgba -s {width}x{height} -r {fps} -i - \
  -i "{audio_file}" \
  -af "adelay={delay_ms}|{delay_ms}" \    # Delay both channels
  -c:v libx264 -preset {preset} -crf {crf} \
  -c:a aac -b:a 192k \
  -shortest \
  "{output}.mp4"
```

**With fade in/out:**
```bash
ffmpeg -y \
  -f rawvideo -pix_fmt rgba -s {width}x{height} -r {fps} -i - \
  -i "{audio_file}" \
  -af "afade=t=in:st=0:d={fade_in},afade=t=out:st={fade_out_start}:d={fade_out}" \
  -c:v libx264 -preset {preset} -crf {crf} \
  -c:a aac -b:a 192k \
  -shortest \
  "{output}.mp4"
```

**Combined (offset + fades):**
- Chain audio filters with commas: `-af "adelay=...,afade=...,afade=..."`
- Or use `-ss` for negative offset (skip into audio)

### Audio Timing

| Offset Value | Behavior |
|--------------|----------|
| `0.0` | Audio and animation start together |
| `-2.5` | Skip 2.5 seconds into audio at animation start |
| `+3.0` | 3 seconds of silence, then audio begins |

### Sync Considerations

- Animation duration may differ from audio duration
- `-shortest` uses the shorter of the two
- User can set animation duration to match audio
- Future: auto-set animation duration from audio file

---

## UI Integration

### Audio Panel

Controls for audio file loading, playback, live capture, and signal monitoring.

```
┌─────────────────────────────────────────────────┐
│ Audio                                       [×] │
├─────────────────────────────────────────────────┤
│ ── Audio File ────────────────────────────────  │
│ File: music.mp3                       [Load]    │
│ Duration: 3:24  |  44.1kHz  |  Stereo           │
│ Analysis: Complete                              │
│                                                 │
│ ── Playback ─────────────────────────────────── │
│ [▶]  [■]  0:45 / 3:24                           │
│ [────────────●──────] position slider            │
│                                                 │
│ ── Live Input ────────────────────────────────  │
│ Device: [System Default        ▼]  [● Capture]  │
│ Level: ████████░░░░░░░░                         │
│                                                 │
│ ── Signal Monitor ────────────────────────────  │
│ amplitude      ████████░░ 0.78                  │
│ energy_low     ███░░░░░░░ 0.32                  │
│ energy_mid     █████░░░░░ 0.51                  │
│ energy_high    ██░░░░░░░░ 0.18                  │
│ spectral_cent. █████████░ 0.89                  │
│ spectral_flux  ██████░░░░ 0.62                  │
│ onset          [●]                              │
└─────────────────────────────────────────────────┘
```

### Animation Panel Audio Integration

- **Sync Audio** checkbox — toggles `AnimationController.sync_audio`
- When enabled, loaded audio plays/pauses/stops with the animation timeline
- Timeline scrubbing pauses audio during drag, seeks on release

### Track Editor Signal Integration

- Signal name is selected via **dropdown** (populated from `SignalManager.signal_names()`)
- No manual text entry required
- Shows "No signals available" if no audio has been analyzed

### Future UI Improvements

- Waveform display above timeline
- Signal history sparklines in monitor
- Smoothing preset dropdown (Kick, Bass, Vocal, etc.)

---

## WASM Considerations

### Platform-Specific Backends

| Feature | Desktop | WASM |
|---------|---------|------|
| Audio input | cpal | web-sys ScriptProcessorNode |
| Audio output | cpal | Web Audio API (AudioBufferSourceNode) |
| File decoding | symphonia | symphonia (works in WASM) |
| File loading | filesystem | drag-drop / file picker |
| Export with audio | Not yet implemented | Not supported |

### WASM Live Capture Architecture

Uses `ScriptProcessorNode` (not AudioWorklet) for simpler implementation:

```
┌────────────────────────────────────────────────────────────────┐
│                        Browser                                  │
│                                                                │
│  getUserMedia() ──▶ MediaStream ──▶ MediaStreamAudioSourceNode │
│                                              │                  │
│                                              ▼                  │
│                                    ScriptProcessorNode          │
│                                    (onaudioprocess callback)    │
│                                              │                  │
└──────────────────────────────────────────────│──────────────────┘
                                               │ (direct callback)
                                               ▼
┌──────────────────────────────────────────────────────────────────┐
│                        WASM Module                                │
│                                                                  │
│  Audio samples ──▶ Analysis ──▶ AtomicSignals                    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### WASM Audio Playback

Uses `AudioBufferSourceNode` (one-shot nodes, recreated per play/seek):
- Samples deinterleaved and copied to per-channel `AudioBuffer`
- Position tracked via `AudioContext.currentTime()` offset math
- `onended` callbacks detached before `stop()` to prevent stale async race conditions

### Limitations

- **Export:** No FFmpeg in browser, video-only export on WASM
- **File access:** Drag-drop or file picker only (no filesystem)
- **ScriptProcessorNode:** Deprecated API, but AudioWorklet requires separate JS file and more complexity

---

## Implementation Phases

### Phase 0: Signal System Foundation ✅
- [x] `Signal` struct with `SignalType` (Continuous, Trigger, Scalar)
- [x] `.signal` binary file format (read/write)
- [x] `SignalManager` with CRUD, file I/O, producer management
- [x] `SignalProducer` trait
- [x] `TrackSource::Signal` variant in animation system
- [x] Signal evaluation with `SignalManager` context
- [x] Wire `SignalManager` to App and pass through animation chain

### Phase 1: Audio Infrastructure ✅
- [x] Audio dependencies (cpal, symphonia, rustfft, ringbuf) — always included, no feature flag
- [x] `AudioManager` implementing `SignalProducer`
- [x] Audio decoding via symphonia (MP3, WAV, FLAC, OGG)
- [x] `AudioData` struct with interleaving, mono conversion, channel extraction
- [x] Wire `AudioManager` to `SignalManager` via `import_from_producer()`

### Phase 2: Offline Analysis ✅
- [x] STFT-based analysis (2048 FFT, 512 hop, Hann window, 128 mel bands)
- [x] 7 signals: `amplitude`, `energy_low/mid/high`, `spectral_centroid`, `spectral_flux`, `onset`
- [ ] BPM detection and `beat_phase` (future)
- [ ] Background thread analysis with progress (currently synchronous)

### Phase 3: Animation Integration ✅
- [x] `TrackSource::Signal` with `signal_name`, `min_output`, `max_output`, `smoothing`
- [x] Frame-rate independent exponential smoothing
- [x] Animation serialization (load/save signal tracks)
- [ ] Trigger envelope (attack/decay) for onset signals (future)

### Phase 4: Audio Playback ✅
- [x] Desktop playback via cpal (`playback.rs`) with sample-rate resampling
- [x] WASM playback via Web Audio API (`playback_wasm.rs`) with AudioBufferSourceNode
- [x] `sync_audio` checkbox in Animation Panel
- [x] Audio-animation sync: start/stop/pause with animation state machine
- [x] Drift correction via `sync_to_time()` (100ms threshold)
- [x] Timeline scrubbing: pause during drag, seek+resume on drop
- [x] Seek, play/pause/stop on both platforms
- [x] End detection (desktop: atomic flag in stream callback, WASM: onended event)
- [x] WASM: detach `onended` before stopping to prevent stale async callbacks

### Phase 5: Live Capture (Desktop) ✅
- [x] cpal input stream (`capture_native.rs`)
- [x] Lock-free `AtomicSignals` for audio thread → main thread transfer
- [x] Device selection UI
- [x] Real-time analysis (7 signals)

### Phase 5b: Live Capture (WASM) ✅
- [x] ScriptProcessorNode-based capture (`capture_wasm.rs`)
- [x] web-sys getUserMedia bindings
- [x] Ring buffer bridging
- [x] Permission request UI
- [x] Same analysis pipeline as desktop

### Phase 6: Export Integration (Partial)
- [x] Signals available during animation export (offline evaluation)
- [ ] FFmpeg pipeline with audio muxing (future)

### Phase 7: UI ✅
- [x] Audio panel: file info, playback controls, signal monitor, live capture controls
- [x] Signal dropdown in track editor (populated from SignalManager)
- [x] "Sync Audio" checkbox in Animation Panel
- [ ] Waveform display above tracks (future)

### Phase 8: Polish
- [ ] WASM compatibility testing
- [ ] Error handling improvements
- [ ] Documentation updates

### Phase 9: ML Analysis (Optional, Future)
- [ ] `tract-onnx` integration for ML-derived signals
- [ ] Vocal presence, genre, mood, instrument detection
- [ ] See ML section below for full plan

---

## Open Questions

(None currently)

## Resolved Decisions

### Signal Architecture
- **Generalized signals:** Signals are source-agnostic - audio is just one producer
- **TrackSource::Signal:** Works with any signal source (no separate `TrackSource::Audio`)
- **Binary file format:** `.signal` files for storing/exchanging signals (compact, not human-readable)
- **SignalManager:** Central coordinator for all signal sources
- **SignalProducer trait:** Audio, MIDI, external sources all implement this
- **No feature flag:** Audio dependencies always included (zero cost when unused)

### Audio System
- **Spectral flux:** Implemented directly in `AudioAnalyzer` (no external `microdsp` dependency)
- **Separate playback:** `AudioPlayer` is independent from `AudioManager` (analysis vs playback)
- **Platform playback:** Desktop uses cpal, WASM uses Web Audio API AudioBufferSourceNode
- **Animation sync:** `sync_audio` flag on AnimationController, toggled via UI checkbox
- **Scrub behavior:** Pause audio during drag, seek+resume on release (avoids scratch artifacts)
- **Smoothing:** Frame-rate independent exponential smoothing (alpha scales with dt)
- **Multiple audio files:** Single file per animation
- **WASM live capture:** ScriptProcessorNode via web-sys (not AudioWorklet due to complexity)
- **MIDI support:** Not in scope (potential future feature)
- **Audio effects:** No - purely analysis and playback, no audio modification

---

## References

- Original audio code: https://github.com/calibas/rust-godot/tree/master/src
- cpal documentation: https://docs.rs/cpal
- symphonia documentation: https://docs.rs/symphonia
- Mel spectrogram theory: https://en.wikipedia.org/wiki/Mel-frequency_cepstrum
- Onset detection: https://www.ee.columbia.edu/~dpwe/pubs/Bello05-ijmm.pdf
