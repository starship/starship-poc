//! Host functions for querying the daemon.

#[cfg(target_arch = "wasm32")]
use crate::{read_msg, write_msg};

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn _plugin_host_get_env(packed: u64) -> u64;
    fn _plugin_host_exec(packed: u64) -> u64;
    fn _plugin_host_exec_uncached(packed: u64) -> u64;
    fn _plugin_host_file_exists(packed: u64) -> u32;
}

/// Get the provided env variable
pub fn get_env(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let packed_input = write_msg(&name.to_string());
        let packed_output = unsafe { _plugin_host_get_env(packed_input) };
        unsafe { read_msg(packed_output) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = name;
        panic!("host functions only available in WASM");
    }
}

/// Execute the provided command
pub fn exec(cmd: &str, args: &[&str]) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let request = (cmd, args);
        let packed_input = write_msg(&request);
        let packed_output = unsafe { _plugin_host_exec(packed_input) };
        unsafe { read_msg(packed_output) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (cmd, args);
        panic!("host functions only available in WASM");
    }
}

/// Execute the provided command without caching the result.
///
/// Use for commands whose output depends on state beyond the binary itself
/// (e.g. `git branch`, `pwd`).
pub fn exec_uncached(cmd: &str, args: &[&str]) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let request = (cmd, args);
        let packed_input = write_msg(&request);
        let packed_output = unsafe { _plugin_host_exec_uncached(packed_input) };
        unsafe { read_msg(packed_output) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (cmd, args);
        panic!("host functions only available in WASM");
    }
}

/// Check whether the provided file path exists
pub fn file_exists(path: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let packed_input = write_msg(&path.to_string());
        unsafe { _plugin_host_file_exists(packed_input) != 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = path;
        panic!("host functions only available in WASM");
    }
}
