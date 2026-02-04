//! Signal system for animation-reactive data
//!
//! This module provides a generalized signal storage and management system
//! that can be used to drive animation tracks from various sources (audio
//! analysis, MIDI, sensors, etc.).
//!
//! # Architecture
//!
//! - **Signal**: Time-series data with metadata (name, sample rate, type)
//! - **SignalType**: Classification (Continuous, Trigger, Scalar)
//! - **SignalManager**: Central coordinator for loading/saving/accessing signals
//! - **SignalProducer**: Trait for live signal sources
//!
//! # File Format
//!
//! Signals are stored in a compact binary format (.signal):
//! - Header: magic, version, type, flags, sample_rate, lengths
//! - Body: name (UTF-8), data (f32 array), optional metadata (JSON)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

/// Magic bytes for .signal file format
const SIGNAL_MAGIC: &[u8; 4] = b"FSIG";

/// Current file format version
const SIGNAL_VERSION: u16 = 1;

/// A time-series signal that can drive animation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// User-facing name (e.g., "bass_energy", "beat_trigger")
    pub name: String,

    /// Sample rate in Hz (samples per second)
    /// For audio-derived signals, typically matches audio analysis rate (e.g., 86 Hz for 512-sample hop at 44.1kHz)
    pub sample_rate: f64,

    /// Type classification for UI and interpolation hints
    pub signal_type: SignalType,

    /// Raw signal data (one f32 per sample)
    pub data: Vec<f32>,

    /// Optional metadata (source info, analysis params, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SignalMetadata>,
}

/// Classification of signal behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    /// Smooth continuous values (e.g., spectral centroid, RMS energy)
    /// Suitable for linear interpolation
    Continuous,

    /// Discrete events/impulses (e.g., beat triggers, onsets)
    /// Values are typically 0 or 1, no interpolation between samples
    Trigger,

    /// Single scalar value with unit (e.g., BPM, key signature)
    /// Not time-varying within a signal file
    Scalar,
}

/// Optional metadata about signal origin and parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalMetadata {
    /// Source description (e.g., "audio_file: track.mp3", "live_input")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Analysis parameters used to generate this signal
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, serde_json::Value>,

    /// Unit string for Scalar type (e.g., "BPM", "Hz")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Creation timestamp (ISO 8601)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Additional custom fields
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Signal {
    /// Create a new signal with the given parameters
    pub fn new(name: String, sample_rate: f64, signal_type: SignalType, data: Vec<f32>) -> Self {
        Self {
            name,
            sample_rate,
            signal_type,
            data,
            metadata: None,
        }
    }

    /// Create a new signal with metadata
    pub fn with_metadata(
        name: String,
        sample_rate: f64,
        signal_type: SignalType,
        data: Vec<f32>,
        metadata: SignalMetadata,
    ) -> Self {
        Self {
            name,
            sample_rate,
            signal_type,
            data,
            metadata: Some(metadata),
        }
    }

    /// Get the duration of this signal in seconds
    pub fn duration(&self) -> f64 {
        if self.sample_rate > 0.0 {
            self.data.len() as f64 / self.sample_rate
        } else {
            0.0
        }
    }

    /// Get value at a specific time (in seconds)
    ///
    /// For Continuous signals, linearly interpolates between samples.
    /// For Trigger signals, returns the nearest sample value.
    /// For Scalar signals, returns the first (and only meaningful) value.
    pub fn value_at(&self, time: f64) -> Option<f32> {
        if self.data.is_empty() || self.sample_rate <= 0.0 {
            return None;
        }

        match self.signal_type {
            SignalType::Scalar => {
                // Scalar: return first value regardless of time
                Some(self.data[0])
            }
            SignalType::Trigger => {
                // Trigger: nearest sample, no interpolation
                let sample_idx = (time * self.sample_rate).round() as usize;
                self.data.get(sample_idx.min(self.data.len() - 1)).copied()
            }
            SignalType::Continuous => {
                // Continuous: linear interpolation
                let sample_pos = time * self.sample_rate;
                let idx0 = sample_pos.floor() as usize;
                let idx1 = idx0 + 1;

                if idx0 >= self.data.len() {
                    return Some(*self.data.last().unwrap());
                }
                if idx1 >= self.data.len() {
                    return Some(self.data[idx0]);
                }

                let t = sample_pos.fract() as f32;
                let v0 = self.data[idx0];
                let v1 = self.data[idx1];
                Some(v0 + (v1 - v0) * t)
            }
        }
    }

    /// Save signal to binary file
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_binary(&mut writer)
    }

    /// Load signal from binary file
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        Self::read_binary(&mut reader)
    }

    /// Write signal to binary format
    pub fn write_binary<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Prepare metadata JSON if present
        let metadata_bytes = self
            .metadata
            .as_ref()
            .map(|m| serde_json::to_vec(m).unwrap_or_default())
            .unwrap_or_default();

        let name_bytes = self.name.as_bytes();

        // Write header (30 bytes)
        writer.write_all(SIGNAL_MAGIC)?; // 4 bytes
        writer.write_all(&SIGNAL_VERSION.to_le_bytes())?; // 2 bytes
        writer.write_all(&[self.signal_type.to_u8()])?; // 1 byte
        writer.write_all(&[0u8])?; // 1 byte flags (reserved)
        writer.write_all(&self.sample_rate.to_le_bytes())?; // 8 bytes
        writer.write_all(&(self.data.len() as u64).to_le_bytes())?; // 8 bytes
        writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?; // 2 bytes
        writer.write_all(&(metadata_bytes.len() as u32).to_le_bytes())?; // 4 bytes

        // Write variable sections
        writer.write_all(name_bytes)?;

        // Write data as raw f32 bytes
        for &value in &self.data {
            writer.write_all(&value.to_le_bytes())?;
        }

        // Write metadata JSON if present
        if !metadata_bytes.is_empty() {
            writer.write_all(&metadata_bytes)?;
        }

        writer.flush()
    }

    /// Read signal from binary format
    pub fn read_binary<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // Read header
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != SIGNAL_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid signal file magic",
            ));
        }

        let mut version_bytes = [0u8; 2];
        reader.read_exact(&mut version_bytes)?;
        let version = u16::from_le_bytes(version_bytes);
        if version > SIGNAL_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unsupported signal file version: {}", version),
            ));
        }

        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)?;
        let signal_type = SignalType::from_u8(type_byte[0]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid signal type")
        })?;

        let mut flags = [0u8; 1];
        reader.read_exact(&mut flags)?; // Reserved, ignore for now

        let mut sample_rate_bytes = [0u8; 8];
        reader.read_exact(&mut sample_rate_bytes)?;
        let sample_rate = f64::from_le_bytes(sample_rate_bytes);

        let mut data_len_bytes = [0u8; 8];
        reader.read_exact(&mut data_len_bytes)?;
        let data_len = u64::from_le_bytes(data_len_bytes) as usize;

        let mut name_len_bytes = [0u8; 2];
        reader.read_exact(&mut name_len_bytes)?;
        let name_len = u16::from_le_bytes(name_len_bytes) as usize;

        let mut metadata_len_bytes = [0u8; 4];
        reader.read_exact(&mut metadata_len_bytes)?;
        let metadata_len = u32::from_le_bytes(metadata_len_bytes) as usize;

        // Read name
        let mut name_bytes = vec![0u8; name_len];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in signal name")
        })?;

        // Read data
        let mut data = Vec::with_capacity(data_len);
        for _ in 0..data_len {
            let mut value_bytes = [0u8; 4];
            reader.read_exact(&mut value_bytes)?;
            data.push(f32::from_le_bytes(value_bytes));
        }

        // Read metadata if present
        let metadata = if metadata_len > 0 {
            let mut metadata_bytes = vec![0u8; metadata_len];
            reader.read_exact(&mut metadata_bytes)?;
            serde_json::from_slice(&metadata_bytes).ok()
        } else {
            None
        };

        Ok(Self {
            name,
            sample_rate,
            signal_type,
            data,
            metadata,
        })
    }
}

impl SignalType {
    /// Convert to byte for binary format
    fn to_u8(self) -> u8 {
        match self {
            SignalType::Continuous => 0,
            SignalType::Trigger => 1,
            SignalType::Scalar => 2,
        }
    }

    /// Convert from byte
    fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(SignalType::Continuous),
            1 => Some(SignalType::Trigger),
            2 => Some(SignalType::Scalar),
            _ => None,
        }
    }
}

impl Default for SignalType {
    fn default() -> Self {
        SignalType::Continuous
    }
}

/// Trait for live signal producers (audio input, MIDI, sensors, etc.)
///
/// Implementations provide real-time signal values during animation playback.
pub trait SignalProducer: Send {
    /// Get list of signal names this producer can provide
    fn signal_names(&self) -> Vec<String>;

    /// Get current live value for a signal (real-time, no buffering)
    /// Returns None if signal name is unknown or source is inactive
    fn get_live_value(&self, name: &str) -> Option<f32>;

    /// Get complete buffered signal (for offline analysis results)
    /// Returns None if signal hasn't been computed yet
    fn get_signal(&self, name: &str) -> Option<Signal>;

    /// Check if this producer is currently active/connected
    fn is_active(&self) -> bool;
}

/// Central manager for all signals
///
/// Handles loading/saving signal files and coordinating live producers.
#[derive(Default)]
pub struct SignalManager {
    /// Loaded/computed signals indexed by name
    signals: HashMap<String, Signal>,

    /// Active live signal producers
    #[allow(dead_code)]
    live_producers: Vec<Box<dyn SignalProducer>>,
}

impl SignalManager {
    /// Create a new empty signal manager
    pub fn new() -> Self {
        Self {
            signals: HashMap::new(),
            live_producers: Vec::new(),
        }
    }

    /// Get a signal by name
    pub fn get(&self, name: &str) -> Option<&Signal> {
        self.signals.get(name)
    }

    /// Get mutable reference to a signal by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Signal> {
        self.signals.get_mut(name)
    }

    /// Add or replace a signal
    pub fn insert(&mut self, signal: Signal) {
        self.signals.insert(signal.name.clone(), signal);
    }

    /// Remove a signal by name
    pub fn remove(&mut self, name: &str) -> Option<Signal> {
        self.signals.remove(name)
    }

    /// Get all signal names
    pub fn signal_names(&self) -> Vec<&str> {
        self.signals.keys().map(|s| s.as_str()).collect()
    }

    /// Get number of loaded signals
    pub fn len(&self) -> usize {
        self.signals.len()
    }

    /// Check if no signals are loaded
    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }

    /// Load a signal from file and add it to the manager
    pub fn load_from_file(&mut self, path: &Path) -> std::io::Result<&Signal> {
        let signal = Signal::load_from_file(path)?;
        let name = signal.name.clone();
        self.signals.insert(name.clone(), signal);
        Ok(self.signals.get(&name).unwrap())
    }

    /// Save a signal to file
    pub fn save_to_file(&self, name: &str, path: &Path) -> std::io::Result<()> {
        let signal = self.signals.get(name).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Signal '{}' not found", name),
            )
        })?;
        signal.save_to_file(path)
    }

    /// Register a live signal producer
    pub fn add_producer(&mut self, producer: Box<dyn SignalProducer>) {
        self.live_producers.push(producer);
    }

    /// Get live value for a signal (checks producers first, then stored signals)
    pub fn get_live_value(&self, name: &str, time: f64) -> Option<f32> {
        // Check live producers first
        for producer in &self.live_producers {
            if producer.is_active() {
                if let Some(value) = producer.get_live_value(name) {
                    return Some(value);
                }
            }
        }

        // Fall back to stored signal
        self.signals.get(name).and_then(|s| s.value_at(time))
    }

    /// Get value at specific time (for offline/export mode)
    pub fn get_value_at(&self, name: &str, time: f64) -> Option<f32> {
        self.signals.get(name).and_then(|s| s.value_at(time))
    }

    /// Clear all signals and producers
    pub fn clear(&mut self) {
        self.signals.clear();
        self.live_producers.clear();
    }

    /// Iterate over all signals
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Signal)> {
        self.signals.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_signal_value_at_continuous() {
        let signal = Signal::new(
            "test".to_string(),
            10.0, // 10 Hz
            SignalType::Continuous,
            vec![0.0, 1.0, 2.0, 3.0, 4.0],
        );

        // Exact sample points
        assert!((signal.value_at(0.0).unwrap() - 0.0).abs() < 0.001);
        assert!((signal.value_at(0.1).unwrap() - 1.0).abs() < 0.001);
        assert!((signal.value_at(0.2).unwrap() - 2.0).abs() < 0.001);

        // Interpolated points
        assert!((signal.value_at(0.05).unwrap() - 0.5).abs() < 0.001);
        assert!((signal.value_at(0.15).unwrap() - 1.5).abs() < 0.001);

        // Beyond end
        assert!((signal.value_at(10.0).unwrap() - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_signal_value_at_trigger() {
        let signal = Signal::new(
            "beat".to_string(),
            10.0,
            SignalType::Trigger,
            vec![0.0, 1.0, 0.0, 0.0, 1.0],
        );

        // Should return nearest sample, no interpolation
        assert!((signal.value_at(0.0).unwrap() - 0.0).abs() < 0.001);
        assert!((signal.value_at(0.1).unwrap() - 1.0).abs() < 0.001);
        assert!((signal.value_at(0.04).unwrap() - 0.0).abs() < 0.001); // 0.4 rounds to 0
        assert!((signal.value_at(0.06).unwrap() - 1.0).abs() < 0.001); // 0.6 rounds to 1
    }

    #[test]
    fn test_signal_value_at_scalar() {
        let signal = Signal::new("bpm".to_string(), 1.0, SignalType::Scalar, vec![120.0]);

        // Should always return first value
        assert!((signal.value_at(0.0).unwrap() - 120.0).abs() < 0.001);
        assert!((signal.value_at(100.0).unwrap() - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_signal_duration() {
        let signal = Signal::new(
            "test".to_string(),
            44100.0,
            SignalType::Continuous,
            vec![0.0; 44100],
        );

        assert!((signal.duration() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_signal_binary_roundtrip() {
        let original = Signal::with_metadata(
            "test_signal".to_string(),
            86.0,
            SignalType::Continuous,
            vec![0.1, 0.5, 0.9, 0.3],
            SignalMetadata {
                source: Some("test.mp3".to_string()),
                unit: None,
                params: HashMap::new(),
                created_at: Some("2025-01-01T00:00:00Z".to_string()),
                extra: HashMap::new(),
            },
        );

        // Write to buffer
        let mut buffer = Vec::new();
        original.write_binary(&mut buffer).unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let loaded = Signal::read_binary(&mut cursor).unwrap();

        assert_eq!(loaded.name, original.name);
        assert!((loaded.sample_rate - original.sample_rate).abs() < 0.001);
        assert_eq!(loaded.signal_type, original.signal_type);
        assert_eq!(loaded.data.len(), original.data.len());
        for (a, b) in loaded.data.iter().zip(original.data.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
        assert!(loaded.metadata.is_some());
        assert_eq!(
            loaded.metadata.as_ref().unwrap().source,
            original.metadata.as_ref().unwrap().source
        );
    }

    #[test]
    fn test_signal_manager_basic() {
        let mut manager = SignalManager::new();

        let signal = Signal::new(
            "energy".to_string(),
            100.0,
            SignalType::Continuous,
            vec![0.0, 0.5, 1.0],
        );

        manager.insert(signal);

        assert_eq!(manager.len(), 1);
        assert!(manager.get("energy").is_some());
        assert!(manager.get("nonexistent").is_none());

        // Test value lookup
        let value = manager.get_value_at("energy", 0.01);
        assert!(value.is_some());
        assert!((value.unwrap() - 0.5).abs() < 0.001);
    }
}
