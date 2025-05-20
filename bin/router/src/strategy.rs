use crate::collector::CollectorType;
use crate::executor::ExecutorType;
use crate::interface::DexType;
use base58::ToBase58;
use borsh::BorshDeserialize;
use burberry::{ActionSubmitter, Strategy};
use solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::ptr::read;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::Instant;
use tokio_stream::StreamExt;
use tracing::info;

pub struct SingleStrategy {
    pub receiver_msg: HashMap<String, Vec<(Pubkey, Vec<Pubkey>, Vec<u128>, Instant)>>,
}

#[burberry::async_trait]
impl Strategy<CollectorType, ExecutorType> for SingleStrategy {
    async fn process_event(
        &mut self,
        event: CollectorType,
        _submitter: Arc<dyn ActionSubmitter<ExecutorType>>,
    ) {
        match event {
            CollectorType::Single((
                tx,
                account_key,
                owner,
                filters,
                receiver_timestamp,
                instant,
            )) => {
                let txn = tx.unwrap().as_slice().to_base58();
                let ready_tx = match (account_key, owner) {
                    (None, None) => Some((txn, instant.elapsed().as_nanos())),
                    (Some(key), Some(owner)) => {
                        let pool_id_or_vault = Pubkey::try_from(key).unwrap();
                        let maybe_owner = Pubkey::try_from(owner).unwrap();
                        // 金库
                        let (pool_id, vault) = if maybe_owner
                            != DexType::RaydiumAMM.get_program_id()
                            && maybe_owner != DexType::PumpFunAMM.get_program_id()
                        {
                            let items = filters.first().unwrap().split(":").collect::<Vec<&str>>();
                            (
                                Pubkey::from_str(items.first().unwrap()).unwrap(),
                                Some(pool_id_or_vault),
                            )
                        } else {
                            (pool_id_or_vault, None)
                        };
                        if let Some(value) = self.receiver_msg.get_mut(&txn) {
                            match value.iter().position(|v| v.0 == pool_id) {
                                None => {
                                    value.push((
                                        pool_id,
                                        if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                            vault.map_or(vec![pool_id], |v| vec![v])
                                        } else {
                                            vault.map_or(vec![], |v| vec![v])
                                        },
                                        if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                            vault.map_or(vec![receiver_timestamp], |_| {
                                                vec![receiver_timestamp]
                                            })
                                        } else {
                                            vault.map_or(vec![], |_| vec![receiver_timestamp])
                                        },
                                        instant,
                                    ));
                                }
                                Some(index) => {
                                    let v = value.get_mut(index).unwrap();
                                    v.1.push(vault.map_or(pool_id, |v| v));
                                    v.2.push(receiver_timestamp);
                                }
                            }
                        } else {
                            self.receiver_msg.insert(
                                txn,
                                vec![(
                                    pool_id,
                                    if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                        vault.map_or(vec![pool_id], |v| vec![v])
                                    } else {
                                        vault.map_or(vec![], |v| vec![v])
                                    },
                                    if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                        vault.map_or(vec![receiver_timestamp], |_| {
                                            vec![receiver_timestamp]
                                        })
                                    } else {
                                        vault.map_or(vec![], |_| vec![receiver_timestamp])
                                    },
                                    instant,
                                )],
                            );
                        };
                        None
                    }
                    _ => None,
                };
                if let Some((txn, cost)) = ready_tx {
                    match self.receiver_msg.remove(&txn) {
                        None => {}
                        Some(ready_data) => {
                            let log = ready_data
                                .into_iter()
                                .map(|(pool_id, accounts, accounts_receiver_timestamp, _)| {
                                    let mut account_push_timestamp = accounts
                                        .into_iter()
                                        .zip(accounts_receiver_timestamp)
                                        .map(|(account, receiver_timestamp)| {
                                            format!(
                                                "账户 : {:?}, GRPC推送时间 : {:?}μs",
                                                account.to_string(),
                                                receiver_timestamp
                                            )
                                        })
                                        .collect::<Vec<_>>();
                                    account_push_timestamp
                                        .insert(0, format!("池子 : {:?}", pool_id.to_string()));
                                    account_push_timestamp
                                })
                                .collect::<Vec<_>>();
                            info!(
                                "\n单订阅 tx : {:?},\n耗时 : {:?}ns\n推送过程 : \n{:#?}",
                                txn, cost, log
                            );
                        }
                    }
                }
                ()
            }
            CollectorType::Multiple((
                tx,
                account_key,
                owner,
                filters,
                receiver_timestamp,
                instant,
            )) => {}
        }
    }
}

pub struct MultiStrategy {
    pub receiver_msg: HashMap<
        String,
        (
            Option<
                Vec<(
                    Option<Pubkey>,
                    Option<Pubkey>,
                    Vec<String>,
                    Vec<u128>,
                    Instant,
                )>,
            >,
            Option<u128>,
            Option<Instant>,
        ),
    >,
}

#[burberry::async_trait]
impl Strategy<CollectorType, ExecutorType> for MultiStrategy {
    async fn process_event(
        &mut self,
        event: CollectorType,
        _submitter: Arc<dyn ActionSubmitter<ExecutorType>>,
    ) {
        match event {
            CollectorType::Multiple((
                tx,
                account_key,
                owner,
                filters,
                receiver_timestamp,
                instant,
            )) => {
                let cost = instant.elapsed().as_nanos();
                let txn = tx.unwrap().as_slice().to_base58();
                info!("tx : {:?}, account : {:?}, owner : {:?}, receiver_timestamp : {:?}, instant : {:?}",
                    txn,
                    account_key.as_ref().map_or("".to_string(), |key| Pubkey::try_from(key.as_slice()).unwrap().to_string()),
                    owner.as_ref().map_or("".to_string(), |key| Pubkey::try_from(key.as_slice()).unwrap().to_string()),
                    receiver_timestamp,
                   instant.elapsed().as_nanos(),
                );
                match (account_key, owner) {
                    // tx
                    (None, None) => {
                        if let Some(value) = self.receiver_msg.get_mut(&txn) {
                            value.1 = Some(receiver_timestamp);
                            value.2 = Some(instant);
                        } else {
                            self.receiver_msg.insert(
                                txn.clone(),
                                (None, Some(receiver_timestamp), Some(instant)),
                            );
                        }
                    }
                    // pool or vault
                    (Some(key), Some(owner)) => {
                        let pool_id_or_vault = Pubkey::try_from(key).unwrap();
                        let maybe_owner = Pubkey::try_from(owner).unwrap();
                        // 金库
                        let (pool_id, vault, owner) = if maybe_owner
                            != DexType::RaydiumAMM.get_program_id()
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
                        if let Some(value) = self.receiver_msg.get_mut(&txn) {
                            if let Some(ref mut account_data) = value.0 {
                                match account_data
                                    .iter()
                                    .position(|v| v.0.as_ref().is_some_and(|key| key == &pool_id))
                                {
                                    None => {
                                        account_data.push((
                                            Some(pool_id),
                                            Some(owner),
                                            if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                                vault.map_or(vec![pool_id.to_string()], |v| {
                                                    vec![v.to_string()]
                                                })
                                            } else {
                                                vault.map_or(vec![], |v| vec![v.to_string()])
                                            },
                                            if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                                vault.map_or(vec![receiver_timestamp], |_| {
                                                    vec![receiver_timestamp]
                                                })
                                            } else {
                                                vault.map_or(vec![], |_| vec![receiver_timestamp])
                                            },
                                            instant,
                                        ));
                                    }
                                    Some(index) => {
                                        let v = account_data.get_mut(index).unwrap();
                                        v.2.push(
                                            vault.map_or(pool_id.to_string(), |v| v.to_string()),
                                        );
                                        v.3.push(receiver_timestamp);
                                    }
                                }
                            } else {
                                value.0 = Some(vec![(
                                    Some(pool_id),
                                    Some(owner),
                                    if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                        vault.map_or(vec![pool_id.to_string()], |v| {
                                            vec![v.to_string()]
                                        })
                                    } else {
                                        vault.map_or(vec![], |v| vec![v.to_string()])
                                    },
                                    if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                        vault.map_or(vec![receiver_timestamp], |_| {
                                            vec![receiver_timestamp]
                                        })
                                    } else {
                                        vault.map_or(vec![], |_| vec![receiver_timestamp])
                                    },
                                    instant,
                                )])
                            }
                        } else {
                            self.receiver_msg.insert(
                                txn.clone(),
                                (
                                    Some(vec![(
                                        Some(pool_id),
                                        Some(owner),
                                        if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                            vault.map_or(vec![pool_id.to_string()], |v| {
                                                vec![v.to_string()]
                                            })
                                        } else {
                                            vault.map_or(vec![], |v| vec![v.to_string()])
                                        },
                                        if maybe_owner == DexType::RaydiumAMM.get_program_id() {
                                            vault.map_or(vec![receiver_timestamp], |_| {
                                                vec![receiver_timestamp]
                                            })
                                        } else {
                                            vault.map_or(vec![], |_| vec![receiver_timestamp])
                                        },
                                        instant,
                                    )]),
                                    None,
                                    None,
                                ),
                            );
                        };
                    }
                    _ => {}
                };
                let receiver_data = self.receiver_msg.get(&txn).unwrap();
                let transaction_ready =
                    receiver_data.1.as_ref().is_some() && receiver_data.2.as_ref().is_some();
                // tx没过来
                if !transaction_ready {
                    return;
                }
                let ready_position = match receiver_data.0 {
                    None => {
                        vec![]
                    }
                    Some(ref account_datas) => {
                        let mut position = Vec::with_capacity(account_datas.len());
                        for (index, item) in account_datas.iter().enumerate() {
                            let pool_received =
                                item.0.as_ref().is_some() && item.1.as_ref().is_some();
                            if !pool_received {
                                continue;
                            }
                            let mut account_ready = false;
                            if item.1.as_ref().unwrap() == &DexType::RaydiumAMM.get_program_id() {
                                account_ready = item.2.len() == 3;
                            } else {
                                account_ready = item.2.len() == 2;
                            }
                            if account_ready {
                                position.push(index);
                            }
                        }
                        position
                    }
                };
                if !ready_position.is_empty() {
                    let value = self.receiver_msg.get_mut(&txn).unwrap();
                    let tx_receive_timestamp = value.1.unwrap();
                    let option = value.0.as_mut().unwrap();
                    let mut log = Vec::with_capacity(option.len());
                    let vec1 = option
                        .iter_mut()
                        .enumerate()
                        .flat_map(|(index, item)| {
                            if !ready_position.contains(&index) {
                                return None;
                            }
                            let mut item_log = Vec::with_capacity(10);
                            item_log.push(format!("池子 : {:?}", item.0.unwrap().to_string()));
                            let tx_position =
                                item.3.iter().position(|v| v >= &tx_receive_timestamp);
                            if let Some(position) = tx_position {
                                item.2.insert(position, txn.clone());
                                item.3.insert(position, tx_receive_timestamp);
                            } else {
                                item.2.push(txn.clone());
                                item.3.push(tx_receive_timestamp);
                            }
                            item_log.extend(
                                item.2
                                    .iter()
                                    .zip(item.3.iter())
                                    .map(|(account, timestamp)| {
                                        format!(
                                            "账户 : {:?}, GRPC推送时间 : {:?}",
                                            account.to_string(),
                                            timestamp
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            );
                            Some(item_log)
                        })
                        .collect::<Vec<_>>();
                    log.extend(vec1);
                    info!(
                        "\n多订阅 tx : {:?},\n耗时 : {:?}ns\n推送过程 : \n{:#?}",
                        txn, cost, log
                    );
                }
                ()
            }
            _ => {}
        }
    }
}
