use crate::collector::CollectorType;
use crate::executor::ExecutorType;
use ahash::{AHashMap, AHashSet};
use burberry::{ActionSubmitter, Strategy};
use chrono::{DateTime, Local};
use smallvec::SmallVec;
use solana_sdk::pubkey::Pubkey;
use std::hash::Hash;
use std::sync::Arc;

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
                    // let log = process_data(
                    //     &mut self.receiver_msg,
                    //     tx,
                    //     account_key,
                    //     owner,
                    //     receiver_timestamp,
                    //     &self.specify_pool,
                    //     &self.pool_ids,
                    //     &self.vault_to_pool,
                    // );
                    // if let Some((tx, msg, cost)) = log {
                    //     info!(
                    //         "{}\ntx : {:?}\n耗时 : {}\n推送过程 : \n{:#?}",
                    //         a,
                    //         tx.as_slice().to_base58(),
                    //         cost,
                    //         msg
                    //     );
                    // }
                }
                _ => {}
            }
        }
    }
}


