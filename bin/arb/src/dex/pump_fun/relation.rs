use crate::dex::DexType::PumpFunAMM;
use crate::dex::{AccountInfo, AccountRelationRecord, AccountType, DexType};
use crate::dex_data::DexJson;

#[derive(Debug)]
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

#[cfg(test)]
mod test {
    use crate::dex::pump_fun::PumpFunAMMRelationRecord;
    use crate::dex::raydium_amm::RaydiumAMMRelationRecord;
    use crate::dex::{AccountRelationRecord, AccountType, DexType};
    use crate::dex_data::DexJson;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    #[test]
    fn test_pumpfun_relation() -> anyhow::Result<()> {
        let dex_json = DexJson {
            pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v")?,
            owner: Pubkey::from_str("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA")?,
            mint_a: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            mint_b: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            vault_a: Pubkey::from_str("nML7msD1MiJHxFvhv4po1u6C4KpWr64ugKqc75DMuD2")?,
            vault_b: Pubkey::from_str("EjHirXt2bQd2DDNveagHHCWYzUwtY1iwNbBrV5j84e6j")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9",
            )?),
        };
        let relation = RaydiumAMMRelationRecord.get_account_info(vec![dex_json.clone()].as_slice())?;
        let relation = relation.expect("");
        let relation_0 = relation.0;
        relation_0.iter().enumerate().for_each(|(i, r)| {
            assert_eq!(r.dex_type, DexType::PumpFunAMM);
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
    fn test_pumpfun_relation_error_owner() -> anyhow::Result<()> {
        let dex_json = DexJson {
            pool: Pubkey::from_str("Gf7sXMoP8iRw4iiXmJ1nq4vxcRycbGXy5RL8a8LnTd3v")?,
            owner: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
            mint_a: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?,
            mint_b: Pubkey::from_str("So11111111111111111111111111111111111111112")?,
            vault_a: Pubkey::from_str("nML7msD1MiJHxFvhv4po1u6C4KpWr64ugKqc75DMuD2")?,
            vault_b: Pubkey::from_str("EjHirXt2bQd2DDNveagHHCWYzUwtY1iwNbBrV5j84e6j")?,
            address_lookup_table_address: Some(Pubkey::from_str(
                "7KgsCbuJAXxELXhpzc9PwX7GenoF3UsuuW71qW1Gr3u9",
            )?),
        };
        let relation = PumpFunAMMRelationRecord.get_account_info(vec![dex_json.clone()].as_slice())?;
        let relation = relation.unwrap();
        assert_eq!(relation.0.len(), 0);
        assert!(relation.1.is_none());
        Ok(())
    }
}
