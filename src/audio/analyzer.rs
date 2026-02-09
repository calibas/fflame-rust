//! Audio signal analysis via STFT and spectral features
//!
//! Provides offline analysis of audio data to extract animation-ready signals:
//! - Energy bands (low, mid, high frequency)
//! - Onset detection (transients, beats)
//! - Spectral features (centroid, flux)

use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::HashMap;
use std::f32::consts::PI;

use crate::signal::{Signal, SignalType};

/// Configuration for audio analysis
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    /// FFT window size (power of 2, typically 1024-4096)
    pub fft_size: usize,
    /// Hop size between windows (typically fft_size / 4)
    pub hop_size: usize,
    /// Number of mel bands for spectrogram
    pub mel_bands: usize,
    /// Minimum frequency for mel scale (Hz)
    pub mel_min_freq: f32,
    /// Maximum frequency for mel scale (Hz)
    pub mel_max_freq: f32,
    /// Onset detection threshold (0.0-1.0)
    pub onset_threshold: f32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            hop_size: 512,
            mel_bands: 128,
            mel_min_freq: 20.0,
            mel_max_freq: 20000.0,
            onset_threshold: 0.3,
        }
    }
}

/// Audio analyzer for extracting signals from audio data
pub struct AudioAnalyzer {
    config: AnalysisConfig,
    sample_rate: u32,
    /// Precomputed Hann window
    window: Vec<f32>,
    /// Mel filterbank matrix
    mel_filterbank: Vec<Vec<f32>>,
}

impl AudioAnalyzer {
    /// Create a new analyzer for the given sample rate
    pub fn new(sample_rate: u32, config: AnalysisConfig) -> Self {
        let window = Self::hann_window(config.fft_size);
        let mel_filterbank = Self::compute_mel_filterbank(
            config.fft_size,
            sample_rate,
            config.mel_bands,
            config.mel_min_freq,
            config.mel_max_freq,
        );

        Self {
            config,
            sample_rate,
            window,
            mel_filterbank,
        }
    }

    /// Analyze audio samples and return extracted signals
    pub fn analyze(&self, samples: &[f32]) -> HashMap<String, Signal> {
        let mut signals = HashMap::new();
        let signal_rate = self.sample_rate as f64 / self.config.hop_size as f64;

        // Compute STFT
        let stft = self.compute_stft(samples);
        if stft.is_empty() {
            return signals;
        }

        // Compute magnitude spectrogram
        let magnitudes: Vec<Vec<f32>> = stft
            .iter()
            .map(|frame| frame.iter().map(|c| c.norm()).collect())
            .collect();

        // Compute mel spectrogram
        let mel_spec = self.compute_mel_spectrogram(&magnitudes);

        // Extract energy bands
        let (low, mid, high) = self.compute_energy_bands(&mel_spec);
        signals.insert(
            "energy_low".to_string(),
            Signal::new("energy_low".to_string(), signal_rate, SignalType::Continuous, low.clone()),
        );
        signals.insert(
            "energy_mid".to_string(),
            Signal::new("energy_mid".to_string(), signal_rate, SignalType::Continuous, mid),
        );
        signals.insert(
            "energy_high".to_string(),
            Signal::new("energy_high".to_string(), signal_rate, SignalType::Continuous, high),
        );

        // Compute total amplitude (RMS of full spectrum)
        let amplitude = self.compute_amplitude(&magnitudes);
        signals.insert(
            "amplitude".to_string(),
            Signal::new("amplitude".to_string(), signal_rate, SignalType::Continuous, amplitude),
        );

        // Compute spectral centroid
        let centroid = self.compute_spectral_centroid(&magnitudes);
        signals.insert(
            "spectral_centroid".to_string(),
            Signal::new(
                "spectral_centroid".to_string(),
                signal_rate,
                SignalType::Continuous,
                centroid,
            ),
        );

        // Compute spectral flux and onset detection
        let flux = self.compute_spectral_flux(&magnitudes);
        signals.insert(
            "spectral_flux".to_string(),
            Signal::new("spectral_flux".to_string(), signal_rate, SignalType::Continuous, flux.clone()),
        );

        // Onset detection from spectral flux
        let onsets = self.detect_onsets(&flux);
        signals.insert(
            "onset".to_string(),
            Signal::new("onset".to_string(), signal_rate, SignalType::Trigger, onsets.clone()),
        );

        // BPM detection (using energy_low for robust autocorrelation) and beat_phase generation
        if let Some(bpm) = self.detect_bpm(&low, signal_rate) {
            signals.insert(
                "bpm".to_string(),
                Signal::new("bpm".to_string(), signal_rate, SignalType::Scalar, vec![bpm]),
            );

            let beat_phase = self.generate_beat_phase(&onsets, bpm, signal_rate);

            // Generate beat trigger: 1.0 on each beat, 0.0 otherwise
            let mut beat = vec![0.0f32; beat_phase.len()];
            for i in 1..beat_phase.len() {
                if beat_phase[i] < beat_phase[i - 1] - 0.5 {
                    beat[i] = 1.0; // Phase wrapped → beat
                }
            }
            // First beat at phase anchor
            if !beat_phase.is_empty() && beat_phase[0] < 0.01 {
                beat[0] = 1.0;
            }

            signals.insert(
                "beat".to_string(),
                Signal::new("beat".to_string(), signal_rate, SignalType::Trigger, beat),
            );
            signals.insert(
                "beat_phase".to_string(),
                Signal::new("beat_phase".to_string(), signal_rate, SignalType::Continuous, beat_phase),
            );
        }

        signals
    }

    /// Compute Short-Time Fourier Transform
    fn compute_stft(&self, samples: &[f32]) -> Vec<Vec<Complex<f32>>> {
        let fft_size = self.config.fft_size;
        let hop_size = self.config.hop_size;

        if samples.len() < fft_size {
            return Vec::new();
        }

        let num_frames = (samples.len() - fft_size) / hop_size + 1;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        let mut result = Vec::with_capacity(num_frames);
        let mut buffer: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); fft_size];

        for frame_idx in 0..num_frames {
            let start = frame_idx * hop_size;

            // Apply window and convert to complex
            for i in 0..fft_size {
                let sample = samples.get(start + i).copied().unwrap_or(0.0);
                buffer[i] = Complex::new(sample * self.window[i], 0.0);
            }

            // Compute FFT in-place
            fft.process(&mut buffer);

            // Keep only positive frequencies (first half + 1)
            let positive_freqs = fft_size / 2 + 1;
            result.push(buffer[..positive_freqs].to_vec());
        }

        result
    }

    /// Compute mel spectrogram from magnitude spectrogram
    fn compute_mel_spectrogram(&self, magnitudes: &[Vec<f32>]) -> Vec<Vec<f32>> {
        magnitudes
            .iter()
            .map(|frame| {
                self.mel_filterbank
                    .iter()
                    .map(|filter| {
                        frame
                            .iter()
                            .zip(filter.iter())
                            .map(|(m, f)| m * f)
                            .sum::<f32>()
                    })
                    .collect()
            })
            .collect()
    }

    /// Extract energy in low, mid, and high frequency bands
    fn compute_energy_bands(&self, mel_spec: &[Vec<f32>]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let num_bands = self.config.mel_bands;
        // Split into thirds: low (bass), mid (mids), high (treble)
        let low_end = num_bands / 3;
        let mid_end = 2 * num_bands / 3;

        let mut low = Vec::with_capacity(mel_spec.len());
        let mut mid = Vec::with_capacity(mel_spec.len());
        let mut high = Vec::with_capacity(mel_spec.len());

        for frame in mel_spec {
            let low_energy: f32 = frame[..low_end].iter().map(|x| x * x).sum();
            let mid_energy: f32 = frame[low_end..mid_end].iter().map(|x| x * x).sum();
            let high_energy: f32 = frame[mid_end..].iter().map(|x| x * x).sum();

            low.push(low_energy.sqrt());
            mid.push(mid_energy.sqrt());
            high.push(high_energy.sqrt());
        }

        // Normalize each band to 0-1
        let normalize = |v: &mut Vec<f32>| {
            let max = v.iter().cloned().fold(0.0f32, f32::max);
            if max > 0.0 {
                for x in v.iter_mut() {
                    *x /= max;
                }
            }
        };

        normalize(&mut low);
        normalize(&mut mid);
        normalize(&mut high);

        (low, mid, high)
    }

    /// Compute overall amplitude (RMS)
    fn compute_amplitude(&self, magnitudes: &[Vec<f32>]) -> Vec<f32> {
        let mut amplitude: Vec<f32> = magnitudes
            .iter()
            .map(|frame| {
                let sum_sq: f32 = frame.iter().map(|m| m * m).sum();
                (sum_sq / frame.len() as f32).sqrt()
            })
            .collect();

        // Normalize to 0-1
        let max = amplitude.iter().cloned().fold(0.0f32, f32::max);
        if max > 0.0 {
            for a in &mut amplitude {
                *a /= max;
            }
        }

        amplitude
    }

    /// Compute spectral centroid (brightness indicator)
    fn compute_spectral_centroid(&self, magnitudes: &[Vec<f32>]) -> Vec<f32> {
        let freq_bins: Vec<f32> = (0..magnitudes[0].len())
            .map(|i| i as f32 * self.sample_rate as f32 / self.config.fft_size as f32)
            .collect();

        let max_freq = self.sample_rate as f32 / 2.0;

        let mut centroid: Vec<f32> = magnitudes
            .iter()
            .map(|frame| {
                let total_magnitude: f32 = frame.iter().sum();
                if total_magnitude > 1e-10 {
                    let weighted_sum: f32 = frame
                        .iter()
                        .zip(freq_bins.iter())
                        .map(|(m, f)| m * f)
                        .sum();
                    weighted_sum / total_magnitude / max_freq // Normalize to 0-1
                } else {
                    0.0
                }
            })
            .collect();

        // Clamp to 0-1
        for c in &mut centroid {
            *c = c.clamp(0.0, 1.0);
        }

        centroid
    }

    /// Compute spectral flux (rate of spectral change)
    fn compute_spectral_flux(&self, magnitudes: &[Vec<f32>]) -> Vec<f32> {
        if magnitudes.len() < 2 {
            return vec![0.0; magnitudes.len()];
        }

        let mut flux = Vec::with_capacity(magnitudes.len());
        flux.push(0.0); // First frame has no previous

        for i in 1..magnitudes.len() {
            let prev = &magnitudes[i - 1];
            let curr = &magnitudes[i];

            // Half-wave rectified difference (only positive changes)
            let diff: f32 = curr
                .iter()
                .zip(prev.iter())
                .map(|(c, p)| (c - p).max(0.0))
                .sum();

            flux.push(diff);
        }

        // Normalize to 0-1
        let max = flux.iter().cloned().fold(0.0f32, f32::max);
        if max > 0.0 {
            for f in &mut flux {
                *f /= max;
            }
        }

        flux
    }

    /// Detect onsets from spectral flux using adaptive threshold
    fn detect_onsets(&self, flux: &[f32]) -> Vec<f32> {
        if flux.is_empty() {
            return Vec::new();
        }

        let mut onsets = vec![0.0f32; flux.len()];

        // Compute local mean for adaptive threshold
        let window_size = 10; // frames

        for i in 0..flux.len() {
            // Local mean
            let start = i.saturating_sub(window_size);
            let end = (i + window_size + 1).min(flux.len());
            let local_mean: f32 = flux[start..end].iter().sum::<f32>() / (end - start) as f32;

            // Threshold is base + scaled local mean
            let threshold = self.config.onset_threshold * 0.5 + local_mean * 1.5;

            if flux[i] > threshold {
                onsets[i] = 1.0;
            }
        }

        // Post-processing: ensure minimum gap between onsets
        let min_gap = 3; // minimum frames between onsets
        let mut last_onset: Option<usize> = None;

        for i in 0..onsets.len() {
            if onsets[i] > 0.0 {
                if let Some(last) = last_onset {
                    if i - last < min_gap {
                        onsets[i] = 0.0;
                        continue;
                    }
                }
                last_onset = Some(i);
            }
        }

        onsets
    }

    /// Detect BPM via autocorrelation of a continuous energy signal
    ///
    /// Uses energy_low (bass energy) which tracks kick drums and bass hits —
    /// much more reliable than sparse binary onset signals for autocorrelation.
    /// Searches for periodicity across 60-200 BPM range.
    fn detect_bpm(&self, energy: &[f32], signal_rate: f64) -> Option<f32> {
        if energy.len() < (signal_rate * 4.0) as usize {
            return None; // Need at least 4 seconds for reliable detection
        }

        // BPM search range
        let min_bpm = 60.0f64;
        let max_bpm = 200.0f64;

        // Convert BPM range to lag range (in samples at signal_rate)
        let min_lag = (signal_rate * 60.0 / max_bpm).ceil() as usize;
        let max_lag = (signal_rate * 60.0 / min_bpm).floor() as usize;

        if max_lag >= energy.len() || min_lag >= max_lag {
            return None;
        }

        // Compute mean for zero-centered autocorrelation
        let mean: f32 = energy.iter().sum::<f32>() / energy.len() as f32;

        // Compute autocorrelation at lag=0 (normalization factor)
        let mut corr_zero = 0.0f32;
        for v in energy {
            let d = v - mean;
            corr_zero += d * d;
        }
        if corr_zero < 1e-10 {
            return None;
        }

        // Compute normalized autocorrelation for each lag in the BPM range
        let mut best_lag = min_lag;
        let mut best_corr = f32::NEG_INFINITY;

        for lag in min_lag..=max_lag {
            let mut corr = 0.0f32;
            let n = energy.len() - lag;
            for i in 0..n {
                corr += (energy[i] - mean) * (energy[i + lag] - mean);
            }
            // Normalize to [-1, 1]
            corr /= corr_zero;

            if corr > best_corr {
                best_corr = corr;
                best_lag = lag;
            }
        }

        // Require minimum correlation strength for a real beat
        if best_corr < 0.05 {
            return None;
        }

        let bpm = (signal_rate * 60.0 / best_lag as f64) as f32;

        // Resolve half/double time ambiguity: prefer 80-160 BPM range
        let preferred_min = 80.0f32;
        let preferred_max = 160.0f32;

        let autocorr_at = |lag: usize| -> f32 {
            if lag >= energy.len() { return 0.0; }
            let n = energy.len() - lag;
            let mut c = 0.0f32;
            for i in 0..n {
                c += (energy[i] - mean) * (energy[i + lag] - mean);
            }
            c / corr_zero
        };

        let mut final_bpm = bpm;
        if bpm < preferred_min && bpm * 2.0 <= preferred_max {
            let double_corr = autocorr_at(best_lag / 2);
            if double_corr > best_corr * 0.7 {
                final_bpm = bpm * 2.0;
            }
        } else if bpm > preferred_max && bpm / 2.0 >= preferred_min {
            let half_corr = autocorr_at(best_lag * 2);
            if half_corr > best_corr * 0.7 {
                final_bpm = bpm / 2.0;
            }
        }

        Some((final_bpm * 10.0).round() / 10.0)
    }

    /// Generate a beat_phase signal: 0-1 sawtooth locked to the detected BPM
    ///
    /// Finds the first onset to anchor the phase, then generates a repeating
    /// 0→1 ramp at the detected tempo.
    fn generate_beat_phase(&self, onsets: &[f32], bpm: f32, signal_rate: f64) -> Vec<f32> {
        let beat_period_samples = signal_rate * 60.0 / bpm as f64;
        let num_samples = onsets.len();

        // Find the first strong onset to anchor the beat grid
        let first_onset = onsets.iter().position(|&v| v > 0.0).unwrap_or(0);

        let mut phase = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            // Distance from the anchor onset, modulo one beat period
            let offset = (i as f64 - first_onset as f64).rem_euclid(beat_period_samples);
            let p = (offset / beat_period_samples) as f32;
            phase.push(p.clamp(0.0, 1.0));
        }

        phase
    }

    /// Generate a Hann window of given size
    fn hann_window(size: usize) -> Vec<f32> {
        (0..size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos()))
            .collect()
    }

    /// Compute mel filterbank matrix
    fn compute_mel_filterbank(
        fft_size: usize,
        sample_rate: u32,
        num_bands: usize,
        min_freq: f32,
        max_freq: f32,
    ) -> Vec<Vec<f32>> {
        let num_bins = fft_size / 2 + 1;

        // Convert frequency to mel scale
        let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
        let mel_to_hz = |mel: f32| 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0);

        let mel_min = hz_to_mel(min_freq);
        let mel_max = hz_to_mel(max_freq.min(sample_rate as f32 / 2.0));

        // Create mel band center frequencies
        let mel_points: Vec<f32> = (0..=num_bands + 1)
            .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (num_bands + 1) as f32)
            .collect();

        let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

        // Convert to FFT bin indices
        let bin_points: Vec<usize> = hz_points
            .iter()
            .map(|&hz| {
                ((hz * fft_size as f32 / sample_rate as f32).round() as usize).min(num_bins - 1)
            })
            .collect();

        // Create triangular filters
        let mut filterbank = Vec::with_capacity(num_bands);

        for band in 0..num_bands {
            let mut filter = vec![0.0f32; num_bins];

            let left = bin_points[band];
            let center = bin_points[band + 1];
            let right = bin_points[band + 2];

            // Rising slope
            if center > left {
                for bin in left..center {
                    filter[bin] = (bin - left) as f32 / (center - left) as f32;
                }
            }

            // Falling slope
            if right > center {
                for bin in center..=right {
                    filter[bin] = (right - bin) as f32 / (right - center) as f32;
                }
            }

            filterbank.push(filter);
        }

        filterbank
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window() {
        let window = AudioAnalyzer::hann_window(256);
        assert_eq!(window.len(), 256);
        // Hann window starts and ends near 0
        assert!(window[0] < 0.01);
        assert!(window[255] < 0.01);
        // Peak is in the middle
        assert!(window[128] > 0.99);
    }

    #[test]
    fn test_mel_filterbank() {
        let filterbank = AudioAnalyzer::compute_mel_filterbank(1024, 44100, 40, 20.0, 8000.0);
        assert_eq!(filterbank.len(), 40);
        // Each filter should be a vector of 513 bins (1024/2 + 1)
        assert_eq!(filterbank[0].len(), 513);
        // Filters should sum to positive values (not empty)
        let sum: f32 = filterbank[0].iter().sum();
        assert!(sum > 0.0);
    }

    #[test]
    fn test_analyze_empty() {
        let analyzer = AudioAnalyzer::new(44100, AnalysisConfig::default());
        let signals = analyzer.analyze(&[]);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_analyze_short() {
        let analyzer = AudioAnalyzer::new(44100, AnalysisConfig::default());
        // Too short for one frame
        let signals = analyzer.analyze(&vec![0.0; 100]);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_analyze_sine_wave() {
        let analyzer = AudioAnalyzer::new(44100, AnalysisConfig::default());

        // Generate 1 second of 440Hz sine wave
        let samples: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let signals = analyzer.analyze(&samples);

        // Should have all expected signals
        assert!(signals.contains_key("amplitude"));
        assert!(signals.contains_key("energy_low"));
        assert!(signals.contains_key("energy_mid"));
        assert!(signals.contains_key("energy_high"));
        assert!(signals.contains_key("spectral_centroid"));
        assert!(signals.contains_key("spectral_flux"));
        assert!(signals.contains_key("onset"));

        // Amplitude should be non-zero for sine wave
        let amp = &signals["amplitude"];
        let max_amp = amp.data.iter().cloned().fold(0.0f32, f32::max);
        assert!(max_amp > 0.5, "Amplitude should be significant for sine wave");
    }

    #[test]
    fn test_bpm_detection_120bpm() {
        let sample_rate = 44100u32;
        let config = AnalysisConfig::default();
        let signal_rate = sample_rate as f64 / config.hop_size as f64;
        let analyzer = AudioAnalyzer::new(sample_rate, config);

        // Generate 5 seconds of audio with kick drum pulses at 120 BPM
        // 120 BPM = 2 beats/sec = one beat every 0.5s = every 22050 samples
        let duration_secs = 5.0;
        let total_samples = (sample_rate as f64 * duration_secs) as usize;
        let beat_interval = sample_rate as f64 * 60.0 / 120.0; // samples per beat

        let mut samples = vec![0.0f32; total_samples];
        let mut beat_pos = 0.0;
        while (beat_pos as usize) < total_samples {
            // Short burst of low-frequency energy (kick drum simulation)
            let start = beat_pos as usize;
            let burst_len = 1000.min(total_samples - start);
            for i in 0..burst_len {
                let t = i as f32 / sample_rate as f32;
                // 80 Hz sine with fast decay
                samples[start + i] = (2.0 * PI * 80.0 * t).sin() * (-t * 30.0).exp();
            }
            beat_pos += beat_interval;
        }

        let signals = analyzer.analyze(&samples);

        // Should detect BPM
        assert!(signals.contains_key("bpm"), "BPM signal should be present");
        let detected_bpm = signals["bpm"].data[0];
        // Allow ±5 BPM tolerance (autocorrelation resolution depends on signal rate)
        assert!(
            (detected_bpm - 120.0).abs() < 5.0,
            "Expected ~120 BPM, got {:.1} BPM",
            detected_bpm
        );

        // Should have beat_phase
        assert!(signals.contains_key("beat_phase"), "beat_phase signal should be present");
        let phase = &signals["beat_phase"].data;
        assert!(!phase.is_empty());

        // Phase should be in 0-1 range
        for &p in phase {
            assert!(p >= 0.0 && p <= 1.0, "Phase {} out of range", p);
        }

        // Phase should have the right number of cycles (~10 beats in 5 seconds at 120 BPM)
        // Count zero crossings (phase resets from near 1.0 back to near 0.0)
        let mut beat_count = 0;
        for i in 1..phase.len() {
            if phase[i] < 0.1 && phase[i - 1] > 0.9 {
                beat_count += 1;
            }
        }
        // Expect roughly 10 beats (120 BPM * 5s / 60 = 10), allow ±2
        assert!(
            beat_count >= 8 && beat_count <= 12,
            "Expected ~10 beat cycles, got {}",
            beat_count
        );
    }

    #[test]
    fn test_bpm_detection_short_audio() {
        let analyzer = AudioAnalyzer::new(44100, AnalysisConfig::default());
        // 1 second of silence - too short or no beats
        let samples = vec![0.0f32; 44100];
        let signals = analyzer.analyze(&samples);
        // BPM might not be detected for silence, which is fine
        if let Some(bpm_signal) = signals.get("bpm") {
            // If detected, should be in valid range
            let bpm = bpm_signal.data[0];
            assert!(bpm >= 60.0 && bpm <= 200.0, "BPM {} out of valid range", bpm);
        }
    }
}
