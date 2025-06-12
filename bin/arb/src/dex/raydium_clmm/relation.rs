use crate::dex::raydium_clmm::state::POOL_TICK_ARRAY_BITMAP_SEED;
use crate::dex::DexType::RaydiumCLMM;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;
use solana_sdk::pubkey::Pubkey;

#[derive(Debug)]
pub struct RaydiumCLMMRelationRecord;

impl AccountRelationRecord for RaydiumCLMMRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == RaydiumCLMM.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(RaydiumCLMM, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            RaydiumCLMM,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            RaydiumCLMM,
                            AccountType::MintVault,
                            json.vault_b,
                            json.pool,
                        ),
                        AccountInfo::new(
                            RaydiumCLMM,
                            AccountType::TickArrayBitmap,
                            Pubkey::find_program_address(
                                &[POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(), json.pool.as_ref()],
                                RaydiumCLMM.get_ref_program_id(),
                            )
                            .0,
                            json.pool,
                        ),
                    ]
                })
                .flatten()
                .collect::<Vec<_>>(),
            Some((RaydiumCLMM, AccountType::TickArray)),
        )))
    }
}
