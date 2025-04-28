use serde::{Deserialize, Deserializer};
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize)]
pub struct ClmmJsonInfo {
    pub amm_config_index: u16,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_0: Pubkey,
    pub mint_0_decimals: u8,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_1: Pubkey,
    pub mint_1_decimals: u8,
}

#[derive(Debug, Deserialize)]
pub struct AmmJsonInfo {
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub pool_id: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_0: Pubkey,
    pub mint_0_decimals: u8,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_1: Pubkey,
    pub mint_1_decimals: u8,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_0_vault: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub mint_1_vault: Pubkey,
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}
