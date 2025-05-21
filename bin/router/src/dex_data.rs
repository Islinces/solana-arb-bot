use std::fs::File;
use std::str::FromStr;
use serde::{Deserialize, Deserializer};
use solana_sdk::pubkey::Pubkey;
use tracing::error;

pub fn get_dex_data(dex_json_path: String) -> Vec<DexJson> {
    let dex_jsons: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    dex_jsons
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexJson {
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub pool: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub owner: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "vaultA")]
    pub vault_a: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "vaultB")]
    pub vault_b: Pubkey,
    #[serde(
        deserialize_with = "deserialize_option_pubkey",
        rename = "addressLookupTableAddress"
    )]
    pub address_lookup_table_address: Option<Pubkey>,
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> anyhow::Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(Pubkey::from_str(s.as_str()).unwrap())
}

fn deserialize_option_pubkey<'de, D>(deserializer: D) -> anyhow::Result<Option<Pubkey>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: anyhow::Result<String, _> = Deserialize::deserialize(deserializer);
    if s.is_err() {
        return Ok(None);
    }
    Ok(Some(Pubkey::from_str(s?.as_str()).unwrap()))
}