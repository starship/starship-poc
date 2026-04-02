use starship_plugin_sdk::export_plugin;

#[derive(Default)]
struct TestPlugin;

#[export_plugin]
impl TestPlugin {
    const NAME: &str = "test";

    pub fn value(&self) -> &str {
        "hello"
    }
}

fn main() {}
