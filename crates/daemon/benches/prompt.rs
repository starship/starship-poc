use divan::{AllocProfiler, Bencher, black_box};
use starship_common::{Module, Prompt, ShellContext};
use std::path::PathBuf;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

fn handle_request(context: ShellContext) -> Prompt {
    let user = context.user.unwrap_or_default();
    let pwd = context
        .pwd
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Prompt {
        left: vec![
            Module {
                name: "user".into(),
                output: user.into(),
            },
            Module {
                name: "directory".into(),
                output: pwd.into(),
            },
        ],
        right: vec![],
    }
}

#[divan::bench]
fn prompt(bencher: Bencher) {
    let json = serde_json::to_string(&ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    })
    .unwrap();

    bencher.bench(|| {
        let context: ShellContext = serde_json::from_str(black_box(&json)).unwrap();
        let prompt = handle_request(context);
        serde_json::to_string(&prompt).unwrap();
    });
}
