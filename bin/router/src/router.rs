use anyhow::anyhow;
use chrono::format::Numeric::Quarter;
use dex::interface::{DexInterface, DexPoolInterface};
use dex::trigger::TriggerEvent;
use solana_program::pubkey::Pubkey;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};
use tracing::info;

pub struct Routing {
    // key: mint
    // value: (mint,pool)
    pub mint_edge: HashMap<Pubkey, Vec<(Pubkey, Pubkey)>>,
    // pub edge: Vec<Vec<Pubkey>>,
    // key ： 池子pubkey， value： 池子
    pub pool: RwLock<HashMap<Pubkey, Box<dyn DexPoolInterface>>>,
}

impl Routing {
    pub fn from(all_dex: Vec<Arc<dyn DexInterface>>) -> Routing {
        let mut mint_edge = HashMap::<Pubkey, Vec<(Pubkey, Pubkey)>>::new();
        let all_pools = all_dex
            .into_iter()
            .flat_map(|dex| dex.get_base_pools())
            .collect::<Vec<_>>();
        let pool_size = all_pools.len();
        for pool in all_pools.into_iter() {
            // pool_map.insert(pool.get_pool_id(), pool.clone_box());
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
        info!(
            "Init Rout, size : {:?}, map : {:?}",
            mint_edge.len(),
            mint_edge
        );
        Self {
            mint_edge,
            pool: RwLock::new(HashMap::with_capacity(pool_size)),
        }
    }

    pub fn fill_snapshot(&mut self, pool: Box<dyn DexPoolInterface>) -> anyhow::Result<Pubkey> {
        if let Ok(mut write_guard) = self.pool.write() {
            let pool_id = pool.get_pool_id();
            info!("接收到池子{:?}快照", pool_id);
            write_guard.insert(pool_id, pool);
            Ok(pool_id)
        } else {
            Err(anyhow!(
                "填充快照[{:?}]获取pool写锁失败",
                pool.get_pool_id()
            ))
        }
    }

    pub fn trigger_after_update_pool(
        &mut self,
        trigger_event: Box<dyn TriggerEvent>,
        input_mint: Pubkey,
        amount_in: u64,
    ) -> anyhow::Result<Pubkey> {
        let pool_id = trigger_event.get_pool_id();
        let txn = trigger_event.get_txn();
        let changed_pool = if let Ok(mut write_guard) = self.pool.write() {
            match write_guard.entry(pool_id) {
                Entry::Occupied(mut exists_entry) => {
                    exists_entry.get_mut().update_data(trigger_event)
                }
                Entry::Vacant(_) => Err(anyhow!("")),
            }
        } else {
            return Err(anyhow!(
                "更新池子[{:?}]失败：获取pool写锁失败",
                trigger_event.get_pool_id()
            ));
        };
        if changed_pool.is_err() {
            return changed_pool;
        }
        info!("寻找路由: txn : {:?}, 触发池子: {:?}", txn, pool_id);
        let route_step = self.find_route(input_mint, amount_in, Some(pool_id.clone()));
        info!("route step : {:?}", route_step);
        if route_step.is_some() {
            changed_pool
        } else {
            Err(anyhow!("[{}]没有找到合适的路由", pool_id))
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
        info!(
            "当前池子{:?}作为输入开始寻找路由",
            changed_pool.as_ref().unwrap()
        );
        info!(
            "Step 1 : pool_id : {:?}, amount_in_mint : {:?}, amount_in : {}, amount_out : {}",
            changed_pool.as_ref().unwrap(),
            input_mint,
            amount_in,
            next_amount_in
        );
        if let Some(edges) = self.mint_edge.get(&next_input_mint) {
            if let Some(best_second_step) = edges
                .iter()
                .filter(|(m, pool)| {
                    let x1 = *m == input_mint && pool != &changed_pool.unwrap();
                    x1
                })
                .map(|(output_mint, pool_id)| {
                    if let Some(amount_out) = pool_reader
                        .get(&pool_id)?
                        .quote(next_amount_in, next_input_mint)
                    {
                        info!(
                            "Step 2 : pool_id : {:?}, amount_in_mint : {:?}, amount_in : {}, amount_out : {}",
                            pool_id,
                            next_input_mint,
                            next_amount_in,
                            amount_out
                        );
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
        info!(
            "当前池子{:?}作为输出开始寻找路由",
            changed_pool.as_ref().unwrap()
        );
        if let Some(edges) = self.mint_edge.get(&input_mint) {
            final_route_step = edges
                .iter()
                .filter(|(_, pool_id)| pool_id != &changed_pool.unwrap())
                .map(|(output_mint, pool_id)| {
                    if let Some(next_amount_in) =
                        pool_reader.get(pool_id)?.quote(amount_in, input_mint)
                    {
                        info!(
                            "Step 1 : pool_id : {:?}, amount_in_mint : {:?}, amount_in : {}, amount_out : {}",
                            pool_id,
                            input_mint,
                            amount_in,
                            next_amount_in
                        );
                        Some(RouteStep {
                            input_mint,
                            output_mint: *output_mint,
                            pool_id: *pool_id,
                            amount_in,
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
                        info!(
                            "Step 2 : pool_id : {:?}, amount_in_mint : {:?}, amount_in : {}, amount_out : {}",
                            changed_pool.as_ref().unwrap(),
                            step.output_mint,
                            step.amount_out,
                            amount_out
                        );
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
