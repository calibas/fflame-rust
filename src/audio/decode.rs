//! Audio file decoding via symphonia
//!
//! Supports MP3, WAV, FLAC, and OGG formats.

use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Error type for audio decoding
#[derive(Debug)]
pub enum DecodeError {
    /// File not found or cannot be read
    IoError(std::io::Error),
    /// Unsupported or invalid audio format
    FormatError(String),
    /// Decoding error
    DecodeError(String),
    /// No audio tracks found in file
    NoAudioTrack,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::IoError(e) => write!(f, "I/O error: {}", e),
            DecodeError::FormatError(s) => write!(f, "Format error: {}", s),
            DecodeError::DecodeError(s) => write!(f, "Decode error: {}", s),
            DecodeError::NoAudioTrack => write!(f, "No audio track found"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<std::io::Error> for DecodeError {
    fn from(e: std::io::Error) -> Self {
        DecodeError::IoError(e)
    }
}

impl From<SymphoniaError> for DecodeError {
    fn from(e: SymphoniaError) -> Self {
        match e {
            SymphoniaError::IoError(io) => DecodeError::IoError(io),
            SymphoniaError::DecodeError(s) => DecodeError::DecodeError(s.to_string()),
            SymphoniaError::Unsupported(s) => DecodeError::FormatError(s.to_string()),
            _ => DecodeError::DecodeError(format!("{}", e)),
        }
    }
}

/// Decoded audio data stored as f32 samples
#[derive(Clone)]
pub struct AudioData {
    /// Interleaved audio samples (f32, normalized to -1.0..1.0)
    pub samples: Vec<f32>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u16,
}

impl AudioData {
    /// Get total duration in seconds
    pub fn duration(&self) -> f64 {
        let total_samples = self.samples.len() / self.channels as usize;
        total_samples as f64 / self.sample_rate as f64
    }

    /// Get total number of frames (samples per channel)
    pub fn num_frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Convert to mono by averaging channels
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }

        let num_frames = self.num_frames();
        let mut mono = Vec::with_capacity(num_frames);

        for i in 0..num_frames {
            let mut sum = 0.0f32;
            for ch in 0..self.channels as usize {
                sum += self.samples[i * self.channels as usize + ch];
            }
            mono.push(sum / self.channels as f32);
        }

        mono
    }

    /// Get a specific channel's samples
    pub fn channel(&self, channel: usize) -> Vec<f32> {
        if channel >= self.channels as usize {
            return Vec::new();
        }

        let num_frames = self.num_frames();
        let mut result = Vec::with_capacity(num_frames);

        for i in 0..num_frames {
            result.push(self.samples[i * self.channels as usize + channel]);
        }

        result
    }
}

/// Audio file decoder using symphonia
pub struct AudioDecoder;

impl AudioDecoder {
    /// Decode an audio file from disk
    pub fn decode_file(path: &Path) -> Result<AudioData, DecodeError> {
        let file = std::fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint based on file extension
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        Self::decode_stream(mss, hint)
    }

    /// Decode audio from raw bytes (for WASM file picker)
    pub fn decode_bytes(data: &[u8]) -> Result<AudioData, DecodeError> {
        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
        let hint = Hint::new();

        Self::decode_stream(mss, hint)
    }

    /// Internal: decode from a MediaSourceStream
    fn decode_stream(mss: MediaSourceStream, hint: Hint) -> Result<AudioData, DecodeError> {
        // Probe the format
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| DecodeError::FormatError(format!("Failed to probe format: {}", e)))?;

        let mut format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(DecodeError::NoAudioTrack)?;

        let track_id = track.id;

        // Get codec parameters
        let codec_params = track.codec_params.clone();
        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| DecodeError::FormatError("Unknown sample rate".to_string()))?;
        let channels = codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(2);

        // Create decoder
        let decoder_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &decoder_opts)
            .map_err(|e| DecodeError::FormatError(format!("Failed to create decoder: {}", e)))?;

        // Decode all packets
        let mut samples: Vec<f32> = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break; // End of stream
                }
                Err(e) => return Err(e.into()),
            };

            // Skip packets from other tracks
            if packet.track_id() != track_id {
                continue;
            }

            // Decode the packet
            let decoded = match decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(_)) => continue, // Skip bad frames
                Err(e) => return Err(e.into()),
            };

            // Convert to f32 and append
            Self::append_samples(&decoded, &mut samples);
        }

        Ok(AudioData {
            samples,
            sample_rate,
            channels,
        })
    }

    /// Convert decoded audio buffer to f32 samples and append to output
    fn append_samples(decoded: &AudioBufferRef, output: &mut Vec<f32>) {
        match decoded {
            AudioBufferRef::F32(buf) => {
                // Already f32, just copy interleaved
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        output.push(buf.chan(ch)[frame]);
                    }
                }
            }
            AudioBufferRef::F64(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        output.push(buf.chan(ch)[frame] as f32);
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        output.push(buf.chan(ch)[frame] as f32 / 32768.0);
                    }
                }
            }
            AudioBufferRef::S24(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        // S24 is stored as i32 with upper 8 bits unused
                        output.push(buf.chan(ch)[frame].0 as f32 / 8388608.0);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        output.push(buf.chan(ch)[frame] as f32 / 2147483648.0);
                    }
                }
            }
            AudioBufferRef::U8(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                output.reserve(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        // U8 is unsigned, center is 128
                        output.push((buf.chan(ch)[frame] as f32 - 128.0) / 128.0);
                    }
                }
            }
            // Other formats (U16, U24, U32, S8) are rare - skip silently
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_data_duration() {
        let data = AudioData {
            samples: vec![0.0; 88200], // 2 seconds stereo at 44100Hz
            sample_rate: 44100,
            channels: 2,
        };
        assert!((data.duration() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_data_to_mono() {
        let data = AudioData {
            samples: vec![0.5, -0.5, 1.0, 0.0], // 2 frames stereo
            sample_rate: 44100,
            channels: 2,
        };
        let mono = data.to_mono();
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.0).abs() < 0.001); // (0.5 + -0.5) / 2
        assert!((mono[1] - 0.5).abs() < 0.001); // (1.0 + 0.0) / 2
    }

    #[test]
    fn test_audio_data_channel() {
        let data = AudioData {
            samples: vec![0.1, 0.2, 0.3, 0.4], // 2 frames stereo
            sample_rate: 44100,
            channels: 2,
        };
        let left = data.channel(0);
        let right = data.channel(1);
        assert_eq!(left, vec![0.1, 0.3]);
        assert_eq!(right, vec![0.2, 0.4]);
    }
}
