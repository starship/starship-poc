//! Guest-side memory management and serialization.
//!
//! These functions are compiled into the WASM plugin binary. They allow the
//! host (daemon) to allocate/deallocate memory in the guest's address space,
//! and let plugins serialize/deserialize data across the WASM boundary.

use crate::bitwise::{from_bitwise, into_bitwise};
use serde::{Deserialize, Serialize};

/// Allocate `len` bytes in the guest's memory and return a pointer to it.
///
/// Called by the host when it needs to pass data TO the plugin.
/// The host will:
///   1. Call alloc(len) to get a pointer
///   2. Write serialized bytes to that pointer
///   3. Call the plugin function with the packed (ptr, len)
///
/// We use Vec to allocate, then `forget` it so Rust doesn't free it.
/// The memory stays allocated until explicitly deallocated.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    // Create a Vec with the requested capacity
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    // Get the raw pointer to the buffer
    let ptr = buf.as_mut_ptr();
    // "Forget" the Vec - this prevents Rust from dropping it when this function ends.
    // The memory stays allocated, and we return the pointer to the host.
    std::mem::forget(buf);
    ptr
}

/// Deallocate memory previously allocated by `alloc`.
///
/// Takes a packed (ptr, len) as u64. We reconstruct the Vec and let it drop,
/// which frees the memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(packed: u64) {
    let (ptr, len) = from_bitwise(packed);
    // Reconstruct the Vec from the raw parts.
    // This is safe because:
    //   - ptr came from a Vec we allocated
    //   - len is the original length
    //   - capacity equals len (we allocated exactly what we needed)
    unsafe {
        let _ = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    }
    // Vec drops here, freeing the memory
}

/// Serialize a value and return a packed (ptr, len).
///
/// Used by plugins to return data to the host.
/// The plugin serializes its return value, and the host reads it from memory.
pub fn write_msg<T: Serialize>(value: &T) -> u64 {
    // Serialize using serde_json.
    // JSON naturally supports schema evolution: unknown fields are ignored,
    // and missing fields can use #[serde(default)].
    let mut buffer = serde_json::to_vec(value).expect("serialization failed");
    // Shrink capacity to match length so dealloc can correctly free the memory.
    // serde_json::to_vec may allocate extra capacity during serialization.
    buffer.shrink_to_fit();
    let len = buffer.len();
    let ptr = buffer.as_ptr();
    // Forget the buffer so it stays in memory for the host to read
    std::mem::forget(buffer);
    into_bitwise(ptr as u32, len as u32)
}

/// Deserialize a value from a packed (ptr, len).
///
/// Used by plugins to read input data from the host.
pub unsafe fn read_msg<T: for<'de> Deserialize<'de>>(packed: u64) -> T {
    let (ptr, len) = from_bitwise(packed);
    // Reconstruct the Vec to get ownership of the bytes
    let buffer = unsafe { Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize) };
    // Deserialize and return. The Vec drops here, freeing the input buffer.
    // JSON supports schema evolution: unknown fields are ignored by serde,
    // and missing fields can use #[serde(default)].
    serde_json::from_slice(&buffer).expect("deserialization failed")
}
