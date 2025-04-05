use dex::account_write::AccountWrite;
use dex::interface::{Dex, Pool};
use solana_program::pubkey::Pubkey;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::RwLock;

pub struct Routing {
    // key: mint
    // value: (mint,pool)
    pub mint_edge: HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
    // pub edge: Vec<Vec<Pubkey>>,
    // key ： 池子pubkey， value： 池子
    pub pool: RwLock<HashMap<Pubkey, Box<dyn Pool>>>,
}

impl Routing {
    pub fn new(dexs: Vec<Box<dyn Dex>>) -> Routing {
        let mut mint_edge = HashMap::<Pubkey, Vec<(Pubkey, Pubkey)>>::new();
        let all_pools: Vec<Box<dyn Pool>> = dexs.iter().flat_map(|dex| dex.get_pools()).collect();
        let mut pool_map = HashMap::<Pubkey, Box<dyn Pool>>::new();
        for pool in all_pools.into_iter() {
            pool_map.insert(pool.get_pool_id(), pool.clone_box());
            match mint_edge.entry(pool.get_mint_0()) {
                Entry::Occupied(mut value) => value
                    .get_mut()
                    .push((pool.get_mint_1(), pool.get_pool_id())),
                Entry::Vacant(value) => {
                    value.insert(vec![(pool.get_mint_1(), pool.get_pool_id())]);
                }
            }
            match mint_edge.entry(pool.get_mint_1()) {
                Entry::Occupied(mut value) => value
                    .get_mut()
                    .push((pool.get_mint_0(), pool.get_pool_id())),
                Entry::Vacant(value) => {
                    value.insert(vec![(pool.get_mint_0(), pool.get_pool_id())]);
                }
            }
        }
        Self {
            mint_edge,
            pool: RwLock::new(pool_map),
        }
    }

    pub fn find_route(
        &self,
        input_mint: Pubkey,
        amount_in: u64,
        changed_pool: Option<Pubkey>,
    ) -> Option<(RouteStep, RouteStep)> {
        if None == changed_pool {
            return None;
        }
        let pool_reader = self.pool.read().unwrap();
        let changed_pool_info = match pool_reader.get(&changed_pool.unwrap()) {
            None => return None,
            Some(v) => v,
        };
        let next_amount_in = match changed_pool_info.quote(amount_in, input_mint) {
            Some(v) if v > u64::MIN => v,
            _ => return None,
        };
        let next_input_mint = if input_mint == changed_pool_info.get_mint_0() {
            changed_pool_info.get_mint_1()
        } else {
            input_mint
        };
        let mut final_route_step = None;
        if let Some(edges) = self.mint_edge.get(&next_input_mint) {
            if let Some(best_second_step) = edges
                .iter()
                .filter(|(m, pool)| *m == input_mint && pool != &changed_pool.unwrap())
                .map(|(output_mint, pool_id)| {
                    if let Some(amount_out) = pool_reader
                        .get(&pool_id)?
                        .quote(next_amount_in, next_input_mint)
                    {
                        Some(RouteStep {
                            input_mint: next_input_mint,
                            output_mint: *output_mint,
                            pool_id: *pool_id,
                            amount_in: next_amount_in,
                            amount_out,
                        })
                    } else {
                        None
                    }
                })
                .filter(|result| result.is_some_and(|step| step.amount_out > amount_in))
                .map(|result| result.unwrap())
                .max_by_key(|result| result.amount_out)
            {
                final_route_step = Some((
                    RouteStep {
                        input_mint,
                        output_mint: next_input_mint,
                        pool_id: changed_pool.unwrap(),
                        amount_in,
                        amount_out: next_amount_in,
                    },
                    best_second_step,
                ));
            }
        }
        if final_route_step.is_some() {
            return final_route_step;
        }
        if let Some(edges) = self.mint_edge.get(&input_mint) {
            final_route_step = edges
                .iter()
                .filter(|(_, pool_id)| pool_id != &changed_pool.unwrap())
                .map(|(output_mint, pool_id)| {
                    if let Some(next_amount_in) =
                        pool_reader.get(pool_id)?.quote(amount_in, input_mint)
                    {
                        Some(RouteStep {
                            input_mint: input_mint,
                            output_mint: *output_mint,
                            pool_id: *pool_id,
                            amount_in: amount_in,
                            amount_out: next_amount_in,
                        })
                    } else {
                        None
                    }
                })
                .filter(|step| step.is_some())
                .map(|step| step.unwrap())
                .map(|step| {
                    if let Some(amount_out) =
                        changed_pool_info.quote(step.amount_out, step.output_mint)
                    {
                        Some((
                            step,
                            RouteStep {
                                input_mint: step.output_mint,
                                output_mint: step.input_mint,
                                pool_id: changed_pool.unwrap(),
                                amount_in: step.amount_out,
                                amount_out,
                            },
                        ))
                    } else {
                        None
                    }
                })
                .filter(|x| x.is_some_and(|x| x.1.amount_out > amount_in))
                .map(|x| x.unwrap())
                .max_by_key(|x| x.1.amount_out);
        }
        final_route_step
    }

    pub fn update_pool(&self, account_write: AccountWrite) {
        let write_guard = self.pool.write().unwrap();
        if let Some(pool) = write_guard.get(&account_write.pubkey) {
            pool.update_data(account_write);
        }
    }
}

impl Debug for Routing {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("mint_edge : {:#?}", self.mint_edge))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RouteStep {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub pool_id: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
}

// impl RouteStep {
//     fn new(input_mint: Pubkey, pool_id: Pubkey, amount_in: u64, amount_out: u64) -> Self {
//         Self {
//             input_mint,
//             pool_id,
//             amount_in,
//             amount_out,
//         }
//     }
// }
