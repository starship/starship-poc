/// Render a plain string with an `owo_colors::Style`, returning the ANSI-wrapped result.
#[must_use]
pub fn paint(text: &str, style: owo_colors::Style) -> String {
    format!("{}", style.style(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styled::{Color, Span, Style, StyledContent};
    use owo_colors::style;

    #[test]
    fn styled_text_renders_to_ansi_escape_codes() {
        let styled = StyledContent(vec![Span {
            text: "error".into(),
            style: Style {
                fg: Some(Color::Red),
                ..Default::default()
            },
        }]);
        assert_eq!(styled.to_string(), paint("error", style().red()));
    }

    #[test]
    fn plain_text_renders_unchanged() {
        let text = StyledContent(vec![Span::plain("hello".into())]);
        assert_eq!(text.to_string(), "hello");
    }

    #[test]
    fn bold_and_fg_render_together() {
        let styled = StyledContent(vec![Span {
            text: "ok".into(),
            style: Style {
                fg: Some(Color::Green),
                bold: true,
                ..Default::default()
            },
        }]);
        assert_eq!(styled.to_string(), paint("ok", style().green().bold()));
    }

    #[test]
    fn all_effects_render() {
        let styled = StyledContent(vec![Span {
            text: "x".into(),
            style: Style {
                fg: Some(Color::Cyan),
                bg: Some(Color::Black),
                bold: true,
                italic: true,
                underline: true,
                dimmed: true,
                strikethrough: true,
            },
        }]);
        assert_eq!(
            styled.to_string(),
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
    fn unstyled_span_passes_through() {
        let styled = StyledContent(vec![Span::plain("bare".into())]);
        assert_eq!(styled.to_string(), "bare");
    }
}
