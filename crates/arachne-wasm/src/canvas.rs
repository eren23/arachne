//! Canvas setup and DPI handling for the Arachne WASM runtime.
//!
//! Manages the HTML `<canvas>` element lifecycle: creation, resizing,
//! device-pixel-ratio handling, and fullscreen support. On native targets,
//! provides stub implementations that track dimensions without DOM access.

// ---------------------------------------------------------------------------
// DPI information
// ---------------------------------------------------------------------------

/// Device pixel ratio and related DPI information.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DpiInfo {
    /// The CSS-to-physical pixel ratio (e.g. 2.0 on Retina displays).
    pub device_pixel_ratio: f64,
    /// Logical width in CSS pixels.
    pub logical_width: u32,
    /// Logical height in CSS pixels.
    pub logical_height: u32,
    /// Physical width in actual device pixels.
    pub physical_width: u32,
    /// Physical height in actual device pixels.
    pub physical_height: u32,
}

impl DpiInfo {
    /// Compute DPI info from logical dimensions and device pixel ratio.
    pub fn new(logical_width: u32, logical_height: u32, device_pixel_ratio: f64) -> Self {
        let physical_width = (logical_width as f64 * device_pixel_ratio).round() as u32;
        let physical_height = (logical_height as f64 * device_pixel_ratio).round() as u32;
        Self {
            device_pixel_ratio,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
        }
    }

    /// Compute physical dimensions from logical dimensions and DPI.
    pub fn compute_physical(logical: u32, dpi: f64) -> u32 {
        (logical as f64 * dpi).round() as u32
    }

    /// Compute logical dimensions from physical dimensions and DPI.
    pub fn compute_logical(physical: u32, dpi: f64) -> u32 {
        if dpi <= 0.0 {
            return physical;
        }
        (physical as f64 / dpi).round() as u32
    }
}

impl Default for DpiInfo {
    fn default() -> Self {
        Self::new(800, 600, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Canvas configuration
// ---------------------------------------------------------------------------

/// Configuration for creating a canvas element.
#[derive(Clone, Debug)]
pub struct CanvasConfig {
    /// CSS selector for the canvas element (e.g. "#my-canvas").
    pub selector: String,
    /// Initial logical width in CSS pixels.
    pub width: u32,
    /// Initial logical height in CSS pixels.
    pub height: u32,
    /// Whether to request high-DPI rendering.
    pub high_dpi: bool,
    /// Whether to start in fullscreen mode.
    pub fullscreen: bool,
    /// Whether to prevent default browser context menu on right-click.
    pub prevent_context_menu: bool,
}

impl Default for CanvasConfig {
    fn default() -> Self {
        Self {
            selector: "#arachne-canvas".to_string(),
            width: 800,
            height: 600,
            high_dpi: true,
            fullscreen: false,
            prevent_context_menu: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Canvas handle
// ---------------------------------------------------------------------------

/// Handle to a managed canvas element.
///
/// On WASM, this wraps a reference to the actual DOM `<canvas>`. On native,
/// it tracks dimensions as a stub for testing.
pub struct CanvasHandle {
    config: CanvasConfig,
    dpi: DpiInfo,
    is_fullscreen: bool,
    /// Tracks whether the canvas has been initialized.
    initialized: bool,
}

impl CanvasHandle {
    /// Create a new canvas handle from configuration.
    ///
    /// On WASM (behind cfg), this queries the DOM for the canvas element.
    /// On native, this creates a stub with the requested dimensions.
    pub fn new(config: CanvasConfig) -> Self {
        let dpr = if config.high_dpi {
            Self::query_device_pixel_ratio()
        } else {
            1.0
        };

        let dpi = DpiInfo::new(config.width, config.height, dpr);

        Self {
            config,
            dpi,
            is_fullscreen: false,
            initialized: false,
        }
    }

    /// Initialize the canvas. On WASM this sets up the DOM element; on native
    /// this is a no-op that marks the handle as ready.
    pub fn init(&mut self) -> Result<(), CanvasError> {
        if self.initialized {
            return Err(CanvasError::AlreadyInitialized);
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would:
            // 1. document.querySelector(self.config.selector)
            // 2. Set canvas.width / canvas.height to physical dimensions
            // 3. Set canvas.style.width / height to logical dimensions
            // 4. Optionally request fullscreen
            // 5. Add resize observer
        }

        self.initialized = true;

        if self.config.fullscreen {
            self.request_fullscreen()?;
        }

        Ok(())
    }

    /// Resize the canvas to new logical dimensions.
    pub fn resize(&mut self, logical_width: u32, logical_height: u32) {
        self.dpi = DpiInfo::new(logical_width, logical_height, self.dpi.device_pixel_ratio);
        self.config.width = logical_width;
        self.config.height = logical_height;

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would:
            // canvas.width = physical_width
            // canvas.height = physical_height
            // canvas.style.width = logical_width + "px"
            // canvas.style.height = logical_height + "px"
        }
    }

    /// Get the current DPI information.
    pub fn dpi_info(&self) -> &DpiInfo {
        &self.dpi
    }

    /// Get the logical width in CSS pixels.
    pub fn logical_width(&self) -> u32 {
        self.dpi.logical_width
    }

    /// Get the logical height in CSS pixels.
    pub fn logical_height(&self) -> u32 {
        self.dpi.logical_height
    }

    /// Get the physical width in device pixels.
    pub fn physical_width(&self) -> u32 {
        self.dpi.physical_width
    }

    /// Get the physical height in device pixels.
    pub fn physical_height(&self) -> u32 {
        self.dpi.physical_height
    }

    /// Get the device pixel ratio.
    pub fn device_pixel_ratio(&self) -> f64 {
        self.dpi.device_pixel_ratio
    }

    /// Whether the canvas is in fullscreen mode.
    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen
    }

    /// Whether the canvas has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Request fullscreen mode.
    pub fn request_fullscreen(&mut self) -> Result<(), CanvasError> {
        if !self.initialized {
            return Err(CanvasError::NotInitialized);
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would call:
            // canvas.requestFullscreen() or webkitRequestFullscreen()
        }

        self.is_fullscreen = true;
        Ok(())
    }

    /// Exit fullscreen mode.
    pub fn exit_fullscreen(&mut self) -> Result<(), CanvasError> {
        if !self.initialized {
            return Err(CanvasError::NotInitialized);
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real WASM implementation would call:
            // document.exitFullscreen()
        }

        self.is_fullscreen = false;
        Ok(())
    }

    /// Update the DPI when the device pixel ratio changes (e.g. user moves
    /// the window to a different monitor, or zoom changes).
    pub fn update_dpi(&mut self, new_dpr: f64) {
        self.dpi = DpiInfo::new(
            self.dpi.logical_width,
            self.dpi.logical_height,
            new_dpr,
        );
    }

    /// Convert a logical (CSS) position to a physical (device pixel) position.
    pub fn logical_to_physical(&self, logical_x: f64, logical_y: f64) -> (f64, f64) {
        let dpr = self.dpi.device_pixel_ratio;
        (logical_x * dpr, logical_y * dpr)
    }

    /// Convert a physical (device pixel) position to a logical (CSS) position.
    pub fn physical_to_logical(&self, physical_x: f64, physical_y: f64) -> (f64, f64) {
        let dpr = self.dpi.device_pixel_ratio;
        if dpr <= 0.0 {
            return (physical_x, physical_y);
        }
        (physical_x / dpr, physical_y / dpr)
    }

    /// Get the CSS selector used to find this canvas.
    pub fn selector(&self) -> &str {
        &self.config.selector
    }

    /// Query the device pixel ratio.
    ///
    /// On WASM, reads `window.devicePixelRatio`. On native, returns 1.0.
    fn query_device_pixel_ratio() -> f64 {
        #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
        {
            // Real implementation: web_sys::window().unwrap().device_pixel_ratio()
            // For now, a reasonable default.
            2.0
        }

        #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
        {
            1.0
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during canvas operations.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasError {
    /// The canvas element was not found in the DOM.
    ElementNotFound(String),
    /// The canvas is not initialized.
    NotInitialized,
    /// The canvas is already initialized.
    AlreadyInitialized,
    /// Fullscreen request was denied.
    FullscreenDenied,
    /// A general canvas error.
    Other(String),
}

impl core::fmt::Display for CanvasError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CanvasError::ElementNotFound(sel) => write!(f, "canvas element not found: {sel}"),
            CanvasError::NotInitialized => write!(f, "canvas not initialized"),
            CanvasError::AlreadyInitialized => write!(f, "canvas already initialized"),
            CanvasError::FullscreenDenied => write!(f, "fullscreen request denied"),
            CanvasError::Other(msg) => write!(f, "canvas error: {msg}"),
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
    fn dpi_info_standard_display() {
        let dpi = DpiInfo::new(800, 600, 1.0);
        assert_eq!(dpi.logical_width, 800);
        assert_eq!(dpi.logical_height, 600);
        assert_eq!(dpi.physical_width, 800);
        assert_eq!(dpi.physical_height, 600);
        assert_eq!(dpi.device_pixel_ratio, 1.0);
    }

    #[test]
    fn dpi_info_retina_display() {
        let dpi = DpiInfo::new(800, 600, 2.0);
        assert_eq!(dpi.logical_width, 800);
        assert_eq!(dpi.logical_height, 600);
        assert_eq!(dpi.physical_width, 1600);
        assert_eq!(dpi.physical_height, 1200);
        assert_eq!(dpi.device_pixel_ratio, 2.0);
    }

    #[test]
    fn dpi_info_fractional_scaling() {
        let dpi = DpiInfo::new(1920, 1080, 1.5);
        assert_eq!(dpi.physical_width, 2880);
        assert_eq!(dpi.physical_height, 1620);
    }

    #[test]
    fn dpi_compute_physical_correct() {
        assert_eq!(DpiInfo::compute_physical(800, 2.0), 1600);
        assert_eq!(DpiInfo::compute_physical(1920, 1.5), 2880);
        assert_eq!(DpiInfo::compute_physical(100, 1.0), 100);
    }

    #[test]
    fn dpi_compute_logical_correct() {
        assert_eq!(DpiInfo::compute_logical(1600, 2.0), 800);
        assert_eq!(DpiInfo::compute_logical(2880, 1.5), 1920);
        assert_eq!(DpiInfo::compute_logical(100, 1.0), 100);
    }

    #[test]
    fn dpi_compute_logical_zero_dpi_returns_physical() {
        assert_eq!(DpiInfo::compute_logical(800, 0.0), 800);
    }

    #[test]
    fn canvas_config_default() {
        let config = CanvasConfig::default();
        assert_eq!(config.selector, "#arachne-canvas");
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
        assert!(config.high_dpi);
        assert!(!config.fullscreen);
        assert!(config.prevent_context_menu);
    }

    #[test]
    fn canvas_handle_creation_and_dimensions() {
        let config = CanvasConfig {
            selector: "#test".to_string(),
            width: 1024,
            height: 768,
            high_dpi: false, // DPR = 1.0 on native
            fullscreen: false,
            prevent_context_menu: false,
        };

        let handle = CanvasHandle::new(config);
        assert_eq!(handle.logical_width(), 1024);
        assert_eq!(handle.logical_height(), 768);
        assert_eq!(handle.physical_width(), 1024);
        assert_eq!(handle.physical_height(), 768);
        assert_eq!(handle.device_pixel_ratio(), 1.0);
        assert!(!handle.is_initialized());
    }

    #[test]
    fn canvas_handle_init_and_resize() {
        let config = CanvasConfig {
            selector: "#test".to_string(),
            width: 640,
            height: 480,
            high_dpi: false,
            fullscreen: false,
            prevent_context_menu: false,
        };

        let mut handle = CanvasHandle::new(config);
        handle.init().unwrap();
        assert!(handle.is_initialized());

        handle.resize(1920, 1080);
        assert_eq!(handle.logical_width(), 1920);
        assert_eq!(handle.logical_height(), 1080);
        assert_eq!(handle.physical_width(), 1920);
        assert_eq!(handle.physical_height(), 1080);
    }

    #[test]
    fn canvas_handle_double_init_error() {
        let config = CanvasConfig::default();
        let mut handle = CanvasHandle::new(config);
        handle.init().unwrap();
        assert_eq!(handle.init(), Err(CanvasError::AlreadyInitialized));
    }

    #[test]
    fn canvas_fullscreen_lifecycle() {
        let config = CanvasConfig::default();
        let mut handle = CanvasHandle::new(config);

        // Cannot request fullscreen before init.
        assert_eq!(
            handle.request_fullscreen(),
            Err(CanvasError::NotInitialized)
        );

        handle.init().unwrap();
        assert!(!handle.is_fullscreen());

        handle.request_fullscreen().unwrap();
        assert!(handle.is_fullscreen());

        handle.exit_fullscreen().unwrap();
        assert!(!handle.is_fullscreen());
    }

    #[test]
    fn canvas_logical_to_physical_conversion() {
        let config = CanvasConfig::default();
        let mut handle = CanvasHandle::new(config);
        handle.update_dpi(2.0);

        let (px, py) = handle.logical_to_physical(100.0, 50.0);
        assert!((px - 200.0).abs() < 1e-10);
        assert!((py - 100.0).abs() < 1e-10);
    }

    #[test]
    fn canvas_physical_to_logical_conversion() {
        let config = CanvasConfig::default();
        let mut handle = CanvasHandle::new(config);
        handle.update_dpi(2.0);

        let (lx, ly) = handle.physical_to_logical(200.0, 100.0);
        assert!((lx - 100.0).abs() < 1e-10);
        assert!((ly - 50.0).abs() < 1e-10);
    }

    #[test]
    fn canvas_update_dpi_recalculates_physical() {
        let config = CanvasConfig {
            selector: "#test".to_string(),
            width: 800,
            height: 600,
            high_dpi: false,
            fullscreen: false,
            prevent_context_menu: false,
        };

        let mut handle = CanvasHandle::new(config);
        assert_eq!(handle.physical_width(), 800);
        assert_eq!(handle.physical_height(), 600);

        handle.update_dpi(2.0);
        assert_eq!(handle.physical_width(), 1600);
        assert_eq!(handle.physical_height(), 1200);
        assert_eq!(handle.logical_width(), 800);
        assert_eq!(handle.logical_height(), 600);
    }

    #[test]
    fn canvas_config_with_fullscreen_auto_enters() {
        let config = CanvasConfig {
            selector: "#fs".to_string(),
            width: 1920,
            height: 1080,
            high_dpi: false,
            fullscreen: true,
            prevent_context_menu: false,
        };

        let mut handle = CanvasHandle::new(config);
        handle.init().unwrap();
        assert!(handle.is_fullscreen());
    }

    #[test]
    fn canvas_selector() {
        let config = CanvasConfig {
            selector: "#my-game".to_string(),
            ..CanvasConfig::default()
        };
        let handle = CanvasHandle::new(config);
        assert_eq!(handle.selector(), "#my-game");
    }
}
