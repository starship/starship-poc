//! The nodejs plugin's real binary, driven through the daemon's plugin client.

use starship_plugin_sdk::test::{self, Dir, Env};
use starship_runtime::plugin::test_helpers::PluginFixture;

#[test]
fn binary_reports_the_version_for_a_node_project() {
    // The binary runs commands for real, so put a fake `node` on its PATH.
    let dir = Dir::new()
        .file("package.json", "{}")
        .executable("bin/node", "v20.1.0\n");
    let env = Env::new().var("PATH", dir.path("bin").to_str().unwrap());

    let mut plugin = PluginFixture::from_binary(env!("CARGO_BIN_EXE_starship-plugin-nodejs"));
    let process = plugin.process();

    assert!(
        process
            .begin_render(1, &test::context(&dir, &env))
            .applicable
    );
    assert_eq!(process.call_method(1, "version"), "20.1.0");
}
