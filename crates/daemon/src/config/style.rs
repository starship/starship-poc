use anyhow::Result;
use mlua::{FromLua, Lua, MultiValue, Result as LuaResult, UserData, Value};
use starship_common::styled::{Color, Style, StyledContent};

pub fn register_style_functions(lua: &Lua) -> Result<()> {
    // Colors
    lua.globals()
        .set("black", create_fg_fn(lua, Color::Black)?)?;
    lua.globals().set("red", create_fg_fn(lua, Color::Red)?)?;
    lua.globals()
        .set("green", create_fg_fn(lua, Color::Green)?)?;
    lua.globals()
        .set("yellow", create_fg_fn(lua, Color::Yellow)?)?;
    lua.globals().set("blue", create_fg_fn(lua, Color::Blue)?)?;
    lua.globals()
        .set("magenta", create_fg_fn(lua, Color::Magenta)?)?;
    lua.globals().set("cyan", create_fg_fn(lua, Color::Cyan)?)?;
    lua.globals()
        .set("white", create_fg_fn(lua, Color::White)?)?;

    Ok(())
}

fn create_fg_fn(lua: &Lua, color: Color) -> Result<mlua::Function> {
    lua.create_function(move |_, args: MultiValue| {
        let children = collect_children(args)?;
        Ok(LuaStyledContent(StyledContent::Styled {
            style: Style {
                fg: Some(color),
                ..Default::default()
            },
            children,
        }))
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
            _ => Err(mlua::Error::RuntimeError("expected string".to_string())),
        })
        .collect()
}

/// Wrapper for `StyledContent`, enabling it to be used in Lua.
pub struct LuaStyledContent(pub StyledContent);
impl UserData for LuaStyledContent {}
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
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "StyledContent".to_string(),
                message: Some("expected string or StyledContent".to_string()),
            }),
        }
    }
}
