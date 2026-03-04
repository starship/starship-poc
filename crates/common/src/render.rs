use crate::styled::{Color, StyledContent};

/// Render the structured prompt representation into a string.
pub fn render_prompt(prompt: &StyledContent) -> String {
    match prompt {
        StyledContent::Text(text) => text.clone(),
        StyledContent::Styled {
            style, children, ..
        } => {
            let content = children.iter().map(render_prompt).collect();

            if let Some(color) = style.fg {
                format!("\x1b[{}m{}\x1b[0m", fg_color(color), content)
            } else {
                content
            }
        }
    }
}

const fn fg_color(color: Color) -> &'static str {
    match color {
        Color::Black => "30",
        Color::Red => "31",
        Color::Green => "32",
        Color::Yellow => "33",
        Color::Blue => "34",
        Color::Magenta => "35",
        Color::Cyan => "36",
        Color::White => "37",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styled::Style;

    #[test]
    fn styled_text_renders_to_ansi_escape_codes() {
        let styled = StyledContent::Styled {
            style: Style {
                fg: Some(Color::Red),
                ..Default::default()
            },
            children: vec![StyledContent::Text("error".into())],
        };
        assert_eq!(render_prompt(&styled), "\x1b[31merror\x1b[0m");
    }

    #[test]
    fn plain_text_renders_unchanged() {
        let text = StyledContent::Text("hello".into());
        assert_eq!(render_prompt(&text), "hello");
    }
}
