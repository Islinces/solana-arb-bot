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
                path_caches.get_mut().insert(0, path_cache_entry);
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
            let (pool_reader, mut cache_writer) =
                (self.pool.read().unwrap(), self.path_cache.write().unwrap());

            let mut best_result = None;
            let mut max_profit = 0;
            for (output_mint, pool_id) in edges.iter().filter(|(m, pool)| *m != input_mint) {
                let first_step =
                    match cache_writer.get(&input_mint, output_mint, amount_in, pool_id) {
                        Some(cached) if changed_pool.map_or(true, |p| p != *pool_id) => cached,
                        _ => {
                            let amount_out = pool_reader.get(pool_id)?.quote(amount_in, input_mint);
                            let step = RouteStep::new(input_mint, *pool_id, amount_in, amount_out);
                            cache_writer.insert(
                                input_mint,
                                *output_mint,
                                amount_in,
                                QuoteCacheEntry::new(*pool_id, amount_out, step.clone()),
                            );
                            step
                        }
                    };

                if first_step.amount_out == u64::MIN {
                    continue;
                }

                if let Some(second_step) = self
                    .mint_edge
                    .get(output_mint)
                    .iter()
                    .flat_map(|edges| {
                        edges
                            .iter()
                            .filter(|(m, pid)| m == &input_mint && pid != pool_id)
                            .filter_map(|(_, next_pool_id)| {
                                cache_writer
                                    .get(
                                        output_mint,
                                        &input_mint,
                                        first_step.amount_out,
                                        next_pool_id,
                                    )
                                    .or_else(|| {
                                        let amount = pool_reader
                                            .get(next_pool_id)?
                                            .quote(first_step.amount_out, *output_mint);
                                        (amount > amount_in).then(|| {
                                            let step = RouteStep::new(
                                                *output_mint,
                                                *next_pool_id,
                                                first_step.amount_out,
                                                amount,
                                            );
                                            cache_writer.insert(
                                                *output_mint,
                                                input_mint,
                                                first_step.amount_out,
                                                QuoteCacheEntry::new(
                                                    *next_pool_id,
                                                    amount,
                                                    step.clone(),
                                                ),
                                            );
                                            step
                                        })
                                    })
                            })
                            .max_by_key(|s| s.amount_out)
                    })
                    .next()
                {
                    if second_step.amount_out > max_profit {
                        max_profit = second_step.amount_out;
                        best_result = Some((first_step, second_step));
                    }
                }
            }
            return best_result;
        }
        None
    }

    pub fn update_pool(&self, account_write: AccountWrite) {
        let write_guard = self.pool.write().unwrap();
        let mut cache_write_guard = self.path_cache.write().unwrap();
        if let Some(pool) = write_guard.get(&account_write.pubkey) {
            pool.update_data(account_write);
            cache_write_guard.invalidate_cache(&pool.get_pool_id());
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
