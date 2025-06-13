use crate::dex::DexType::MeteoraDAMMV2;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

#[derive(Debug)]
pub struct MeteoraDAMMV2RelationRecord;

impl AccountRelationRecord for MeteoraDAMMV2RelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == MeteoraDAMMV2.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(MeteoraDAMMV2, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            MeteoraDAMMV2,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            MeteoraDAMMV2,
                            AccountType::MintVault,
                            json.vault_b,
                            json.pool,
                        ),
                    ]
                })
                .flatten()
                .collect::<Vec<_>>(),
            None,
        )))
    }
}
