use starship_plugin_sdk::{Ctx, VcsPlugin, export_vcs_plugin};

/// Stub VCS plugin used by runtime tests to exercise the `#[export_vcs_plugin]`
/// surface without depending on a real VCS like git.
///
/// `detect_depth` returns `Some(0)` when `.vcs-test-marker` is present in the
/// render's pwd, `None` otherwise, letting tests flip the gate at will.
#[derive(Default)]
struct VcsTestPlugin;

impl VcsPlugin for VcsTestPlugin {
    const NAME: &'static str = "vcs-test";
    const SHADOWS: &'static [&'static str] = &["other-vcs"];

    fn detect_depth(&self, ctx: &Ctx) -> Option<u32> {
        ctx.file_exists(".vcs-test-marker").then_some(0)
    }

    fn root(&self, _ctx: &Ctx) -> Option<String> {
        Some("/tmp/vcs-test".to_string())
    }

    fn branch(&self, _ctx: &Ctx) -> Option<String> {
        Some("main".to_string())
    }
}

#[export_vcs_plugin]
impl VcsTestPlugin {
    pub fn change_id(&self) -> String {
        "stub-change-id".to_string()
    }
}
