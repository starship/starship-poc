use anyhow::Result;
use starship_common::{ShellContext, init_tracing, socket};
use tracing::instrument;

#[instrument]
fn main() -> Result<()> {
    let _guard = init_tracing();
    prompt()
}

#[instrument(name = "starship")]
fn prompt() -> Result<()> {
    let stream = socket::connect()?;
    let shell_context = construct_shell_context();
    let prompt = starship::run(stream, &shell_context)?;
    print!("{}", prompt);

    Ok(())
}

fn construct_shell_context() -> ShellContext {
    let pwd = std::env::current_dir().ok();
    let user = std::env::var_os("USER").map(|os| os.to_string_lossy().to_string());

    ShellContext { pwd, user }
}
