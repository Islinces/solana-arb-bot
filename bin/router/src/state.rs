use std::fmt::{Debug, Display, Formatter};
use crate::interface::DexType;
use ahash::{AHashMap, AHashSet, AHasher};
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::Instant;
use yellowstone_grpc_proto::geyser::{SubscribeUpdateAccount, SubscribeUpdateTransactionInfo};
use yellowstone_grpc_proto::prelude::{
    TokenBalance, Transaction, TransactionStatusMeta, UiTokenAmount,
};

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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TxId(pub [u8; 64]);

impl Hash for TxId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut hasher = AHasher::default();
        self.0.hash(&mut hasher);
        state.write_u64(hasher.finish());
    }
}

impl From<Vec<u8>> for TxId {
    fn from(value: Vec<u8>) -> Self {
        let txn: [u8; 64] = value.try_into().unwrap();
        Self(txn)
    }
}

#[derive(Debug, Clone)]
pub struct CacheValue(pub (AHashSet<Pubkey>, Instant));

impl CacheValue {
    pub fn new(account: Pubkey, instant: Instant) -> Self {
        let mut value = Self((AHashSet::with_capacity(3), instant));
        value.insert(account);
        value
    }

    pub fn insert(&mut self, pubkey: Pubkey) {
        self.0 .0.insert(pubkey);
    }

    pub fn is_ready(&self, condition: impl FnOnce(usize) -> bool) -> bool {
        condition(self.0 .0.len())
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
    pub fn new(
        pre: &TokenBalance,
        post: &TokenBalance,
        account_keys: &Vec<String>,
        vault_to_pool: Arc<AHashMap<Pubkey, (Pubkey, Pubkey)>>,
    ) -> Option<Self> {
        let account_index = pre.account_index as usize;
        match (pre.ui_token_amount.as_ref(), post.ui_token_amount.as_ref()) {
            (Some(pre_amount), Some(post_amount)) => {
                if pre_amount.ui_amount == post_amount.ui_amount {
                    None
                } else {
                    let vault_account = Pubkey::from_str(&account_keys[account_index]).unwrap();
                    let owner = Pubkey::from_str(pre.owner.as_str()).unwrap();
                    match DexType::is_follow_vault(&vault_account, &owner, vault_to_pool.clone()) {
                        Some((dex_type, pool_id)) => Some(Self {
                            dex_type,
                            pool_id,
                            account_index,
                            vault_account,
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
