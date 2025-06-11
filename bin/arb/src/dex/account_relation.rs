use crate::dex::meteora_dlmm::MeteoraDLMMAccountRelation;
use crate::dex::orca_whirlpools::OrcaWhirlAccountRelationRecord;
use crate::dex::pump_fun::PumpFunAMMRelationRecord;
use crate::dex::raydium_amm::RaydiumAMMRelationRecord;
use crate::dex::raydium_clmm::RaydiumCLMMRelationRecord;
use crate::dex::{AccountType, DexType};
use crate::dex_data::DexJson;
use ahash::AHashMap;
use anyhow::anyhow;
use enum_dispatch::enum_dispatch;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::OnceCell;

static ACCOUNT_RELATION_CACHE: OnceCell<Arc<AHashMap<Pubkey, AccountInfo>>> = OnceCell::const_new();
static SUPPLEMENTARY_ACCOUNT_RELATION_CACHE: OnceCell<Arc<AHashMap<DexType, AccountType>>> =
    OnceCell::const_new();

#[enum_dispatch]
pub trait AccountRelationRecord {
    fn get_account_info(
        &self,
        dex_json: &[DexJson],
    ) -> anyhow::Result<Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)>>;
}

#[enum_dispatch(AccountRelationRecord)]
pub enum AccountRelationRecordType {
    MeteoraDLMM(MeteoraDLMMAccountRelation),
    OrcaWhirl(OrcaWhirlAccountRelationRecord),
    PumpFunAMM(PumpFunAMMRelationRecord),
    RaydiumAmm(RaydiumAMMRelationRecord),
    RaydiumCLMM(RaydiumCLMMRelationRecord),
}

pub(crate) fn init_account_relations(dex_data: &[DexJson]) -> anyhow::Result<()> {
    let mut account_mapping = AHashMap::with_capacity(1000);
    let mut supplementary_account_mapping = AHashMap::with_capacity(1000);
    for record_type in vec![
        AccountRelationRecordType::from(MeteoraDLMMAccountRelation),
        AccountRelationRecordType::from(OrcaWhirlAccountRelationRecord),
        AccountRelationRecordType::from(PumpFunAMMRelationRecord),
        AccountRelationRecordType::from(RaydiumAMMRelationRecord),
        AccountRelationRecordType::from(RaydiumCLMMRelationRecord),
    ] {
        let relation_infos: Option<(Vec<AccountInfo>, Option<(DexType, AccountType)>)> =
            record_type.get_account_info(dex_data)?;
        if let Some((relations, supplementary)) = relation_infos {
            relations.into_iter().for_each(|rel| {
                account_mapping.insert(rel.account_key, rel);
            });
            if let Some((dex_type, account_type)) = supplementary {
                supplementary_account_mapping.insert(dex_type, account_type);
            }
        }
    }
    ACCOUNT_RELATION_CACHE
        .set(Arc::new(account_mapping))
        .map_or(Err(anyhow!("初始化AccountRelation失败")), |_| Ok(()))?;
    SUPPLEMENTARY_ACCOUNT_RELATION_CACHE
        .set(Arc::new(supplementary_account_mapping))
        .map_or(Err(anyhow!("初始化AccountRelation失败")), |_| Ok(()))
}

#[inline]
pub fn is_follow_vault(vault_account: &Pubkey) -> Option<(Pubkey, DexType)> {
    match ACCOUNT_RELATION_CACHE.get()?.get(vault_account) {
        None => None,
        Some(a) => Some((a.pool_id.clone(), a.dex_type)),
    }
}

pub fn get_dex_type_and_account_type(
    owner: &Pubkey,
    account_key: &Pubkey,
) -> Option<(DexType, AccountType)> {
    let relation_info = ACCOUNT_RELATION_CACHE.get()?.get(account_key);
    match relation_info {
        None => {
            let dex_type = DexType::try_from(owner).ok()?;
            let account_type = SUPPLEMENTARY_ACCOUNT_RELATION_CACHE
                .get()?
                .get(&dex_type)?
                .clone();
            Some((dex_type, account_type))
        }
        Some(rel) => Some((rel.dex_type.clone(), rel.account_type.clone())),
    }
}

#[derive(Debug)]
pub struct AccountInfo {
    dex_type: DexType,
    account_type: AccountType,
    account_key: Pubkey,
    pool_id: Pubkey,
}

impl AccountInfo {
    pub fn new(
        dex_type: DexType,
        account_type: AccountType,
        account_key: Pubkey,
        pool_id: Pubkey,
    ) -> Self {
        Self {
            dex_type,
            account_type,
            account_key,
            pool_id,
        }
    }
}
