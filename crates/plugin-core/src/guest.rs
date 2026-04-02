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
/// Uses a boxed slice to guarantee capacity == len, avoiding the soundness
/// issue where `Vec::with_capacity` may allocate more than requested.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let boxed: Box<[u8]> = vec![0u8; len as usize].into_boxed_slice();
    Box::into_raw(boxed) as *mut u8
}

/// Deallocate memory previously allocated by `alloc` or `write_msg`.
///
/// Takes a packed (ptr, len) as u64. Reconstructs the boxed slice and drops it.
///
/// # Safety
/// The packed value must have come from a previous allocation in this module.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(packed: u64) {
    let (ptr, len) = from_bitwise(packed);
    unsafe {
        let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, len as usize);
        let _ = Box::from_raw(slice as *mut [u8]);
    }
}

/// Serialize a value and return a packed (ptr, len).
///
/// Used by plugins to return data to the host.
/// The plugin serializes its return value, and the host reads it from memory.
///
/// Converts to a boxed slice to guarantee capacity == len before leaking.
pub fn write_msg<T: Serialize>(value: &T) -> u64 {
    let boxed: Box<[u8]> = serde_json::to_vec(value)
        .expect("serialization failed")
        .into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *mut u8;
    into_bitwise(ptr as u32, len as u32)
}

/// Deserialize a value from a packed (ptr, len).
///
/// Used by plugins to read input data from the host.
///
/// # Safety
/// The packed value must point to valid JSON data previously written to guest memory.
pub unsafe fn read_msg<T: for<'de> Deserialize<'de>>(packed: u64) -> T {
    let (ptr, len) = from_bitwise(packed);
    let boxed = unsafe {
        let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, len as usize);
        Box::from_raw(slice as *mut [u8])
    };
    serde_json::from_slice(&boxed).expect("deserialization failed")
}
