use crate::cache::Pool;
use crate::dex::common::utils::change_option_ignore_none_old;
use crate::interface::{DexType, GrpcAccountUpdateType, GrpcMessage, SubscribeKey};
use anyhow::anyhow;
use borsh::BorshDeserialize;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts,
};

#[derive(Debug, Clone)]
pub struct RaydiumAMMPoolState {
    pub mint_0_vault: Option<Pubkey>,
    pub mint_1_vault: Option<Pubkey>,
    pub mint_0_vault_amount: Option<u64>,
    pub mint_1_vault_amount: Option<u64>,
    pub mint_0_need_take_pnl: Option<u64>,
    pub mint_1_need_take_pnl: Option<u64>,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
}

impl RaydiumAMMPoolState {
    pub fn new(
        mint_0_vault: Option<Pubkey>,
        mint_1_vault: Option<Pubkey>,
        mint_0_vault_amount: Option<u64>,
        mint_1_vault_amount: Option<u64>,
        mint_0_need_take_pnl: Option<u64>,
        mint_1_need_take_pnl: Option<u64>,
        swap_fee_numerator: u64,
        swap_fee_denominator: u64,
    ) -> Self {
        Self {
            mint_0_vault,
            mint_1_vault,
            mint_0_vault_amount,
            mint_1_vault_amount,
            mint_0_need_take_pnl,
            mint_1_need_take_pnl,
            swap_fee_numerator,
            swap_fee_denominator,
        }
    }

    pub fn try_update(&mut self, grpc_message: GrpcMessage) -> anyhow::Result<()> {
        match grpc_message {
            GrpcMessage::RaydiumAmmMonitorData {
                mint_0_vault_amount,
                mint_1_vault_amount,
                mint_0_need_take_pnl,
                mint_1_need_take_pnl,
                ..
            } => {
                let mut changed = change_option_ignore_none_old(
                    &mut self.mint_0_vault_amount,
                    mint_0_vault_amount,
                );
                changed |= change_option_ignore_none_old(
                    &mut self.mint_1_vault_amount,
                    mint_1_vault_amount,
                );
                changed |= change_option_ignore_none_old(
                    &mut self.mint_0_need_take_pnl,
                    mint_0_need_take_pnl,
                );
                changed |= change_option_ignore_none_old(
                    &mut self.mint_1_need_take_pnl,
                    mint_1_need_take_pnl,
                );
                if changed {
                    Ok(())
                } else {
                    Err(anyhow!(""))
                }
            }
            _ => Err(anyhow!("")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RaydiumAMMInstructionItem {
    pub pool_id: Pubkey,
    pub mint_0: Pubkey,
    pub mint_1: Pubkey,
    pub mint_0_vault: Pubkey,
    pub mint_1_vault: Pubkey,
    pub alt: AddressLookupTableAccount,
    pub zero_to_one: bool,
}

impl Display for RaydiumAMMInstructionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {},{:?}",
            DexType::RaydiumAMM,
            self.pool_id,
            self.zero_to_one
        )
    }
}

#[derive(Debug, Clone, BorshDeserialize)]
pub struct PoolMonitorData {
    pub mint_0_need_take_pnl: u64,
    pub mint_1_need_take_pnl: u64,
}

impl PoolMonitorData {
    pub fn subscribe_request(pools: &[Pool]) -> (SubscribeKey, SubscribeRequest) {
        let mut subscribe_pool_accounts = HashMap::new();
        subscribe_pool_accounts.insert(
            format!("{:?}", DexType::RaydiumAMM),
            SubscribeRequestFilterAccounts {
                account: pools
                    .iter()
                    .map(|pool| pool.pool_id.to_string())
                    .collect::<Vec<_>>(),
                ..Default::default()
            },
        );
        let pool_request = SubscribeRequest {
            accounts: subscribe_pool_accounts,
            commitment: Some(CommitmentLevel::Processed).map(|x| x as i32),
            accounts_data_slice: vec![
                // state_data.need_take_pnl_coin
                SubscribeRequestAccountsDataSlice {
                    offset: 192,
                    length: 8,
                },
                // state_data.need_take_pnl_pc
                SubscribeRequestAccountsDataSlice {
                    offset: 200,
                    length: 8,
                },
            ],
            ..Default::default()
        };
        (
            (DexType::RaydiumAMM, GrpcAccountUpdateType::Pool),
            pool_request,
        )
    }
}
