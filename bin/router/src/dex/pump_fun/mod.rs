use std::str::FromStr;
use rand::Rng;
use solana_sdk::pubkey::Pubkey;
use crate::interface::DexType;

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

/// 字段顺序不要动
#[derive(Default)]
pub(crate) struct Pool {
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub coin_creator: Pubkey,
    // 全局配置，fee相关
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
    // 池子data里没有，在初始化缓存的时候计算之后设置进来的
    pub coin_creator_vault_authority: Pubkey,
    pub coin_creator_vault_ata: Pubkey,
}

pub fn global_config_key() -> Pubkey {
    Pubkey::find_program_address(
        &[b"global_config"],
        DexType::PumpFunAMM.get_ref_program_id(),
    )
        .0
}