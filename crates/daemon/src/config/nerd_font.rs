use std::sync::LazyLock;

use anyhow::Result;
use mlua::Lua;
use nerd_fonts::NerdFonts;

static NERD_FONTS: LazyLock<NerdFonts> = LazyLock::new(|| NerdFonts {
    nf: NerdFonts::load(),
});

pub fn register_icon_function(lua: &Lua) -> Result<()> {
    lua.globals().set("icon", create_icon_fn(lua)?)?;
    Ok(())
}

fn create_icon_fn(lua: &Lua) -> Result<mlua::Function> {
    lua.create_function(move |_, icon_name: String| Ok(NERD_FONTS.get(&icon_name)))
        .map_err(anyhow::Error::from)
}
