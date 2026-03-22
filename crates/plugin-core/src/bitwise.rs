//! Bitwise Packing
//!
//! WASM functions can only pass/return primitive types (i32, i64, f32, f64).
//! We need to pass TWO values (pointer and length), so we pack them into one u64.
//! Lower 32 bits = pointer, Upper 32 bits = length.

/// Pack a pointer and length into a single u64.
///
/// WASM32 uses 32-bit pointers, so this is safe - we're not losing precision.
/// The pointer goes in the lower 32 bits, length in the upper 32 bits.
#[inline(always)]
pub const fn into_bitwise(ptr: u32, len: u32) -> u64 {
    (ptr as u64) | ((len as u64) << 32)
}

/// Unpack a u64 back into (pointer, length).
#[inline(always)]
pub const fn from_bitwise(value: u64) -> (u32, u32) {
    let ptr = (value & 0xFFFF_FFFF) as u32; // Lower 32 bits
    let len = (value >> 32) as u32; // Upper 32 bits
    (ptr, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let ptr: u32 = 0x1234_5678;
        let len: u32 = 0x9ABC_DEF0;
        let packed = into_bitwise(ptr, len);
        let (ptr2, len2) = from_bitwise(packed);
        assert_eq!(ptr, ptr2);
        assert_eq!(len, len2);
    }
}
