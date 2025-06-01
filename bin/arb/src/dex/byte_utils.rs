use solana_sdk::pubkey::Pubkey;
use std::ptr;

#[inline(always)]
pub unsafe fn read_from<T>(bytes: &[u8]) -> T {
    ptr::read_unaligned(bytes.as_ptr() as *const T)
}

#[inline(always)]
pub unsafe fn read_u16(bytes: &[u8]) -> u16 {
    // *(bytes.as_ptr() as *const u16)
    ptr::read_unaligned(bytes.as_ptr() as *const u16)
}

#[inline(always)]
pub unsafe fn read_i32(bytes: &[u8]) -> i32 {
    // *(bytes.as_ptr() as *const i32)
    ptr::read_unaligned(bytes.as_ptr() as *const i32)
}

#[inline(always)]
pub unsafe fn read_u32(bytes: &[u8]) -> u32 {
    // *(bytes.as_ptr() as *const u32)
    ptr::read_unaligned(bytes.as_ptr() as *const u32)
}

#[inline(always)]
pub unsafe fn read_u64(bytes: &[u8]) -> u64 {
    // *(bytes.as_ptr() as *const u64)
    ptr::read_unaligned(bytes.as_ptr() as *const u64)
}

#[inline(always)]
pub unsafe fn read_u128(bytes: &[u8]) -> u128 {
    ptr::read_unaligned(bytes.as_ptr() as *const u128)
}

#[inline(always)]
pub unsafe fn read_pubkey(bytes: &[u8]) -> Pubkey {
    ptr::read_unaligned(bytes.as_ptr() as *const Pubkey)
    // Pubkey::from(*(bytes.as_ptr() as *const [u8; 32]))
}
