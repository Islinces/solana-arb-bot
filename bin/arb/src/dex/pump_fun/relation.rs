use crate::dex::DexType::{MeteoraDLMM, PumpFunAMM};
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

pub struct PumpFunAMMRelationRecord;

impl AccountRelationRecord for PumpFunAMMRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == PumpFunAMM.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(PumpFunAMM, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            PumpFunAMM,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            PumpFunAMM,
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
