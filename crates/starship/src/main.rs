use anyhow::Result;
use clap::Parser;
use starship_common::{RenderContext, init_tracing, socket};
use starship_runtime::ConfigLoader;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
#[derive(Parser)]
struct Args {
    /// Run without connecting to the daemon
    #[arg(long)]
    no_daemon: bool,
}

fn main() -> Result<()> {
    let _guard = init_tracing();
    let _span = tracing::info_span!("main").entered();
    let args = Args::parse();
    let ctx = construct_render_context();

    if args.no_daemon {
        let output = ConfigLoader::new()?.render(&ctx)?;
        print!("{}", output.format);
    } else {
        let stream = connect_or_spawn_daemon()?;
        let prompt = starship::run(stream, &ctx)?;
        print!("{prompt}");
    }

    Ok(())
}

/// The shell's state for this prompt. The working directory falls back to
/// `$PWD`, then `/`, when the process can't read it (e.g. it was deleted).
fn construct_render_context() -> RenderContext {
    let pwd = std::env::current_dir()
        .ok()
        .or_else(|| std::env::var_os("PWD").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"));
    // Variables that aren't valid UTF-8 are skipped.
    let env = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect();

    RenderContext { pwd, env }
}

fn connect_or_spawn_daemon() -> Result<std::os::unix::net::UnixStream> {
    if let Ok(stream) = socket::connect() {
        return Ok(stream);
    }

    // Daemon not running — spawn it
    let daemon_bin = std::env::current_exe()?
        .parent()
        .expect("executable should have parent dir")
        .join("starship-daemon");

    Command::new(&daemon_bin)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn daemon at {}: {e}", daemon_bin.display()))?;

    // Poll for socket readiness
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(10));
        if let Ok(stream) = socket::connect() {
            return Ok(stream);
        }
    }

    anyhow::bail!("daemon did not start within 500ms")
}
