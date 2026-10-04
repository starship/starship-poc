use starship_plugin_sdk::{export_plugin, Ctx, Plugin};

#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    const NAME: &str = "test";

    fn is_applicable(&self, _ctx: &Ctx) -> bool {
        true
    }
}

#[export_plugin]
impl TestPlugin {
    pub fn value(&self) -> &str {
        "hello"
    }

    pub fn pwd(&self, ctx: &Ctx) -> String {
        ctx.pwd().display().to_string()
    }
}

fn main() {}
