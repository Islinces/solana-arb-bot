pub mod data_slice;
pub mod instruction;
pub mod quote;
pub mod state;
mod account_subscriber;
mod snapshot_init;

pub use account_subscriber::*;
pub use snapshot_init::*;

pub use account_subscriber::*;

use rand::Rng;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub const PUMP_FUN_AMM_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");

/// pump fun fee 钱包列表，随机取一个
const PUMPSWAP_FEE_ACCOUNTS: [&str; 8] = [
    "AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY",
    "7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX",
    "62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV",
    "G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP",
    "9rPYyANsfQZw3DnDmKE3YCQF5E8oD89UXoHn9JFEhJUz",
    "JCRGumoE9Qi5BBgULTgdgTLjSgkCMSbF62ZZfGs84JeU",
    "7VtfL8fvgNfhz17qKRMjzQEXgbdpnHHHQRh54R9jP2RJ",
    "FWsW1xNtWscwNmKv6wVsU1iTzRN6wmmk3MjxRP5tT7hz",
];

pub(crate) fn get_fee_account_with_rand() -> Pubkey {
    Pubkey::from_str(PUMPSWAP_FEE_ACCOUNTS[rand::rng().random_range(0..=7)]).unwrap()
}
