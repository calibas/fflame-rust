//! Signal generator: procedural waveforms that produce Signal data
//!
//! Generators create time-series signals from mathematical waveforms.
//! Output range is 0.0 to 1.0 — amplitude mapping to parameter ranges
//! is handled by animation tracks (TrackSource::Signal).

use serde::{Deserialize, Serialize};

use super::{Signal, SignalType};

/// Waveform type for signal generators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveformType {
    Sine,
    Triangle,
    Sawtooth,
    Square,
    Noise,
}

impl WaveformType {
    /// All available waveform types
    pub const ALL: &'static [WaveformType] = &[
        WaveformType::Sine,
        WaveformType::Triangle,
        WaveformType::Sawtooth,
        WaveformType::Square,
        WaveformType::Noise,
    ];

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            WaveformType::Sine => "Sine",
            WaveformType::Triangle => "Triangle",
            WaveformType::Sawtooth => "Sawtooth",
            WaveformType::Square => "Square",
            WaveformType::Noise => "Noise",
        }
    }
}

impl std::fmt::Display for WaveformType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Configuration for a procedural signal generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    /// Signal name (e.g., "sine_1hz")
    pub name: String,
    /// Waveform shape
    pub waveform: WaveformType,
    /// Frequency in Hz
    pub frequency: f64,
    /// Phase offset (0.0 to 1.0)
    #[serde(default)]
    pub phase: f64,
}

impl GeneratorConfig {
    /// Create a new generator with default parameters
    pub fn new(name: String) -> Self {
        Self {
            name,
            waveform: WaveformType::Sine,
            frequency: 1.0,
            phase: 0.0,
        }
    }

    /// Generate a Signal from this config.
    ///
    /// Pre-computes the entire waveform at the given sample rate and duration.
    /// Output range: 0.0 to 1.0
    pub fn generate_signal(&self, duration: f64, sample_rate: f64) -> Signal {
        let num_samples = (duration * sample_rate).ceil() as usize;
        let mut data = Vec::with_capacity(num_samples);

        // For noise, we need a deterministic seed based on the name
        let mut noise_state: u32 = self.name.bytes().fold(0x12345678u32, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u32)
        });

        for i in 0..num_samples {
            let t = i as f64 / sample_rate;
            let phase = t * self.frequency + self.phase;

            let wave = match self.waveform {
                WaveformType::Sine => {
                    // Sine: -1..1 → 0..1
                    ((phase * std::f64::consts::TAU).sin() + 1.0) * 0.5
                }
                WaveformType::Triangle => {
                    let t_mod = phase - phase.floor();
                    if t_mod < 0.5 {
                        t_mod * 2.0
                    } else {
                        2.0 - t_mod * 2.0
                    }
                }
                WaveformType::Sawtooth => {
                    phase - phase.floor()
                }
                WaveformType::Square => {
                    if phase.fract() < 0.5 { 0.0 } else { 1.0 }
                }
                WaveformType::Noise => {
                    // Simple PCG-like PRNG for deterministic noise
                    noise_state = noise_state.wrapping_mul(747796405).wrapping_add(2891336453);
                    let word = ((noise_state >> ((noise_state >> 28).wrapping_add(4))) ^ noise_state).wrapping_mul(277803737);
                    let result = (word >> 22) ^ word;
                    (result as f64) / (u32::MAX as f64)
                }
            };

            data.push(wave as f32);
        }

        Signal::new(
            self.name.clone(),
            sample_rate,
            if self.waveform == WaveformType::Noise {
                SignalType::Continuous
            } else {
                SignalType::Continuous
            },
            data,
        )
    }
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self::new("generator_1".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_generator() {
        let config = GeneratorConfig {
            name: "test_sine".to_string(),
            waveform: WaveformType::Sine,
            frequency: 1.0,
            phase: 0.0,
        };
        let signal = config.generate_signal(1.0, 100.0);
        assert_eq!(signal.data.len(), 100);
        // At t=0 phase=0: sin(0)=0, mapped to 0.5
        assert!((signal.data[0] - 0.5).abs() < 0.01);
        // All values in 0..1 range
        for &v in &signal.data {
            assert!(v >= 0.0 && v <= 1.0, "Value {} out of range", v);
        }
    }

    #[test]
    fn test_sawtooth_generator() {
        let config = GeneratorConfig {
            name: "test_saw".to_string(),
            waveform: WaveformType::Sawtooth,
            frequency: 1.0,
            phase: 0.0,
        };
        let signal = config.generate_signal(1.0, 100.0);
        assert_eq!(signal.data.len(), 100);
        // At t=0: 0.0
        assert!((signal.data[0] - 0.0).abs() < 0.02);
        // Values should ramp up
        assert!(signal.data[50] > signal.data[10]);
    }

    #[test]
    fn test_noise_generator() {
        let config = GeneratorConfig {
            name: "test_noise".to_string(),
            waveform: WaveformType::Noise,
            frequency: 1.0,
            phase: 0.0,
        };
        let signal = config.generate_signal(1.0, 100.0);
        assert_eq!(signal.data.len(), 100);
        // All values in 0..1 range
        for &v in &signal.data {
            assert!(v >= 0.0 && v <= 1.0, "Noise value {} out of range", v);
        }
        // Noise should have variation (not all same value)
        let min = signal.data.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = signal.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max - min > 0.1, "Noise should have variation");
    }
}
