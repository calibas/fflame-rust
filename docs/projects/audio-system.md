# Audio System Design

## Overview

Add optional audio integration to the fractal flame renderer, enabling:
1. **Offline audio analysis** - Pre-analyze audio files to extract signals for animation
2. **Live audio input** - Real-time audio analysis during playback (limited features)
3. **Audio playback** - Play audio synced to animation timeline
4. **Audio export** - Include audio in exported MP4 files

**Core principle:** Zero impact on normal program performance. Audio is fully optional.

---

## Architecture

### Module Structure

```
src/signal/
  mod.rs              - Signal struct, SignalType, SignalManager
  file.rs             - .signal binary file format (read/write)
  producer.rs         - SignalProducer trait

src/audio/
  mod.rs              - Public API, AudioManager (implements SignalProducer)
  analyzer.rs         - STFT, mel spectrogram, onset detection (ported from existing code)
  playback.rs         - Audio file playback with timeline sync
  capture.rs          - Live audio input (platform-agnostic interface)
  capture_native.rs   - Desktop capture via cpal
  capture_wasm.rs     - WASM capture via web-sys + AudioWorklet
  decode.rs           - Audio file decoding (symphonia)
  export.rs           - Audio muxing into MP4 (FFmpeg, desktop only)
```

### Dependencies

```toml
[dependencies]
# Audio I/O (desktop only for input, both for output)
cpal = { version = "0.15", optional = true }

# Audio decoding (MP3, WAV, FLAC, OGG, etc.) - works on WASM too
symphonia = { version = "0.5", optional = true, features = ["mp3", "wav", "flac", "ogg"] }

# FFT analysis
rustfft = { version = "6.2", optional = true }

# Thread-safe buffers
ringbuf = { version = "0.4", optional = true }

# Spectral flux novelty detection
microdsp = { version = "0.1", optional = true }

[features]
default = []
audio = ["cpal", "symphonia", "rustfft", "ringbuf", "microdsp"]
```

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
│  • signal: String (name or file path)                       │
│  • output_min/max, smoothing, trigger params                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Signal Struct

```rust
/// A time-indexed signal that can drive animation parameters.
/// Source-agnostic - could come from audio, MIDI, sensors, files, etc.
#[derive(Clone)]
pub struct Signal {
    /// Human-readable name (e.g., "energy_low", "kick_trigger")
    pub name: String,

    /// Sample rate in Hz (how many values per second)
    pub sample_rate: f64,

    /// Type of signal (affects how it's interpreted)
    pub signal_type: SignalType,

    /// Time-indexed values, starting at t=0
    pub data: Vec<f32>,

    /// Optional metadata about signal origin
    pub metadata: Option<SignalMetadata>,
}

pub enum SignalType {
    /// Continuous value, typically 0.0-1.0
    Continuous,
    /// Binary trigger (1.0 = triggered, 0.0 = not)
    Trigger,
    /// Scalar with units (e.g., BPM, Hz)
    Scalar { unit: String },
}

pub struct SignalMetadata {
    /// What produced this signal ("audio", "midi", "external", etc.)
    pub source_type: String,
    /// Source-specific info (audio file hash, analysis params, etc.)
    pub source_info: serde_json::Value,
}
```

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
/// Manages all signal sources and provides unified access
pub struct SignalManager {
    /// Loaded signals (from files or generated)
    signals: HashMap<String, Signal>,

    /// Live signal producers (audio capture, etc.)
    live_producers: Vec<Box<dyn SignalProducer>>,
}

impl SignalManager {
    /// Get signal value at specific time
    pub fn get_value(&self, signal_name: &str, time: f64) -> Option<f32>;

    /// Get current live value (for real-time sources)
    pub fn get_live_value(&self, signal_name: &str) -> Option<f32>;

    /// Load signal from .signal file
    pub fn load_signal_file(&mut self, path: &Path) -> Result<()>;

    /// Save signal to .signal file
    pub fn save_signal_file(&self, signal_name: &str, path: &Path) -> Result<()>;

    /// Register a live signal producer
    pub fn register_producer(&mut self, producer: Box<dyn SignalProducer>);

    /// List all available signals
    pub fn list_signals(&self) -> Vec<&str>;
}

/// Trait for things that produce signals (audio analyzer, MIDI, etc.)
pub trait SignalProducer: Send {
    /// Get the names of signals this producer provides
    fn signal_names(&self) -> Vec<String>;

    /// Get current live value for a signal
    fn get_live_value(&self, name: &str) -> Option<f32>;

    /// Get computed signal data (for offline analysis)
    fn get_signal(&self, name: &str) -> Option<Signal>;
}
```

### Future Signal Sources

| Source | Status | Description |
|--------|--------|-------------|
| Audio file analysis | Planned (this doc) | FFT, onset detection, ML |
| Audio live capture | Planned (this doc) | Real-time mic/loopback |
| .signal file | Planned (this doc) | Pre-computed, portable |
| MIDI file | Future | Note events → triggers |
| MIDI live | Future | Real-time MIDI input |
| CSV/JSON import | Future | External data sources |
| OSC network | Future | Live data from other apps |
| Procedural | Future | Math expressions (sin, noise) |

---

## Audio Signals

Audio analysis is one **signal producer**. It generates signals from audio files or live input.

### Signal Types

Audio analysis produces **signals** - time-varying values that can drive animation parameters.

```rust
/// A single audio signal that can be attached to animation tracks
pub struct AudioSignal {
    pub name: String,
    pub signal_type: SignalType,
    /// Sample rate of the signal data (not audio sample rate)
    /// Typically 100-1000 Hz depending on analysis hop size
    pub sample_rate: f64,
    /// Time-indexed values, starting at t=0
    pub data: Vec<f32>,
}

pub enum SignalType {
    /// Continuous value 0.0-1.0 (band energy, amplitude, etc.)
    Continuous,
    /// Binary trigger (onset detected = 1.0, else 0.0)
    Trigger,
    /// Value with units (BPM, frequency in Hz, etc.)
    Scalar { unit: String },
}
```

### Available Signals

| Signal Name | Type | Latency | Description |
|-------------|------|---------|-------------|
| `amplitude` | Continuous | ~3ms | Overall RMS amplitude (0-1) |
| `amplitude_peak` | Continuous | ~3ms | Peak amplitude with decay |
| `energy_low` | Continuous | ~6ms | Low band energy (20-150 Hz) |
| `energy_mid` | Continuous | ~6ms | Mid band energy (150-2000 Hz) |
| `energy_high` | Continuous | ~6ms | High band energy (2000-20000 Hz) |
| `onset_low` | Trigger | ~6ms | Bass/kick onset detected |
| `onset_mid` | Trigger | ~6ms | Snare/vocal onset detected |
| `onset_high` | Trigger | ~6ms | Hi-hat/cymbal onset detected |
| `onset_any` | Trigger | ~6ms | Any band onset detected |
| `spectral_centroid` | Continuous | ~12ms | "Brightness" of sound (Hz, normalized) |
| `spectral_flux` | Continuous | ~12ms | Rate of spectral change |
| `bpm` | Scalar | Offline only | Detected tempo |
| `beat_phase` | Continuous | Offline only | 0-1 sawtooth synced to beat grid |
| `mel_bin_N` | Continuous | ~50ms | Individual mel spectrogram bin (N=0-63) |

**Latency tiers:**
- **Tier 1 (~3ms):** Time-domain only, 128-sample window
- **Tier 2 (~6ms):** Small FFT (256-512 samples), basic frequency analysis
- **Tier 3 (~12ms):** Medium FFT (1024 samples), spectral features
- **Tier 4 (~50ms+):** Large FFT (2048-4096), mel spectrogram, offline only

### Live vs Offline Availability

| Signal | Live | Offline |
|--------|------|---------|
| amplitude, amplitude_peak | Yes | Yes |
| energy_low/mid/high | Yes | Yes |
| onset_low/mid/high/any | Yes | Yes |
| spectral_centroid | Yes | Yes |
| spectral_flux | Yes | Yes |
| bpm | No | Yes |
| beat_phase | No | Yes |
| mel_bin_N | No | Yes |

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
This is **fully optional** via a separate feature flag.

### Feature Flag

```toml
[dependencies]
# ONNX inference - choose one:
# tract: Pure Rust, no native deps, slower but portable (works on WASM)
tract-onnx = { version = "0.21", optional = true }
# ort: ONNX Runtime bindings, faster but requires native libs (no WASM)
ort = { version = "2.0", optional = true }

[features]
audio = ["cpal", "symphonia", "rustfft", "ringbuf", "microdsp"]
audio-ml = ["audio", "tract-onnx"]  # ML with tract (portable, WASM-compatible)
audio-ml-fast = ["audio", "ort"]    # ML with ort (faster, desktop only)
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

```rust
// In animation file
{
  "target": "Saturation",
  "source": {
    "type": "Signal",
    "signal": "vocal_presence",  // ML-derived signal
    "output_min": 0.5,
    "output_max": 1.0,
    "smoothing": 0.7  // High smoothing for ML signals (they update slowly)
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

Suggested mappings from audio signals to fractal parameters:

| Audio Signal | Good For Driving | Why |
|--------------|------------------|-----|
| `amplitude` | Zoom, Scale, Brightness | Overall energy, smooth |
| `amplitude_peak` | Flash effects, Exposure | Fast transients |
| `energy_low` | Zoom, Transform scale | Bass = "weight", slow movement |
| `energy_mid` | Rotation speed, Pan | Melodic content, medium energy |
| `energy_high` | Saturation, Fine detail params | Hi-hats, sparkle |
| `onset_low` | Trigger zoom pulse, Transform snap | Kick drum hits |
| `onset_mid` | Trigger rotation, Color shift | Snare hits |
| `onset_high` | Trigger sparkle, Small param bumps | Hi-hat ticks |
| `spectral_centroid` | Color warmth, Hue shift | "Brightness" of sound |
| `spectral_flux` | Variation weights, Chaos params | Rate of change |
| `beat_phase` | Cyclic parameters (rotation, pan) | Syncs to tempo |
| `pitch` | Hue (map pitch to color wheel) | Musical pitch |
| `chroma_N` | Individual color channels | Harmonic content |
| `vocal_presence` | Saturation, Foreground emphasis | Voice = focus |
| `harmonic_energy` | Smooth/flowing parameters | Sustained tones |
| `percussive_energy` | Sharp/transient parameters | Drums, attacks |

### Smoothing Patterns

Different signal types benefit from different attack/decay characteristics:

```rust
/// Attack/decay smoothing for reactive visuals
pub struct SignalSmoother {
    /// How fast to respond to increases (0.0-1.0, higher = faster)
    pub attack: f32,
    /// How fast to respond to decreases (0.0-1.0, higher = faster)
    pub decay: f32,
    current_value: f32,
}

impl SignalSmoother {
    pub fn process(&mut self, input: f32) -> f32 {
        let rate = if input > self.current_value { self.attack } else { self.decay };
        self.current_value += (input - self.current_value) * rate;
        self.current_value
    }
}
```

**Recommended presets:**

| Use Case | Attack | Decay | Effect |
|----------|--------|-------|--------|
| Kick drum response | 0.9 | 0.1 | Fast hit, slow fade |
| Hi-hat shimmer | 0.8 | 0.6 | Quick response both ways |
| Bass swell | 0.3 | 0.2 | Smooth, weighty |
| Vocal presence | 0.4 | 0.3 | Medium, natural |
| ML signals | 0.2 | 0.2 | Very smooth (compensate for update rate) |

### New TrackSource Variant

```rust
// In src/animation/mod.rs

pub enum TrackSource {
    Keyframes { keyframes: Vec<Keyframe> },
    Oscillator { /* existing */ },

    /// Signal-driven track - value comes from any signal source
    /// (audio analysis, .signal file, MIDI, external data, etc.)
    Signal {
        /// Name of the signal (e.g., "energy_low", "onset_any")
        /// Can also be a path to a .signal file
        signal: String,
        /// Output range mapping
        output_min: f64,
        output_max: f64,
        /// Smoothing factor (0 = no smoothing, 1 = max smoothing)
        smoothing: f64,
        /// For trigger signals: hold time in seconds before returning to min
        trigger_hold: Option<f64>,
        /// For trigger signals: attack/decay envelope
        trigger_attack: Option<f64>,
        trigger_decay: Option<f64>,
    },
}
```

### Example Animation with Signal Track

```json
{
  "name": "Audio Reactive Zoom",
  "duration": 30.0,
  "audio": {
    "file": "music.mp3",
    "offset": -2.5,       // Start 2.5 seconds into the audio at animation t=0
    "fade_in": 0.0,       // Fade in duration (for export)
    "fade_out": 2.0       // Fade out duration (for export)
  },
  "tracks": [
    {
      "target": "Zoom",
      "source": {
        "type": "Signal",
        "signal": "energy_low",
        "output_min": 1.0,
        "output_max": 2.0,
        "smoothing": 0.3
      },
      "interpolation": "Linear"
    },
    {
      "target": "Exposure",
      "source": {
        "type": "Signal",
        "signal": "onset_any",
        "output_min": 1.0,
        "output_max": 1.5,
        "trigger_hold": 0.1,
        "trigger_attack": 0.01,
        "trigger_decay": 0.2
      },
      "interpolation": "Linear"
    }
  ]
}
```

### Signal Track Evaluation

```rust
impl Track {
    pub fn evaluate_at(&self, time: f64, signal_manager: Option<&SignalManager>) -> Option<f64> {
        match &self.source {
            TrackSource::Signal { signal, output_min, output_max, smoothing, .. } => {
                let manager = signal_manager?;
                let raw_value = manager.get_value(signal, time)?;

                // Apply smoothing (exponential moving average against previous frame)
                let smoothed = self.apply_smoothing(raw_value, *smoothing);

                // Map to output range
                Some(output_min + smoothed * (output_max - output_min))
            }
            // ... existing variants
        }
    }
}
```

---

## AudioManager

Central coordinator for all audio functionality.

### Animation Audio Config

Stored in the animation file:

```rust
/// Audio configuration for an animation
#[derive(Serialize, Deserialize, Clone)]
pub struct AnimationAudioConfig {
    /// Path to audio file (relative to animation file, or absolute)
    pub file: PathBuf,

    /// Time offset in seconds
    /// - Negative: skip into audio (audio at t=-offset plays at animation t=0)
    /// - Positive: delay audio start (silence until animation reaches this time)
    /// - Zero: audio and animation start together
    pub offset: f64,

    /// Fade in duration in seconds (for export only)
    pub fade_in: f64,

    /// Fade out duration in seconds (for export only)
    pub fade_out: f64,
}
```

### AudioManager Struct

```rust
pub struct AudioManager {
    /// Decoded audio data (if file loaded)
    audio_data: Option<AudioData>,

    /// Pre-computed signals from offline analysis
    offline_signals: HashMap<String, AudioSignal>,

    /// Live capture state
    live_capture: Option<LiveCaptureState>,

    /// Current playback state
    playback: Option<PlaybackState>,

    /// Analysis configuration
    config: AudioAnalysisConfig,
}

impl AudioManager {
    /// Load and analyze an audio file (offline analysis)
    pub fn load_file(&mut self, path: &Path) -> Result<AudioFileInfo>;

    /// Get signal value at specific time (for offline playback)
    pub fn get_signal_at(&self, signal_name: &str, time: f64) -> Option<f32>;

    /// Get current signal value (for live input)
    pub fn get_signal_live(&self, signal_name: &str) -> Option<f32>;

    /// Start audio playback synced to animation
    pub fn start_playback(&mut self, start_time: f64);

    /// Seek playback to specific time
    pub fn seek(&mut self, time: f64);

    /// Stop playback
    pub fn stop_playback(&mut self);

    /// Start live audio capture
    pub fn start_live_capture(&mut self, device: Option<&str>) -> Result<()>;

    /// Stop live capture
    pub fn stop_live_capture(&mut self);

    /// Check if audio is available
    pub fn has_audio(&self) -> bool;

    /// Get audio file info for export
    pub fn get_audio_for_export(&self) -> Option<&AudioData>;
}
```

---

## Offline Analysis Pipeline

### Flow

```
Audio File (MP3/WAV/FLAC)
    │
    ▼
┌─────────────────────┐
│  symphonia decode   │  → PCM samples (f32, mono, resampled to 44.1kHz)
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Tier 1 Analysis    │  → amplitude, amplitude_peak
│  (128 sample hops)  │     ~344 signals/sec
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Tier 2 Analysis    │  → energy_low/mid/high, onsets
│  (256-512 FFT)      │     ~172 signals/sec
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Tier 3 Analysis    │  → spectral_centroid, spectral_flux
│  (1024 FFT)         │     ~86 signals/sec
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Tier 4 Analysis    │  → mel_bins, beat detection, BPM
│  (4096 FFT)         │     ~22 signals/sec
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Beat Grid Sync     │  → beat_phase (aligned to detected BPM)
└─────────────────────┘
    │
    ▼
HashMap<String, AudioSignal>  (all signals at their native sample rates)
```

### Analysis is done on a background thread

- File load returns immediately with basic info (duration, format)
- Analysis runs on background thread with progress callback
- Signals become available as each tier completes
- Full analysis of a 3-minute track: ~1-2 seconds

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

### DSP Thread Internals

Detailed view of what happens inside the DSP analysis:

```
Audio Input (cpal/web-sys)
    │
    ├── Ring buffer → DSP Thread
    │                     │
    │                     ├── FFT (rustfft, 256-512 samples)
    │                     │     │
    │                     │     ├── Spectral features (centroid, flux, rolloff)
    │                     │     ├── Band energy (low/mid/high)
    │                     │     ├── Onset detection (per-band)
    │                     │     └── Mel spectrogram → [Ring buffer for ML]
    │                     │
    │                     ├── [Optional] HPSS
    │                     │     ├── Harmonic mask → cleaner pitch/chromagram
    │                     │     └── Percussive mask → sharper onset detection
    │                     │
    │                     ├── [Optional] Pitch detection (YIN/MPM, separate buffer)
    │                     │
    │                     └── [Optional] Chromagram (larger FFT, lower rate)
    │
    └── [Optional] ML Thread (separate, async)
                          │
                          ├── tract/ort inference on mel frames
                          └── Classification results → AtomicSignals
```

All DSP runs in one thread at 3-6ms hop rate. Optional features (HPSS, pitch, chromagram)
can be enabled/disabled. ML inference runs asynchronously on accumulated windows.

### Lock-Free Signal Transfer

```rust
/// Atomic signal values for lock-free audio thread → main thread transfer
struct AtomicSignals {
    amplitude: AtomicU32,      // f32 bits
    amplitude_peak: AtomicU32,
    energy_low: AtomicU32,
    energy_mid: AtomicU32,
    energy_high: AtomicU32,
    onset_flags: AtomicU8,     // bit flags: low=0x01, mid=0x02, high=0x04
    spectral_centroid: AtomicU32,
    spectral_flux: AtomicU32,
}
```

### Buffer Sizing for Low Latency

| Setting | Value | Latency |
|---------|-------|---------|
| cpal buffer | 128 samples | 2.9ms |
| Ring buffer | 256 samples | 5.8ms |
| Tier 1 analysis | 128 samples | 0ms (runs every callback) |
| Tier 2 FFT | 512 samples | ~6ms (overlapping) |
| Total | | **~6ms** for Tier 1+2 signals |

---

## Audio Playback

### Sync Strategy

Animation drives timing, audio follows:

1. `AnimationController` maintains master timeline
2. On `play()`, `AudioManager::start_playback(current_time)` begins audio
3. On `seek()`, `AudioManager::seek(time)` repositions audio
4. On `pause()`, `AudioManager::pause_playback()` pauses audio
5. No drift correction needed - both use system clock

### Playback Implementation

```rust
struct PlaybackState {
    /// Audio samples ready for playback
    samples: Arc<Vec<f32>>,
    /// Current playback position (sample index)
    position: Arc<AtomicUsize>,
    /// Whether playback is active
    playing: Arc<AtomicBool>,
    /// cpal output stream
    stream: cpal::Stream,
}
```

The cpal output callback reads from `samples` at `position`, incrementing atomically. Main thread controls `playing` and can set `position` for seeking.

---

## Export with Audio

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

### Audio Panel (new panel in egui_dock)

Controls for audio file loading, preview, timing, live capture, and signal monitoring.
(Waveform timeline display is in Animation panel, not here.)

```
┌─────────────────────────────────────────────────┐
│ Audio                                       [×] │
├─────────────────────────────────────────────────┤
│ ── Audio File ────────────────────────────────  │
│ File: music.mp3                       [Load]    │
│ Duration: 3:24  |  44.1kHz  |  Stereo           │
│ Analysis: Complete ████████████████ 100%        │
│                                                 │
│ ── Preview & Timing ──────────────────────────  │
│ [▶]  [■]  0:45 / 3:24                           │
│ [▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄]  ← scrubber │
│                                                 │
│ Animation offset: [ -2.5 ] sec                  │
│   (negative = skip into audio at anim start)   │
│   (positive = silence before audio starts)     │
│ [Set to current position]                       │
│                                                 │
│ ── Export Audio Settings ─────────────────────  │
│ Fade in:  [ 0.0 ] sec                           │
│ Fade out: [ 2.0 ] sec                           │
│                                                 │
│ ── Live Input ────────────────────────────────  │
│ Device: [System Default        ▼]  [● Capture]  │
│ Level: ████████░░░░░░░░ -12dB                   │
│                                                 │
│ ── Signal Monitor ────────────────────────────  │
│ ┌─────────────────────────────────────────────┐ │
│ │ amplitude      ████████░░ 0.78              │ │
│ │ energy_low     ███░░░░░░░ 0.32    [graph]   │ │
│ │ energy_mid     █████░░░░░ 0.51    [graph]   │ │
│ │ energy_high    ██░░░░░░░░ 0.18    [graph]   │ │
│ │ spectral_cent. █████████░ 0.89              │ │
│ │ onset_low      [●]                          │ │
│ │ onset_mid      [ ]                          │ │
│ │ onset_high     [●]                          │ │
│ │ bpm            128.4                        │ │
│ └─────────────────────────────────────────────┘ │
│                                                 │
│ [Show All Signals...]  [Signal History Graph]   │
└─────────────────────────────────────────────────┘
```

**Signal visualization options:**
- Real-time meter bars (current value)
- Mini sparkline graphs (last ~2 seconds of history)
- Expandable full signal history graph (scrollable, shows full track analysis)
- Onset triggers shown as blinking indicators
- Click signal to see detailed view / copy to clipboard for animation track

### Animation Panel Integration

**Waveform display:**
- Simple waveform overview rendered above tracks connected to audio
- Shows full duration of audio file
- Playhead indicator synced to animation timeline
- No zoom/scroll for now (simple overview only)

**When adding an Audio track:**
1. Show dropdown of available signals (grouped by category: DSP, ML)
2. Show output range sliders (min/max)
3. Show smoothing preset dropdown (Kick, Hi-hat, Bass, Vocal, ML) + custom
4. For triggers: show hold/attack/decay controls
5. Live preview of mapped value (small meter showing current output)
6. Waveform appears above track when audio is loaded

---

## WASM Considerations

### Platform-Specific Backends

cpal does NOT support audio input on WASM (only output). However, we can implement
live capture using the Web Audio API directly via web-sys.

| Feature | Desktop | WASM |
|---------|---------|------|
| Audio input | cpal | web-sys + AudioWorklet |
| Audio output | cpal | Web Audio API |
| File decoding | symphonia | symphonia (works in WASM) |
| File loading | filesystem | drag-drop / file picker |
| Export with audio | FFmpeg | Not supported (video only) |
| ML inference | tract (full speed) | tract (~2-3x slower, Tier 1 only for live) |

### WASM Live Capture Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                        Browser                                  │
│                                                                │
│  getUserMedia() ──▶ MediaStream ──▶ MediaStreamAudioSourceNode │
│                                              │                  │
│                                              ▼                  │
│                                      AudioWorkletNode           │
│                                      (runs on audio thread)     │
│                                              │                  │
│                                              ▼ postMessage()    │
│                                      MessagePort                │
│                                              │                  │
└──────────────────────────────────────────────│──────────────────┘
                                               │
                                               ▼ (web-sys callback)
┌──────────────────────────────────────────────────────────────────┐
│                        WASM Module                                │
│                                                                  │
│  Ring Buffer ◀── audio samples (Vec<f32>)                        │
│       │                                                          │
│       ▼                                                          │
│  Tier 1+2 Analysis ──▶ AtomicSignals ──▶ AudioManager API       │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Implementation Details

1. **Permission request:** Call `navigator.mediaDevices.getUserMedia({ audio: true })`
2. **AudioWorklet processor:** JavaScript worklet that buffers samples and posts to main thread
3. **web-sys bindings:** Receive audio data in Rust via message event handler
4. **Same analysis code:** Once samples are in the ring buffer, same analysis pipeline as desktop

### Required web-sys Features

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = [
    # Existing features...
    # Audio capture
    "AudioContext",
    "AudioWorklet",
    "AudioWorkletNode",
    "AudioWorkletNodeOptions",
    "MediaDevices",
    "MediaStream",
    "MediaStreamAudioSourceNode",
    "MediaStreamConstraints",
    "Navigator",
    # Audio playback
    "AudioBuffer",
    "AudioBufferSourceNode",
    "AudioDestinationNode",
    "GainNode",
] }
```

### Limitations

- **Export:** No FFmpeg in browser, so export is video-only on WASM
  - Users can combine audio externally, or we could offer WebM muxing via browser APIs (future)
- **File access:** Must use drag-drop or file picker (no filesystem access)
- **Latency:** May be slightly higher than desktop (~10-15ms vs ~6ms) due to message passing overhead

---

## Implementation Phases

### Phase 0: Signal System Foundation
- [x] Create `src/signal/mod.rs` with `Signal` struct
- [x] Implement `SignalType` enum (Continuous, Trigger, Scalar)
- [x] Implement `.signal` binary file format (read/write)
- [x] Create `SignalManager` with basic load/save/get operations
- [x] Add `TrackSource::Signal` variant to animation system
- [x] Signal evaluation in `Track::evaluate_at()`
- [x] Wire SignalManager to App and pass to animation evaluation

### Phase 1: Audio Infrastructure
- [x] Add `audio` feature flag and dependencies
- [x] Create `src/audio/mod.rs` with `AudioManager` (implements `SignalProducer`)
- [x] Implement `decode.rs` with symphonia (MP3/WAV decode)
- [x] Wire AudioManager to SignalManager (via `import_from_producer()`)

### Phase 2: Offline Analysis
- [x] Port mel spectrogram processor (from Godot project)
- [x] Implement tiered analysis pipeline (STFT-based)
- [x] Add `amplitude`, `energy_*` signals
- [x] Add `spectral_centroid`, `spectral_flux` signals
- [x] Add `onset` signal (transient detection)
- [ ] Implement BPM detection and `beat_phase` (future)
- [ ] Background thread analysis with progress (future - currently synchronous)

### Phase 3: Animation Integration
- [x] Add `TrackSource::Signal` variant
- [x] Implement signal track evaluation in `Track::evaluate_at()`
- [x] Signal smoothing (exponential)
- [x] Update animation serialization (load/save signal tracks)
- [ ] Trigger envelope (attack/decay) for onset signals (future)

### Phase 4: Audio Playback
- [x] Implement cpal output stream (desktop: `playback.rs`)
- [x] Implement Web Audio API playback (WASM: `playback_wasm.rs`)
- [x] Sync with animation timeline
- [x] Seek support
- [x] Play/pause/stop controls
- [x] End detection (both platforms)

### Phase 5: Live Capture (Desktop)
- [x] Implement cpal input stream (`capture_native.rs`)
- [x] Lock-free signal transfer (AtomicSignals)
- [x] Device selection UI
- [x] Low-latency Tier 1+2 analysis (real-time FFT)

### Phase 5b: Live Capture (WASM)
- [x] ScriptProcessorNode-based capture (`capture_wasm.rs`)
- [x] web-sys bindings for getUserMedia
- [x] Ring buffer bridging
- [x] Permission request UI flow
- [x] Same analysis pipeline as desktop

### Phase 6: Export Integration
- [x] Audio-aware export path (signals available during export)
- [ ] Modify FFmpeg pipeline to include audio in MP4 (future)
- [ ] Handle duration mismatches (future)
- [ ] Progress indication for audio muxing (future)

### Phase 7: UI
- [x] Audio panel (file info, playback controls, signal meters)
- [x] Live capture controls (start/stop, device selection)
- [x] Signal monitor (real-time values for all signals)
- [ ] Animation panel integration (waveform display above tracks) (future)
- [ ] Full audio track editor UI (future)

### Phase 8: Polish
- [ ] WASM compatibility testing (both live capture and offline)
- [ ] Error handling and user feedback
- [ ] Documentation

### Phase 9: ML Analysis (Optional)
- [ ] Add `audio-ml` feature flag with `tract-onnx` dependency
- [ ] ML thread infrastructure (separate from DSP thread)
- [ ] Mel spectrogram ring buffer for ML input
- [ ] `tract` ONNX model loading and inference
- [ ] Tier 1 model: vocal presence detection (Silero VAD or similar)
- [ ] Tier 2 model: instrument/genre classification
- [ ] ML signal integration with AudioManager
- [ ] Model download/caching system (for larger models)
- [ ] WASM testing with lightweight models

---

## Open Questions

(None currently)

## Resolved Decisions

### Signal Architecture
- **Generalized signals:** Signals are source-agnostic - audio is just one producer
- **TrackSource::Signal:** Replaces `TrackSource::Audio` - works with any signal source
- **Binary file format:** `.signal` files for storing/exchanging signals (compact, not human-readable)
- **SignalManager:** Central coordinator for all signal sources
- **SignalProducer trait:** Audio, MIDI, external sources all implement this
- **Live recording:** Optional, low-priority feature (capture live signals for later replay)

### Audio System
- **microdsp:** Confirmed available on crates.io - will use for spectral flux detection
- **Multiple audio files:** Single file per animation (may revisit for layering in future)
- **WASM live capture:** Will implement via web-sys + AudioWorklet (cpal doesn't support WASM input)
- **Waveform visualization:** Simple overview, displayed in Animation panel above connected tracks
- **MIDI support:** Not in initial scope (potential future optional feature)
- **Audio effects:** No - purely analysis, no audio modification
- **Audio offset:** Support via `offset` field - negative skips into audio, positive delays start
- **Audio preview:** Audio Panel includes play/pause/scrub controls for finding start point
- **Export fades:** FFmpeg `afade` filter for fade in/out (configured per-animation)
- **Signal visualization:** Audio Panel shows real-time meters + mini sparklines + expandable history

---

## References

- Original audio code: https://github.com/calibas/rust-godot/tree/master/src
- cpal documentation: https://docs.rs/cpal
- symphonia documentation: https://docs.rs/symphonia
- Mel spectrogram theory: https://en.wikipedia.org/wiki/Mel-frequency_cepstrum
- Onset detection: https://www.ee.columbia.edu/~dpwe/pubs/Bello05-ijmm.pdf
