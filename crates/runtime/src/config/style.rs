use anyhow::Result;
use mlua::{FromLua, Lua, MultiValue, Result as LuaResult, UserData, Value};
use starship_common::styled::{Color, Style, StyledContent};

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
        let children = collect_children(args)?;
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
        Ok(LuaStyledContent(StyledContent::Styled { style, children }))
    })
    .map_err(anyhow::Error::from)
}

fn create_effect_fn(lua: &Lua, apply: fn(&mut Style)) -> Result<mlua::Function> {
    lua.create_function(move |_, args: MultiValue| {
        let children = collect_children(args)?;
        let mut style = Style::default();
        apply(&mut style);
        Ok(LuaStyledContent(StyledContent::Styled { style, children }))
    })
    .map_err(anyhow::Error::from)
}

fn collect_children(args: MultiValue) -> LuaResult<Vec<StyledContent>> {
    args.into_iter()
        .map(|arg| match arg {
            Value::String(s) => Ok(StyledContent::Text(s.to_str()?.to_string())),
            Value::UserData(ud) => {
                let content = ud.borrow::<LuaStyledContent>()?;
                Ok(content.0.clone())
            }
            _ => Err(mlua::Error::RuntimeError(
                "expected string or StyledContent".to_string(),
            )),
        })
        .collect()
}

/// Wrapper for `StyledContent`, enabling it to be used in Lua.
pub struct LuaStyledContent(pub StyledContent);
impl UserData for LuaStyledContent {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__concat", |_, this, other: Value| {
            let right = match other {
                Value::String(s) => StyledContent::Text(s.to_str()?.to_string()),
                Value::UserData(ud) => ud.borrow::<LuaStyledContent>()?.0.clone(),
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "cannot concatenate with non-string/StyledContent".to_string(),
                    ))
                }
            };
            Ok(LuaStyledContent(StyledContent::Styled {
                style: Style::default(),
                children: vec![this.0.clone(), right],
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
