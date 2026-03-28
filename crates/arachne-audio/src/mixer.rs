//! Audio mixer with multi-channel support, volume, panning, and fading.

use crate::source::AudioSource;

/// Maximum number of simultaneous audio channels.
pub const MAX_CHANNELS: usize = 32;

/// Default output buffer size in frames.
pub const DEFAULT_BUFFER_FRAMES: usize = 1024;

/// An opaque handle to a playing channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelHandle(pub u32);

/// Playback state of a channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

/// Configuration for playing a source.
#[derive(Clone, Copy, Debug)]
pub struct PlayConfig {
    /// Volume multiplier [0.0, 1.0+].
    pub volume: f32,
    /// Stereo pan [-1.0 (left) .. 1.0 (right)].
    pub pan: f32,
    /// Whether the source should loop.
    pub looping: bool,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pan: 0.0,
            looping: false,
        }
    }
}

/// A fade operation (linear ramp).
#[derive(Clone, Copy, Debug)]
struct Fade {
    /// Starting volume.
    from: f32,
    /// Target volume.
    to: f32,
    /// Total fade duration in samples.
    total_samples: usize,
    /// Current sample within the fade.
    current_sample: usize,
}

impl Fade {
    fn new(from: f32, to: f32, duration_samples: usize) -> Self {
        Self {
            from,
            to,
            total_samples: duration_samples.max(1),
            current_sample: 0,
        }
    }

    /// Returns the current fade multiplier and advances by one sample.
    #[inline]
    fn next(&mut self) -> f32 {
        if self.current_sample >= self.total_samples {
            return self.to;
        }
        let t = self.current_sample as f32 / self.total_samples as f32;
        self.current_sample += 1;
        self.from + (self.to - self.from) * t
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.current_sample >= self.total_samples
    }
}

/// Internal state for a single mixer channel.
struct Channel {
    source: AudioSource,
    handle: ChannelHandle,
    state: PlayState,
    volume: f32,
    pan: f32,
    looping: bool,
    /// Current playback position in frames.
    position: usize,
    /// Optional active fade.
    fade: Option<Fade>,
}

/// Multi-channel audio mixer.
///
/// Mixes up to [`MAX_CHANNELS`] simultaneous audio sources into a stereo
/// output buffer.
pub struct AudioMixer {
    channels: Vec<Channel>,
    master_volume: f32,
    next_handle: u32,
}

impl AudioMixer {
    /// Creates a new mixer.
    pub fn new(_sample_rate: u32) -> Self {
        Self {
            channels: Vec::new(),
            master_volume: 1.0,
            next_handle: 1,
        }
    }

    /// Sets the master volume.
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume;
    }

    /// Returns the master volume.
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Returns the number of active (playing or paused) channels.
    pub fn active_channels(&self) -> usize {
        self.channels.len()
    }

    /// Plays an audio source with the given configuration.
    /// Returns a handle to control the channel, or `None` if all channels are full.
    pub fn play(&mut self, source: AudioSource, config: PlayConfig) -> Option<ChannelHandle> {
        // Remove stopped channels to make room
        self.channels.retain(|ch| ch.state != PlayState::Stopped);

        if self.channels.len() >= MAX_CHANNELS {
            return None;
        }

        let handle = ChannelHandle(self.next_handle);
        self.next_handle += 1;

        self.channels.push(Channel {
            source,
            handle,
            state: PlayState::Playing,
            volume: config.volume,
            pan: config.pan,
            looping: config.looping,
            position: 0,
            fade: None,
        });

        Some(handle)
    }

    /// Stops a channel immediately.
    pub fn stop(&mut self, handle: ChannelHandle) {
        if let Some(ch) = self.find_channel_mut(handle) {
            ch.state = PlayState::Stopped;
        }
    }

    /// Pauses a channel.
    pub fn pause(&mut self, handle: ChannelHandle) {
        if let Some(ch) = self.find_channel_mut(handle) {
            if ch.state == PlayState::Playing {
                ch.state = PlayState::Paused;
            }
        }
    }

    /// Resumes a paused channel.
    pub fn resume(&mut self, handle: ChannelHandle) {
        if let Some(ch) = self.find_channel_mut(handle) {
            if ch.state == PlayState::Paused {
                ch.state = PlayState::Playing;
            }
        }
    }

    /// Sets the volume for a specific channel.
    pub fn set_volume(&mut self, handle: ChannelHandle, volume: f32) {
        if let Some(ch) = self.find_channel_mut(handle) {
            ch.volume = volume;
        }
    }

    /// Sets the pan for a specific channel.
    pub fn set_pan(&mut self, handle: ChannelHandle, pan: f32) {
        if let Some(ch) = self.find_channel_mut(handle) {
            ch.pan = pan.clamp(-1.0, 1.0);
        }
    }

    /// Starts a fade in on a channel (from 0 to current volume).
    pub fn fade_in(&mut self, handle: ChannelHandle, duration_secs: f32) {
        if let Some(ch) = self.find_channel_mut(handle) {
            let samples = (duration_secs * ch.source.sample_rate as f32) as usize;
            ch.fade = Some(Fade::new(0.0, ch.volume, samples));
        }
    }

    /// Starts a fade out on a channel (from current volume to 0).
    pub fn fade_out(&mut self, handle: ChannelHandle, duration_secs: f32) {
        if let Some(ch) = self.find_channel_mut(handle) {
            let samples = (duration_secs * ch.source.sample_rate as f32) as usize;
            ch.fade = Some(Fade::new(ch.volume, 0.0, samples));
        }
    }

    /// Returns the state of a channel.
    pub fn state(&self, handle: ChannelHandle) -> Option<PlayState> {
        self.channels
            .iter()
            .find(|ch| ch.handle == handle)
            .map(|ch| ch.state)
    }

    /// Mixes all active channels into the output buffer (interleaved stereo).
    ///
    /// The output buffer is zeroed first, then each playing channel contributes
    /// its samples with volume and panning applied.
    pub fn mix(&mut self, output: &mut [f32]) {
        // Zero the output buffer
        for s in output.iter_mut() {
            *s = 0.0;
        }

        let frame_count = output.len() / 2;

        for ch in &mut self.channels {
            if ch.state != PlayState::Playing {
                continue;
            }

            let src = &ch.source;
            let src_frames = src.frame_count();

            for frame in 0..frame_count {
                if ch.position >= src_frames {
                    if ch.looping {
                        ch.position = 0;
                    } else {
                        ch.state = PlayState::Stopped;
                        break;
                    }
                }

                let (mut left, mut right) = src.read_frame(ch.position);
                ch.position += 1;

                // Apply fade if active
                let vol = if let Some(ref mut fade) = ch.fade {
                    let fade_vol = fade.next();
                    if fade.is_done() {
                        // If faded to zero, stop
                        if fade.to == 0.0 {
                            ch.state = PlayState::Stopped;
                        } else {
                            ch.volume = fade.to;
                        }
                    }
                    fade_vol
                } else {
                    ch.volume
                };

                // Apply volume
                left *= vol;
                right *= vol;

                // Apply panning using constant-power-like equal-gain law
                // pan: -1 = full left, 0 = center, 1 = full right
                let pan = ch.pan.clamp(-1.0, 1.0);
                let left_gain = ((1.0 - pan) * 0.5).sqrt();
                let right_gain = ((1.0 + pan) * 0.5).sqrt();

                let out_idx = frame * 2;
                output[out_idx] += left * left_gain * self.master_volume;
                output[out_idx + 1] += right * right_gain * self.master_volume;
            }
        }

        // Clean up finished fades
        for ch in &mut self.channels {
            if let Some(ref fade) = ch.fade {
                if fade.is_done() {
                    ch.fade = None;
                }
            }
        }

        // Remove stopped channels
        self.channels.retain(|ch| ch.state != PlayState::Stopped);
    }

    fn find_channel_mut(&mut self, handle: ChannelHandle) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|ch| ch.handle == handle)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::AudioSource;

    const EPSILON: f32 = 1e-5;

    fn assert_f32_approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < EPSILON,
            "assertion failed: {a} != {b} (within epsilon {EPSILON})"
        );
    }

    /// Creates a test source with constant value across all samples.
    fn const_source(value: f32, frames: usize) -> AudioSource {
        let samples: Vec<f32> = (0..frames * 2).map(|_| value).collect();
        AudioSource::new(samples, 2, 48000)
    }

    #[test]
    fn test_mix_two_sources_half_volume() {
        let mut mixer = AudioMixer::new(48000);

        // Source A: all samples = 0.6
        let a = const_source(0.6, 1024);
        // Source B: all samples = 0.4
        let b = const_source(0.4, 1024);

        mixer.play(a, PlayConfig { volume: 0.5, pan: 0.0, looping: false });
        mixer.play(b, PlayConfig { volume: 0.5, pan: 0.0, looping: false });

        let mut output = vec![0.0f32; 1024 * 2];
        mixer.mix(&mut output);

        // At center pan with constant-power panning:
        // left_gain = sqrt(0.5), right_gain = sqrt(0.5)
        // For each source at vol 0.5, master 1.0:
        // contribution = value * 0.5 * sqrt(0.5) * 1.0
        // Two sources: 0.6*0.5*sqrt(0.5) + 0.4*0.5*sqrt(0.5) = (0.3+0.2)*sqrt(0.5)
        let sqrt_half = (0.5f32).sqrt();
        let expected = (0.3 + 0.2) * sqrt_half;

        for i in 0..100 {
            assert_f32_approx(output[i * 2], expected);
            assert_f32_approx(output[i * 2 + 1], expected);
        }
    }

    #[test]
    fn test_panning_hard_left() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 512);
        mixer.play(src, PlayConfig { volume: 1.0, pan: -1.0, looping: false });

        let mut output = vec![0.0f32; 512 * 2];
        mixer.mix(&mut output);

        // Hard left: left_gain = sqrt(1.0) = 1.0, right_gain = sqrt(0.0) = 0.0
        for i in 0..512 {
            assert_f32_approx(output[i * 2], 1.0);     // left: full
            assert_f32_approx(output[i * 2 + 1], 0.0); // right: silent
        }
    }

    #[test]
    fn test_panning_hard_right() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 512);
        mixer.play(src, PlayConfig { volume: 1.0, pan: 1.0, looping: false });

        let mut output = vec![0.0f32; 512 * 2];
        mixer.mix(&mut output);

        for i in 0..512 {
            assert_f32_approx(output[i * 2], 0.0);     // left: silent
            assert_f32_approx(output[i * 2 + 1], 1.0); // right: full
        }
    }

    #[test]
    fn test_panning_center_equal() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 512);
        mixer.play(src, PlayConfig { volume: 1.0, pan: 0.0, looping: false });

        let mut output = vec![0.0f32; 512 * 2];
        mixer.mix(&mut output);

        // Center pan: both channels should be equal
        let sqrt_half = (0.5f32).sqrt();
        for i in 0..512 {
            assert_f32_approx(output[i * 2], sqrt_half);
            assert_f32_approx(output[i * 2 + 1], sqrt_half);
        }
    }

    #[test]
    fn test_looping() {
        let mut mixer = AudioMixer::new(48000);
        // 10-frame source
        let src = const_source(0.5, 10);
        let handle = mixer.play(src, PlayConfig { volume: 1.0, pan: 0.0, looping: true }).unwrap();

        // Mix 25 frames -> should loop around
        let mut output = vec![0.0f32; 25 * 2];
        mixer.mix(&mut output);

        // Channel should still be playing (looped)
        assert_eq!(mixer.state(handle), Some(PlayState::Playing));
    }

    #[test]
    fn test_stop_at_end() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(0.5, 10);
        let handle = mixer.play(src, PlayConfig { volume: 1.0, pan: 0.0, looping: false }).unwrap();

        let mut output = vec![0.0f32; 20 * 2];
        mixer.mix(&mut output);

        // Should be removed (stopped)
        assert_eq!(mixer.state(handle), None);
    }

    #[test]
    fn test_pause_resume() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 1024);
        let handle = mixer.play(src, PlayConfig::default()).unwrap();

        assert_eq!(mixer.state(handle), Some(PlayState::Playing));
        mixer.pause(handle);
        assert_eq!(mixer.state(handle), Some(PlayState::Paused));

        // Mix while paused — paused channels should not contribute
        let mut output = vec![0.0f32; 10 * 2];
        mixer.mix(&mut output);
        for &s in &output {
            assert_f32_approx(s, 0.0);
        }

        mixer.resume(handle);
        assert_eq!(mixer.state(handle), Some(PlayState::Playing));
    }

    #[test]
    fn test_stop_channel() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 1024);
        let handle = mixer.play(src, PlayConfig::default()).unwrap();
        mixer.stop(handle);

        let mut output = vec![0.0f32; 10 * 2];
        mixer.mix(&mut output);
        // Stopped channels removed, output is silence
        for &s in &output {
            assert_f32_approx(s, 0.0);
        }
    }

    #[test]
    fn test_master_volume() {
        let mut mixer = AudioMixer::new(48000);
        mixer.set_master_volume(0.5);
        let src = const_source(1.0, 512);
        mixer.play(src, PlayConfig { volume: 1.0, pan: -1.0, looping: false });

        let mut output = vec![0.0f32; 512 * 2];
        mixer.mix(&mut output);

        // Hard left, volume 1.0, master 0.5
        for i in 0..512 {
            assert_f32_approx(output[i * 2], 0.5);
        }
    }

    #[test]
    fn test_max_channels() {
        let mut mixer = AudioMixer::new(48000);
        for _ in 0..MAX_CHANNELS {
            let src = const_source(0.01, 100);
            assert!(mixer.play(src, PlayConfig::default()).is_some());
        }
        // 33rd channel should fail
        let src = const_source(0.01, 100);
        assert!(mixer.play(src, PlayConfig::default()).is_none());
    }

    #[test]
    fn test_fade_in() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 48000);
        let handle = mixer.play(src, PlayConfig { volume: 1.0, pan: -1.0, looping: false }).unwrap();

        // Fade in over 100 samples
        let fade_secs = 100.0 / 48000.0;
        mixer.fade_in(handle, fade_secs);

        let mut output = vec![0.0f32; 200 * 2];
        mixer.mix(&mut output);

        // First sample should be near 0 (fade from 0)
        assert!(output[0].abs() < 0.02);
        // Sample 50 should be about halfway
        let mid = output[50 * 2];
        assert!(mid > 0.3 && mid < 0.7, "mid fade value: {mid}");
        // After fade completes (~sample 100), should be at full volume
        assert!(output[150 * 2] > 0.9);
    }

    #[test]
    fn test_fade_out() {
        let mut mixer = AudioMixer::new(48000);
        let src = const_source(1.0, 48000);
        let handle = mixer.play(src, PlayConfig { volume: 1.0, pan: -1.0, looping: false }).unwrap();

        let fade_secs = 100.0 / 48000.0;
        mixer.fade_out(handle, fade_secs);

        let mut output = vec![0.0f32; 200 * 2];
        mixer.mix(&mut output);

        // First sample should be near full
        assert!(output[0] > 0.9);
        // After fade, channel stops (faded to 0)
        // The channel should be gone
        assert_eq!(mixer.state(handle), None);
    }

    #[test]
    fn bench_mix_32_channels_1024_frames() {
        use std::hint::black_box;

        let mut mixer = AudioMixer::new(48000);
        for _ in 0..32 {
            let src = const_source(0.03, 2048);
            mixer.play(src, PlayConfig {
                volume: 0.5,
                pan: 0.0,
                looping: true,
            });
        }

        let mut output = vec![0.0f32; 1024 * 2];

        let start = std::time::Instant::now();
        let iterations = 1000;
        for _ in 0..iterations {
            mixer.mix(black_box(&mut output));
        }
        let elapsed = start.elapsed();
        let per_mix = elapsed / iterations;

        eprintln!(
            "Mix 32ch x 1024 frames: {:?} per call ({} iterations in {:?})",
            per_mix, iterations, elapsed
        );

        // Must be < 1ms per call
        assert!(
            per_mix.as_micros() < 1000,
            "Mix took {:?}, exceeding 1ms target",
            per_mix
        );
    }
}
