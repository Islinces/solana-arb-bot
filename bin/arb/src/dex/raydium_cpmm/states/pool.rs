use crate::dex::utils::read_from;
use crate::dex::{DynamicCache, FromCache, StaticCache};
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use serde_with::{serde_as, DisplayFromStr};

/// Seed to derive account address and signature
const POOL_SEED: &str = "pool";
const POOL_LP_MINT_SEED: &str = "pool_lp_mint";
const POOL_VAULT_SEED: &str = "pool_vault";

const Q32: u128 = (u32::MAX as u128) + 1; // 2^32

pub enum PoolStatusBitIndex {
    Deposit,
    Withdraw,
    Swap,
}

#[derive(PartialEq, Eq)]
enum PoolStatusBitFlag {
    Enable,
    Disable,
}

#[repr(C, packed)]
#[derive(Debug)]
// #[serde_as]
// #[cfg_attr(feature = "print_data_after_update", derive(Serialize, Deserialize))]
pub struct PoolState {
    /// Which config the pool belongs
    /// 8,32
    // #[serde_as(as = "DisplayFromStr")]
    pub amm_config: Pubkey,
    /// Token A
    /// 72,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_0_vault: Pubkey,
    /// Token B
    /// 104,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_1_vault: Pubkey,
    /// Mint information for token A
    /// 168,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_0_mint: Pubkey,
    /// Mint information for token B
    /// 200,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_1_mint: Pubkey,

    /// token_0 program
    /// 232,32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_0_program: Pubkey,
    /// token_1 program
    /// 264，32
    // #[serde_as(as = "DisplayFromStr")]
    pub token_1_program: Pubkey,

    /// observation account to store oracle data
    /// 296，32
    // #[serde_as(as = "DisplayFromStr")]
    pub observation_key: Pubkey,
    /// Bitwise representation of the state of the pool
    /// bit0, 1: disable deposit(value is 1), 0: normal
    /// bit1, 1: disable withdraw(value is 2), 0: normal
    /// bit2, 1: disable swap(value is 4), 0: normal
    /// 329，1
    pub status: u8,
    /// The amounts of token_0 and token_1 that are owed to the liquidity provider.
    /// 341，8
    pub protocol_fees_token_0: u64,
    // 349，8
    pub protocol_fees_token_1: u64,
    // 357，8
    pub fund_fees_token_0: u64,
    // 365，8
    pub fund_fees_token_1: u64,
    /// The timestamp allowed for swap in the pool.
    /// 373，8
    pub open_time: u64,
}

impl FromCache for PoolState {
    fn from_cache(
        account_key: &Pubkey,
        static_cache: RwLockReadGuard<StaticCache>,
        dynamic_cache: &DynamicCache,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let static_data = static_cache.get(account_key)?;
        let dynamic_data = dynamic_cache.get(account_key)?;
        let dynamic_data = dynamic_data.value().as_slice();
        Some(Self::from_slice_data(static_data, dynamic_data))
    }
}

impl PoolState {
    pub fn from_slice_data(static_data: &[u8], dynamic_data: &[u8]) -> Self {
        unsafe {
            let amm_config = read_from::<Pubkey>(&static_data[0..32 * 1]);
            let token_0_vault = read_from::<Pubkey>(&static_data[32..32 * 2]);
            let token_1_vault = read_from::<Pubkey>(&static_data[64..32 * 3]);
            let token_0_mint = read_from::<Pubkey>(&static_data[96..32 * 4]);
            let token_1_mint = read_from::<Pubkey>(&static_data[128..32 * 5]);
            let token_0_program = read_from::<Pubkey>(&static_data[160..32 * 6]);
            let token_1_program = read_from::<Pubkey>(&static_data[192..32 * 7]);
            let observation_key = read_from::<Pubkey>(&static_data[224..32 * 8]);
            let open_time = read_from::<u64>(&static_data[256..256 + 8]);

            let status = read_from::<u8>(&dynamic_data[0..1]);
            let protocol_fees_token_0 = read_from::<u64>(&dynamic_data[1..9]);
            let protocol_fees_token_1 = read_from::<u64>(&dynamic_data[9..17]);
            let fund_fees_token_0 = read_from::<u64>(&dynamic_data[17..25]);
            let fund_fees_token_1 = read_from::<u64>(&dynamic_data[25..33]);

            Self {
                amm_config,
                token_0_vault,
                token_1_vault,
                token_0_mint,
                token_1_mint,
                token_0_program,
                token_1_program,
                observation_key,
                status,
                protocol_fees_token_0,
                protocol_fees_token_1,
                fund_fees_token_0,
                fund_fees_token_1,
                open_time,
            }
        }
    }

    pub fn vault_amount_without_fee(&self, vault_0: u64, vault_1: u64) -> (u64, u64) {
        (
            vault_0
                .checked_sub(self.protocol_fees_token_0 + self.fund_fees_token_0)
                .unwrap(),
            vault_1
                .checked_sub(self.protocol_fees_token_1 + self.fund_fees_token_1)
                .unwrap(),
        )
    }
}
