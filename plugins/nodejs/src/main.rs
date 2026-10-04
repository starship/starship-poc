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

#[cfg(test)]
mod tests {
    use starship_plugin_sdk::assert_plugin;
    use starship_plugin_sdk::testing::{self, TestCtx};

    use super::NodejsPlugin;

    #[test]
    fn applies_when_package_json_exists() {
        assert!(!testing::applicable(&NodejsPlugin, &TestCtx::new()));
        let project = TestCtx::new().file("package.json", "{}");
        assert!(testing::applicable(&NodejsPlugin, &project));
    }

    #[test]
    fn reads_the_version_without_the_v() {
        let ctx = TestCtx::new().command("node", "v20.1.0");
        assert_plugin!(NodejsPlugin, ctx, "version" => Some("20.1.0"));
    }

    #[test]
    fn version_is_nil_without_node() {
        let ctx = TestCtx::new().env("PATH", "/nonexistent");
        assert_plugin!(NodejsPlugin, ctx, "version" => None::<&str>);
    }
}
