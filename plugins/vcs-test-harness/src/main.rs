use starship_plugin_sdk::{Ctx, VcsPlugin, export_vcs_plugin};

/// Stub VCS plugin that exercises the `#[export_vcs_plugin]` surface without
/// depending on a real VCS like git.
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

#[cfg(test)]
mod tests {
    use starship_plugin_sdk::test::prelude::*;

    use super::VcsTestPlugin;

    #[test]
    fn detects_at_depth_zero_with_the_marker() {
        let dir = Dir::new().file(".vcs-test-marker", "");

        let empty = render!(VcsTestPlugin);
        let marked = render!(VcsTestPlugin { dir });

        assert_eq!(empty.depth(), None);
        assert_eq!(marked.depth(), Some(0));
        assert!(marked.applicable());
    }

    #[test]
    fn routes_trait_and_inherent_methods() {
        let render = render!(VcsTestPlugin);

        assert_eq!(render.read("root"), "/tmp/vcs-test");
        assert_eq!(render.read("branch"), "main");
        assert_eq!(render.read("change_id"), "stub-change-id");
    }
}
