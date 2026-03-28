//! Audio decoders for WAV and OGG Vorbis formats.

use crate::source::AudioSource;

// ---------------------------------------------------------------------------
// WAV Decoder
// ---------------------------------------------------------------------------

/// Errors that can occur during WAV decoding.
#[derive(Clone, Debug, PartialEq)]
pub enum WavError {
    TooShort,
    InvalidRiff,
    InvalidWave,
    MissingFmt,
    MissingData,
    UnsupportedFormat(u16),
    UnsupportedBitsPerSample(u16),
}

/// Reads a little-endian u16 from a byte slice at offset.
#[inline]
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Reads a little-endian u32 from a byte slice at offset.
#[inline]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// Decodes a WAV file from raw bytes into an `AudioSource`.
///
/// Supports PCM 16-bit integer WAV files, mono or stereo.
/// Samples are converted to f32 in the range [-1.0, 1.0].
pub fn decode_wav(data: &[u8]) -> Result<AudioSource, WavError> {
    if data.len() < 44 {
        return Err(WavError::TooShort);
    }

    // Check RIFF header
    if &data[0..4] != b"RIFF" {
        return Err(WavError::InvalidRiff);
    }

    // Check WAVE format
    if &data[8..12] != b"WAVE" {
        return Err(WavError::InvalidWave);
    }

    // Find chunks by scanning
    let mut pos = 12;
    let mut fmt_offset = None;
    let mut data_offset = None;
    let mut data_size = 0u32;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = read_u32_le(data, pos + 4);

        if chunk_id == b"fmt " {
            fmt_offset = Some(pos + 8);
        } else if chunk_id == b"data" {
            data_offset = Some(pos + 8);
            data_size = chunk_size;
        }

        pos += 8 + chunk_size as usize;
        // Align to 2-byte boundary
        if pos % 2 != 0 {
            pos += 1;
        }
    }

    let fmt_off = fmt_offset.ok_or(WavError::MissingFmt)?;
    let data_off = data_offset.ok_or(WavError::MissingData)?;

    // Parse fmt chunk
    let audio_format = read_u16_le(data, fmt_off);
    if audio_format != 1 {
        // 1 = PCM
        return Err(WavError::UnsupportedFormat(audio_format));
    }

    let channels = read_u16_le(data, fmt_off + 2);
    let sample_rate = read_u32_le(data, fmt_off + 4);
    // bytes 8..11: byte rate (skip)
    // bytes 12..13: block align (skip)
    let bits_per_sample = read_u16_le(data, fmt_off + 14);

    if bits_per_sample != 16 {
        return Err(WavError::UnsupportedBitsPerSample(bits_per_sample));
    }

    assert!(
        channels == 1 || channels == 2,
        "WAV channels must be 1 or 2, got {channels}"
    );

    // Clamp data_size to available bytes
    let available = data.len().saturating_sub(data_off);
    let actual_data_size = (data_size as usize).min(available);

    // Convert 16-bit PCM to f32
    let sample_count = actual_data_size / 2;
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let byte_offset = data_off + i * 2;
        if byte_offset + 1 >= data.len() {
            break;
        }
        let raw = i16::from_le_bytes([data[byte_offset], data[byte_offset + 1]]);
        samples.push(raw as f32 / 32768.0);
    }

    Ok(AudioSource::new(samples, channels as u32, sample_rate))
}

/// Builds a valid WAV file from raw parameters (for testing).
pub fn build_test_wav(channels: u16, sample_rate: u32, samples_i16: &[i16]) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * block_align as u32;
    let data_size = (samples_i16.len() * 2) as u32;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(file_size as usize + 8);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples_i16 {
        wav.extend_from_slice(&s.to_le_bytes());
    }

    wav
}

// ---------------------------------------------------------------------------
// Streaming WAV Decoder
// ---------------------------------------------------------------------------

/// A streaming WAV decoder that reads frames on demand from in-memory bytes.
pub struct WavStream {
    channels: u32,
    sample_rate: u32,
    data_offset: usize,
    data_end: usize,
    current_pos: usize,
    raw_data: Vec<u8>,
}

impl WavStream {
    /// Creates a streaming WAV decoder from raw bytes.
    pub fn new(data: Vec<u8>) -> Result<Self, WavError> {
        if data.len() < 44 {
            return Err(WavError::TooShort);
        }
        if &data[0..4] != b"RIFF" {
            return Err(WavError::InvalidRiff);
        }
        if &data[8..12] != b"WAVE" {
            return Err(WavError::InvalidWave);
        }

        let mut pos = 12;
        let mut fmt_offset = None;
        let mut data_offset = None;
        let mut data_size = 0u32;

        while pos + 8 <= data.len() {
            let chunk_id = &data[pos..pos + 4];
            let chunk_size = read_u32_le(&data, pos + 4);
            if chunk_id == b"fmt " {
                fmt_offset = Some(pos + 8);
            } else if chunk_id == b"data" {
                data_offset = Some(pos + 8);
                data_size = chunk_size;
            }
            pos += 8 + chunk_size as usize;
            if pos % 2 != 0 {
                pos += 1;
            }
        }

        let fmt_off = fmt_offset.ok_or(WavError::MissingFmt)?;
        let data_off = data_offset.ok_or(WavError::MissingData)?;

        let audio_format = read_u16_le(&data, fmt_off);
        if audio_format != 1 {
            return Err(WavError::UnsupportedFormat(audio_format));
        }

        let channels = read_u16_le(&data, fmt_off + 2) as u32;
        let sample_rate = read_u32_le(&data, fmt_off + 4);
        let bits_per_sample = read_u16_le(&data, fmt_off + 14);
        if bits_per_sample != 16 {
            return Err(WavError::UnsupportedBitsPerSample(bits_per_sample));
        }

        let available = data.len().saturating_sub(data_off);
        let actual_data_size = (data_size as usize).min(available);

        Ok(Self {
            channels,
            sample_rate,
            data_offset: data_off,
            data_end: data_off + actual_data_size,
            current_pos: data_off,
            raw_data: data,
        })
    }
}

impl crate::source::StreamingSource for WavStream {
    fn read_frames(&mut self, buffer: &mut [f32]) -> usize {
        let channels = self.channels as usize;
        let frames_requested = buffer.len() / channels;
        let bytes_per_frame = channels * 2; // 16-bit = 2 bytes per sample
        let bytes_remaining = self.data_end.saturating_sub(self.current_pos);
        let frames_available = bytes_remaining / bytes_per_frame;
        let frames_to_read = frames_requested.min(frames_available);

        for i in 0..frames_to_read {
            for c in 0..channels {
                let byte_off = self.current_pos + (i * channels + c) * 2;
                let raw = i16::from_le_bytes([
                    self.raw_data[byte_off],
                    self.raw_data[byte_off + 1],
                ]);
                buffer[i * channels + c] = raw as f32 / 32768.0;
            }
        }
        self.current_pos += frames_to_read * bytes_per_frame;
        frames_to_read
    }

    fn channels(&self) -> u32 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn reset(&mut self) {
        self.current_pos = self.data_offset;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::StreamingSource;

    const EPSILON: f32 = 1e-6;

    fn assert_f32_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < EPSILON,
            "assertion failed: {a} != {b} (within epsilon {EPSILON})"
        );
    }

    #[test]
    fn test_decode_wav_mono() {
        // Create a mono 16-bit PCM WAV with known samples
        let samples_i16: Vec<i16> = (0..200).map(|i| (i * 100) as i16).collect();
        let wav_data = build_test_wav(1, 44100, &samples_i16);

        let source = decode_wav(&wav_data).unwrap();
        assert_eq!(source.channels, 1);
        assert_eq!(source.sample_rate, 44100);
        assert_eq!(source.samples.len(), 200);

        // Verify first 100 samples match reference within 1e-6
        for i in 0..100 {
            let expected = (i as i16 * 100) as f32 / 32768.0;
            assert_f32_approx(source.samples[i], expected);
        }
    }

    #[test]
    fn test_decode_wav_stereo() {
        let mut samples_i16 = Vec::new();
        for i in 0..100 {
            samples_i16.push((i * 50) as i16); // left
            samples_i16.push((-i * 50) as i16); // right
        }
        let wav_data = build_test_wav(2, 48000, &samples_i16);

        let source = decode_wav(&wav_data).unwrap();
        assert_eq!(source.channels, 2);
        assert_eq!(source.sample_rate, 48000);
        assert_eq!(source.frame_count(), 100);

        // Verify first 100 samples (50 stereo frames)
        for i in 0..50 {
            let expected_l = (i as i16 * 50) as f32 / 32768.0;
            let expected_r = (-(i as i16) * 50) as f32 / 32768.0;
            assert_f32_approx(source.samples[i * 2], expected_l);
            assert_f32_approx(source.samples[i * 2 + 1], expected_r);
        }
    }

    #[test]
    fn test_decode_wav_sample_accuracy() {
        // Exhaustive test: verify ALL samples match within 1e-6
        let samples_i16: Vec<i16> = (-500..500).collect();
        let wav_data = build_test_wav(1, 44100, &samples_i16);

        let source = decode_wav(&wav_data).unwrap();
        assert_eq!(source.samples.len(), 1000);

        for (i, &raw) in samples_i16.iter().enumerate() {
            let expected = raw as f32 / 32768.0;
            assert_f32_approx(source.samples[i], expected);
        }
    }

    #[test]
    fn test_decode_wav_extreme_values() {
        let samples_i16 = vec![i16::MIN, i16::MAX, 0, 1, -1];
        let wav_data = build_test_wav(1, 44100, &samples_i16);
        let source = decode_wav(&wav_data).unwrap();

        assert_f32_approx(source.samples[0], -1.0); // i16::MIN / 32768
        assert_f32_approx(source.samples[1], 32767.0 / 32768.0); // i16::MAX / 32768
        assert_f32_approx(source.samples[2], 0.0);
        assert_f32_approx(source.samples[3], 1.0 / 32768.0);
        assert_f32_approx(source.samples[4], -1.0 / 32768.0);
    }

    #[test]
    fn test_decode_wav_invalid_riff() {
        let mut data = vec![0u8; 44];
        data[0..4].copy_from_slice(b"XXXX");
        data[4..8].copy_from_slice(&36u32.to_le_bytes());
        data[8..12].copy_from_slice(b"WAVE");
        assert_eq!(decode_wav(&data).unwrap_err(), WavError::InvalidRiff);
    }

    #[test]
    fn test_decode_wav_too_short() {
        assert_eq!(decode_wav(b"RIFF").unwrap_err(), WavError::TooShort);
    }

    #[test]
    fn test_decode_wav_invalid_wave() {
        let mut data = vec![0u8; 44];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"XXXX");
        assert_eq!(decode_wav(&data).unwrap_err(), WavError::InvalidWave);
    }

    #[test]
    fn test_wav_stream() {
        let samples_i16: Vec<i16> = (0..100).map(|i| (i * 200) as i16).collect();
        let wav_data = build_test_wav(1, 44100, &samples_i16);

        let mut stream = WavStream::new(wav_data).unwrap();
        assert_eq!(stream.channels(), 1);
        assert_eq!(stream.sample_rate(), 44100);

        // Read first 50 frames
        let mut buf = vec![0.0f32; 50];
        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 50);

        for i in 0..50 {
            let expected = (i as i16 * 200) as f32 / 32768.0;
            assert_f32_approx(buf[i], expected);
        }

        // Read remaining
        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 50);

        // Reset and re-read
        stream.reset();
        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 50);
        assert_f32_approx(buf[0], 0.0);
    }

    #[test]
    fn test_roundtrip_build_decode() {
        // Build a WAV, decode it, verify it matches
        let original: Vec<i16> = (0..256).map(|i| (i as f32 / 256.0 * 32767.0) as i16).collect();
        let wav = build_test_wav(2, 48000, &original);
        let decoded = decode_wav(&wav).unwrap();

        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.sample_rate, 48000);
        assert_eq!(decoded.samples.len(), original.len());

        for (i, &raw) in original.iter().enumerate() {
            let expected = raw as f32 / 32768.0;
            assert_f32_approx(decoded.samples[i], expected);
        }
    }
}
