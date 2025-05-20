use crate::interface::DexType;
use base58::ToBase58;
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;
use tracing::info;

pub struct MessageProcessor(pub bool);

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
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some((tx, account_key, owner, filters, receiver_timestamp, instant))  = message_receiver.recv() => {
                        let log = process_data(&mut receiver_msg, tx, account_key, owner, filters, receiver_timestamp, instant);
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
    filters: Vec<String>,
    receiver_timestamp: DateTime<Local>,
    instant: Instant,
) -> Option<(String, u128, Vec<String>)> {
    let txn = tx.as_slice().to_base58();
    let pool_id_or_vault = Pubkey::try_from(account_key).unwrap();
    let maybe_owner = Pubkey::try_from(owner).unwrap();
    info!(
        "tx : {:?}, account : {:?}, owner : {:?}, timestamp : {:?}",
        txn, pool_id_or_vault, maybe_owner, receiver_timestamp
    );
    // 金库
    let (pool_id, vault, owner) = if maybe_owner != DexType::RaydiumAMM.get_program_id()
        && maybe_owner != DexType::PumpFunAMM.get_program_id()
    {
        let items = filters.first().unwrap().split(":").collect::<Vec<&str>>();
        (
            Pubkey::from_str(items.first().unwrap()).unwrap(),
            Some(pool_id_or_vault),
            Pubkey::from_str(items.last().unwrap()).unwrap(),
        )
    } else {
        (pool_id_or_vault, None, maybe_owner)
    };
    let ready_index = if let Some(value) = receiver_msg.get_mut(&txn) {
        match value.iter().position(|v| v.0 == pool_id) {
            None => {
                value.push((
                    pool_id,
                    if owner == DexType::RaydiumAMM.get_program_id() {
                        vault.map_or(vec![pool_id], |v| vec![v])
                    } else {
                        vault.map_or(vec![], |v| vec![v])
                    },
                    if owner == DexType::RaydiumAMM.get_program_id() {
                        vault.map_or(vec![receiver_timestamp], |_| vec![receiver_timestamp])
                    } else {
                        vault.map_or(vec![], |_| vec![receiver_timestamp])
                    },
                    instant,
                ));
                None
            }
            Some(index) => {
                let v = value.get_mut(index).unwrap();
                v.1.push(vault.map_or(pool_id, |v| v));
                v.2.push(receiver_timestamp);
                if owner == DexType::RaydiumAMM.get_program_id() && v.1.len() == 3 {
                    Some(index)
                } else {
                    None
                }
            }
        }
    } else {
        receiver_msg.insert(
            txn.clone(),
            vec![(
                pool_id,
                if owner == DexType::RaydiumAMM.get_program_id() {
                    vault.map_or(vec![pool_id], |v| vec![v])
                } else {
                    vault.map_or(vec![], |v| vec![v])
                },
                if owner == DexType::RaydiumAMM.get_program_id() {
                    vault.map_or(vec![receiver_timestamp], |_| vec![receiver_timestamp])
                } else {
                    vault.map_or(vec![], |_| vec![receiver_timestamp])
                },
                instant,
            )],
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
                        if owner == DexType::PumpFunAMM.get_program_id() {
                            DexType::PumpFunAMM.to_string()
                        } else if owner == DexType::RaydiumAMM.get_program_id() {
                            DexType::RaydiumAMM.to_string()
                        } else {
                            "".to_string()
                        }
                    ),
                );
                (
                    ready_data.is_empty(),
                    Some((
                        txn.clone(),
                        instant.elapsed().as_nanos(),
                        account_push_timestamp,
                    )),
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
