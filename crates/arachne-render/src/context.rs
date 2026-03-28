use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// Presentation mode for windowed rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentMode {
    /// Vertical-sync (maps to `wgpu::PresentMode::Fifo`).
    Vsync,
    /// No sync — may tear (maps to `wgpu::PresentMode::Immediate`).
    Immediate,
    /// Low-latency vsync (maps to `wgpu::PresentMode::Mailbox`).
    Mailbox,
}

impl Default for PresentMode {
    fn default() -> Self {
        Self::Vsync
    }
}

impl PresentMode {
    fn to_wgpu(self) -> wgpu::PresentMode {
        match self {
            Self::Vsync => wgpu::PresentMode::Fifo,
            Self::Immediate => wgpu::PresentMode::Immediate,
            Self::Mailbox => wgpu::PresentMode::Mailbox,
        }
    }
}

/// Errors that can occur during render context creation.
#[derive(Debug)]
pub enum RenderError {
    NoAdapter,
    DeviceRequest(wgpu::RequestDeviceError),
    Surface(wgpu::CreateSurfaceError),
    HandleUnavailable,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "no suitable GPU adapter found"),
            Self::DeviceRequest(e) => write!(f, "device request failed: {e}"),
            Self::Surface(e) => write!(f, "surface creation failed: {e}"),
            Self::HandleUnavailable => write!(f, "window handle unavailable"),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<wgpu::RequestDeviceError> for RenderError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        Self::DeviceRequest(e)
    }
}

impl From<wgpu::CreateSurfaceError> for RenderError {
    fn from(e: wgpu::CreateSurfaceError) -> Self {
        Self::Surface(e)
    }
}

/// Surface state for windowed rendering.
struct SurfaceState {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

/// Core GPU context: owns wgpu Instance, Adapter, Device, Queue, and optional Surface.
pub struct RenderContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_state: Option<SurfaceState>,
    pending_frame: Option<wgpu::SurfaceTexture>,
}

impl RenderContext {
    /// Create a headless render context (no surface). Useful for tests and offscreen rendering.
    pub async fn new_headless() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arachne"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                ..Default::default()
            }, None)
            .await?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface_state: None,
            pending_frame: None,
        })
    }

    /// Create a render context with a surface for windowed rendering.
    ///
    /// On native, `target` is typically an `Arc<winit::window::Window>`.
    /// On WASM, it would be a canvas element.
    pub async fn new_with_surface(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(target)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("arachne"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                ..Default::default()
            }, None)
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface_state: Some(SurfaceState { surface, config }),
            pending_frame: None,
        })
    }

    /// Create a render context from a window handle.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `window` outlives the returned `RenderContext`.
    /// This is the standard pattern for game engines where the window is created
    /// before the renderer and destroyed after it.
    pub async fn new_with_window<W: HasWindowHandle + HasDisplayHandle>(
        window: &W,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: Caller guarantees the window outlives the RenderContext.
        let surface: wgpu::Surface<'static> = unsafe {
            let target = wgpu::SurfaceTargetUnsafe::from_window(window)
                .map_err(|_| RenderError::HandleUnavailable)?;
            instance.create_surface_unsafe(target)?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RenderError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("arachne"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    ..Default::default()
                },
                None,
            )
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface_state: Some(SurfaceState { surface, config }),
            pending_frame: None,
        })
    }

    /// Resize the surface. No-op if headless.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(state) = &mut self.surface_state {
            state.config.width = width;
            state.config.height = height;
            state.surface.configure(&self.device, &state.config);
        }
    }

    /// Reconfigure the surface with new dimensions and presentation mode.
    /// No-op if headless or if width/height is zero.
    pub fn configure_surface(&mut self, width: u32, height: u32, present_mode: PresentMode) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some(state) = &mut self.surface_state {
            state.config.width = width;
            state.config.height = height;
            state.config.present_mode = present_mode.to_wgpu();
            state.surface.configure(&self.device, &state.config);
        }
    }

    /// Get the current surface texture for rendering. Returns `None` if headless.
    ///
    /// The texture is stored internally; call [`present`](Self::present) when
    /// rendering is complete.
    pub fn current_texture(&mut self) -> Option<&wgpu::SurfaceTexture> {
        self.pending_frame = self.surface_state
            .as_ref()
            .and_then(|s| s.surface.get_current_texture().ok());
        self.pending_frame.as_ref()
    }

    /// Take ownership of the pending surface texture, if any.
    /// Used by [`crate::pipeline_2d::RenderFrame`] to own the surface texture for a frame.
    pub fn take_pending_frame(&mut self) -> Option<wgpu::SurfaceTexture> {
        self.pending_frame.take()
    }

    /// Present the current frame. No-op if headless or no frame was acquired.
    pub fn present(&mut self) {
        if let Some(frame) = self.pending_frame.take() {
            frame.present();
        }
    }

    /// The preferred surface texture format, or a sensible default for headless.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_state
            .as_ref()
            .map(|s| s.config.format)
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb)
    }

    pub fn surface_size(&self) -> (u32, u32) {
        self.surface_state
            .as_ref()
            .map(|s| (s.config.width, s.config.height))
            .unwrap_or((0, 0))
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[inline]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    #[inline]
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    #[inline]
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_headless_context() {
        let ctx = pollster::block_on(RenderContext::new_headless());
        assert!(ctx.is_ok(), "headless context creation failed: {:?}", ctx.err());
        let ctx = ctx.unwrap();
        // Verify we got a device
        let info = ctx.adapter().get_info();
        eprintln!("Adapter: {} ({:?})", info.name, info.backend);
        assert!(!info.name.is_empty());
    }

    #[test]
    fn headless_has_no_surface() {
        let mut ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        assert!(ctx.current_texture().is_none());
        assert_eq!(ctx.surface_size(), (0, 0));
    }

    #[test]
    fn surface_format_default() {
        let ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        assert_eq!(ctx.surface_format(), wgpu::TextureFormat::Rgba8UnormSrgb);
    }

    #[test]
    fn present_mode_default_is_vsync() {
        assert_eq!(PresentMode::default(), PresentMode::Vsync);
    }

    #[test]
    fn present_mode_variants_constructible() {
        let modes = [PresentMode::Vsync, PresentMode::Immediate, PresentMode::Mailbox];
        assert_eq!(modes[0].to_wgpu(), wgpu::PresentMode::Fifo);
        assert_eq!(modes[1].to_wgpu(), wgpu::PresentMode::Immediate);
        assert_eq!(modes[2].to_wgpu(), wgpu::PresentMode::Mailbox);
    }

    #[test]
    fn srgb_format_preferred() {
        // Given a list of formats, the sRGB selection logic picks sRGB.
        let formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        let picked = formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(formats[0]);
        assert!(picked.is_srgb());
        assert_eq!(picked, wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    fn configure_surface_zero_size_no_panic() {
        let mut ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        // Should be no-op, must not panic.
        ctx.configure_surface(0, 0, PresentMode::default());
        ctx.configure_surface(100, 0, PresentMode::Vsync);
        ctx.configure_surface(0, 100, PresentMode::Immediate);
    }

    #[test]
    fn present_headless_is_noop() {
        let mut ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        // Should not panic on headless context.
        ctx.present();
    }

    #[test]
    fn resize_zero_is_noop() {
        let mut ctx = pollster::block_on(RenderContext::new_headless()).unwrap();
        ctx.resize(0, 0);
        assert_eq!(ctx.surface_size(), (0, 0));
    }
}
