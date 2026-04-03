use owo_colors::{AnsiColors, DynColors};
use serde::{Deserialize, Serialize};

/// Represents an ANSI-formatted string.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum StyledContent {
    Text(String),
    Styled { style: Style, children: Vec<Self> },
}

/// Serde-friendly style for the daemon-client wire format.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dimmed: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Style {
    #[must_use]
    pub fn to_owo(&self) -> owo_colors::Style {
        let mut owo = owo_colors::Style::new();
        if let Some(fg) = self.fg {
            owo = owo.color(DynColors::Ansi(fg.into()));
        }
        if let Some(bg) = self.bg {
            owo = owo.on_color(DynColors::Ansi(bg.into()));
        }
        if self.bold {
            owo = owo.bold();
        }
        if self.italic {
            owo = owo.italic();
        }
        if self.dimmed {
            owo = owo.dimmed();
        }
        if self.underline {
            owo = owo.underline();
        }
        if self.strikethrough {
            owo = owo.strikethrough();
        }
        owo
    }
}

/// Serde-friendly color for the daemon-client wire format.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl From<Color> for AnsiColors {
    fn from(color: Color) -> Self {
        match color {
            Color::Black => Self::Black,
            Color::Red => Self::Red,
            Color::Green => Self::Green,
            Color::Yellow => Self::Yellow,
            Color::Blue => Self::Blue,
            Color::Magenta => Self::Magenta,
            Color::Cyan => Self::Cyan,
            Color::White => Self::White,
        }
    }
}
