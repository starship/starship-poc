use serde::{Deserialize, Serialize};

/// Represents an ANSI-formatted string.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StyledContent {
    Text(String),
    Styled { style: Style, children: Vec<Self> },
}

/// Serde-friendly style for the daemon-client wire format.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
    pub fn to_anstyle(&self) -> anstyle::Style {
        let mut effects = anstyle::Effects::new();
        if self.bold {
            effects = effects.insert(anstyle::Effects::BOLD);
        }
        if self.italic {
            effects = effects.insert(anstyle::Effects::ITALIC);
        }
        if self.underline {
            effects = effects.insert(anstyle::Effects::UNDERLINE);
        }
        if self.dimmed {
            effects = effects.insert(anstyle::Effects::DIMMED);
        }
        if self.strikethrough {
            effects = effects.insert(anstyle::Effects::STRIKETHROUGH);
        }

        anstyle::Style::new()
            .fg_color(self.fg.map(Into::into))
            .bg_color(self.bg.map(Into::into))
            .effects(effects)
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

impl From<Color> for anstyle::Color {
    fn from(color: Color) -> Self {
        Self::Ansi(match color {
            Color::Black => anstyle::AnsiColor::Black,
            Color::Red => anstyle::AnsiColor::Red,
            Color::Green => anstyle::AnsiColor::Green,
            Color::Yellow => anstyle::AnsiColor::Yellow,
            Color::Blue => anstyle::AnsiColor::Blue,
            Color::Magenta => anstyle::AnsiColor::Magenta,
            Color::Cyan => anstyle::AnsiColor::Cyan,
            Color::White => anstyle::AnsiColor::White,
        })
    }
}
