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
    use starship_plugin_sdk::test::prelude::*;

    use super::NodejsPlugin;

    #[test]
    fn applies_when_package_json_exists() {
        let dir = Dir::new().file("package.json", "{}");

        let empty = render!(NodejsPlugin);
        let project = render!(NodejsPlugin { dir });

        assert!(!empty.applicable());
        assert!(project.applicable());
    }

    #[test]
    fn reads_the_version_without_the_v() {
        let dir = Dir::new().file("package.json", "{}");
        let commands = Commands::new().ok("node --version", "v20.1.0\n");

        let render = render!(NodejsPlugin { dir, commands });

        assert_eq!(render.read("version"), "20.1.0");
    }

    #[test]
    fn version_is_nil_when_node_is_missing() {
        let dir = Dir::new().file("package.json", "{}");
        let commands = Commands::new().fails("node --version");

        let render = render!(NodejsPlugin { dir, commands });

        assert!(render.read("version").is_null());
    }
}
