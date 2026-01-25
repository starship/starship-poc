use std::{os::unix::net::UnixStream, path::PathBuf};

use divan::{Bencher, black_box};
use starship_common::ShellContext;
use starship_daemon::{config::ConfigLoader, handle_client};

fn main() {
    divan::main();
}

fn context() -> ShellContext {
    ShellContext {
        pwd: Some(PathBuf::from("/Users/test/projects/starship")),
        user: Some("testuser".into()),
    }
}

const MINIMAL: &str = r#"
  return { format = "$ " }
"#;
const WITH_MODULES: &str = r#"
  return { format = ctx.pwd .. " " .. ctx.user .. " $ " }
"#;

#[divan::bench(args = [MINIMAL, WITH_MODULES])]
fn cold_start_render(config: &str) {
    let mut loader = ConfigLoader::from_source(config).unwrap();
    let (client, server) = UnixStream::pair().unwrap();
    std::thread::spawn(move || handle_client(server, &mut loader).unwrap());
    starship::run(client, &context()).unwrap();
}

#[divan::bench(args = [MINIMAL, WITH_MODULES])]
fn cached_render(bencher: Bencher, config: &str) {
    let mut loader = ConfigLoader::from_source(config).unwrap();
    bencher.bench_local(|| {
        let (client, server) = UnixStream::pair().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| handle_client(server, &mut loader).unwrap());
            black_box(starship::run(client, &context()).unwrap())
        })
    });
}
