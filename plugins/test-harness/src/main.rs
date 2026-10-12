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
    use starship_plugin_sdk::test::prelude::*;

    use super::TestPlugin;

    #[test]
    fn applies_when_the_marker_exists() {
        let dir = Dir::new().file(".starship-test-marker", "");

        let empty = render!(TestPlugin);
        let marked = render!(TestPlugin { dir });

        assert!(!empty.applicable());
        assert!(marked.applicable());
    }

    #[test]
    fn reads_env_from_the_render() {
        let env = Env::new().var("HOME", "/home/shell").var("USER", "shell");

        let with_env = render!(TestPlugin { env });
        let without_env = render!(TestPlugin);

        assert_eq!(with_env.read("home"), "/home/shell");
        assert_eq!(with_env.read("user"), "shell");
        assert!(without_env.read("home").is_null());
    }

    #[test]
    fn reports_the_render_pwd() {
        let dir = Dir::new().dir("src");

        let render = render!(TestPlugin { dir, pwd: "src" });

        assert_eq!(render.read("dir"), render.path("src").to_str().unwrap());
    }
}
