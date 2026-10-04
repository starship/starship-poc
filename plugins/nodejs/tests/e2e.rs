//! The nodejs plugin's real binary, driven through the daemon's plugin client.

use starship_plugin_sdk::testing::TestCtx;
use starship_runtime::plugin::test_helpers::PluginFixture;

#[test]
fn binary_reports_the_version_for_a_node_project() {
    let mut plugin = PluginFixture::from_binary(env!("CARGO_BIN_EXE_starship-plugin-nodejs"));
    assert_eq!(plugin.name(), "nodejs");

    let ctx = TestCtx::new()
        .file("package.json", "{}")
        .command("node", "v20.1.0");
    let process = plugin.process();
    assert!(process.begin_render(1, &ctx.context()).applicable);
    assert_eq!(process.call_method(1, "version"), "20.1.0");
}
