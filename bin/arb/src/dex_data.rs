use anyhow::anyhow;
use serde::{Deserialize, Deserializer};
use solana_sdk::pubkey::Pubkey;
use std::fs::File;
use std::str::FromStr;
use tracing::{error, info};

pub fn load_dex_json(
    dex_json_path: String,
    follow_mints: &[Pubkey],
) -> anyhow::Result<Vec<DexJson>> {
    info!("加载DexJson...");
    let mut dex_data: Vec<DexJson> = match File::open(dex_json_path.as_str()) {
        Ok(file) => serde_json::from_reader(file).expect("解析【dex_data.json】失败"),
        Err(e) => {
            error!("{}", e);
            vec![]
        }
    };
    if dex_data.is_empty() {
        Err(anyhow!("json文件无数据"))
    } else {
        // 删除不涉及关注的Mint的池子
        dex_data.retain(|v| follow_mints.contains(&v.mint_a) || follow_mints.contains(&v.mint_b));
        if dex_data.is_empty() {
            Err(anyhow!(
                "json文件中无涉及程序关注的Mint的池子，程序关注的Mint : {:?}",
                follow_mints
            ))
        } else {
            info!("涉及关注的Mint的池子数量 : {}", dex_data.len());
            Ok(dex_data)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexJson {
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub pool: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey")]
    pub owner: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "mintA")]
    pub mint_a: Pubkey,
    #[serde(deserialize_with = "deserialize_pubkey", rename = "mintB")]
    pub mint_b: Pubkey,
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