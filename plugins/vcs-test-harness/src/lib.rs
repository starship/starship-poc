use starship_plugin_sdk::{export_vcs_plugin, host, VcsPlugin};

/// Stub VCS plugin used by runtime tests to exercise the `#[export_vcs_plugin]`
/// ABI surface without depending on a real VCS like git.
///
/// `detect_depth` returns `Some(0)` when `.vcs-test-marker` is present in the
/// plugin's working directory, `None` otherwise — letting tests flip the gate
/// at will.
#[derive(Default)]
struct VcsTestPlugin;

impl VcsPlugin for VcsTestPlugin {
    const NAME: &'static str = "vcs-test";
    const SHADOWS: &'static [&'static str] = &["other-vcs"];

    fn detect_depth(&self) -> Option<u32> {
        if host::file_exists(".vcs-test-marker") {
            Some(0)
        } else {
            None
        }
    }

    fn root(&self) -> Option<String> {
        Some("/tmp/vcs-test".to_string())
    }

    fn branch(&self) -> Option<String> {
        Some("main".to_string())
    }
}

#[export_vcs_plugin]
impl VcsTestPlugin {
    pub fn change_id(&self) -> Option<String> {
        Some("stub-change-id".to_string())
    }
}
