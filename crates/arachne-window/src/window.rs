/// Window wrapper around [`winit::window::Window`].
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::config::WindowConfig;

/// A thin wrapper around the platform window.
pub struct ArachneWindow {
    inner: Window,
}

impl ArachneWindow {
    /// Create a new window on the given event loop using `config`.
    pub fn new(event_loop: &ActiveEventLoop, config: &WindowConfig) -> Self {
        let mut attrs = WindowAttributes::default()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(config.width, config.height))
            .with_resizable(config.resizable)
            .with_decorations(config.decorations)
            .with_transparent(config.transparent);

        if let Some((w, h)) = config.min_size {
            attrs = attrs.with_min_inner_size(PhysicalSize::new(w, h));
        }
        if let Some((w, h)) = config.max_size {
            attrs = attrs.with_max_inner_size(PhysicalSize::new(w, h));
        }

        let window = event_loop
            .create_window(attrs)
            .expect("failed to create window");

        Self { inner: window }
    }

    /// Request the window surface to be redrawn.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }

    /// Physical pixel dimensions of the window client area.
    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.inner.inner_size();
        (size.width, size.height)
    }

    /// DPI / HiDPI scale factor reported by the platform.
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Change the window title at runtime.
    pub fn set_title(&self, title: &str) {
        self.inner.set_title(title);
    }

    /// Show or hide the mouse cursor while over the window.
    pub fn set_cursor_visible(&self, visible: bool) {
        self.inner.set_cursor_visible(visible);
    }
}

impl HasWindowHandle for ArachneWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for ArachneWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.inner.display_handle()
    }
}
