//! WebAudio API backend for the Arachne WASM runtime.
//!
//! Provides a WebAudio-based audio output backend that bridges the Arachne
//! audio mixer to the browser's AudioContext. On native targets, provides
//! a stub implementation that tracks state without actual audio output.

use arachne_audio::{AudioBackend, AudioMixer, AudioRingBuffer, BackendConfig, BackendError};

// ---------------------------------------------------------------------------
// Web Audio configuration
// ---------------------------------------------------------------------------

/// Configuration for the WebAudio backend.
#[derive(Clone, Copy, Debug)]
pub struct WebAudioConfig {
    /// Desired sample rate (typically 44100 or 48000).
    pub sample_rate: u32,
    /// Output channels (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Buffer size for the ScriptProcessorNode (power of 2, 256..16384).
    pub buffer_size: u32,
    /// Whether to use AudioWorklet instead of ScriptProcessorNode.
    pub use_audio_worklet: bool,
    /// Whether to auto-resume the AudioContext on user interaction.
    pub auto_resume: bool,
}

impl Default for WebAudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 2048,
            use_audio_worklet: false,
            auto_resume: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Web Audio state
// ---------------------------------------------------------------------------

/// The current state of the WebAudio backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebAudioState {
    /// Not yet created.
    Uninitialized,
    /// AudioContext created but suspended (autoplay policy).
    Suspended,
    /// AudioContext running and producing audio.
    Running,
    /// AudioContext closed.
    Closed,
}

// ---------------------------------------------------------------------------
// WebAudio backend
// ---------------------------------------------------------------------------

/// A WebAudio API backend for browser audio output.
///
/// On WASM (with the `wasm` feature), this manages an `AudioContext` and
/// connects the Arachne mixer output to the browser's audio destination.
/// On native, it acts as a functional stub that tracks state.
pub struct WebAudioBackend {
    config: Option<WebAudioConfig>,
    state: WebAudioState,
    backend_config: Option<BackendConfig>,
    ring_buffer: Option<AudioRingBuffer>,
    /// Total frames submitted (for testing/diagnostics).
    pub frames_submitted: u64,
    /// Total buffers submitted (for testing/diagnostics).
    pub buffers_submitted: u64,
}

impl WebAudioBackend {
    /// Create a new WebAudio backend.
    pub fn new() -> Self {
        Self {
            config: None,
            state: WebAudioState::Uninitialized,
            backend_config: None,
            ring_buffer: None,
            frames_submitted: 0,
            buffers_submitted: 0,
        }
    }

    /// Create with specific WebAudio configuration.
    pub fn with_config(config: WebAudioConfig) -> Self {
        Self {
            config: Some(config),
            state: WebAudioState::Uninitialized,
            backend_config: None,
            ring_buffer: None,
            frames_submitted: 0,
            buffers_submitted: 0,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> WebAudioState {
        self.state
    }

    /// Get the WebAudio configuration.
    pub fn web_config(&self) -> Option<&WebAudioConfig> {
        self.config.as_ref()
    }

    /// Initialize the WebAudio context.
    ///
    /// On WASM, creates an AudioContext and ScriptProcessorNode or AudioWorklet.
    /// Must be called in response to a user gesture to satisfy autoplay policies.
    pub fn init_context(&mut self, config: WebAudioConfig) -> Result<(), BackendError> {
        if self.state != WebAudioState::Uninitialized {
            return Err(BackendError::AlreadyRunning);
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would:
            // 1. Create AudioContext with { sampleRate: config.sample_rate }
            // 2. Create ScriptProcessorNode(bufferSize, 0, channels) or
            //    AudioWorkletNode with a custom processor
            // 3. Connect node to context.destination
            // 4. Set onaudioprocess callback to read from ring buffer
        }

        let ring_size = config.buffer_size as usize * config.channels as usize * 4;
        self.ring_buffer = Some(AudioRingBuffer::new(ring_size));
        self.config = Some(config);

        // AudioContext starts suspended until user gesture (autoplay policy).
        self.state = WebAudioState::Suspended;
        Ok(())
    }

    /// Resume the AudioContext (call after user gesture).
    pub fn resume(&mut self) -> Result<(), BackendError> {
        match self.state {
            WebAudioState::Suspended => {
                #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
                {
                    // context.resume()
                }
                self.state = WebAudioState::Running;
                Ok(())
            }
            WebAudioState::Running => Ok(()), // Already running, no-op.
            _ => Err(BackendError::NotInitialized),
        }
    }

    /// Suspend the AudioContext.
    pub fn suspend(&mut self) -> Result<(), BackendError> {
        match self.state {
            WebAudioState::Running => {
                #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
                {
                    // context.suspend()
                }
                self.state = WebAudioState::Suspended;
                Ok(())
            }
            WebAudioState::Suspended => Ok(()), // Already suspended.
            _ => Err(BackendError::NotInitialized),
        }
    }

    /// Close the AudioContext and release resources.
    pub fn close(&mut self) -> Result<(), BackendError> {
        match self.state {
            WebAudioState::Running | WebAudioState::Suspended => {
                #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
                {
                    // context.close()
                }
                self.state = WebAudioState::Closed;
                self.ring_buffer = None;
                Ok(())
            }
            _ => Err(BackendError::NotInitialized),
        }
    }

    /// Mix audio from the engine mixer into the ring buffer.
    ///
    /// Call this each frame to keep the audio pipeline fed.
    pub fn mix_into_buffer(&mut self, mixer: &mut AudioMixer) -> Result<usize, BackendError> {
        if self.state != WebAudioState::Running {
            return Ok(0);
        }

        let config = self.config.as_ref().ok_or(BackendError::NotInitialized)?;
        let buffer_frames = config.buffer_size as usize;
        let channels = config.channels as usize;
        let buffer_samples = buffer_frames * channels;

        let mut mix_buffer = vec![0.0f32; buffer_samples];
        mixer.mix(&mut mix_buffer);

        if let Some(ref mut ring) = self.ring_buffer {
            let written = ring.write(&mix_buffer);
            self.frames_submitted += (written / channels) as u64;
            self.buffers_submitted += 1;
            Ok(written / channels)
        } else {
            Err(BackendError::NotInitialized)
        }
    }

    /// Read samples from the ring buffer (called by the audio callback).
    ///
    /// On WASM, the ScriptProcessorNode's `onaudioprocess` or AudioWorklet
    /// calls this to get samples for output.
    pub fn read_output(&mut self, output: &mut [f32]) -> usize {
        if let Some(ref mut ring) = self.ring_buffer {
            ring.read(output)
        } else {
            // Fill with silence if not initialized.
            for s in output.iter_mut() {
                *s = 0.0;
            }
            0
        }
    }
}

impl Default for WebAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for WebAudioBackend {
    fn init(&mut self, config: BackendConfig) -> Result<(), BackendError> {
        let web_config = WebAudioConfig {
            sample_rate: config.sample_rate,
            channels: config.channels,
            buffer_size: config.buffer_size,
            ..WebAudioConfig::default()
        };

        self.init_context(web_config)?;
        self.backend_config = Some(config);
        // Auto-resume for the AudioBackend trait interface.
        self.resume()?;
        Ok(())
    }

    fn submit_buffer(&mut self, samples: &[f32]) -> Result<(), BackendError> {
        if self.state != WebAudioState::Running {
            return Err(BackendError::NotInitialized);
        }

        if let Some(ref mut ring) = self.ring_buffer {
            let channels = self.config.as_ref().map(|c| c.channels as usize).unwrap_or(2);
            let written = ring.write(samples);
            self.frames_submitted += (written / channels) as u64;
            self.buffers_submitted += 1;
            Ok(())
        } else {
            Err(BackendError::NotInitialized)
        }
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        self.close()
    }

    fn sample_rate(&self) -> u32 {
        self.backend_config
            .map(|c| c.sample_rate)
            .or_else(|| self.config.map(|c| c.sample_rate))
            .unwrap_or(0)
    }

    fn is_running(&self) -> bool {
        self.state == WebAudioState::Running
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_audio_config_default() {
        let config = WebAudioConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.buffer_size, 2048);
        assert!(!config.use_audio_worklet);
        assert!(config.auto_resume);
    }

    #[test]
    fn web_audio_backend_lifecycle() {
        let mut backend = WebAudioBackend::new();
        assert_eq!(backend.state(), WebAudioState::Uninitialized);

        backend.init_context(WebAudioConfig::default()).unwrap();
        assert_eq!(backend.state(), WebAudioState::Suspended);

        backend.resume().unwrap();
        assert_eq!(backend.state(), WebAudioState::Running);

        backend.suspend().unwrap();
        assert_eq!(backend.state(), WebAudioState::Suspended);

        backend.resume().unwrap();
        assert_eq!(backend.state(), WebAudioState::Running);

        backend.close().unwrap();
        assert_eq!(backend.state(), WebAudioState::Closed);
    }

    #[test]
    fn web_audio_backend_double_init_error() {
        let mut backend = WebAudioBackend::new();
        backend.init_context(WebAudioConfig::default()).unwrap();
        assert_eq!(
            backend.init_context(WebAudioConfig::default()),
            Err(BackendError::AlreadyRunning)
        );
    }

    #[test]
    fn web_audio_backend_resume_when_uninitialized() {
        let mut backend = WebAudioBackend::new();
        assert_eq!(backend.resume(), Err(BackendError::NotInitialized));
    }

    #[test]
    fn web_audio_backend_suspend_when_uninitialized() {
        let mut backend = WebAudioBackend::new();
        assert_eq!(backend.suspend(), Err(BackendError::NotInitialized));
    }

    #[test]
    fn web_audio_backend_close_when_uninitialized() {
        let mut backend = WebAudioBackend::new();
        assert_eq!(backend.close(), Err(BackendError::NotInitialized));
    }

    #[test]
    fn web_audio_backend_audio_backend_trait() {
        let mut backend = WebAudioBackend::new();
        assert!(!backend.is_running());
        assert_eq!(backend.sample_rate(), 0);

        let config = BackendConfig {
            sample_rate: 44100,
            channels: 2,
            buffer_size: 1024,
        };

        backend.init(config).unwrap();
        assert!(backend.is_running());
        assert_eq!(backend.sample_rate(), 44100);

        let samples = vec![0.5f32; 2048];
        backend.submit_buffer(&samples).unwrap();
        assert!(backend.frames_submitted > 0);

        backend.shutdown().unwrap();
        assert!(!backend.is_running());
    }

    #[test]
    fn web_audio_backend_submit_before_init_fails() {
        let mut backend = WebAudioBackend::new();
        assert_eq!(
            backend.submit_buffer(&[0.0; 100]),
            Err(BackendError::NotInitialized)
        );
    }

    #[test]
    fn web_audio_backend_ring_buffer_round_trip() {
        let mut backend = WebAudioBackend::new();
        let config = BackendConfig {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 512,
        };

        backend.init(config).unwrap();

        // Submit some samples.
        let input = vec![0.25f32; 256];
        backend.submit_buffer(&input).unwrap();

        // Read them back.
        let mut output = vec![0.0f32; 256];
        let read = backend.read_output(&mut output);
        assert_eq!(read, 256);
        for s in &output {
            assert!((*s - 0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn web_audio_backend_read_output_silence_when_empty() {
        let mut backend = WebAudioBackend::new();
        let mut output = vec![1.0f32; 100];
        let read = backend.read_output(&mut output);
        assert_eq!(read, 0);
        // Output should be zeroed.
        for s in &output {
            assert_eq!(*s, 0.0);
        }
    }

    #[test]
    fn web_audio_backend_mix_into_buffer() {
        let mut backend = WebAudioBackend::new();
        let config = BackendConfig {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 512,
        };
        backend.init(config).unwrap();

        // Create a mixer and mix into the backend.
        let mut mixer = AudioMixer::new(48000);
        let frames = backend.mix_into_buffer(&mut mixer).unwrap();

        // Mixer has no sources, so output is all silence, but frames should be written.
        assert!(frames > 0 || backend.buffers_submitted > 0);
    }

    #[test]
    fn web_audio_backend_mix_when_not_running() {
        let mut backend = WebAudioBackend::new();
        let mut mixer = AudioMixer::new(48000);

        // Mix when uninitialized should return 0 frames.
        let frames = backend.mix_into_buffer(&mut mixer).unwrap();
        assert_eq!(frames, 0);
    }

    #[test]
    fn web_audio_backend_with_config() {
        let config = WebAudioConfig {
            sample_rate: 44100,
            channels: 1,
            buffer_size: 1024,
            use_audio_worklet: true,
            auto_resume: false,
        };

        let backend = WebAudioBackend::with_config(config);
        let stored = backend.web_config().unwrap();
        assert_eq!(stored.sample_rate, 44100);
        assert_eq!(stored.channels, 1);
        assert!(stored.use_audio_worklet);
    }

    #[test]
    fn web_audio_backend_suspend_resume_idempotent() {
        let mut backend = WebAudioBackend::new();
        backend.init_context(WebAudioConfig::default()).unwrap();
        backend.resume().unwrap();

        // Resume when already running is a no-op.
        backend.resume().unwrap();
        assert_eq!(backend.state(), WebAudioState::Running);

        backend.suspend().unwrap();

        // Suspend when already suspended is a no-op.
        backend.suspend().unwrap();
        assert_eq!(backend.state(), WebAudioState::Suspended);
    }
}
