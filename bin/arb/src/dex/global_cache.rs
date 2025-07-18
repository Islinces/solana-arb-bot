use crate::dex::utils::read_from;
use crate::dex::{FromCache, CLOCK_ID, MINT2022_PROGRAM_ID, MINT_PROGRAM_ID};
use ahash::{AHashMap, RandomState};
use dashmap::DashMap;
use parking_lot::RwLock;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use std::sync::Arc;
use tokio::sync::OnceCell;

static GLOBAL_CACHE: OnceCell<GlobalCache> = OnceCell::const_new();

pub(crate) fn init_global_cache(cache: GlobalCache) {
    GLOBAL_CACHE.set(cache).unwrap()
}

pub fn get_global_cache() -> &'static GlobalCache {
    GLOBAL_CACHE.get().unwrap()
}

#[derive(Debug)]
pub struct DynamicCache(DashMap<Pubkey, Arc<Vec<u8>>, RandomState>);
#[derive(Debug)]
pub struct StaticCache(AHashMap<Pubkey, Arc<Vec<u8>>>);
#[derive(Debug)]
pub struct AltCache(AHashMap<Pubkey, Vec<AddressLookupTableAccount>>);

#[derive(Debug)]
pub struct GlobalCache {
    dynamic_account_cache: DynamicCache,
    static_account_cache: StaticCache,
    alt_cache: RwLock<AltCache>,
}

impl GlobalCache {
    pub fn init() -> Self {
        Self {
            dynamic_account_cache: DynamicCache::new(10000),
            static_account_cache: StaticCache::new(),
            alt_cache: RwLock::new(AltCache::new()),
        }
    }

    pub fn upsert_dynamic(&self, account_key: Pubkey, value: Vec<u8>) -> Option<Arc<Vec<u8>>> {
        self.dynamic_account_cache.insert(account_key, value)
    }

    pub fn upsert_static(&mut self, account_key: Pubkey, value: Vec<u8>) -> Option<Arc<Vec<u8>>> {
        self.static_account_cache.insert(account_key, value)
    }

    pub fn upsert_alt(&self, pool_id: Pubkey, alts: Vec<AddressLookupTableAccount>) {
        self.alt_cache.write().insert(pool_id, alts)
    }

    fn get_account_data<T: FromCache>(&self, account_key: &Pubkey) -> Option<T> {
        let static_data = self.static_account_cache.get(account_key);
        let dynamic_data = self.dynamic_account_cache.get(account_key);
        T::from_cache(static_data, dynamic_data).ok()
    }
}

impl DynamicCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self(DashMap::with_capacity_and_hasher_and_shard_amount(
            capacity,
            RandomState::default(),
            128,
        ))
    }

    pub fn get(&self, account_key: &Pubkey) -> Option<Arc<Vec<u8>>> {
        self.0.get(account_key).map(|v| v.value().clone())
    }

    pub fn insert(&self, account_key: Pubkey, data: Vec<u8>) -> Option<Arc<Vec<u8>>> {
        self.0.insert(account_key, Arc::new(data))
    }
}

impl StaticCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, account_key: &Pubkey) -> Option<Arc<Vec<u8>>> {
        self.0.get(account_key).map_or(None, |v| Some(v.clone()))
    }

    pub fn insert(&mut self, account_key: Pubkey, data: Vec<u8>) -> Option<Arc<Vec<u8>>> {
        self.0.insert(account_key, Arc::new(data))
    }
}

impl AltCache {
    pub(crate) fn new() -> Self {
        Self(AHashMap::with_capacity(1_000))
    }

    pub fn get(&self, pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
        self.0.get(pool_id).map_or(None, |v| Some(v.clone()))
    }

    pub fn insert(&mut self, pool_id: Pubkey, alts: Vec<AddressLookupTableAccount>) {
        if !alts.is_empty() {
            self.0.insert(pool_id, alts);
        }
    }
}

pub fn get_account_data<T: FromCache>(account_key: &Pubkey) -> Option<T> {
    get_global_cache().get_account_data::<T>(account_key)
}

pub fn get_token_program(mint: &Pubkey) -> Pubkey {
    if get_global_cache().static_account_cache.0.contains_key(mint) {
        MINT2022_PROGRAM_ID
    } else {
        MINT_PROGRAM_ID
    }
}

pub fn get_token2022_data(mint_key: &Pubkey) -> Option<TransferFeeConfig> {
    let static_data = &get_global_cache().static_account_cache;
    static_data.get(mint_key).map_or(None, |data| {
        StateWithExtensions::<spl_token_2022::state::Mint>::unpack(data.as_slice()).map_or(
            None,
            |mint_extensions| {
                mint_extensions
                    .get_extension::<TransferFeeConfig>()
                    .map_or(None, |result| Some(result.clone()))
            },
        )
    })
}

pub fn get_clock() -> Option<Clock> {
    get_global_cache()
        .dynamic_account_cache
        .get(&CLOCK_ID)
        .map_or(None, |result| {
            Some(unsafe { read_from::<Clock>(result.as_slice()) })
        })
}

pub fn get_alt(pool_id: &Pubkey) -> Option<Vec<AddressLookupTableAccount>> {
    get_global_cache().alt_cache.read().get(pool_id)
}

pub fn update_cache(account_key: Pubkey, data: Vec<u8>) -> anyhow::Result<()> {
    get_global_cache()
        .dynamic_account_cache
        .insert(account_key, data);
    Ok(())
}
