use starship_plugin_sdk::{export_plugin, host};

#[derive(Default)]
struct NodejsPlugin;

#[export_plugin]
impl NodejsPlugin {
    const NAME: &str = "nodejs";

    pub fn version(&self) -> Option<String> {
        host::exec("node", &["--version"]).map(|v| v.trim().trim_start_matches('v').to_string())
    }
}
