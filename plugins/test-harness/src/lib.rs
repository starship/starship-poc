use starship_plugin_sdk::{export_plugin, host, Plugin};

/// Test plugin that exercises all host APIs.
///
/// Used by runtime tests to verify host function implementations
/// without depending on external tools like `node`.
#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    const NAME: &str = "test";

    fn is_applicable(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use starship_runtime::plugin_fixture;
    use std::fs;

    #[test]
    fn home_returns_env_var() {
        let mut plugin = plugin_fixture!();
        assert!(plugin.get("home").is_some());
    }

    #[test]
    fn pwd_returns_working_directory() {
        let mut plugin = plugin_fixture!();
        let pwd = plugin.get("pwd").expect("pwd should return a string");
        let actual = fs::canonicalize(&pwd).expect("pwd output resolves");
        let expected = fs::canonicalize(&plugin.dir).expect("tempdir path resolves");
        assert_eq!(actual, expected);
    }

    #[test]
    fn inapplicable_without_marker() {
        let mut plugin = plugin_fixture!();
        assert!(!plugin.is_applicable());
    }

    #[test]
    fn applicable_with_marker() {
        let mut plugin = plugin_fixture!();
        fs::write(plugin.dir.join(".starship-test-marker"), "").unwrap();
        assert!(plugin.is_applicable());
    }
}
