use crate::styled::StyledContent;
use owo_colors::OwoColorize;

/// Render a plain string with an `owo_colors::Style`, returning the ANSI-wrapped result.
#[must_use]
pub fn paint(text: &str, style: owo_colors::Style) -> String {
    format!("{}", style.style(text))
}

/// Render the structured prompt representation into a string.
pub fn render_prompt(prompt: &StyledContent) -> String {
    match prompt {
        StyledContent::Text(text) => text.clone(),
        StyledContent::Styled { style, children } => {
            let owo = style.to_owo();
            let content: String = children.iter().map(render_prompt).collect();
            if owo.is_plain() {
                content
            } else {
                format!("{}", content.style(owo))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::paint;
    use super::*;
    use crate::styled::{Color, Style};
    use owo_colors::style;

    #[test]
    fn styled_text_renders_to_ansi_escape_codes() {
        let styled = StyledContent::Styled {
            style: Style {
                fg: Some(Color::Red),
                ..Default::default()
            },
            children: vec![StyledContent::Text("error".into())],
        };
        assert_eq!(render_prompt(&styled), paint("error", style().red()));
    }

    #[test]
    fn plain_text_renders_unchanged() {
        let text = StyledContent::Text("hello".into());
        assert_eq!(render_prompt(&text), "hello");
    }

    #[test]
    fn bold_and_fg_render_together() {
        let styled = StyledContent::Styled {
            style: Style {
                fg: Some(Color::Green),
                bold: true,
                ..Default::default()
            },
            children: vec![StyledContent::Text("ok".into())],
        };
        assert_eq!(render_prompt(&styled), paint("ok", style().green().bold()),);
    }

    #[test]
    fn all_effects_render() {
        let styled = StyledContent::Styled {
            style: Style {
                fg: Some(Color::Cyan),
                bg: Some(Color::Black),
                bold: true,
                italic: true,
                underline: true,
                dimmed: true,
                strikethrough: true,
            },
            children: vec![StyledContent::Text("x".into())],
        };
        assert_eq!(
            render_prompt(&styled),
            paint(
                "x",
                style()
                    .cyan()
                    .on_black()
                    .bold()
                    .italic()
                    .underline()
                    .dimmed()
                    .strikethrough()
            ),
        );
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
