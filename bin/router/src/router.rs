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
        for pool in all_pools.iter() {
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

    pub fn find_route(&self, input_mint: Pubkey, amount_in: u64) -> Option<Vec<RouteStep>> {
        if let Some(edges) = self.mint_edge.get(&input_mint) {
            let mut route_step: Option<Vec<RouteStep>> = None;
            for (output_mint, pool_id) in edges {
                let amount_out = self
                    .pool
                    .read()
                    .unwrap()
                    .get(pool_id)
                    .unwrap()
                    .quote(amount_in, input_mint);
                println!(
                    "pool_id : {:?}, output_mint : {:?}，amount_in : {:?}, amount_out : {:?}",
                    pool_id, output_mint, amount_in, amount_out
                );
                if amount_out == u64::MIN {
                    continue;
                }
                let mut best_child_route_step = None;
                if let Some(next_edges) = self.mint_edge.get(output_mint) {
                    for (next_mint, next_pool_id) in next_edges {
                        if next_mint != &input_mint || pool_id == next_pool_id {
                            continue;
                        }
                        let next_amount_out = self
                            .pool
                            .read()
                            .unwrap()
                            .get(next_pool_id)
                            .unwrap()
                            .quote(amount_out, *output_mint);
                        println!(
                            "child step, next_pool_id : {:?}, output_mint : {:?}，amount_in : {:?}, amount_out : {:?}",
                            next_pool_id, next_mint, amount_out,next_amount_out
                        );
                        if next_amount_out == u64::MIN || next_amount_out <= amount_in {
                            continue;
                        }
                        match &best_child_route_step {
                            None => {
                                best_child_route_step = Some(RouteStep::new(
                                    *output_mint,
                                    *next_pool_id,
                                    amount_out,
                                    next_amount_out,
                                ))
                            }
                            Some(previous) => {
                                if previous.amount_out < next_amount_out {
                                    best_child_route_step = Some(RouteStep::new(
                                        *output_mint,
                                        *next_pool_id,
                                        amount_out,
                                        next_amount_out,
                                    ))
                                }
                            }
                        }
                    }
                }
                println!("best_child_route_step : {:?}", best_child_route_step);
                match best_child_route_step {
                    None => continue,
                    Some(_) => {}
                }
                println!("previous route step : {:#?}", route_step);
                println!(
                    "current route step : {:#?}",
                    vec![
                        RouteStep::new(input_mint, *pool_id, amount_in, amount_out),
                        best_child_route_step.unwrap()
                    ]
                );
                match route_step {
                    None => {
                        route_step = Some(vec![
                            RouteStep::new(input_mint, *pool_id, amount_in, amount_out),
                            best_child_route_step.unwrap(),
                        ]);
                    }
                    Some(ref mut previous) => {
                        let previous_final_amount_out = previous.last().unwrap().amount_out;
                        if previous_final_amount_out <= best_child_route_step.unwrap().amount_out {
                            previous.clear();
                            previous
                                .push(RouteStep::new(input_mint, *pool_id, amount_in, amount_out));
                            previous.push(best_child_route_step.unwrap());
                        }
                    }
                }
            }
            return route_step;
        }
        None
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
    pub pool_id: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
}

impl RouteStep {

    fn new(input_mint: Pubkey, pool_id: Pubkey, amount_in: u64, amount_out: u64) -> Self {
        Self {
            input_mint,
            pool_id,
            amount_in,
            amount_out,
        }
    }

}
