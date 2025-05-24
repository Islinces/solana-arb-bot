use ahash::{AHashSet, AHasher};
use chrono::{DateTime, Local};
use solana_sdk::pubkey::Pubkey;
use std::hash::{Hash, Hasher};
use tokio::time::Instant;
use yellowstone_grpc_proto::geyser::SubscribeUpdateAccount;

pub struct GrpcMessage {
    pub tx: Vec<u8>,
    pub account_key: Vec<u8>,
    pub owner_key: Vec<u8>,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub received_timestamp: DateTime<Local>,
}

impl From<SubscribeUpdateAccount> for GrpcMessage {
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
