use crate::dex::DexType::RaydiumAMM;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

#[derive(Debug)]
pub struct RaydiumAMMRelationRecord;

impl AccountRelationRecord for RaydiumAMMRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>> {
        Ok(Some((
            dex_json
                .iter()
                .filter(|json| &json.owner == RaydiumAMM.get_ref_program_id())
                .map(|json| {
                    vec![
                        AccountInfo::new(RaydiumAMM, AccountType::Pool, json.pool, json.pool),
                        AccountInfo::new(
                            RaydiumAMM,
                            AccountType::MintVault,
                            json.vault_a,
                            json.pool,
                        ),
                        AccountInfo::new(
                            RaydiumAMM,
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

#[cfg(test)]
mod test {
    use crate::dex::raydium_amm::RaydiumAMMRelationRecord;
    use crate::dex::{AccountRelationRecord, AccountType, DexType};
    use crate::dex_data::DexJson;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[test]
    fn test_raydium_amm_relation() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        let relation = RaydiumAMMRelationRecord.get_account_info(dex_json.as_slice())?;
        let dex_json = dex_json.pop().unwrap();
        let relation = relation.expect("");
        let relation_0 = relation.0;
        relation_0.iter().enumerate().for_each(|(i, r)| {
            assert_eq!(r.dex_type, DexType::RaydiumAMM);
            assert_eq!(r.pool_id, dex_json.pool,);
            if i == 0 {
                assert_eq!(r.account_type, AccountType::Pool);
                assert_eq!(r.account_key, dex_json.pool);
            } else if i == 1 {
                assert_eq!(r.account_type, AccountType::MintVault);
                assert_eq!(r.account_key, dex_json.vault_a);
            } else if i == 2 {
                assert_eq!(r.account_type, AccountType::MintVault);
                assert_eq!(r.account_key, dex_json.vault_b);
            } else {
                panic!("账户关系数量超过3个")
            }
        });
        assert!(relation.1.is_none());
        Ok(())
    }

    #[test]
    fn test_raydium_amm_relation_error_owner() -> anyhow::Result<()> {
        let mut dex_json = vec![DexJson {
            pool: Pubkey::from_str("58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            mint_b: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            vault_a: Pubkey::from_str("DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz")?,
            vault_b: Pubkey::from_str("HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "E59uBXGqn83xN17kMbBVfU1M7T4wHG91eiygHb88Aovb",
            )?),
        }];
        let relation = RaydiumAMMRelationRecord.get_account_info(dex_json.as_slice())?;
        let dex_json = dex_json.pop().unwrap();
        let relation = relation.expect("");
        assert_eq!(relation.0.len(), 0);
        assert!(relation.1.is_none());
        Ok(())
    }
}
