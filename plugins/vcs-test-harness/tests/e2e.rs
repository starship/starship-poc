//! The stub VCS plugin's real binary, driven through the daemon's plugin client.

use std::fs;

use starship_runtime::plugin::PluginKind;
use starship_runtime::plugin::test_helpers::PluginFixture;

#[test]
fn binary_reports_a_vcs_manifest_and_depth() {
    let mut plugin =
        PluginFixture::from_binary(env!("CARGO_BIN_EXE_starship-plugin-vcs-test-harness"));
    assert_eq!(plugin.name(), "vcs-test");
    assert_eq!(plugin.kind(), PluginKind::Vcs);
    assert_eq!(plugin.shadows(), ["other-vcs"]);

    assert_eq!(plugin.detect_depth(), None);
    fs::write(plugin.dir.join(".vcs-test-marker"), "").unwrap();
    assert_eq!(plugin.detect_depth(), Some(0));
    assert_eq!(plugin.get("branch").as_deref(), Some("main"));
}
