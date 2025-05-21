use crate::interface::{DexType, RAYDIUM_AMM_VAULT_OWNER};
use base58::ToBase58;
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;
use tracing::info;

pub struct MessageProcessor(pub bool, pub Option<String>);

impl MessageProcessor {
    pub async fn start(
        &mut self,
        mut message_receiver: UnboundedReceiver<(
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            Vec<String>,
            DateTime<Local>,
            Instant,
        )>,
        mut receiver_msg: HashMap<
            String,
            Vec<(Pubkey, Vec<Pubkey>, Vec<DateTime<Local>>, Instant)>,
        >,
    ) {
        let single_mode = self.0.clone();
        let specify_pool = self.1.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some((tx, account_key, owner, filters, receiver_timestamp, instant))  = message_receiver.recv() => {
                        let log = process_data(&mut receiver_msg, tx, account_key, owner, filters, receiver_timestamp, instant, &specify_pool);
                        if let Some((tx, cost, msg)) = log {
                            info!(
                                "\n{:?} tx : {:?},\n耗时 : {:?}ns\n推送过程 : \n{:#?}",
                                if single_mode {
                                    "单订阅"
                                }else{
                                    "多订阅"
                                },
                                tx, cost, msg
                            );
                        }
                    }
                }
            }
        });
    }
}

fn process_data(
    receiver_msg: &mut HashMap<String, Vec<(Pubkey, Vec<Pubkey>, Vec<DateTime<Local>>, Instant)>>,
    tx: Vec<u8>,
    account_key: Vec<u8>,
    owner: Vec<u8>,
    mut filters: Vec<String>,
    receiver_timestamp: DateTime<Local>,
    instant: Instant,
    specify_pool: &Option<String>,
) -> Option<(String, u128, Vec<String>)> {
    let txn = tx.as_slice().to_base58();
    let unique_key = filters.remove(0);
    let unique_key_items = unique_key.split(":").collect::<Vec<&str>>();
    let program_id = *unique_key_items.first().unwrap();
    let pool_id = Pubkey::from_str(*unique_key_items.last().unwrap()).unwrap();
    let account = Pubkey::try_from(account_key.clone()).unwrap();
    let wait_account_len = if program_id == DexType::RaydiumAMM.get_str_program_id() {
        3
    } else if program_id == DexType::PumpFunAMM.get_str_program_id() {
        2
    } else {
        0
    };
    // let mint_vault = if account == pool_id {
    //     None
    // } else {
    //     Some(account)
    // };
    info!(
        "tx : {:?}, account : {:?}, timestamp : {:?}",
        txn,
        Pubkey::try_from(account_key).unwrap(),
        receiver_timestamp
    );
    let ready_index = if let Some(value) = receiver_msg.get_mut(&txn) {
        match value.iter().position(|v| v.0 == pool_id) {
            None => {
                value.push((pool_id, vec![account], vec![receiver_timestamp], instant));
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
            vec![(pool_id, vec![account], vec![receiver_timestamp], instant)],
        );
        None
    };
    if let Some(position) = ready_index {
        let empty = match receiver_msg.get_mut(&txn) {
            None => (false, None),
            Some(ready_data) => {
                let (pool_id, accounts, timestamp, instant) = ready_data.remove(position);
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
                        if program_id == DexType::PumpFunAMM.get_str_program_id() {
                            DexType::PumpFunAMM.to_string()
                        } else if program_id == DexType::RaydiumAMM.get_str_program_id() {
                            DexType::RaydiumAMM.to_string()
                        } else {
                            "".to_string()
                        }
                    ),
                );
                (
                    ready_data.is_empty(),
                    if specify_pool
                        .as_ref()
                        .is_some_and(|v| v != &pool_id.to_string())
                    {
                        None
                    } else {
                        Some((
                            txn.clone(),
                            instant.elapsed().as_nanos(),
                            account_push_timestamp,
                        ))
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
