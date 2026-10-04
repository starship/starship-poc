use starship_plugin_sdk::{Ctx, Plugin, export_plugin};

/// Test plugin that exercises every `Ctx` helper.
///
/// Its end-to-end tests and benches check process-level behavior (startup,
/// discovery, crashes, exit on EOF) against the real binary, without
/// depending on external tools like `node`.
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

    /// The shell's `USER`, from the render context.
    pub fn user(&self, ctx: &Ctx) -> Option<String> {
        ctx.env("USER").map(str::to_string)
    }

    /// The render's pwd, read without running anything.
    pub fn dir(&self, ctx: &Ctx) -> String {
        ctx.pwd().display().to_string()
    }

    /// Runs `pwd`, returning the directory commands run in.
    pub fn pwd(&self, ctx: &Ctx) -> Option<String> {
        ctx.exec_uncached("pwd", &[]).map(|s| s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use starship_plugin_sdk::assert_plugin;
    use starship_plugin_sdk::testing::{self, TestCtx};

    use super::TestPlugin;

    #[test]
    fn applies_when_the_marker_exists() {
        assert!(!testing::applicable(&TestPlugin, &TestCtx::new()));
        let marked = TestCtx::new().file(".starship-test-marker", "");
        assert!(testing::applicable(&TestPlugin, &marked));
    }

    #[test]
    fn reads_env_from_the_render() {
        let ctx = TestCtx::new()
            .env("HOME", "/home/shell")
            .env("USER", "shell");
        assert_plugin!(TestPlugin, ctx,
            "home" => Some("/home/shell"),
            "user" => Some("shell"),
        );
        assert_plugin!(TestPlugin, TestCtx::new(), "home" => None::<&str>);
    }

    #[test]
    fn commands_run_in_the_render_pwd() {
        let ctx = TestCtx::new();
        assert_plugin!(TestPlugin, ctx, "dir" => ctx.pwd());

        let pwd = testing::read(&TestPlugin, &ctx, "pwd");
        let pwd = std::fs::canonicalize(pwd.as_str().expect("pwd output")).unwrap();
        assert_eq!(pwd, std::fs::canonicalize(ctx.pwd()).unwrap());
    }
}
