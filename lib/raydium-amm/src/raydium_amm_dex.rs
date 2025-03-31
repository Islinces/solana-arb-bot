use dex::interface::{Dex, Pool};
use crate::amm_pool::AmmPool;

pub struct RaydiumAmmDex {
    pub amm_pool: Vec<AmmPool>,
}

impl RaydiumAmmDex {
    pub fn new(amm_pool: Vec<AmmPool>) -> Self {
        Self { amm_pool }
    }
}

impl Dex for RaydiumAmmDex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>> {
        self.amm_pool
            .iter()
            .map(|pool| Box::new(pool.clone()) as Box<dyn Pool>)
            .collect()
    }
}
