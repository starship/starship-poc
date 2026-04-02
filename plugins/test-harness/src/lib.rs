use starship_plugin_sdk::{export_plugin, host, Plugin};

/// Test plugin that exercises all host APIs.
///
/// Used by runtime tests to verify host function implementations
/// without depending on external tools like `node`.
#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    const NAME: &str = "test";

    fn is_active(&self) -> bool {
        host::file_exists(".starship-test-marker")
    }
}

#[export_plugin]
impl TestPlugin {
    /// Reads `HOME` env var via `host::get_env`.
    pub fn home(&self) -> Option<String> {
        host::get_env("HOME")
    }

    /// Runs `pwd` via `host::exec`, returning the working directory.
    pub fn pwd(&self) -> Option<String> {
        host::exec("pwd", &[]).map(|s| s.trim().to_string())
    }
}
