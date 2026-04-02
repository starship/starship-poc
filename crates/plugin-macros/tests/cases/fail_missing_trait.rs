use starship_plugin_sdk::export_plugin;

#[derive(Default)]
struct BadPlugin;

#[export_plugin]
impl BadPlugin {
    pub fn value(&self) -> &str {
        "hello"
    }
}

fn main() {}
