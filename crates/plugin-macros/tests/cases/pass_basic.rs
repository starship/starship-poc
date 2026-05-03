use starship_plugin_sdk::{export_plugin, Plugin};

#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    const NAME: &str = "test";

    fn is_applicable(&self) -> bool {
        true
    }
}

#[export_plugin]
impl TestPlugin {
    pub fn value(&self) -> &str {
        "hello"
    }
}

fn main() {}
