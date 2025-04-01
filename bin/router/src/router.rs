use dex::account_write::AccountWrite;
use dex::interface::{Dex, Pool};
use solana_program::pubkey::Pubkey;
use spl_token_2022::state::Account;
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
    path_cache: RwLock<QuoteCache>,
}

struct QuoteCache {
    cache: HashMap<(Pubkey, Pubkey, u64), Vec<QuoteCacheEntry>>,
    last_expire_timestamp_millis: u64,
    max_age_millis: u64,
}

impl QuoteCache {
    fn get(
        &mut self,
        from_mint: &Pubkey,
        to_mint: &Pubkey,
        amount_in: u64,
        pool_id: &Pubkey,
    ) -> Option<(RouteStep)> {
        let Some(caches) = self.cache.get_mut(&(*from_mint, *to_mint, amount_in)) else {
            return None;
        };
        Self::expire_old_key(self.max_age_millis, caches);
        let mut path_cache = None;
        for path in caches {
            if path.pool_id == *pool_id {
                path_cache = Some(path.route_step);
                break;
            }
        }
        path_cache
    }

    fn insert(
        &mut self,
        from_mint: Pubkey,
        to_mint: Pubkey,
        amount_in: u64,
        path_cache_entry: QuoteCacheEntry,
    ) {
        let mut entry = self.cache.entry((from_mint, to_mint, amount_in));
        match entry {
            Entry::Occupied(ref mut path_caches) => {
                path_caches.get_mut().push(path_cache_entry);
                path_caches
                    .get_mut()
                    .sort_by_key(|entry| entry.timestamp_millis);
                path_caches.get_mut().reverse();
            }
            Entry::Vacant(path_caches) => {
                path_caches.insert(vec![path_cache_entry]);
            }
        }
    }

    fn invalidate_cache(&mut self, pool_id: &Pubkey) {
        self.cache.retain(|_, path_caches| {
            path_caches.retain(|entry| entry.pool_id != *pool_id);
            !path_caches.is_empty()
        });
    }

    fn expire_old_key(max_age_millis: u64, caches: &mut Vec<QuoteCacheEntry>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        caches.retain(|entry| {
            entry.timestamp_millis > now || now - entry.timestamp_millis < max_age_millis
        });
    }
}

struct QuoteCacheEntry {
    pool_id: Pubkey,
    amount_out: u64,
    timestamp_millis: u64,
    route_step: RouteStep,
}

impl QuoteCacheEntry {
    fn new(pool_id: Pubkey, amount_out: u64, route_step: RouteStep) -> Self {
        let timestamp_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self {
            pool_id,
            amount_out,
            timestamp_millis,
            route_step,
        }
    }
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
            path_cache: RwLock::new(QuoteCache {
                cache: Default::default(),
                last_expire_timestamp_millis: 0,
                max_age_millis: 10,
            }),
        }
    }

    pub fn find_route(
        &self,
        input_mint: Pubkey,
        amount_in: u64,
        changed_pool: Option<Pubkey>,
    ) -> Option<(RouteStep, RouteStep)> {
        if let Some(edges) = self.mint_edge.get(&input_mint) {
            let read_guard = self.pool.read().unwrap();
            let mut path_cache_writer = self.path_cache.write().unwrap();
            let mut route_step: Option<(RouteStep, RouteStep)> = None;
            for (output_mint, pool_id) in edges {
                // 尝试从缓存中获取
                // 未命中缓存，或命中缓存且 first_pool_id 和 changed_pool 一致时删除 first_pool_id 相关的缓存
                // TODO：何时更新pool数据？
                let mut first_route_step =
                    match path_cache_writer.get(&input_mint, &output_mint, amount_in, pool_id) {
                        None => None,
                        Some(path_cache) => match changed_pool {
                            None => None,
                            Some(chaned_pool_id) => {
                                if &chaned_pool_id == pool_id {
                                    path_cache_writer.invalidate_cache(pool_id);
                                    None
                                } else {
                                    Some(path_cache)
                                }
                            }
                        },
                    };
                let first_route_step = match first_route_step {
                    None => {
                        // 没有命中缓存，执行quote
                        let amount_out = read_guard
                            .get(pool_id)
                            .unwrap()
                            .quote(amount_in, input_mint);
                        let step = RouteStep::new(input_mint, *pool_id, amount_in, amount_out);
                        // 放入缓存
                        path_cache_writer.insert(
                            input_mint,
                            *output_mint,
                            amount_in,
                            QuoteCacheEntry::new(*pool_id, amount_out, step.clone()),
                        );
                        step
                    }
                    Some(route_step) => route_step,
                };
                let next_amount_in = first_route_step.amount_out;
                // amount_out为0，则跳过
                if next_amount_in == u64::MIN {
                    continue;
                }
                let first_pool_id = pool_id;
                let next_input_mint = output_mint;
                let mut second_best_route_step = None;
                if let Some(next_edges) = self.mint_edge.get(output_mint) {
                    for (next_out_mint, next_pool_id) in next_edges {
                        if first_pool_id == next_pool_id || next_out_mint != &input_mint {
                            continue;
                        }
                        let mut second_route_step_from_cache = match path_cache_writer.get(
                            &next_input_mint,
                            &next_out_mint,
                            next_amount_in,
                            next_pool_id,
                        ) {
                            None => {
                                let next_amount_out = read_guard
                                    .get(next_pool_id)
                                    .unwrap()
                                    .quote(next_amount_in, *next_input_mint);
                                let step = RouteStep::new(
                                    *next_input_mint,
                                    *next_pool_id,
                                    next_amount_in,
                                    next_amount_out,
                                );
                                path_cache_writer.insert(
                                    *next_input_mint,
                                    *next_out_mint,
                                    next_amount_in,
                                    QuoteCacheEntry::new(
                                        *next_pool_id,
                                        next_amount_out,
                                        step.clone(),
                                    ),
                                );
                                step
                            }
                            Some(v) => v,
                        };

                        if second_route_step_from_cache.amount_out == u64::MIN
                            || second_route_step_from_cache.amount_out <= amount_in
                        {
                            continue;
                        }
                        match second_best_route_step {
                            None => second_best_route_step = Some(second_route_step_from_cache),
                            Some(previous) => {
                                if previous.amount_out < second_route_step_from_cache.amount_out {
                                    second_best_route_step = Some(second_route_step_from_cache)
                                }
                            }
                        }
                    }
                }
                if let None = second_best_route_step {
                    continue;
                }
                let second_best_route_step = second_best_route_step.unwrap();
                match route_step {
                    None => route_step = Some((first_route_step, second_best_route_step)),
                    Some(ref mut previous) => {
                        let previous_final_amount_out = previous.1.amount_out;
                        if previous_final_amount_out <= second_best_route_step.amount_out {
                            previous.1 = second_best_route_step;
                        }
                    }
                }
            }
            return route_step;
        }
        None
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
