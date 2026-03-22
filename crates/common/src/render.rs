use crate::styled::StyledContent;

/// Render the structured prompt representation into a string.
pub fn render_prompt(prompt: &StyledContent) -> String {
    match prompt {
        StyledContent::Text(text) => text.clone(),
        StyledContent::Styled {
            style, children, ..
        } => {
            let ansi = style.to_anstyle();
            let content: String = children.iter().map(render_prompt).collect();
            // {style} emits ANSI open codes, {style:#} emits reset
            format!("{ansi}{content}{ansi:#}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styled::{Color, Style};

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

    #[test]
    fn bold_and_fg_render_together() {
        let style = Style {
            fg: Some(Color::Green),
            bold: true,
            ..Default::default()
        };
        let ansi = style.to_anstyle();
        let styled = StyledContent::Styled {
            style,
            children: vec![StyledContent::Text("ok".into())],
        };
        assert_eq!(render_prompt(&styled), format!("{ansi}ok{ansi:#}"));
    }

    #[test]
    fn all_effects_render() {
        let style = Style {
            fg: Some(Color::Cyan),
            bg: Some(Color::Black),
            bold: true,
            italic: true,
            underline: true,
            dimmed: true,
            strikethrough: true,
        };
        let ansi = style.to_anstyle();
        let styled = StyledContent::Styled {
            style,
            children: vec![StyledContent::Text("x".into())],
        };
        assert_eq!(render_prompt(&styled), format!("{ansi}x{ansi:#}"));
    }

    #[test]
    fn unstyled_node_passes_through() {
        let styled = StyledContent::Styled {
            style: Style::default(),
            children: vec![StyledContent::Text("bare".into())],
        };
        assert_eq!(render_prompt(&styled), "bare");
    }
}
