use crate::dex::meteora_dlmm::commons::{
    derive_bin_array_pda, BIN_ARRAY_BITMAP_SIZE, EXTENSION_BINARRAY_BITMAP_SIZE,
};
use crate::dex::meteora_dlmm::METEORA_DLMM_PROGRAM_ID;
use crate::dex::subscriber::{AccountSubscriber, SubscriptionAccounts};
use crate::dex_data::DexJson;
use solana_sdk::pubkey::Pubkey;
use tracing::error;

pub struct MeteoraDLMMAccountSubscriber;

impl AccountSubscriber for MeteoraDLMMAccountSubscriber {
    fn get_subscription_accounts(&self, dex_json: &[DexJson]) -> Option<SubscriptionAccounts> {
        let dex_json = dex_json
            .iter()
            .filter(|json| json.owner == METEORA_DLMM_PROGRAM_ID)
            .collect::<Vec<_>>();
        if dex_json.is_empty() {
            return None;
        }
        let subscribed_accounts = dex_json
            .iter()
            .filter_map(|json| {
                let mut accounts = Vec::with_capacity(13000);
                match get_all_bin_array_keys(&json.pool) {
                    Ok(bin_array_keys) => {
                        accounts.extend(bin_array_keys);
                    }
                    Err(_) => {
                        error!("[MeteoraDLMM]池子[{}]生成BinArray pubkey失败", json.pool);
                        return None;
                    }
                }
                accounts.push(json.pool);
                accounts.push(
                    crate::dex::meteora_dlmm::commons::derive_bin_array_bitmap_extension(
                        &json.pool,
                    ),
                );
                Some(accounts)
            })
            .flatten()
            .collect::<Vec<_>>();
        Some(SubscriptionAccounts {
            tx_include_accounts: vec![METEORA_DLMM_PROGRAM_ID],
            account_subscribe_owners: vec![METEORA_DLMM_PROGRAM_ID],
            subscribed_accounts,
            need_clock: true,
            ..Default::default()
        })
    }
}

pub fn get_all_bin_array_keys(pool_id: &Pubkey) -> anyhow::Result<Vec<Pubkey>> {
    let (min_bin_array_start_id, max_bin_array_start_id) = bitmap_range();
    Ok((min_bin_array_start_id..=max_bin_array_start_id)
        .into_iter()
        .map(|bin_start_id| derive_bin_array_pda(pool_id, bin_start_id as i64))
        .collect::<Vec<_>>())
}

fn bitmap_range() -> (i32, i32) {
    (
        -BIN_ARRAY_BITMAP_SIZE * (EXTENSION_BINARRAY_BITMAP_SIZE as i32 + 1),
        BIN_ARRAY_BITMAP_SIZE * (EXTENSION_BINARRAY_BITMAP_SIZE as i32 + 1) - 1,
    )
}
