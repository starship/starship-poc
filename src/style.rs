/// A style is a collection of properties that can format a string
/// using ANSI escape codes.
#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub struct Style {
    /// The style's foreground color, if it has one.
    pub foreground: Option<Color>,

    /// The style's background color, if it has one.
    pub background: Option<Color>,

    /// Whether this style is bold.
    pub is_bold: bool,

    /// Whether this style is dimmed.
    pub is_dimmed: bool,

    /// Whether this style is italicized.
    pub is_italicized: bool,

    /// Whether this style is underlined
    pub is_underlined: bool,
}

impl Style {
    /// Creates a new Style with no properties set.
    pub fn new() -> Style {
        Style::default()
    }

    /// Returns a `Style` with the bold property set.
    pub fn bold(&self) -> Style {
        Style {
            is_bold: true,
            ..*self
        }
    }

    /// Returns a `Style` with the dimmed property set.
    pub fn dimmed(&self) -> Style {
        Style {
            is_dimmed: true,
            ..*self
        }
    }

    /// Returns a `Style` with the italic property set.
    pub fn italic(&self) -> Style {
        Style {
            is_italicized: true,
            ..*self
        }
    }

    /// Returns a `Style` with the underline property set.
    pub fn underline(&self) -> Style {
        Style {
            is_underlined: true,
            ..*self
        }
    }

    /// Returns a `Style` with the foreground color property set.
    pub fn fg(&self, foreground: Color) -> Style {
        Style {
            foreground: Some(foreground),
            ..*self
        }
    }

    /// Returns a `Style` with the background color property set.
    pub fn on(&self, background: Color) -> Style {
        Style {
            background: Some(background),
            ..*self
        }
    }

    /// Return true if this `Style` has no actual styles, and can be written
    /// without any control characters.
    pub fn is_plain(self) -> bool {
        self == Style::default()
    }
}

/// A color is one specific type of ANSI escape code, and can refer
/// to either the foreground or background color.
///
/// These use the standard numeric sequences.
/// See <http://invisible-island.net/xterm/ctlseqs/ctlseqs.html>
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Color {
    /// Color #0 (foreground code `30`, background code `40`).
    ///
    /// This is not necessarily the background color, and using it as one may
    /// render the text hard to read on terminals with dark backgrounds.
    Black,

    /// Color #1 (foreground code `31`, background code `41`).
    Red,

    /// Color #2 (foreground code `32`, background code `42`).
    Green,

    /// Color #3 (foreground code `33`, background code `43`).
    Yellow,

    /// Color #4 (foreground code `34`, background code `44`).
    Blue,

    /// Color #5 (foreground code `35`, background code `45`).
    Purple,

    /// Color #6 (foreground code `36`, background code `46`).
    Cyan,

    /// Color #7 (foreground code `37`, background code `47`).
    ///
    /// As above, this is not necessarily the foreground color, and may be
    /// hard to read on terminals with light backgrounds.
    White,

    /// A color number from 0 to 255, for use in 256-color terminal
    /// environments.
    ///
    /// - Colors 0 to 7 are the `Black` to `White` variants respectively.
    ///   These colors can usually be changed in the terminal emulator.
    /// - Colors 8 to 15 are brighter versions of the eight colors above.
    ///   These can also usually be changed in the terminal emulator, or it
    ///   could be configured to use the original colors and show the text in
    ///   bold instead. It varies depending on the program.
    /// - Colors 16 to 231 contain several palettes of bright colors,
    ///   arranged in six squares measuring six by six each.
    /// - Colors 232 to 255 are shades of grey from black to white.
    ///
    /// It might make more sense to look at a [color chart][cc].
    ///
    /// [cc]: https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg
    Fixed(u8),

    /// A 24-bit RGB color, as specified by ISO-8613-3.
    RGB(u8, u8, u8),
}

impl Color {
    /// Returns a `Style` with the foreground color set to this color.
    pub fn normal(self) -> Style {
        Style {
            foreground: Some(self),
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// bold property set.
    pub fn bold(self) -> Style {
        Style {
            foreground: Some(self),
            is_bold: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// dimmed property set.
    pub fn dimmed(self) -> Style {
        Style {
            foreground: Some(self),
            is_dimmed: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// italic property set.
    pub fn italic(self) -> Style {
        Style {
            foreground: Some(self),
            is_italicized: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// underline property set.
    pub fn underline(self) -> Style {
        Style {
            foreground: Some(self),
            is_underlined: true,
            ..Style::default()
        }
    }

    /// Returns a `Style` with the foreground color set to this color and the
    /// background color property set to the given color.
    pub fn on(self, background: Color) -> Style {
        Style {
            foreground: Some(self),
            background: Some(background),
            ..Style::default()
        }
    }
}

impl From<Color> for Style {
    fn from(color: Color) -> Style {
        color.normal()
    }
}
