use crate::dlmm_pool::DlmmPool;
use dex::interface::{Dex, Pool};

pub struct MeteoraDlmmDex {
    pub dlmm_pools: Vec<DlmmPool>,
}

impl Dex for MeteoraDlmmDex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>> {
        self.dlmm_pools
            .iter()
            .map(|pool| Box::new(pool.clone()) as Box<dyn Pool>)
            .collect()
    }
}
