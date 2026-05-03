use starship_plugin_sdk::{export_plugin, host, Plugin};

#[derive(Default)]
struct NodejsPlugin;

impl Plugin for NodejsPlugin {
    const NAME: &str = "nodejs";

    fn is_applicable(&self) -> bool {
        host::file_exists("package.json")
    }
}

#[export_plugin]
impl NodejsPlugin {
    pub fn version(&self) -> Option<String> {
        host::exec("node", &["--version"]).map(|v| v.trim().trim_start_matches('v').to_string())
    }
}
