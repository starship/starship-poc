use anyhow::Result;
use mlua::{FromLua, Lua, MultiValue, Result as LuaResult, UserData, Value};
use starship_common::styled::{Color, Style, StyledContent};

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
            let mut children: Vec<StyledContent> = Vec::new();

            for value in args {
                if matches!(value, Value::Nil) {
                    continue;
                }
                if !children.is_empty() {
                    children.push(StyledContent::Text(" ".to_string()));
                }
                match value {
                    Value::String(s) => {
                        children.push(StyledContent::Text(s.to_str()?.to_string()));
                    }
                    Value::UserData(ud) => {
                        let content = ud.borrow::<LuaStyledContent>()?;
                        children.push(content.0.clone());
                    }
                    _ => {
                        return Err(mlua::Error::RuntimeError(
                            "compact: expected string, StyledContent, or nil".to_string(),
                        ))
                    }
                }
            }

            if children.len() <= 1 {
                let content = children
                    .into_iter()
                    .next()
                    .unwrap_or(StyledContent::Text(String::new()));
                return Ok(LuaStyledContent(content));
            }
            Ok(LuaStyledContent(StyledContent::Styled {
                style: Style::default(),
                children,
            }))
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
        let Some(children) = collect_children(args)? else {
            return Ok(None);
        };
        let style = if bg {
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
        Ok(Some(LuaStyledContent(StyledContent::Styled {
            style,
            children,
        })))
    })
    .map_err(anyhow::Error::from)
}

fn create_effect_fn(lua: &Lua, apply: fn(&mut Style)) -> Result<mlua::Function> {
    lua.create_function(move |_, args: MultiValue| {
        let Some(children) = collect_children(args)? else {
            return Ok(None);
        };
        let mut style = Style::default();
        apply(&mut style);
        Ok(Some(LuaStyledContent(StyledContent::Styled {
            style,
            children,
        })))
    })
    .map_err(anyhow::Error::from)
}

/// Collects arguments into styled children. Returns `None` if any argument
/// is nil, which lets style functions propagate nil upward for use with `compact`.
fn collect_children(args: MultiValue) -> LuaResult<Option<Vec<StyledContent>>> {
    let mut children = Vec::new();
    for arg in args {
        match arg {
            Value::Nil => return Ok(None),
            Value::String(s) => children.push(StyledContent::Text(s.to_str()?.to_string())),
            Value::UserData(ud) => {
                let content = ud.borrow::<LuaStyledContent>()?;
                children.push(content.0.clone());
            }
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "expected string, StyledContent, or nil".to_string(),
                ))
            }
        }
    }
    Ok(Some(children))
}

/// Wrapper for `StyledContent`, enabling it to be used in Lua.
pub struct LuaStyledContent(pub StyledContent);
impl UserData for LuaStyledContent {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Enables `..` between strings and styled content in Lua, in either order.
        // Plain strings are converted via `FromLua` before reaching this handler.
        // Wraps both sides in an unstyled parent node to preserve the styled tree.
        methods.add_meta_method("__concat", |_, this, other: Self| {
            Ok(Self(StyledContent::Styled {
                style: Style::default(),
                children: vec![this.0.clone(), other.0],
            }))
        });
    }
}
impl From<LuaStyledContent> for StyledContent {
    fn from(val: LuaStyledContent) -> Self {
        val.0
    }
}

impl FromLua for LuaStyledContent {
    fn from_lua(value: Value, _lua: &Lua) -> LuaResult<Self> {
        match value {
            Value::String(s) => Ok(Self(StyledContent::Text(s.to_str()?.to_string()))),
            Value::UserData(ud) => {
                let content = ud.borrow::<Self>()?;
                Ok(Self(content.0.clone()))
            }
            Value::Table(table) => {
                let children: Vec<StyledContent> = table
                    .sequence_values::<Self>()
                    .map(|value| value.map(|content| content.0))
                    .collect::<LuaResult<_>>()?;
                Ok(Self(StyledContent::Styled {
                    style: Style::default(),
                    children,
                }))
            }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "StyledContent".to_string(),
                message: Some("expected string, StyledContent, or array".to_string()),
            }),
        }
    }
}
