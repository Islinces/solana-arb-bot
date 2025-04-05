use crate::pump_fun_pool::PumpFunPool;
use dex::interface::{Dex, Pool};

pub struct PumpFunDex {
    pub pools: Vec<PumpFunPool>,
}

impl Dex for PumpFunDex {
    fn get_pools(&self) -> Vec<Box<dyn Pool>> {
        self.pools
            .iter()
            .map(|pool| Box::new(*pool) as Box<dyn Pool>)
            .collect()
    }
}
