/// Event loop creation and cross-thread signalling.
use winit::event_loop::{EventLoop, EventLoopProxy as WinitProxy};

/// Create a new platform event loop.
pub fn create_event_loop() -> EventLoop<()> {
    EventLoop::new().expect("failed to create event loop")
}

/// Wrapper around [`winit::event_loop::EventLoopProxy`] for cross-thread wake.
pub struct EventLoopProxy {
    inner: WinitProxy<()>,
}

impl EventLoopProxy {
    /// Build from an existing [`EventLoop`].
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        Self {
            inner: event_loop.create_proxy(),
        }
    }

    /// Wake the event loop from another thread.
    pub fn wake(&self) {
        let _ = self.inner.send_event(());
    }
}
