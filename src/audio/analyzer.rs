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
            Signal::new("energy_low".to_string(), signal_rate, SignalType::Continuous, low),
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
            Signal::new("onset".to_string(), signal_rate, SignalType::Trigger, onsets),
        );

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
}
