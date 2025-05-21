use crate::interface::{DexType, RAYDIUM_AMM_VAULT_OWNER};
use ahash::{AHashMap, AHashSet};
use base58::ToBase58;
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;
use tracing::info;

pub struct MessageProcessor(pub bool, pub Option<Pubkey>);

impl MessageProcessor {
    pub async fn start(
        &mut self,
        mut message_receiver: UnboundedReceiver<(Vec<u8>, Vec<u8>, Vec<u8>, DateTime<Local>)>,
        mut receiver_msg: AHashMap<String, Vec<(Pubkey, Vec<Pubkey>, Vec<DateTime<Local>>)>>,
        pool_ids: AHashSet<Pubkey>,
        vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)>,
    ) {
        let single_mode = self.0.clone();
        let specify_pool = self.1.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some((tx, account_key, owner, receiver_timestamp))  = message_receiver.recv() => {
                        let log = process_data(
                            &mut receiver_msg, tx, account_key, owner, receiver_timestamp,
                            &specify_pool,&pool_ids,&vault_to_pool);
                        if let Some((tx, msg)) = log {
                            info!(
                                "\n{:?} tx : {:?}\n推送过程 : \n{:#?}",
                                if single_mode {
                                    "单订阅"
                                }else{
                                    "多订阅"
                                },
                                tx, msg
                            );
                        }
                    }
                }
            }
        });
    }
}

fn process_data(
    receiver_msg: &mut AHashMap<String, Vec<(Pubkey, Vec<Pubkey>, Vec<DateTime<Local>>)>>,
    tx: Vec<u8>,
    account_key: Vec<u8>,
    owner: Vec<u8>,
    receiver_timestamp: DateTime<Local>,
    specify_pool: &Option<Pubkey>,
    _pool_ids: &AHashSet<Pubkey>,
    vault_to_pool: &AHashMap<Pubkey, (Pubkey, Pubkey)>,
) -> Option<(String, Vec<String>)> {
    let txn = tx.as_slice().to_base58();
    let account = Pubkey::try_from(account_key.clone()).unwrap();
    let owner = Pubkey::try_from(owner.as_slice()).unwrap();
    let (pool_id, program_id) = if &owner == DexType::RaydiumAMM.get_ref_program_id()
        || &owner == DexType::PumpFunAMM.get_ref_program_id()
    {
        (account, owner)
    } else {
        vault_to_pool.get(&account).unwrap().clone()
    };

    let wait_account_len = if &program_id == DexType::RaydiumAMM.get_ref_program_id() {
        3
    } else if &program_id == DexType::PumpFunAMM.get_ref_program_id() {
        2
    } else {
        0
    };
    info!(
        "tx : {:?}, account : {:?}, pool_id : {:?}, program_id : {:?}, timestamp : {:?}",
        txn,
        account,
        pool_id,
        program_id,
        receiver_timestamp
    );
    let ready_index = if let Some(value) = receiver_msg.get_mut(&txn) {
        match value.iter().position(|v| v.0 == pool_id) {
            None => {
                value.push((pool_id, vec![account], vec![receiver_timestamp]));
                None
            }
            Some(index) => {
                let v = value.get_mut(index).unwrap();
                v.1.push(account);
                v.2.push(receiver_timestamp);
                if v.1.len() == wait_account_len {
                    Some(index)
                } else {
                    None
                }
            }
        }
    } else {
        receiver_msg.insert(
            txn.clone(),
            vec![(pool_id, vec![account], vec![receiver_timestamp])],
        );
        None
    };
    if let Some(position) = ready_index {
        let empty = match receiver_msg.get_mut(&txn) {
            None => (false, None),
            Some(ready_data) => {
                let (pool_id, accounts, timestamp) = ready_data.remove(position);
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
                        if &program_id == DexType::PumpFunAMM.get_ref_program_id() {
                            DexType::PumpFunAMM.to_string()
                        } else if &program_id == DexType::RaydiumAMM.get_ref_program_id() {
                            DexType::RaydiumAMM.to_string()
                        } else {
                            "".to_string()
                        }
                    ),
                );
                (
                    ready_data.is_empty(),
                    if specify_pool.as_ref().is_none() || specify_pool.as_ref().unwrap() == &account
                    {
                        Some((txn.clone(), account_push_timestamp))
                    } else {
                        None
                    },
                )
            }
        };
        if empty.0 {
            receiver_msg.remove(&txn);
        }
        empty.1
    } else {
        None
    }
}
