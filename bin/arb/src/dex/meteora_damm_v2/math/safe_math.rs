use ruint::aliases::{U256, U512};

use crate::dex::meteora_damm_v2::error::MathOverflow;
use anyhow::{anyhow, Result};

pub trait SafeMath<T>: Sized {
    fn safe_add(self, rhs: Self) -> Result<Self>;
    fn safe_mul(self, rhs: Self) -> Result<Self>;
    fn safe_div(self, rhs: Self) -> Result<Self>;
    fn safe_rem(self, rhs: Self) -> Result<Self>;
    fn safe_sub(self, rhs: Self) -> Result<Self>;
    fn safe_shl(self, offset: T) -> Result<Self>;
    fn safe_shr(self, offset: T) -> Result<Self>;
}

macro_rules! checked_impl {
    ($t:ty, $offset:ty) => {
        impl SafeMath<$offset> for $t {
            #[inline(always)]
            fn safe_add(self, v: $t) -> Result<$t> {
                match self.checked_add(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_sub(self, v: $t) -> Result<$t> {
                match self.checked_sub(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_mul(self, v: $t) -> Result<$t> {
                match self.checked_mul(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_div(self, v: $t) -> Result<$t> {
                match self.checked_div(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_rem(self, v: $t) -> Result<$t> {
                match self.checked_rem(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_shl(self, v: $offset) -> Result<$t> {
                match self.checked_shl(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }

            #[inline(always)]
            fn safe_shr(self, v: $offset) -> Result<$t> {
                match self.checked_shr(v) {
                    Some(result) => Ok(result),
                    None => Err(anyhow!(MathOverflow)),
                }
            }
        }
    };
}

checked_impl!(u16, u32);
checked_impl!(i32, u32);
checked_impl!(u32, u32);
checked_impl!(u64, u32);
checked_impl!(i64, u32);
checked_impl!(u128, u32);
checked_impl!(i128, u32);
checked_impl!(usize, u32);
checked_impl!(U256, usize);
checked_impl!(U512, usize);
