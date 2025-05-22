use crate::collector::CollectorType;
use crate::executor::ExecutorType;
use crate::interface::DexType;
use ahash::{AHashMap, AHashSet};
use base58::ToBase58;
use borsh::BorshDeserialize;
use burberry::{ActionSubmitter, Strategy};
use chrono::{DateTime, Local};
use smallvec::{smallvec, SmallVec};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ptr::read;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::Instant;
use tokio_stream::StreamExt;
use tracing::info;

pub struct MessageStrategy {
    pub receiver_msg: AHashMap<
        [u8; 64],
        SmallVec<
            [(
                Pubkey,
                SmallVec<[Pubkey; 3]>,
                SmallVec<[DateTime<Local>; 3]>,
            ); 4],
        >,
    >,
    pub mod_value: Option<u64>,
    pub single_mode: bool,
    pub specify_pool: Option<Pubkey>,
    pub pool_ids: AHashSet<Pubkey>,
    pub vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)>,
    pub standard_program: bool,
}

#[burberry::async_trait]
impl Strategy<CollectorType, ExecutorType> for MessageStrategy {
    async fn process_event(
        &mut self,
        event: CollectorType,
        _submitter: Arc<dyn ActionSubmitter<ExecutorType>>,
    ) {
        if !self.standard_program {
            match event {
                CollectorType::Message((a, tx, account_key, owner, receiver_timestamp)) => {
                    let log = process_data(
                        &mut self.receiver_msg,
                        tx,
                        account_key,
                        owner,
                        receiver_timestamp,
                        &self.specify_pool,
                        &self.pool_ids,
                        &self.vault_to_pool,
                    );
                    if let Some((tx, msg)) = log {
                        info!(
                            "{}\ntx : {:?}\n推送过程 : \n{:#?}",
                            a,
                            tx.as_slice().to_base58(),
                            msg
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn process_data(
    receiver_msg: &mut AHashMap<
        [u8; 64],
        SmallVec<
            [(
                Pubkey,
                SmallVec<[Pubkey; 3]>,
                SmallVec<[DateTime<Local>; 3]>,
            ); 4],
        >,
    >,
    tx: Vec<u8>,
    account_key: Vec<u8>,
    owner: Vec<u8>,
    receiver_timestamp: DateTime<Local>,
    specify_pool: &Option<Pubkey>,
    _pool_ids: &AHashSet<Pubkey>,
    vault_to_pool: &AHashMap<Pubkey, (Pubkey, Pubkey)>,
) -> Option<([u8; 64], Vec<String>)> {
    let txn: [u8; 64] = tx.try_into().unwrap();
    let account = Pubkey::try_from(account_key).unwrap();
    let owner = Pubkey::try_from(owner).unwrap();
    let raydium_program_id = DexType::RaydiumAMM.get_ref_program_id();
    let pumpfun_program_id = DexType::PumpFunAMM.get_ref_program_id();
    let (pool_id, program_id, wait_account_len) = if &owner == raydium_program_id {
        (&account, raydium_program_id, 3)
    } else if &owner == pumpfun_program_id {
        (&account, pumpfun_program_id, 2)
    } else if let Some((pid, prog_id)) = vault_to_pool.get(&account) {
        (pid, prog_id, 0)
    } else {
        return None;
    };
    // info!(
    //     "tx : {:?}, account : {:?}, pool_id : {:?}, program_id : {:?}, timestamp : {:?}",
    //     txn,
    //     account,
    //     pool_id,
    //     program_id,
    //     receiver_timestamp
    // );
    if let Some(value) = receiver_msg.get_mut(&txn) {
        match value.iter().position(|v| &v.0 == pool_id) {
            None => {
                value.push((
                    pool_id.clone(),
                    {
                        let mut accounts = SmallVec::with_capacity(3);
                        accounts.push(account);
                        accounts
                    },
                    {
                        let mut timestamp = SmallVec::with_capacity(3);
                        timestamp.push(receiver_timestamp);
                        timestamp
                    },
                ));
                None
            }
            Some(index) => {
                let (pool_id, accounts, timestamp) = value.get_mut(index).unwrap();
                accounts.push(account);
                timestamp.push(receiver_timestamp);
                if accounts.len() == wait_account_len {
                    let mut account_push_timestamp = accounts
                        .into_iter()
                        .zip(timestamp)
                        .map(|(account, receiver_timestamp)| {
                            format!(
                                "账户 : {:?}, GRPC推送时间 : {:?}",
                                account.to_string(),
                                receiver_timestamp
                                    .format("%Y-%m-%d %H:%M:%S%.9f")
                                    .to_string()
                            )
                        })
                        .collect::<Vec<_>>();
                    account_push_timestamp.insert(
                        0,
                        format!(
                            "池子 : {:?}, 类型 : {:?}",
                            pool_id.to_string(),
                            if program_id == pumpfun_program_id {
                                DexType::PumpFunAMM.to_string()
                            } else if program_id == raydium_program_id {
                                DexType::RaydiumAMM.to_string()
                            } else {
                                "".to_string()
                            }
                        ),
                    );
                    if specify_pool.is_none_or(|v| &v == pool_id) {
                        Some((txn, account_push_timestamp))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    } else {
        receiver_msg.insert(txn, {
            let mut pools = SmallVec::with_capacity(3);
            pools.push((
                pool_id.clone(),
                {
                    let mut accounts = SmallVec::with_capacity(3);
                    accounts.push(account);
                    accounts
                },
                {
                    let mut timestamp = SmallVec::with_capacity(3);
                    timestamp.push(receiver_timestamp);
                    timestamp
                },
            ));
            pools
        });
        None
    }
}
