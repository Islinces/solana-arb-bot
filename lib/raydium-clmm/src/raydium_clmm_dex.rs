use crate::clmm_pool::ClmmPool;
use dex::interface::{Dex, Pool};

pub struct RaydiumClmmDex {
    pub amm_pool: Vec<ClmmPool>,
}

impl RaydiumClmmDex {
    pub fn new(amm_pool: Vec<ClmmPool>) -> Self {
        Self { amm_pool }
    }
}

impl Dex for RaydiumClmmDex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>> {
        self.amm_pool
            .iter()
            .map(|pool| Box::new(pool.clone()) as Box<dyn Pool>)
            .collect()
    }
}
