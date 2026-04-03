use anyhow::Result;
use mlua::{FromLua, Lua, MultiValue, Result as LuaResult, UserData, Value};
use starship_common::styled::{Color, Span, Style, StyledContent};

/// Registers the `compact` global: filters nil arguments and joins the
/// remaining elements with a space separator.
///
/// Uses varargs instead of a table because Lua tables silently discard
/// nil values, making nil-filtering impossible via table iteration.
///
/// ```lua
/// compact(green("node:", version), ctx.pwd, "❯")
/// ```
pub fn register_compact_function(lua: &Lua) -> Result<()> {
    lua.globals().set(
        "compact",
        lua.create_function(|_, args: MultiValue| {
            let mut spans: Vec<Span> = Vec::new();

            for value in args {
                if matches!(value, Value::Nil) {
                    continue;
                }
                if !spans.is_empty() {
                    spans.push(Span::plain(" ".to_string()));
                }
                match value {
                    Value::String(s) => {
                        spans.push(Span::plain(s.to_str()?.to_string()));
                    }
                    Value::UserData(ud) => {
                        let content = ud.borrow::<LuaStyledContent>()?;
                        spans.extend_from_slice(&content.0);
                    }
                    _ => {
                        return Err(mlua::Error::RuntimeError(
                            "compact: expected string, StyledContent, or nil".to_string(),
                        ))
                    }
                }
            }

            Ok(LuaStyledContent(spans))
        })?,
    )?;
    Ok(())
}

pub fn register_style_functions(lua: &Lua) -> Result<()> {
    let colors = [
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("yellow", Color::Yellow),
        ("blue", Color::Blue),
        ("magenta", Color::Magenta),
        ("cyan", Color::Cyan),
        ("white", Color::White),
    ];

    for (name, color) in colors {
        // Create foreground functions
        lua.globals()
            .set(name, create_color_fn(lua, color, false)?)?;
        // Create background functions
        lua.globals()
            .set(format!("bg_{name}"), create_color_fn(lua, color, true)?)?;
    }

    #[allow(clippy::type_complexity)]
    let effects: [(&str, fn(&mut Style)); 5] = [
        ("bold", |s| s.bold = true),
        ("italic", |s| s.italic = true),
        ("dimmed", |s| s.dimmed = true),
        ("underline", |s| s.underline = true),
        ("strikethrough", |s| s.strikethrough = true),
    ];

    for (name, apply) in effects {
        lua.globals().set(name, create_effect_fn(lua, apply)?)?;
    }

    Ok(())
}

fn create_color_fn(lua: &Lua, color: Color, bg: bool) -> Result<mlua::Function> {
    lua.create_function(move |_, args: MultiValue| {
        let Some(mut spans) = collect_children(args)? else {
            return Ok(None);
        };
        let parent = if bg {
            Style {
                bg: Some(color),
                ..Default::default()
            }
        } else {
            Style {
                fg: Some(color),
                ..Default::default()
            }
        };
        for span in &mut spans {
            span.style = std::mem::take(&mut span.style).merge(&parent);
        }
        Ok(Some(LuaStyledContent(spans)))
    })
    .map_err(anyhow::Error::from)
}

fn create_effect_fn(lua: &Lua, apply: fn(&mut Style)) -> Result<mlua::Function> {
    lua.create_function(move |_, args: MultiValue| {
        let Some(mut spans) = collect_children(args)? else {
            return Ok(None);
        };
        let mut parent = Style::default();
        apply(&mut parent);
        for span in &mut spans {
            span.style = std::mem::take(&mut span.style).merge(&parent);
        }
        Ok(Some(LuaStyledContent(spans)))
    })
    .map_err(anyhow::Error::from)
}

/// Collects arguments into styled spans. Returns `None` if any argument
/// is nil, which lets style functions propagate nil upward for use with `compact`.
fn collect_children(args: MultiValue) -> LuaResult<Option<Vec<Span>>> {
    let mut spans = Vec::new();
    for arg in args {
        match arg {
            Value::Nil => return Ok(None),
            Value::String(s) => spans.push(Span::plain(s.to_str()?.to_string())),
            Value::UserData(ud) => {
                let content = ud.borrow::<LuaStyledContent>()?;
                spans.extend_from_slice(&content.0);
            }
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "expected string, StyledContent, or nil".to_string(),
                ))
            }
        }
    }
    Ok(Some(spans))
}

/// Wrapper for styled spans, enabling them to be used in Lua.
pub struct LuaStyledContent(pub Vec<Span>);
impl UserData for LuaStyledContent {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Enables `..` between strings and styled content in Lua, in either order.
        // Plain strings are converted via `FromLua` before reaching this handler.
        methods.add_meta_method("__concat", |_, this, other: Self| {
            let mut spans = this.0.clone();
            spans.extend(other.0);
            Ok(Self(spans))
        });
    }
}
impl From<LuaStyledContent> for StyledContent {
    fn from(val: LuaStyledContent) -> Self {
        Self(val.0)
    }
}

impl FromLua for LuaStyledContent {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::String(s) => Ok(Self(vec![Span::plain(s.to_str()?.to_string())])),
            Value::UserData(ud) => {
                let content = ud.borrow::<Self>()?;
                Ok(Self(content.0.clone()))
            }
            Value::Table(table) => {
                let mut spans = Vec::new();
                for value in table.sequence_values::<Self>() {
                    spans.extend(value?.0);
                }
                Ok(Self(spans))
            }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "StyledContent".to_string(),
                message: Some("expected string, StyledContent, or array".to_string()),
            }),
        }
    }
}
