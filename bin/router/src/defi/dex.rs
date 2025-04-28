use std::fmt::Debug;

#[async_trait::async_trait]
pub trait Dex: Send + Sync + Debug {
    fn quote(&self, amount_in: u64) -> Option<u64>;
}

