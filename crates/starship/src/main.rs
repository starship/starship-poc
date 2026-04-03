use anyhow::Result;
use clap::Parser;
use starship_common::{init_tracing, socket, ShellContext};
use starship_runtime::{Config, ConfigLoader};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::instrument;

#[derive(Parser)]
struct Args {
    /// Run without connecting to the daemon
    #[arg(long)]
    no_daemon: bool,
}

#[instrument]
fn main() -> Result<()> {
    let _guard = init_tracing();
    let args = Args::parse();
    let ctx = construct_shell_context();

    if args.no_daemon {
        let mut loader = ConfigLoader::new()?;
        let func = loader.load(&ctx)?;
        let output: Config = func.call(())?;
        print!("{}", output.format);
    } else {
        let stream = connect_or_spawn_daemon()?;
        let prompt = starship::run(stream, &ctx)?;
        print!("{prompt}");
    }

    Ok(())
}

fn construct_shell_context() -> ShellContext {
    let pwd = std::env::current_dir().ok();
    let user = std::env::var_os("USER").map(|os| os.to_string_lossy().to_string());

    ShellContext { pwd, user }
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
