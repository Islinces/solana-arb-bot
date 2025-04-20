use chrono::Utc;
use log::{info, warn};
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Sub;

pub trait TriggerEvent: Any + Debug + Sync + Send {
    fn update_and_return_ready_event(
        &mut self,
        push_event: Box<dyn TriggerEvent>,
    ) -> Option<Box<dyn TriggerEvent>>;

    fn get_txn(&self) -> String;

    fn get_pool_id(&self) -> Pubkey;

    fn get_create_timestamp(&self) -> i64;

    fn any(&self) -> &dyn Any;
}

pub struct TriggerEventHolder {
    events: HashMap<String, Vec<Box<dyn TriggerEvent>>>,
}

impl TriggerEventHolder {
    pub fn fetch_event(
        &mut self,
        trigger_event: Box<dyn TriggerEvent>,
    ) -> Option<Box<dyn TriggerEvent>> {
        match self.events.entry(trigger_event.get_txn()) {
            Entry::Occupied(mut entry) => {
                let option = entry
                    .get_mut()
                    .iter_mut()
                    .find(|event| event.get_pool_id().eq(&trigger_event.get_pool_id()));
                match option {
                    Some(event) => event.update_and_return_ready_event(trigger_event),
                    None => {
                        entry.get_mut().push(trigger_event);
                        None
                    }
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![trigger_event]);
                None
            }
        }
    }

    pub fn clear_timeout_event(&mut self, expired_mills: usize) {
        let timestamp_sec = Utc::now().timestamp();
        self.events.retain(|_, events| {
            events.retain(|event| {
                (timestamp_sec
                    .sub(event.get_create_timestamp())
                    .unsigned_abs() as usize)
                    < expired_mills
            });
            !events.is_empty()
        });
    }
}

impl Default for TriggerEventHolder {
    fn default() -> Self {
        Self {
            events: HashMap::with_capacity(1000),
        }
    }
}
