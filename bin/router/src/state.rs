use crate::interface::DexType;
use borsh::BorshDeserialize;
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Debug, Formatter};
use std::ops::Sub;
use yellowstone_grpc_proto::geyser::{SubscribeUpdateAccount, SubscribeUpdateTransactionInfo};
use yellowstone_grpc_proto::prelude::{TokenBalance, Transaction, TransactionStatusMeta};

#[derive(Debug, Clone)]
pub enum GrpcMessage {
    Account(GrpcAccountMsg),
    Transaction(GrpcTransactionMsg),
}

#[derive(Debug, Clone)]
pub struct GrpcAccountMsg {
    pub tx: Vec<u8>,
    pub account_key: Vec<u8>,
    pub owner_key: Vec<u8>,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub received_timestamp: DateTime<Local>,
}

impl From<SubscribeUpdateAccount> for GrpcAccountMsg {
    fn from(subscribe_update_account: SubscribeUpdateAccount) -> Self {
        let time = Local::now();
        let account = subscribe_update_account.account.unwrap();
        Self {
            tx: account.txn_signature.unwrap_or([0; 64].try_into().unwrap()),
            account_key: account.pubkey,
            owner_key: account.owner,
            data: account.data,
            write_version: account.write_version,
            received_timestamp: time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcTransactionMsg {
    pub signature: Vec<u8>,
    pub transaction: Option<Transaction>,
    pub meta: Option<TransactionStatusMeta>,
    pub _index: u64,
    pub received_timestamp: DateTime<Local>,
}

impl From<SubscribeUpdateTransactionInfo> for GrpcTransactionMsg {
    fn from(transaction: SubscribeUpdateTransactionInfo) -> Self {
        let time = Local::now();
        Self {
            signature: transaction.signature,
            transaction: transaction.transaction,
            meta: transaction.meta,
            _index: transaction.index,
            received_timestamp: time,
        }
    }
}

pub struct BalanceChangeInfo {
    pub dex_type: DexType,
    pub pool_id: Pubkey,
    pub account_index: usize,
    pub vault_account: Pubkey,
    pub change_value: f64,
}

impl BalanceChangeInfo {
    pub fn new(pre: &TokenBalance, post: &TokenBalance, account_keys: &[Pubkey]) -> Option<Self> {
        let account_index = pre.account_index as usize;
        match (pre.ui_token_amount.as_ref(), post.ui_token_amount.as_ref()) {
            (Some(pre_amount), Some(post_amount)) => {
                if pre_amount.ui_amount == post_amount.ui_amount {
                    None
                } else {
                    let vault_account = &account_keys[account_index];
                    match crate::account_relation::is_follow_vault(vault_account) {
                        Some((pool_id, dex_type)) => Some(Self {
                            dex_type,
                            pool_id,
                            account_index,
                            vault_account: vault_account.clone(),
                            change_value: post_amount.ui_amount.sub(pre_amount.ui_amount),
                        }),
                        None => None,
                    }
                }
            }
            _ => None,
        }
    }
}

impl Debug for BalanceChangeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("BalanceChangeInfo");
        formatter.field("dex_type", &self.dex_type);
        formatter.field("pool_id", &self.pool_id);
        formatter.field("vault_account", &self.vault_account.to_string());
        formatter.field("change_value", &self.change_value.to_string());
        formatter.finish()
    }
}