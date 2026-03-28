//! Audio backend traits and platform implementations.
//!
//! Provides a common trait for audio output, with concrete implementations
//! for native (cpal) and WASM (WebAudio) platforms.

/// Configuration for initializing an audio backend.
#[derive(Clone, Copy, Debug)]
pub struct BackendConfig {
    /// Desired sample rate in Hz (e.g. 48000).
    pub sample_rate: u32,
    /// Number of output channels (typically 2 for stereo).
    pub channels: u32,
    /// Output buffer size in frames.
    pub buffer_size: u32,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 1024,
        }
    }
}

/// Trait for audio output backends.
///
/// Implementations handle platform-specific audio output, accepting
/// interleaved f32 stereo buffers from the mixer.
pub trait AudioBackend {
    /// Initializes the audio backend with the given configuration.
    fn init(&mut self, config: BackendConfig) -> Result<(), BackendError>;

    /// Submits a buffer of interleaved f32 samples for output.
    fn submit_buffer(&mut self, samples: &[f32]) -> Result<(), BackendError>;

    /// Shuts down the audio backend and releases resources.
    fn shutdown(&mut self) -> Result<(), BackendError>;

    /// Returns the actual sample rate after initialization.
    fn sample_rate(&self) -> u32;

    /// Returns whether the backend is currently initialized and running.
    fn is_running(&self) -> bool;
}

/// Errors from audio backend operations.
#[derive(Clone, Debug, PartialEq)]
pub enum BackendError {
    /// Backend failed to initialize.
    InitFailed(String),
    /// Backend is not initialized.
    NotInitialized,
    /// Buffer submission failed.
    SubmitFailed(String),
    /// Backend is already running.
    AlreadyRunning,
    /// Platform not supported.
    Unsupported,
}

impl core::fmt::Display for BackendError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BackendError::InitFailed(msg) => write!(f, "init failed: {msg}"),
            BackendError::NotInitialized => write!(f, "backend not initialized"),
            BackendError::SubmitFailed(msg) => write!(f, "submit failed: {msg}"),
            BackendError::AlreadyRunning => write!(f, "backend already running"),
            BackendError::Unsupported => write!(f, "platform not supported"),
        }
    }
}

// ---------------------------------------------------------------------------
// Null Backend (for testing and headless use)
// ---------------------------------------------------------------------------

/// A null audio backend that discards all output.
/// Useful for testing and headless operation.
pub struct NullBackend {
    config: Option<BackendConfig>,
    running: bool,
    /// Total frames submitted (for testing).
    pub frames_submitted: u64,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            config: None,
            running: false,
            frames_submitted: 0,
        }
    }
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for NullBackend {
    fn init(&mut self, config: BackendConfig) -> Result<(), BackendError> {
        if self.running {
            return Err(BackendError::AlreadyRunning);
        }
        self.config = Some(config);
        self.running = true;
        Ok(())
    }

    fn submit_buffer(&mut self, samples: &[f32]) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }
        let channels = self.config.as_ref().unwrap().channels as usize;
        if channels > 0 {
            self.frames_submitted += (samples.len() / channels) as u64;
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }
        self.running = false;
        self.config = None;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.config.map(|c| c.sample_rate).unwrap_or(0)
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

// ---------------------------------------------------------------------------
// Ring Buffer for callback-based backends
// ---------------------------------------------------------------------------

/// A lock-free-style ring buffer for passing audio data from the mixer
/// to a callback-driven audio output.
pub struct AudioRingBuffer {
    buffer: Vec<f32>,
    read_pos: usize,
    write_pos: usize,
    capacity: usize,
}

impl AudioRingBuffer {
    /// Creates a new ring buffer with the given capacity in samples.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            read_pos: 0,
            write_pos: 0,
            capacity,
        }
    }

    /// Returns the number of samples available to read.
    pub fn available(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    /// Returns the amount of free space in samples.
    pub fn free_space(&self) -> usize {
        self.capacity - 1 - self.available()
    }

    /// Writes samples into the ring buffer.
    /// Returns the number of samples actually written.
    pub fn write(&mut self, data: &[f32]) -> usize {
        let to_write = data.len().min(self.free_space());
        for i in 0..to_write {
            self.buffer[self.write_pos] = data[i];
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }
        to_write
    }

    /// Reads samples from the ring buffer.
    /// Returns the number of samples actually read.
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        let to_read = output.len().min(self.available());
        for i in 0..to_read {
            output[i] = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }
        to_read
    }

    /// Clears the buffer.
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
    }
}

// ---------------------------------------------------------------------------
// Native Backend stub (requires cpal at runtime)
// ---------------------------------------------------------------------------

/// Native audio backend using the system audio API.
///
/// In a real build, this would use `cpal` to create an output stream.
/// This struct provides the interface; actual audio output requires
/// the `cpal` dependency to be enabled.
pub struct NativeBackend {
    config: Option<BackendConfig>,
    running: bool,
    ring_buffer: Option<AudioRingBuffer>,
}

impl NativeBackend {
    pub fn new() -> Self {
        Self {
            config: None,
            running: false,
            ring_buffer: None,
        }
    }
}

impl Default for NativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for NativeBackend {
    fn init(&mut self, config: BackendConfig) -> Result<(), BackendError> {
        if self.running {
            return Err(BackendError::AlreadyRunning);
        }

        // Create ring buffer (4x buffer size for headroom)
        let ring_size = config.buffer_size as usize * config.channels as usize * 4;
        self.ring_buffer = Some(AudioRingBuffer::new(ring_size));
        self.config = Some(config);
        self.running = true;

        // NOTE: In a real implementation, this would:
        // 1. Initialize cpal default host
        // 2. Get default output device
        // 3. Build output stream with callback that reads from ring_buffer
        // 4. Start the stream

        Ok(())
    }

    fn submit_buffer(&mut self, samples: &[f32]) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }
        if let Some(ref mut ring) = self.ring_buffer {
            ring.write(samples);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }
        self.running = false;
        self.ring_buffer = None;
        self.config = None;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.config.map(|c| c.sample_rate).unwrap_or(0)
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

/// WASM audio backend using Web Audio API.
///
/// In a real build targeting wasm32, this would use `web-sys` to create
/// an AudioContext and ScriptProcessorNode/AudioWorklet.
pub struct WasmBackend {
    config: Option<BackendConfig>,
    running: bool,
}

impl WasmBackend {
    pub fn new() -> Self {
        Self {
            config: None,
            running: false,
        }
    }
}

impl Default for WasmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for WasmBackend {
    fn init(&mut self, config: BackendConfig) -> Result<(), BackendError> {
        if self.running {
            return Err(BackendError::AlreadyRunning);
        }

        // NOTE: In a real WASM build, this would:
        // 1. Create AudioContext (handling autoplay restrictions)
        // 2. Create ScriptProcessorNode or AudioWorklet
        // 3. Connect to destination
        // 4. Set up onaudioprocess callback

        self.config = Some(config);
        self.running = true;
        Ok(())
    }

    fn submit_buffer(&mut self, _samples: &[f32]) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }

        // NOTE: In real WASM implementation, buffer would be written
        // to the AudioWorklet's shared buffer or queued for
        // ScriptProcessorNode callback

        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        if !self.running {
            return Err(BackendError::NotInitialized);
        }

        // NOTE: Would close AudioContext and disconnect nodes

        self.running = false;
        self.config = None;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.config.map(|c| c.sample_rate).unwrap_or(0)
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_backend_lifecycle() {
        let mut backend = NullBackend::new();
        assert!(!backend.is_running());

        backend.init(BackendConfig::default()).unwrap();
        assert!(backend.is_running());
        assert_eq!(backend.sample_rate(), 48000);

        let samples = vec![0.0f32; 2048];
        backend.submit_buffer(&samples).unwrap();
        assert_eq!(backend.frames_submitted, 1024);

        backend.shutdown().unwrap();
        assert!(!backend.is_running());
    }

    #[test]
    fn test_null_backend_double_init() {
        let mut backend = NullBackend::new();
        backend.init(BackendConfig::default()).unwrap();
        assert_eq!(
            backend.init(BackendConfig::default()),
            Err(BackendError::AlreadyRunning)
        );
    }

    #[test]
    fn test_null_backend_submit_before_init() {
        let mut backend = NullBackend::new();
        assert_eq!(
            backend.submit_buffer(&[0.0; 100]),
            Err(BackendError::NotInitialized)
        );
    }

    #[test]
    fn test_native_backend_lifecycle() {
        let mut backend = NativeBackend::new();
        assert!(!backend.is_running());

        backend.init(BackendConfig::default()).unwrap();
        assert!(backend.is_running());

        let samples = vec![0.5f32; 2048];
        backend.submit_buffer(&samples).unwrap();

        backend.shutdown().unwrap();
        assert!(!backend.is_running());
    }

    #[test]
    fn test_wasm_backend_lifecycle() {
        let mut backend = WasmBackend::new();
        assert!(!backend.is_running());

        backend.init(BackendConfig::default()).unwrap();
        assert!(backend.is_running());

        backend.submit_buffer(&[0.0; 100]).unwrap();

        backend.shutdown().unwrap();
        assert!(!backend.is_running());
    }

    #[test]
    fn test_ring_buffer_write_read() {
        let mut ring = AudioRingBuffer::new(16);
        assert_eq!(ring.available(), 0);

        let data = [1.0, 2.0, 3.0, 4.0];
        let written = ring.write(&data);
        assert_eq!(written, 4);
        assert_eq!(ring.available(), 4);

        let mut out = [0.0f32; 4];
        let read = ring.read(&mut out);
        assert_eq!(read, 4);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(ring.available(), 0);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let mut ring = AudioRingBuffer::new(8);

        // Write 5 samples
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        ring.write(&data);

        // Read 3
        let mut out = [0.0f32; 3];
        ring.read(&mut out);
        assert_eq!(out, [1.0, 2.0, 3.0]);

        // Write 4 more (wraps around)
        let data2 = [6.0, 7.0, 8.0, 9.0];
        let written = ring.write(&data2);
        assert_eq!(written, 4);

        // Read all
        let mut out2 = [0.0f32; 6];
        let read = ring.read(&mut out2);
        assert_eq!(read, 6);
        assert_eq!(out2, [4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut ring = AudioRingBuffer::new(4);
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        let written = ring.write(&data);
        // capacity 4 -> max 3 writable (ring buffer reserves 1 slot)
        assert_eq!(written, 3);
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut ring = AudioRingBuffer::new(16);
        ring.write(&[1.0, 2.0, 3.0]);
        assert_eq!(ring.available(), 3);
        ring.clear();
        assert_eq!(ring.available(), 0);
    }
}
