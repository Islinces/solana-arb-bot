use crate::dlmm_pool::DlmmPool;
use dex::interface::{Dex, Pool};

pub struct MeteoraDlmmDex {
    pub pools: Vec<DlmmPool>,
}

impl MeteoraDlmmDex {
    pub fn new(pools: Vec<DlmmPool>) -> Self {
        Self { pools }
    }
}

impl Dex for MeteoraDlmmDex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>> {
        self.pools
            .iter()
            .map(|pool| Box::new(pool.clone()) as Box<dyn Pool>)
            .collect()
    }
}
