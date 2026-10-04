//! Requests a plugin sends to the daemon through the `_host_handle` import.
//! Plugins use these through [`Ctx`](crate::Ctx).

use std::path::Path;

use starship_plugin_core::{HostRequest, HostResponse};

pub(crate) fn exec(cmd: &str, args: &[&str], cwd: &Path, cached: bool) -> Option<String> {
    let request = HostRequest::Exec {
        cmd: cmd.into(),
        args: args.iter().map(ToString::to_string).collect(),
        cwd: cwd.to_path_buf(),
        cached,
    };
    match send(&request) {
        HostResponse::Exec(output) => output,
        HostResponse::FileExists(_) => None,
    }
}

pub(crate) fn file_exists(path: &Path) -> bool {
    let request = HostRequest::FileExists {
        path: path.to_path_buf(),
    };
    matches!(send(&request), HostResponse::FileExists(true))
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
