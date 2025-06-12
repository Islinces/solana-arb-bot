use crate::dex::meteora_dlmm::derive_bin_array_bitmap_extension;
use crate::dex::DexType::MeteoraDLMM;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

#[derive(Debug)]
pub struct MeteoraDLMMAccountRelation;

impl AccountRelationRecord for MeteoraDLMMAccountRelation {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == MeteoraDLMM.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(MeteoraDLMM, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            MeteoraDLMM,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            MeteoraDLMM,
                            AccountType::MintVault,
                            json.vault_b,
                            json.pool,
                        ),
                        AccountInfo::new(
                            MeteoraDLMM,
                            AccountType::BinArrayBitmap,
                            derive_bin_array_bitmap_extension(&json.pool),
                            json.pool,
                        ),
                    ]
                })
                .flatten()
                .collect::<Vec<_>>(),
            Some((MeteoraDLMM, AccountType::BinArray)),
        )))
    }
}
