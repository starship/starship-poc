use serde::{Deserialize, Serialize};

/// Represents an ANSI-formatted string.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum StyledContent {
    Text(String),
    Styled { style: Style, children: Vec<Self> },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dimmed: bool,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
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
