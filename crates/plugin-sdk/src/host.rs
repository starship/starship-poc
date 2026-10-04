//! Host functions for querying the daemon.
//!
//! Every call is one [`HostRequest`] sent through the `_host_handle` import.
//! The host answers from the current render: its working directory, and the
//! exec cache for cached commands.

use starship_plugin_core::{HostRequest, HostResponse};

/// Get the provided env variable.
pub fn get_env(name: &str) -> Option<String> {
    match send(&HostRequest::Env { name: name.into() }) {
        HostResponse::Env(value) => value,
        _ => None,
    }
}

/// Execute the provided command, reusing a cached result when the binary and
/// arguments are unchanged.
pub fn exec(cmd: &str, args: &[&str]) -> Option<String> {
    run(cmd, args, true)
}

/// Execute the provided command without caching the result.
///
/// Use for commands whose output depends on state beyond the binary itself
/// (e.g. `git branch`, `pwd`).
pub fn exec_uncached(cmd: &str, args: &[&str]) -> Option<String> {
    run(cmd, args, false)
}

/// Check whether the provided path, relative to the working directory, exists.
pub fn file_exists(path: &str) -> bool {
    matches!(
        send(&HostRequest::FileExists { path: path.into() }),
        HostResponse::FileExists(true)
    )
}

fn run(cmd: &str, args: &[&str], cached: bool) -> Option<String> {
    let request = HostRequest::Exec {
        cmd: cmd.into(),
        args: args.iter().map(ToString::to_string).collect(),
        cached,
    };
    match send(&request) {
        HostResponse::Exec(output) => output,
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn send(request: &HostRequest) -> HostResponse {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn _host_handle(packed: u64) -> u64;
    }

    let packed = starship_plugin_core::write_msg(request);
    // SAFETY: the host answers with bytes it wrote with our `alloc`.
    unsafe { starship_plugin_core::read_msg(_host_handle(packed)) }
}

#[cfg(not(target_arch = "wasm32"))]
fn send(request: &HostRequest) -> HostResponse {
    let _ = request;
    panic!("host functions are only available inside a WASM plugin");
}
