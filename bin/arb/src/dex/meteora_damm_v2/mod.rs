mod account_subscriber;
mod data_slice;
mod instruction;
mod math;
mod quote;
mod relation;
mod snapshot_loader;
pub mod state;
pub mod old_state;

use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub use account_subscriber::MeteoraDAMMV2AccountSubscriber;
pub use data_slice::MeteoraDAMMV2DataSlicer;
pub use instruction::MeteoraDAMMV2InstructionMaterialConverter;
pub use quote::MeteoraDAMMV2Quoter;
pub use relation::MeteoraDAMMV2RelationRecord;
pub use snapshot_loader::MeteoraDAMMV2SnapshotLoader;

pub(super) const DAMM_V2_PROGRAM_ID: Pubkey =
    pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

const DAMM_V2_POOL_AUTHORITY: Pubkey = pubkey!("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC");

const DAMM_V2_EVENT_AUTHORITY: Pubkey = pubkey!("3rmHSu74h1ZcmAisVcWerTCiRDQbUrBKmcwptYGjHfet");

mod constants {
    pub const LIQUIDITY_SCALE: u8 = 128;

    pub const ONE_Q64: u128 = 1u128 << 64;

    pub const BASIS_POINT_MAX: u64 = 10_000;

    pub mod activation {
        #[cfg(not(feature = "local"))]
        pub const SLOT_BUFFER: u64 = 9000; // 1 slot = 400 mls => 1 hour
        #[cfg(feature = "local")]
        pub const SLOT_BUFFER: u64 = 5;

        #[cfg(not(feature = "local"))]
        pub const TIME_BUFFER: u64 = 3600; // 1 hour
        #[cfg(feature = "local")]
        pub const TIME_BUFFER: u64 = 5; // 5 secs

        #[cfg(not(feature = "local"))]
        pub const MAX_ACTIVATION_SLOT_DURATION: u64 = SLOT_BUFFER * 24 * 31; // 31 days
        #[cfg(feature = "local")]
        pub const MAX_ACTIVATION_SLOT_DURATION: u64 = 30;

        #[cfg(not(feature = "local"))]
        pub const MAX_ACTIVATION_TIME_DURATION: u64 = TIME_BUFFER * 24 * 31; // 31 days
        #[cfg(feature = "local")]
        pub const MAX_ACTIVATION_TIME_DURATION: u64 = 30;

        pub const MAX_VESTING_SLOT_DURATION: u64 = SLOT_BUFFER * 24 * 365 * 10; // 10 years
        pub const MAX_VESTING_TIME_DURATION: u64 = TIME_BUFFER * 24 * 365 * 10; // 10 years

        pub const FIVE_MINUTES_SLOT_BUFFER: u64 = SLOT_BUFFER / 12; // 5 minutes

        pub const FIVE_MINUTES_TIME_BUFFER: u64 = TIME_BUFFER / 12; // 5 minutes

        pub const MAX_FEE_CURVE_TIME_DURATION: u64 = 3600 * 24; // 1 day
        pub const MAX_FEE_CURVE_SLOT_DURATION: u64 = 9000 * 24; // 1 day

        pub const MAX_HIGH_TAX_TIME_DURATION: u64 = TIME_BUFFER / 6; // 10 minutes
        pub const MAX_HIGH_TAX_SLOT_DURATION: u64 = SLOT_BUFFER / 6; // 10 minutes
    }

    /// Store constants related to fees
    pub mod fee {
        /// Default fee denominator. DO NOT simply update it as it will break logic that depends on it as default value.
        pub const FEE_DENOMINATOR: u64 = 1_000_000_000;

        /// Max fee BPS
        pub const MAX_FEE_BPS: u64 = 5000; // 50%
        pub const MAX_FEE_NUMERATOR: u64 = 500_000_000; // 50%

        /// Max basis point. 100% in pct
        pub const MAX_BASIS_POINT: u64 = 10000;

        pub const MIN_FEE_BPS: u64 = 1; // 0.01%
        pub const MIN_FEE_NUMERATOR: u64 = 100_000;

        pub const CUSTOMIZABLE_PROTOCOL_FEE_PERCENT: u8 = 20; // 20%

        pub const CUSTOMIZABLE_HOST_FEE_PERCENT: u8 = 20; // 20%

        pub const MEME_MIN_FEE_UPDATE_WINDOW_DURATION: i64 = 60 * 30; // 30 minutes
    }

    pub mod seeds {
        pub const CONFIG_PREFIX: &[u8] = b"config";
        pub const CUSTOMIZABLE_POOL_PREFIX: &[u8] = b"cpool";
        pub const POOL_PREFIX: &[u8] = b"pool";
        pub const TOKEN_VAULT_PREFIX: &[u8] = b"token_vault";
        pub const POOL_AUTHORITY_PREFIX: &[u8] = b"pool_authority";
        pub const POSITION_PREFIX: &[u8] = b"position";
        pub const POSITION_NFT_ACCOUNT_PREFIX: &[u8] = b"position_nft_account";
        pub const TOKEN_BADGE_PREFIX: &[u8] = b"token_badge";
        pub const REWARD_VAULT_PREFIX: &[u8] = b"reward_vault";
        pub const CLAIM_FEE_OPERATOR_PREFIX: &[u8] = b"cf_operator";
    }
}

mod error {
    pub type PoolError = &'static str;
    pub const MathOverflow: PoolError = "Math operation overflow";
    pub const TypeCastFailed: PoolError = "Type cast error";
    pub const InvalidCollectFeeMode: PoolError = "Invalid collect fee mode";
    pub const PriceRangeViolation: PoolError = "Trade is over price range";
    pub const InvalidActivationType: PoolError = "Invalid activation type";
    pub const InvalidActivationPoint: PoolError = "Invalid activation point";
    pub const ExceedMaxFeeBps: PoolError = "Exceeded max fee bps";
    pub const InvalidInput: PoolError = "Invalid input";
    pub const InvalidFee: PoolError = "Invalid fee setup";
    pub const InvalidParameters: PoolError = "Invalid parameters";
    pub const PoolDisabled: PoolError = "Pool disabled";
    pub const UnableToModifyActivationPoint: PoolError = "Unable to modify activation point";
}

/// Trade (swap) direction
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
enum TradeDirection {
    /// Input token A, output token B
    AtoB,
    /// Input token B, output token A
    BtoA,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
/// Type of the activation
enum ActivationType {
    Slot,
    Timestamp,
}

