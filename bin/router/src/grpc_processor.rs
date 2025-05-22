use crate::interface::{DexType, RAYDIUM_AMM_VAULT_OWNER};
use crate::strategy::process_data;
use ahash::{AHashMap, AHashSet};
use base58::ToBase58;
use chrono::{DateTime, Local};
use smallvec::SmallVec;
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
        mut message_receiver: UnboundedReceiver<(
            String,
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            DateTime<Local>,
        )>,
        mut receiver_msg: AHashMap<
            [u8; 64],
            SmallVec<
                [(
                    Pubkey,
                    SmallVec<[Pubkey; 3]>,
                    SmallVec<[DateTime<Local>; 3]>,
                ); 4],
            >,
        >,
        pool_ids: AHashSet<Pubkey>,
        vault_to_pool: AHashMap<Pubkey, (Pubkey, Pubkey)>,
    ) {
        let specify_pool = self.1.clone();
        tokio::spawn(async move {
            while let Some((a, tx, account_key, owner, receiver_timestamp)) =
                message_receiver.recv().await
            {
                let log = process_data(
                    &mut receiver_msg,
                    tx,
                    account_key,
                    owner,
                    receiver_timestamp,
                    &specify_pool,
                    &pool_ids,
                    &vault_to_pool,
                );
                if let Some((tx, msg)) = log {
                    info!("{}\ntx : {:?}\n推送过程 : \n{:#?}", a, tx, msg);
                }
            }
        });
    }
}
