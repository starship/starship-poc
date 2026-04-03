use std::fmt;

use owo_colors::{AnsiColors, DynColors};
use serde::{Deserialize, Serialize};

/// A single text span with fully resolved styling.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    #[serde(default, skip_serializing_if = "Style::is_plain")]
    pub style: Style,
}

impl Span {
    #[must_use]
    pub fn plain(text: String) -> Self {
        Self {
            text,
            style: Style::default(),
        }
    }
}

/// A rendered prompt as a flat sequence of styled spans.
///
/// Styles are fully resolved at construction time — rendering is a
/// single pass with no recursion or intermediate allocations.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct StyledContent(pub Vec<Span>);

impl fmt::Display for StyledContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for span in &self.0 {
            let owo = span.style.to_owo();
            if owo.is_plain() {
                f.write_str(&span.text)?;
            } else {
                owo.fmt_prefix(f)?;
                f.write_str(&span.text)?;
                owo.fmt_suffix(f)?;
            }
        }
        Ok(())
    }
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
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// Merge with a parent style. Self (inner) takes precedence for colors;
    /// boolean effects are combined with OR.
    #[must_use]
    pub fn merge(mut self, parent: &Style) -> Style {
        self.fg = self.fg.or(parent.fg);
        self.bg = self.bg.or(parent.bg);
        self.bold = self.bold || parent.bold;
        self.italic = self.italic || parent.italic;
        self.dimmed = self.dimmed || parent.dimmed;
        self.underline = self.underline || parent.underline;
        self.strikethrough = self.strikethrough || parent.strikethrough;
        self
    }

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
