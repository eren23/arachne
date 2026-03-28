pub mod source;
pub mod decoder;
pub mod mixer;
pub mod spatial;
pub mod effect;
pub mod backend;
#[cfg(feature = "native-audio")]
pub mod output;

pub use source::{AudioSource, StreamingSource, MemoryStream};
pub use decoder::{decode_wav, WavError, WavStream, build_test_wav};
pub use mixer::{AudioMixer, ChannelHandle, PlayConfig, PlayState, MAX_CHANNELS, DEFAULT_BUFFER_FRAMES};
pub use spatial::{Listener, SpatialSource, DistanceModel, SpatialParams, compute_pan, compute_angle_degrees, compute_spatial};
pub use effect::{LowPassFilter, AdsrEnvelope, AdsrPhase, SchroederReverb};
pub use backend::{AudioBackend, BackendConfig, BackendError, NullBackend, NativeBackend, WasmBackend, AudioRingBuffer};
#[cfg(feature = "native-audio")]
pub use output::{AudioOutput, AudioOutputHandle, NullAudioOutput};
