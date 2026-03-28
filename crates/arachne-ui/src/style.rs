use arachne_math::Color;
use crate::layout::Edges;

/// Visual style for a UI element.
#[derive(Clone, Debug)]
pub struct Style {
    pub background_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub border_radius: f32,
    pub text_color: Color,
    pub font_size: f32,
    pub padding: Edges,
    pub margin: Edges,
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background_color: Color::new(0.2, 0.2, 0.2, 1.0),
            border_color: Color::new(0.4, 0.4, 0.4, 1.0),
            border_width: 1.0,
            border_radius: 4.0,
            text_color: Color::WHITE,
            font_size: 16.0,
            padding: Edges::all(8.0),
            margin: Edges::ZERO,
            opacity: 1.0,
        }
    }
}

/// A collection of styles for different widget states.
#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub default: Style,
    pub hover: Style,
    pub active: Style,
    pub disabled: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),
            default: Style::default(),
            hover: Style {
                background_color: Color::new(0.3, 0.3, 0.3, 1.0),
                border_color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Style::default()
            },
            active: Style {
                background_color: Color::new(0.15, 0.15, 0.15, 1.0),
                border_color: Color::new(0.5, 0.6, 0.8, 1.0),
                border_width: 2.0,
                ..Style::default()
            },
            disabled: Style {
                background_color: Color::new(0.15, 0.15, 0.15, 1.0),
                border_color: Color::new(0.25, 0.25, 0.25, 1.0),
                text_color: Color::new(0.5, 0.5, 0.5, 1.0),
                opacity: 0.5,
                ..Style::default()
            },
        }
    }

    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
            default: Style {
                background_color: Color::new(0.9, 0.9, 0.9, 1.0),
                border_color: Color::new(0.7, 0.7, 0.7, 1.0),
                text_color: Color::BLACK,
                ..Style::default()
            },
            hover: Style {
                background_color: Color::new(0.85, 0.85, 0.85, 1.0),
                border_color: Color::new(0.6, 0.6, 0.6, 1.0),
                text_color: Color::BLACK,
                ..Style::default()
            },
            active: Style {
                background_color: Color::new(0.8, 0.8, 0.8, 1.0),
                border_color: Color::new(0.4, 0.5, 0.7, 1.0),
                border_width: 2.0,
                text_color: Color::BLACK,
                ..Style::default()
            },
            disabled: Style {
                background_color: Color::new(0.85, 0.85, 0.85, 1.0),
                border_color: Color::new(0.75, 0.75, 0.75, 1.0),
                text_color: Color::new(0.6, 0.6, 0.6, 1.0),
                opacity: 0.5,
                ..Style::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_defaults() {
        let theme = Theme::dark();
        assert_eq!(theme.name, "Dark");
        assert!((theme.default.opacity - 1.0).abs() < f32::EPSILON);
        assert!((theme.disabled.opacity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn light_theme_defaults() {
        let theme = Theme::light();
        assert_eq!(theme.name, "Light");
    }

    #[test]
    fn theme_switching() {
        let dark = Theme::dark();
        let light = Theme::light();
        assert_ne!(dark.default.background_color, light.default.background_color);
    }
}
