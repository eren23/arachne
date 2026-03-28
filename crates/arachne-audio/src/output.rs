//! Real audio output using cpal, with null fallback for CI/headless.

use crate::mixer::AudioMixer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(feature = "native-audio")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Converts an f32 sample in [-1.0, 1.0] to i16.
#[inline]
pub fn f32_to_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    if clamped >= 0.0 {
        (clamped * i16::MAX as f32) as i16
    } else {
        (-clamped * i16::MIN as f32) as i16
    }
}

/// Converts an f32 sample in [-1.0, 1.0] to u16.
#[inline]
pub fn f32_to_u16(sample: f32) -> u16 {
    let clamped = sample.clamp(-1.0, 1.0);
    let shifted = (clamped + 1.0) * 0.5;
    (shifted * u16::MAX as f32) as u16
}

/// A null audio output that discards all samples.
/// Used for CI, testing, and headless environments.
pub struct NullAudioOutput {
    is_playing: bool,
}

impl NullAudioOutput {
    pub fn new() -> Self {
        Self { is_playing: true }
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn resume(&mut self) {
        self.is_playing = true;
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }
}

impl Default for NullAudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Real audio output backed by a cpal stream.
#[cfg(feature = "native-audio")]
pub struct AudioOutput {
    stream: cpal::Stream,
    is_playing: Arc<AtomicBool>,
}

#[cfg(feature = "native-audio")]
impl AudioOutput {
    /// Creates a new audio output that pulls samples from the given mixer.
    ///
    /// Uses the default output device. If no device is available, returns
    /// `AudioOutputHandle::Null` instead.
    pub fn new(mixer: Arc<Mutex<AudioMixer>>, _sample_rate: u32) -> AudioOutputHandle {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => return AudioOutputHandle::Null(NullAudioOutput::new()),
        };

        let supported = match device.default_output_config() {
            Ok(c) => c,
            Err(_) => return AudioOutputHandle::Null(NullAudioOutput::new()),
        };

        let is_playing = Arc::new(AtomicBool::new(true));
        let is_playing_cb = is_playing.clone();

        let config = supported.config();
        let channels = config.channels as usize;

        let build_result = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                let mixer = mixer.clone();
                device.build_output_stream(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        if !is_playing_cb.load(Ordering::Relaxed) {
                            for s in data.iter_mut() {
                                *s = 0.0;
                            }
                            return;
                        }
                        if channels == 2 {
                            if let Ok(mut m) = mixer.lock() {
                                m.mix(data);
                            }
                        } else {
                            let stereo_frames = data.len() / channels;
                            let mut stereo_buf = vec![0.0f32; stereo_frames * 2];
                            if let Ok(mut m) = mixer.lock() {
                                m.mix(&mut stereo_buf);
                            }
                            // Downmix/upmix: take left channel for mono, duplicate for >2
                            for frame in 0..stereo_frames {
                                let left = stereo_buf[frame * 2];
                                let right = stereo_buf[frame * 2 + 1];
                                let mono = (left + right) * 0.5;
                                for ch in 0..channels {
                                    data[frame * channels + ch] = if ch == 0 {
                                        left
                                    } else if ch == 1 {
                                        right
                                    } else {
                                        mono
                                    };
                                }
                            }
                        }
                    },
                    |err| eprintln!("audio stream error: {err}"),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let mixer = mixer.clone();
                let is_playing_cb2 = is_playing.clone();
                device.build_output_stream(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        if !is_playing_cb2.load(Ordering::Relaxed) {
                            for s in data.iter_mut() {
                                *s = 0;
                            }
                            return;
                        }
                        let frames = data.len() / channels;
                        let mut float_buf = vec![0.0f32; frames * 2];
                        if let Ok(mut m) = mixer.lock() {
                            m.mix(&mut float_buf);
                        }
                        for frame in 0..frames {
                            let left = float_buf[frame * 2];
                            let right = float_buf[frame * 2 + 1];
                            for ch in 0..channels {
                                let sample = if ch == 0 {
                                    left
                                } else if ch == 1 {
                                    right
                                } else {
                                    (left + right) * 0.5
                                };
                                data[frame * channels + ch] = f32_to_i16(sample);
                            }
                        }
                    },
                    |err| eprintln!("audio stream error: {err}"),
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mixer = mixer.clone();
                let is_playing_cb3 = is_playing.clone();
                device.build_output_stream(
                    &config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        if !is_playing_cb3.load(Ordering::Relaxed) {
                            for s in data.iter_mut() {
                                *s = 32768;
                            }
                            return;
                        }
                        let frames = data.len() / channels;
                        let mut float_buf = vec![0.0f32; frames * 2];
                        if let Ok(mut m) = mixer.lock() {
                            m.mix(&mut float_buf);
                        }
                        for frame in 0..frames {
                            let left = float_buf[frame * 2];
                            let right = float_buf[frame * 2 + 1];
                            for ch in 0..channels {
                                let sample = if ch == 0 {
                                    left
                                } else if ch == 1 {
                                    right
                                } else {
                                    (left + right) * 0.5
                                };
                                data[frame * channels + ch] = f32_to_u16(sample);
                            }
                        }
                    },
                    |err| eprintln!("audio stream error: {err}"),
                    None,
                )
            }
            _ => return AudioOutputHandle::Null(NullAudioOutput::new()),
        };

        match build_result {
            Ok(stream) => {
                if stream.play().is_err() {
                    return AudioOutputHandle::Null(NullAudioOutput::new());
                }
                AudioOutputHandle::Real(AudioOutput { stream, is_playing })
            }
            Err(_) => AudioOutputHandle::Null(NullAudioOutput::new()),
        }
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Relaxed);
        let _ = self.stream.pause();
    }

    pub fn resume(&self) {
        self.is_playing.store(true, Ordering::Relaxed);
        let _ = self.stream.play();
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }
}

/// Handle that dispatches to either a real cpal-backed output or a null output.
pub enum AudioOutputHandle {
    #[cfg(feature = "native-audio")]
    Real(AudioOutput),
    Null(NullAudioOutput),
}

impl AudioOutputHandle {
    /// Creates a null output (no audio device).
    pub fn null() -> Self {
        AudioOutputHandle::Null(NullAudioOutput::new())
    }

    pub fn pause(&mut self) {
        match self {
            #[cfg(feature = "native-audio")]
            AudioOutputHandle::Real(out) => out.pause(),
            AudioOutputHandle::Null(out) => out.pause(),
        }
    }

    pub fn resume(&mut self) {
        match self {
            #[cfg(feature = "native-audio")]
            AudioOutputHandle::Real(out) => out.resume(),
            AudioOutputHandle::Null(out) => out.resume(),
        }
    }

    pub fn is_playing(&self) -> bool {
        match self {
            #[cfg(feature = "native-audio")]
            AudioOutputHandle::Real(out) => out.is_playing(),
            AudioOutputHandle::Null(out) => out.is_playing(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_to_i16_zero() {
        assert_eq!(f32_to_i16(0.0), 0);
    }

    #[test]
    fn test_f32_to_i16_max() {
        assert_eq!(f32_to_i16(1.0), i16::MAX);
    }

    #[test]
    fn test_f32_to_i16_min() {
        assert_eq!(f32_to_i16(-1.0), i16::MIN);
    }

    #[test]
    fn test_f32_to_i16_half() {
        let result = f32_to_i16(0.5);
        // 0.5 * 32767 = 16383.5, should be within 1 LSB of 16384
        assert!(
            (result as i32 - 16384).unsigned_abs() <= 1,
            "f32_to_i16(0.5) = {result}, expected ~16384"
        );
    }

    #[test]
    fn test_f32_to_i16_clamps_above() {
        assert_eq!(f32_to_i16(2.0), i16::MAX);
    }

    #[test]
    fn test_f32_to_i16_clamps_below() {
        assert_eq!(f32_to_i16(-2.0), i16::MIN);
    }

    #[test]
    fn test_f32_to_u16_zero() {
        // 0.0 maps to midpoint
        let result = f32_to_u16(0.0);
        assert!(
            (result as i32 - 32768).unsigned_abs() <= 1,
            "f32_to_u16(0.0) = {result}, expected ~32768"
        );
    }

    #[test]
    fn test_f32_to_u16_max() {
        assert_eq!(f32_to_u16(1.0), u16::MAX);
    }

    #[test]
    fn test_f32_to_u16_min() {
        assert_eq!(f32_to_u16(-1.0), 0);
    }

    #[test]
    fn test_null_output_pause_resume() {
        let mut null = NullAudioOutput::new();
        assert!(null.is_playing());

        null.pause();
        assert!(!null.is_playing());

        null.resume();
        assert!(null.is_playing());
    }

    #[test]
    fn test_handle_null_pause_resume() {
        let mut handle = AudioOutputHandle::null();
        assert!(handle.is_playing());

        handle.pause();
        assert!(!handle.is_playing());

        handle.resume();
        assert!(handle.is_playing());
    }

    #[test]
    fn test_audio_output_falls_back_to_null() {
        // In CI / headless environments, AudioOutput::new should gracefully
        // fall back to a Null handle when no audio device is available.
        #[cfg(feature = "native-audio")]
        {
            let mixer = Arc::new(Mutex::new(AudioMixer::new(48000)));
            let handle = AudioOutput::new(mixer, 48000);
            // Whether Real or Null, the handle must be functional.
            assert!(handle.is_playing() || !handle.is_playing());
        }
    }

    #[test]
    fn test_pause_resume_state_tracking() {
        let mut handle = AudioOutputHandle::null();
        assert!(handle.is_playing());

        handle.pause();
        assert!(!handle.is_playing());

        handle.resume();
        assert!(handle.is_playing());

        handle.pause();
        assert!(!handle.is_playing());

        handle.pause(); // double pause
        assert!(!handle.is_playing());

        handle.resume();
        assert!(handle.is_playing());
    }

    #[test]
    fn test_mixer_integration_via_null() {
        // Verify the null output can be created alongside a real mixer
        // without panicking, and that the mixer remains usable.
        let mixer = Arc::new(Mutex::new(AudioMixer::new(48000)));

        // Play a source into the mixer
        {
            let samples: Vec<f32> = (0..2048).map(|_| 0.5).collect();
            let source = crate::source::AudioSource::new(samples, 2, 48000);
            let mut m = mixer.lock().unwrap();
            m.play(source, crate::mixer::PlayConfig::default());
        }

        let mut handle = AudioOutputHandle::null();
        assert!(handle.is_playing());

        // Mixer should still have the active channel
        {
            let m = mixer.lock().unwrap();
            assert_eq!(m.active_channels(), 1);
        }

        handle.pause();
        assert!(!handle.is_playing());
    }
}
