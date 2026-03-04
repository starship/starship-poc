use anyhow::Result;
use mlua::Lua;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub fn register_icon_function(lua: &Lua) -> Result<()> {
    let icon_fn = lua.create_function(|_, name: String| Ok(ICONS.get(name.as_str()).copied()))?;
    lua.globals().set("icon", icon_fn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_map_contains_known_glyphs() {
        assert_eq!(ICONS.get("cod-git_commit"), Some(&"\u{eafc}"));
    }
}
