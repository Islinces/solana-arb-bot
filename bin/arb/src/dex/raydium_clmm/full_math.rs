use crate::dex::raydium_clmm::big_num::{U128, U256, U512};

pub trait MulDiv<RHS = Self> {
    type Output;

    fn mul_div_floor(self, num: RHS, denom: RHS) -> Option<Self::Output>;

    fn mul_div_ceil(self, num: RHS, denom: RHS) -> Option<Self::Output>;

}

pub trait Upcast256 {
    fn as_u256(self) -> U256;
}
impl Upcast256 for U128 {
    fn as_u256(self) -> U256 {
        U256([self.0[0], self.0[1], 0, 0])
    }
}

pub trait Downcast256 {
    fn as_u128(self) -> U128;
}
impl Downcast256 for U256 {
    fn as_u128(self) -> U128 {
        U128([self.0[0], self.0[1]])
    }
}

pub trait Upcast512 {
    fn as_u512(self) -> U512;
}
impl Upcast512 for U256 {
    fn as_u512(self) -> U512 {
        U512([self.0[0], self.0[1], self.0[2], self.0[3], 0, 0, 0, 0])
    }
}

pub trait Downcast512 {
    fn as_u256(self) -> U256;
}
impl Downcast512 for U512 {
    fn as_u256(self) -> U256 {
        U256([self.0[0], self.0[1], self.0[2], self.0[3]])
    }
}

impl MulDiv for u64 {
    type Output = u64;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, 0);
        let r = (U128::from(self) * U128::from(num)) / U128::from(denom);
        if r > U128::from(u64::MAX) {
            None
        } else {
            Some(r.as_u64())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, 0);
        let r = (U128::from(self) * U128::from(num) + U128::from(denom - 1)) / U128::from(denom);
        if r > U128::from(u64::MAX) {
            None
        } else {
            Some(r.as_u64())
        }
    }
}

impl MulDiv for U128 {
    type Output = U128;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U128::default());
        let r = ((self.as_u256()) * (num.as_u256())) / (denom.as_u256());
        if r > U128::MAX.as_u256() {
            None
        } else {
            Some(r.as_u128())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U128::default());
        let r = (self.as_u256() * num.as_u256() + (denom - 1).as_u256()) / denom.as_u256();
        if r > U128::MAX.as_u256() {
            None
        } else {
            Some(r.as_u128())
        }
    }
}

impl MulDiv for U256 {
    type Output = U256;

    fn mul_div_floor(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U256::default());
        let r = (self.as_u512() * num.as_u512()) / denom.as_u512();
        if r > U256::MAX.as_u512() {
            None
        } else {
            Some(r.as_u256())
        }
    }

    fn mul_div_ceil(self, num: Self, denom: Self) -> Option<Self::Output> {
        assert_ne!(denom, U256::default());
        let r = (self.as_u512() * num.as_u512() + (denom - 1).as_u512()) / denom.as_u512();
        if r > U256::MAX.as_u512() {
            None
        } else {
            Some(r.as_u256())
        }
    }
}