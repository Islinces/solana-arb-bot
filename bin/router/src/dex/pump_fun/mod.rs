use rand::Rng;
use solana_sdk::declare_id;
use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;

mod math;
pub mod pool_state;
pub mod pump_fun;
pub mod state;

#[cfg(not(feature = "devnet"))]
declare_id!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
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
    let mut rng = rand::thread_rng();
    Pubkey::from_str(PUMPSWAP_FEE_ACCOUNTS[rng.gen_range(0..=7)]).unwrap()
}
