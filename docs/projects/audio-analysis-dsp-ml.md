Great focus. Let me research what's available for real-time music analysis and visualization in Rust — spectral features, harmonic analysis, pitch detection, and the smaller efficient ML models that actually work at audio rates.Great question. Here's a comprehensive breakdown of music analysis techniques beyond beat detection, organized by what's practical for real-time DSP in Rust.

---

## Core DSP Analysis Features

These are all computed from the FFT magnitude spectrum you're already computing for beat detection:

**Spectral shape descriptors** (trivial to compute, ~microseconds per frame):

| Feature | What it tells you | Formula essence |
|---------|------------------|-----------------|
| **Spectral centroid** | "Brightness" — center of gravity of spectrum | `Σ(freq × magnitude) / Σ(magnitude)` |
| **Spectral spread** | Bandwidth around centroid | Standard deviation of spectrum |
| **Spectral rolloff** | Frequency below which 85-95% of energy lies | Cumulative sum threshold |
| **Spectral flux** | Rate of change between frames | `Σ(|S[k,t] - S[k,t-1]|)` |
| **Spectral flatness** | Noise-like vs tonal (Wiener entropy) | Geometric mean / arithmetic mean |
| **Spectral crest** | "Peakiness" — inverse of flatness | Max / mean |
| **Zero crossing rate** | Roughness, noise content | Count sign changes in time domain |

These are all single-pass operations over your FFT bins. At 256-hop, you could compute all of them in well under 1ms combined.

**Rust crate status**: `aubio-rs` includes spectral descriptors. The `spectrograms` crate (2025) provides STFT, mel spectrograms, MFCCs, and chromagrams with streaming support. For manual implementation, it's ~20 lines each given an FFT.

---

## Pitch Detection (Monophonic)

The `pitch-detection` crate provides three algorithms:

| Algorithm | Compute | Accuracy | Best for |
|-----------|---------|----------|----------|
| **Autocorrelation** | Fastest | Octave errors common | Simple cases |
| **YIN** | Moderate | Good, fewer octave errors | Voice, single instruments |
| **McLeod (MPM)** | Moderate | Best accuracy | General monophonic |

All run comfortably at 3-6ms resolution. YIN needs a window of at least 2× the lowest expected period (e.g., 25ms for 40Hz), but hop size can be much smaller.

---

## Harmonic Analysis

**Chromagram / Pitch Class Profile**: Maps FFT bins to 12 pitch classes (C, C#, D, ..., B). Useful for chord detection, key estimation, and harmonic visualization. The `spectrograms` crate includes this. Requires larger FFT (2048-4096) for good frequency resolution at low pitches.

**Constant-Q Transform (CQT)**: Logarithmic frequency spacing that matches musical octaves — harmonics of a note form a fixed visual pattern regardless of pitch. More expensive than FFT but better for tonal music. No mature Rust crate exists; you'd implement via octave-by-octave FFT with resampling (librosa's approach).

**Harmonic-Percussive Source Separation (HPSS)**: This is simpler than full stem separation and runs in real-time. It applies median filtering horizontally (keeps sustained harmonics) and vertically (keeps transient percussive content) on the spectrogram. Output: two masks you apply to the original STFT, then inverse-STFT. Useful for:
- Feeding the harmonic part to chromagram/pitch detection (cleaner results)
- Feeding the percussive part to onset/beat detection (sharper transients)

No Rust crate, but straightforward to implement: ~100 lines with `rustfft` and a median filter.

---

## MFCCs (Mel-Frequency Cepstral Coefficients)

Standard feature for speech/timbre analysis. Pipeline: FFT → mel filterbank → log → DCT. The first 13 coefficients capture vocal tract shape while discarding pitch — good for speaker/instrument identification.

Rust options: `aubio-rs`, `mfcc` crate, `spectrograms` crate. Real-time friendly (~0.1ms per frame).

---

## Small Real-Time ML Models

For DSP-first but ML-assisted approaches, here's what's practical:

**Keyword spotting / audio event detection** (TinyML):
- ~50-200KB models
- INT8 quantized CNNs on mel spectrograms
- 5-10ms inference on CPU
- Use `tract` (pure Rust ONNX, used by Sonos for wake word) or `ort` with lightweight models

**YAMNet-style classifiers**:
- MobileNet backbone on mel spectrograms
- ~3MB quantized
- Classifies 500+ audio events
- Edge Impulse and TensorFlow Lite have pre-trained versions exportable to ONNX

**Practical pattern**: Compute mel spectrogram in your DSP pipeline (CPU, real-time), feed 1-second windows to a small CNN via `tract`/`ort` on a separate thread, ~20-50ms latency for classification results.

---

## Visualization Approaches

What you can drive from the features above:

| Visualization | Driven by |
|---------------|-----------|
| Waveform oscilloscope | Raw samples |
| Spectrum bars / EQ display | FFT magnitude bins (often log-grouped) |
| Spectrogram (waterfall) | Rolling STFT magnitudes |
| Mel spectrogram | Mel-scaled filterbank output |
| Chromagram / pitch wheel | 12-bin pitch class energy |
| Beat pulse / flash | Onset detection output |
| "Brightness" indicator | Spectral centroid |
| "Energy" meter | RMS or spectral rolloff |
| Harmonic vs percussive split | HPSS masks |

For reactive visuals, the common pattern is exponential smoothing with different attack/decay rates per frequency band (fast attack for transients, slow decay for sustained tones).

---

## Architecture Summary

```
Audio Input (cpal)
    │
    ├── Ring buffer → DSP Thread
    │                     │
    │                     ├── FFT (rustfft, 256-512 samples)
    │                     │     │
    │                     │     ├── Spectral features (centroid, flux, etc.)
    │                     │     ├── Beat/onset detection
    │                     │     ├── HPSS masks
    │                     │     └── Mel spectrogram
    │                     │
    │                     ├── Pitch detection (YIN/MPM, separate buffer)
    │                     │
    │                     └── Chromagram (larger FFT, lower rate)
    │
    └── [Optional] ML Thread
                          │
                          ├── tract/ort inference on mel frames
                          └── Event classification results
```

All the DSP runs in one thread at your 3-6ms hop rate. ML inference (if used) runs asynchronously on accumulated windows.

---

**Bottom line**: Beyond beat detection, spectral features, pitch detection, chromagrams, and HPSS are all real-time DSP that Rust handles well. The ecosystem has workable crates (`aubio-rs`, `pitch-detection`, `spectrograms`, `rustfft`). For ML, stick to sub-1MB quantized models via `tract` if you want to stay lightweight and real-time.