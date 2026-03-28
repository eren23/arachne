//! PCM audio source buffers and streaming support.

/// A PCM f32 audio source buffer with interleaved samples.
#[derive(Clone, Debug)]
pub struct AudioSource {
    /// Interleaved PCM samples (L, R, L, R, ... for stereo).
    pub samples: Vec<f32>,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Sample rate in Hz (e.g. 44100, 48000).
    pub sample_rate: u32,
}

impl AudioSource {
    /// Creates a new audio source from interleaved PCM data.
    ///
    /// # Panics
    /// Panics if `channels` is not 1 or 2, or if sample count is not
    /// divisible by `channels`.
    pub fn new(samples: Vec<f32>, channels: u32, sample_rate: u32) -> Self {
        assert!(
            channels == 1 || channels == 2,
            "channels must be 1 or 2, got {channels}"
        );
        assert!(
            samples.len() % channels as usize == 0,
            "sample count {} not divisible by channels {channels}",
            samples.len()
        );
        Self {
            samples,
            channels,
            sample_rate,
        }
    }

    /// Returns the number of sample frames (samples per channel).
    #[inline]
    pub fn frame_count(&self) -> usize {
        if self.channels == 0 {
            return 0;
        }
        self.samples.len() / self.channels as usize
    }

    /// Returns the duration in seconds.
    #[inline]
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frame_count() as f32 / self.sample_rate as f32
    }

    /// Reads a stereo pair at the given frame index.
    /// Mono sources return the same value for both channels.
    #[inline]
    pub fn read_frame(&self, frame: usize) -> (f32, f32) {
        if self.channels == 2 {
            let idx = frame * 2;
            if idx + 1 < self.samples.len() {
                (self.samples[idx], self.samples[idx + 1])
            } else {
                (0.0, 0.0)
            }
        } else {
            let val = if frame < self.samples.len() {
                self.samples[frame]
            } else {
                0.0
            };
            (val, val)
        }
    }

    /// Converts a mono source to stereo (duplicating the channel).
    /// If already stereo, returns a clone.
    pub fn to_stereo(&self) -> AudioSource {
        if self.channels == 2 {
            return self.clone();
        }
        let mut stereo = Vec::with_capacity(self.samples.len() * 2);
        for &s in &self.samples {
            stereo.push(s);
            stereo.push(s);
        }
        AudioSource {
            samples: stereo,
            channels: 2,
            sample_rate: self.sample_rate,
        }
    }
}

/// A streaming audio decoder that reads chunks on demand.
pub trait StreamingSource {
    /// Reads the next chunk of interleaved stereo samples into `buffer`.
    /// Returns the number of frames actually read (may be less at end of stream).
    fn read_frames(&mut self, buffer: &mut [f32]) -> usize;

    /// Returns the number of channels.
    fn channels(&self) -> u32;

    /// Returns the sample rate.
    fn sample_rate(&self) -> u32;

    /// Resets to the beginning of the stream.
    fn reset(&mut self);
}

/// A streaming wrapper around an in-memory AudioSource.
pub struct MemoryStream {
    source: AudioSource,
    position: usize,
}

impl MemoryStream {
    pub fn new(source: AudioSource) -> Self {
        Self {
            source,
            position: 0,
        }
    }
}

impl StreamingSource for MemoryStream {
    fn read_frames(&mut self, buffer: &mut [f32]) -> usize {
        let channels = self.source.channels as usize;
        let remaining_samples = self.source.samples.len() - self.position * channels;
        let frames_available = remaining_samples / channels;
        let frames_requested = buffer.len() / channels;
        let frames_to_read = frames_available.min(frames_requested);

        for i in 0..frames_to_read {
            let src_idx = (self.position + i) * channels;
            let dst_idx = i * channels;
            for c in 0..channels {
                buffer[dst_idx + c] = self.source.samples[src_idx + c];
            }
        }
        self.position += frames_to_read;
        frames_to_read
    }

    fn channels(&self) -> u32 {
        self.source.channels
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate
    }

    fn reset(&mut self) {
        self.position = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_stereo() {
        let src = AudioSource::new(vec![0.1, 0.2, 0.3, 0.4], 2, 44100);
        assert_eq!(src.frame_count(), 2);
        assert_eq!(src.channels, 2);
    }

    #[test]
    fn test_new_mono() {
        let src = AudioSource::new(vec![0.1, 0.2, 0.3], 1, 48000);
        assert_eq!(src.frame_count(), 3);
    }

    #[test]
    #[should_panic(expected = "channels must be 1 or 2")]
    fn test_invalid_channels() {
        AudioSource::new(vec![0.0; 9], 3, 44100);
    }

    #[test]
    #[should_panic(expected = "not divisible by channels")]
    fn test_misaligned_samples() {
        AudioSource::new(vec![0.0; 3], 2, 44100);
    }

    #[test]
    fn test_duration() {
        let src = AudioSource::new(vec![0.0; 48000], 1, 48000);
        let d = src.duration_secs();
        assert!((d - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_read_frame_stereo() {
        let src = AudioSource::new(vec![0.1, 0.2, 0.3, 0.4], 2, 44100);
        assert_eq!(src.read_frame(0), (0.1, 0.2));
        assert_eq!(src.read_frame(1), (0.3, 0.4));
    }

    #[test]
    fn test_read_frame_mono() {
        let src = AudioSource::new(vec![0.5, 0.7], 1, 44100);
        assert_eq!(src.read_frame(0), (0.5, 0.5));
        assert_eq!(src.read_frame(1), (0.7, 0.7));
    }

    #[test]
    fn test_to_stereo() {
        let mono = AudioSource::new(vec![0.1, 0.2, 0.3], 1, 44100);
        let stereo = mono.to_stereo();
        assert_eq!(stereo.channels, 2);
        assert_eq!(stereo.frame_count(), 3);
        assert_eq!(stereo.samples, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
    }

    #[test]
    fn test_memory_stream() {
        let src = AudioSource::new(vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 2, 44100);
        let mut stream = MemoryStream::new(src);
        let mut buf = [0.0f32; 4]; // 2 frames
        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 2);
        assert_eq!(buf, [0.1, 0.2, 0.3, 0.4]);

        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 1);
        assert_eq!(buf[0], 0.5);
        assert_eq!(buf[1], 0.6);

        stream.reset();
        let n = stream.read_frames(&mut buf);
        assert_eq!(n, 2);
        assert_eq!(buf, [0.1, 0.2, 0.3, 0.4]);
    }
}
