/// Window configuration and builder.

/// Fullscreen mode for the window.
#[derive(Clone, Debug, PartialEq)]
pub enum FullscreenMode {
    Windowed,
    Borderless,
    Exclusive,
}

impl Default for FullscreenMode {
    fn default() -> Self {
        Self::Windowed
    }
}

/// Configuration for creating an [`ArachneWindow`](crate::ArachneWindow).
#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub vsync: bool,
    pub fullscreen: FullscreenMode,
    pub min_size: Option<(u32, u32)>,
    pub max_size: Option<(u32, u32)>,
    pub transparent: bool,
    pub decorations: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Arachne".into(),
            width: 800,
            height: 600,
            resizable: true,
            vsync: true,
            fullscreen: FullscreenMode::Windowed,
            min_size: None,
            max_size: None,
            transparent: false,
            decorations: true,
        }
    }
}

impl WindowConfig {
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_fullscreen(mut self, mode: FullscreenMode) -> Self {
        self.fullscreen = mode;
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn with_vsync(mut self, vsync: bool) -> Self {
        self.vsync = vsync;
        self
    }

    pub fn with_min_size(mut self, w: u32, h: u32) -> Self {
        self.min_size = Some((w, h));
        self
    }

    pub fn with_max_size(mut self, w: u32, h: u32) -> Self {
        self.max_size = Some((w, h));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.title, "Arachne");
        assert_eq!(cfg.width, 800);
        assert_eq!(cfg.height, 600);
        assert!(cfg.vsync);
        assert!(cfg.resizable);
        assert_eq!(cfg.fullscreen, FullscreenMode::Windowed);
        assert!(!cfg.transparent);
        assert!(cfg.decorations);
        assert!(cfg.min_size.is_none());
        assert!(cfg.max_size.is_none());
    }

    #[test]
    fn builder_chaining() {
        let cfg = WindowConfig::default()
            .with_title("Test")
            .with_size(1024, 768);
        assert_eq!(cfg.title, "Test");
        assert_eq!(cfg.width, 1024);
        assert_eq!(cfg.height, 768);
    }

    #[test]
    fn fullscreen_default_is_windowed() {
        assert_eq!(FullscreenMode::default(), FullscreenMode::Windowed);
        // Ensure all variants exist.
        let _b = FullscreenMode::Borderless;
        let _e = FullscreenMode::Exclusive;
    }

    #[test]
    fn min_max_size() {
        let cfg = WindowConfig::default()
            .with_min_size(320, 240)
            .with_max_size(1920, 1080);
        assert_eq!(cfg.min_size, Some((320, 240)));
        assert_eq!(cfg.max_size, Some((1920, 1080)));
    }

    #[test]
    fn builder_vsync_and_resizable() {
        let cfg = WindowConfig::default()
            .with_vsync(false)
            .with_resizable(false);
        assert!(!cfg.vsync);
        assert!(!cfg.resizable);
    }

    #[test]
    fn builder_fullscreen() {
        let cfg = WindowConfig::default()
            .with_fullscreen(FullscreenMode::Borderless);
        assert_eq!(cfg.fullscreen, FullscreenMode::Borderless);
    }
}
