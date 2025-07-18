use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

#[derive(Debug)]
pub struct RaydiumCPMMAccountRelationRecord;

impl AccountRelationRecord for RaydiumCPMMAccountRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == DexType::RaydiumCPMM.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(
                            DexType::RaydiumCPMM,
                            AccountType::Pool,
                            json.pool,
                            json.pool,
                        ),
                        AccountInfo::new(
                            DexType::RaydiumCPMM,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            DexType::RaydiumCPMM,
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
