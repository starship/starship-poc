use starship_plugin_sdk::{Ctx, Plugin, export_plugin};

/// Test plugin that exercises every `Ctx` helper.
///
/// Used by runtime tests to check what plugins can see of the render,
/// without depending on external tools like `node`.
#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    const NAME: &str = "test";

    fn is_applicable(&self, ctx: &Ctx) -> bool {
        ctx.file_exists(".starship-test-marker")
    }
}

#[export_plugin]
impl TestPlugin {
    /// The shell's `HOME`, from the render context.
    pub fn home(&self, ctx: &Ctx) -> Option<String> {
        ctx.env("HOME").map(str::to_string)
    }

    /// Runs `pwd`, returning the directory commands run in.
    pub fn pwd(&self, ctx: &Ctx) -> Option<String> {
        ctx.exec_uncached("pwd", &[]).map(|s| s.trim().to_string())
    }
}
