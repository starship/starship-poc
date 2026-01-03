use std::path::PathBuf;
use anyhow::Result;
use divan::{black_box, AllocProfiler, Bencher};
use starship_common::{Module, Prompt, ShellContext};

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
fn full_flow(bencher: Bencher) {
    let json = serde_json::to_string(&ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    }).unwrap();

    bencher.bench(|| {
        let context: ShellContext = serde_json::from_str(black_box(&json)).unwrap();
        let prompt = handle_request(context);
        serde_json::to_string(&prompt).unwrap();
    });

}

#[divan::bench]
fn parse_request(bencher: Bencher) {
    let json = r#"{"pwd":"/Users/test/projects/starship","user":"testuser"}"#;

    bencher.bench(|| {
        let _: ShellContext = serde_json::from_str(black_box(json)).unwrap();
    });
}

#[divan::bench]
fn handle_request_only(bencher: Bencher) {
    let context = ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    };

    bencher
        .with_inputs(|| context.clone())
        .bench_values(|ctx| handle_request(ctx));
}

#[divan::bench]
fn serialize_response(bencher: Bencher) {
    let prompt = Prompt {
        left: vec![
            Module {
                name: "user".into(),
                output: "testuser".into(),
            },
            Module {
                name: "directory".into(),
                output: "/Users/test/projects/starship".into(),
            },
        ],
        right: vec![],
    };

    bencher.bench(|| serde_json::to_string(black_box(&prompt)).unwrap());
}
