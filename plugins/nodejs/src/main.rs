use starship_plugin_sdk::{Ctx, Plugin, export_plugin};

#[derive(Default)]
struct NodejsPlugin;

impl Plugin for NodejsPlugin {
    const NAME: &str = "nodejs";

    fn is_applicable(&self, ctx: &Ctx) -> bool {
        ctx.file_exists("package.json")
    }
}

#[export_plugin]
impl NodejsPlugin {
    pub fn version(&self, ctx: &Ctx) -> Option<String> {
        ctx.exec("node", &["--version"])
            .map(|v| v.trim().trim_start_matches('v').to_string())
    }
}
