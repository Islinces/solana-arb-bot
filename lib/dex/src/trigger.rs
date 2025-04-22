use chrono::Utc;
use solana_program::pubkey::Pubkey;
use std::any::Any;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Sub;
use std::time::Duration;
use tokio::time::MissedTickBehavior;

pub trait TriggerEvent: Any + Debug + Sync + Send {
    fn update_and_return_ready_event(
        &mut self,
        push_event: Box<dyn TriggerEvent>,
    ) -> Option<Box<dyn TriggerEvent>>;

    fn get_dex(&self) -> &str;

    fn get_txn(&self) -> String;

    fn get_pool_id(&self) -> Pubkey;

    fn get_create_timestamp(&self) -> i64;

    fn as_any(&self) -> &dyn Any;
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

    pub fn clear_expired_event(&mut self, expired_mills: usize) {
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

    pub fn new_holder_with_expired_interval(
        capacity: Option<usize>,
        expired_mills: Option<u64>,
        miss_behavior: Option<MissedTickBehavior>,
    ) -> (Self, tokio::time::Interval) {
        let mut clear_timeout_update_cache =
            tokio::time::interval(Duration::from_millis(expired_mills.unwrap_or(1000*60)));
        // 只保留最后一次
        clear_timeout_update_cache
            .set_missed_tick_behavior(miss_behavior.unwrap_or(MissedTickBehavior::Skip));
        (
            Self {
                events: HashMap::with_capacity(capacity.unwrap_or(1000)),
            },
            clear_timeout_update_cache,
        )
    }
}

impl Default for TriggerEventHolder {
    fn default() -> Self {
        Self {
            events: HashMap::with_capacity(1000),
        }
    }
}
