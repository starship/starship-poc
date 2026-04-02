#[cfg(test)]
mod tests {
    use mlua::{Lua, LuaOptions, StdLib};

    #[test]
    fn sandboxed_luau_supports_index_metamethod() {
        let lua = Lua::new_with(StdLib::ALL_SAFE, LuaOptions::default()).unwrap();
        lua.sandbox(true).unwrap();
        let proxy = lua.create_table().unwrap();
        let meta = lua.create_table().unwrap();
        meta.set(
            "__index",
            lua.create_function(|_, (_table, key): (mlua::Table, String)| {
                Ok(format!("resolved:{key}"))
            })
            .unwrap(),
        )
        .unwrap();
        let _ = proxy.set_metatable(Some(meta));
        lua.globals().set("test_proxy", proxy).unwrap();
        let result: String = lua.load("return test_proxy.hello").eval().unwrap();
        assert_eq!(result, "resolved:hello");
    }
}
