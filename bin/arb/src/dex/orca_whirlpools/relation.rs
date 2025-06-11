use crate::dex::orca_whirlpools::get_oracle_address;
use crate::dex::DexType::OrcaWhirl;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

pub struct OrcaWhirlAccountRelationRecord;

impl AccountRelationRecord for OrcaWhirlAccountRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == OrcaWhirl.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(OrcaWhirl, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            OrcaWhirl,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            OrcaWhirl,
                            AccountType::MintVault,
                            json.vault_b,
                            json.pool,
                        ),
                        AccountInfo::new(
                            OrcaWhirl,
                            AccountType::Oracle,
                            get_oracle_address(&json.pool).unwrap(),
                            json.pool,
                        ),
                    ]
                })
                .flatten()
                .collect::<Vec<_>>(),
            Some((OrcaWhirl, AccountType::TickArray)),
        )))
    }
}
