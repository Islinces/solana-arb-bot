use arrayref::array_ref;
use dex::interface::Pool;
use raydium_clmm::clmm_pool::ClmmPool;
use raydium_clmm::tickarray_bitmap_extension::TickArrayBitmapExtension;
use raydium_clmm::utils::deserialize_anchor_account;
use raydium_clmm::{config, pool};
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::Mint;
use std::str::FromStr;

#[test]
fn test_quote() {
    let rpc_client = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
    let pool = new_clmm_pool(
        rpc_client,
        Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        8,
    );
    let amount_in = 10_u64.pow(pool.mint_0_decimals as u32);
    let amount_out = pool.quote(
        amount_in,
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
    );
    println!(
        "amount_in: {} SOL, amount_out: {}",
        1,
        amount_out as f64 / 10_u64.pow(pool.mint_1_decimals as u32) as f64
    )
}

fn new_clmm_pool(
    rpc_client: RpcClient,
    program_id: Pubkey,
    mint_0: Pubkey,
    mint_1: Pubkey,
    amm_config_index: u16,
) -> ClmmPool {
    let mut mint0 = Some(mint_0);
    let mut mint1 = Some(mint_1);
    let (amm_config_key, __bump) = Pubkey::find_program_address(
        &[
            config::AMM_CONFIG_SEED.as_bytes(),
            &amm_config_index.to_be_bytes(),
        ],
        &program_id,
    );

    // 根据 AMM CONFIG ACCOUNT + MIN-0 + MINT-1 计算出池子地址 POOL ACCOUNT
    let pool_id_account = if mint0 != None && mint1 != None {
        if mint0.unwrap() > mint1.unwrap() {
            let temp_mint = mint0;
            mint0 = mint1;
            mint1 = temp_mint;
        }

        Some(
            Pubkey::find_program_address(
                &[
                    raydium_clmm::pool::POOL_SEED.as_bytes(),
                    amm_config_key.to_bytes().as_ref(),
                    mint0.unwrap().to_bytes().as_ref(),
                    mint1.unwrap().to_bytes().as_ref(),
                ],
                &program_id,
            )
            .0,
        )
    } else {
        None
    };

    // 根据 POOL ACCOUNT 计算
    let tickarray_bitmap_extension = if pool_id_account != None {
        Some(
            Pubkey::find_program_address(
                &[
                    raydium_clmm::pool::POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
                    pool_id_account.unwrap().to_bytes().as_ref(),
                ],
                &program_id,
            )
            .0,
        )
    } else {
        None
    };
    let load_accounts = vec![
        amm_config_key,
        pool_id_account.unwrap(),
        tickarray_bitmap_extension.unwrap(),
        mint0.unwrap(),
        mint1.unwrap(),
    ];
    let rsps = rpc_client.get_multiple_accounts(&load_accounts).unwrap();
    let [amm_config_account, pool_account, tickarray_bitmap_extension_account, mint0_account, mint1_account] =
        array_ref![rsps, 0, 5];
    let mint0_data = mint0_account.clone().unwrap().data;
    let mint0_state = StateWithExtensions::<Mint>::unpack(&mint0_data).unwrap();
    let mint1_data = mint1_account.clone().unwrap().data;
    let mint1_state = StateWithExtensions::<Mint>::unpack(&mint1_data).unwrap();
    let amm_config_state =
        deserialize_anchor_account::<config::AmmConfig>(amm_config_account.as_ref().unwrap())
            .unwrap();
    let pool_state =
        deserialize_anchor_account::<pool::PoolState>(pool_account.as_ref().unwrap()).unwrap();
    let tickarray_bitmap_extension = deserialize_anchor_account::<TickArrayBitmapExtension>(
        tickarray_bitmap_extension_account.as_ref().unwrap(),
    )
    .unwrap();
    ClmmPool {
        pool_id: pool_id_account.unwrap(),
        owner_id: program_id,
        amm_config: amm_config_key,
        mint_0: mint0.unwrap(),
        mint_1: mint1.unwrap(),
        mint_0_decimals: mint0_state.base.decimals,
        mint_1_decimals: mint1_state.base.decimals,
        tick_spacing: pool_state.tick_spacing,
        liquidity: pool_state.liquidity,
        sqrt_price_x64: pool_state.sqrt_price_x64,
        tick_current: pool_state.tick_current,
        tick_array_bitmap: pool_state.tick_array_bitmap,
        tick_array_bitmap_extension: tickarray_bitmap_extension,
        zero_to_one_tick_arays: raydium_clmm::utils::load_cur_and_next_specify_count_tick_array(
            &rpc_client,
            5,
            &pool_id_account.unwrap(),
            &program_id,
            &pool_state,
            &tickarray_bitmap_extension,
            true,
        ),
        one_to_zero_tick_arays: raydium_clmm::utils::load_cur_and_next_specify_count_tick_array(
            &rpc_client,
            5,
            &pool_id_account.unwrap(),
            &program_id,
            &pool_state,
            &tickarray_bitmap_extension,
            false,
        ),
        trade_fee_rate: amm_config_state.trade_fee_rate,
    }
}