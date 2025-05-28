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

#[repr(C)]
#[derive(Debug)]
pub struct RaydiumAmmPool {
    pub coin_need_take_pnl: u64,
    pub pc_need_take_pnl: u64,
}

impl RaydiumAmmPool {
    pub fn new(data: &[u8]) -> Self {
        Self {
            coin_need_take_pnl: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            pc_need_take_pnl: u64::from_le_bytes(data[8..16].try_into().unwrap()),
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct RaydiumClmmPool {
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    pub tick_array_bitmap: [u64; 16],
}

impl RaydiumClmmPool {
    pub fn new(data: &[u8]) -> Self {
        let mut tick_array_bitmap = [0u64; 16];
        for (i, chunk) in data[36..].chunks_exact(8).enumerate() {
            tick_array_bitmap[i] = u64::from_le_bytes(chunk.try_into().unwrap());
        }
        Self {
            liquidity: u128::from_le_bytes(data[0..16].try_into().unwrap()),
            sqrt_price_x64: u128::from_le_bytes(data[16..32].try_into().unwrap()),
            tick_current: i32::from_le_bytes(data[32..36].try_into().unwrap()),
            tick_array_bitmap,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Tick {
    pub tick: i32,
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
}

impl Tick {
    pub fn new(data: &[u8]) -> Self {
        Self {
            tick: i32::from_le_bytes(data[0..4].try_into().unwrap()),
            liquidity_net: i128::from_le_bytes(data[4..20].try_into().unwrap()),
            liquidity_gross: u128::from_le_bytes(data[20..36].try_into().unwrap()),
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct RaydiumClmmTickArray {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [Tick; 60],
    pub initialized_tick_count: u8,
}

impl RaydiumClmmTickArray {
    pub fn new(data: &[u8]) -> Self {
        let mut ticks = [Tick {
            tick: 0,
            liquidity_net: 0,
            liquidity_gross: 0,
        }; 60];
        for (i, chunk) in data[36..2196].chunks_exact(36).enumerate() {
            ticks[i] = Tick::new(chunk.try_into().unwrap());
        }
        Self {
            pool_id: Pubkey::try_from_slice(data[0..32].as_ref()).unwrap(),
            start_tick_index: i32::from_le_bytes(data[32..36].try_into().unwrap()),
            ticks,
            initialized_tick_count: u8::from_le_bytes(data[2196..2197].try_into().unwrap()),
        }
    }
}
