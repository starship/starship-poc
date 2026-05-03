use starship_plugin_sdk::{export_vcs_plugin, VcsPlugin};

#[derive(Default)]
struct StubVcs;

impl VcsPlugin for StubVcs {
    const NAME: &'static str = "stub";
    const SHADOWS: &'static [&'static str] = &["other"];

    fn detect_depth(&self) -> Option<u32> {
        Some(0)
    }

    fn root(&self) -> Option<String> {
        Some("/tmp".to_string())
    }

    fn branch(&self) -> Option<String> {
        Some("main".to_string())
    }
}

#[export_vcs_plugin]
impl StubVcs {
    pub fn extra(&self) -> Option<String> {
        Some("extra".to_string())
    }
}

fn main() {}
