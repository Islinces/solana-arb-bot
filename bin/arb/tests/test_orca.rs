use arb::dex::orca_whirlpools::accounts::{
    get_oracle_address, get_tick_array_address, Oracle, TickArray, TickArrayFacade, TickFacade,
    WhirlpoolFacade, TICK_ARRAY_SIZE,
};
use arb::dex::orca_whirlpools::error::CoreError;
use arb::dex::orca_whirlpools::{
    get_tick_array_start_tick_index, swap_quote_by_input_token, ExactInSwapQuote, TransferFee,
    Whirlpool, WHIRLPOOL_ID,
};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::account::Account;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::extension::transfer_fee::TransferFeeConfig;
use spl_token_2022::extension::{BaseStateWithExtensions, StateWithExtensions};
use spl_token_2022::state::Mint;
use std::error::Error;
use std::iter::zip;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn test_orca() {
    let rpc = RpcClient::new("https://solana-rpc.publicnode.com".to_string());
    let whirlpool_address =
        Pubkey::from_str("DFVTutNYXD8z4T5cRdgpso1G3sZqQvMHWpW2N99E4DvE").unwrap();
    let mint_address = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let input_amount = 10_u64.pow(9) * 1;

    let result = swap_instructions(&rpc, whirlpool_address, input_amount, mint_address)
        .await
        .unwrap();

    println!("Quote estimated token out: {:#?}", result);
}

pub async fn swap_instructions(
    rpc: &RpcClient,
    whirlpool_address: Pubkey,
    amount: u64,
    specified_mint: Pubkey,
) -> Result<ExactInSwapQuote, CoreError> {
    let whirlpool_info = rpc.get_account(&whirlpool_address).await.unwrap();
    let whirlpool = Whirlpool::from_bytes(&whirlpool_info.data).unwrap();
    let specified_token_a = specified_mint == whirlpool.token_mint_a;

    let tick_arrays = fetch_tick_arrays_or_default(rpc, whirlpool_address, &whirlpool)
        .await
        .unwrap();

    let mint_infos = rpc
        .get_multiple_accounts(&[whirlpool.token_mint_a, whirlpool.token_mint_b])
        .await
        .unwrap();

    let mint_a_info = mint_infos[0]
        .as_ref()
        .ok_or(format!("Mint a not found: {}", whirlpool.token_mint_a))
        .unwrap();

    let mint_b_info = mint_infos[1]
        .as_ref()
        .ok_or(format!("Mint b not found: {}", whirlpool.token_mint_b))
        .unwrap();

    let oracle_address = get_oracle_address(&whirlpool_address).unwrap();
    let oracle = fetch_oracle(rpc, oracle_address, &whirlpool).await.unwrap();

    let current_epoch = rpc.get_epoch_info().await.unwrap().epoch;
    let transfer_fee_a = get_current_transfer_fee(Some(mint_a_info), current_epoch);
    let transfer_fee_b = get_current_transfer_fee(Some(mint_b_info), current_epoch);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    swap_quote_by_input_token(
        amount,
        specified_token_a,
        whirlpool.into(),
        oracle.map(|oracle| oracle.into()),
        tick_arrays.map(|x| x.1).into(),
        timestamp,
        transfer_fee_a,
        transfer_fee_b,
    )
}

async fn fetch_tick_arrays_or_default(
    rpc: &RpcClient,
    whirlpool_address: Pubkey,
    whirlpool: &Whirlpool,
) -> Result<[(Pubkey, TickArrayFacade); 5], Box<dyn Error>> {
    let tick_array_start_index =
        get_tick_array_start_tick_index(whirlpool.tick_current_index, whirlpool.tick_spacing);
    let offset = whirlpool.tick_spacing as i32 * TICK_ARRAY_SIZE as i32;

    let tick_array_indexes = [
        tick_array_start_index,
        tick_array_start_index + offset,
        tick_array_start_index + offset * 2,
        tick_array_start_index - offset,
        tick_array_start_index - offset * 2,
    ];

    let tick_array_addresses: Vec<Pubkey> = tick_array_indexes
        .iter()
        .map(|&x| get_tick_array_address(&whirlpool_address, x).map(|y| y.0))
        .collect::<Result<Vec<Pubkey>, _>>()?;

    let tick_array_infos = rpc.get_multiple_accounts(&tick_array_addresses).await?;

    let maybe_tick_arrays: Vec<Option<TickArrayFacade>> = tick_array_infos
        .iter()
        .map(|x| x.as_ref().and_then(|y| TickArray::from_bytes(&y.data).ok()))
        .map(|x| x.map(|y| y.into()))
        .collect();

    let tick_arrays: Vec<TickArrayFacade> = maybe_tick_arrays
        .iter()
        .enumerate()
        .map(|(i, x)| x.unwrap_or(uninitialized_tick_array(tick_array_indexes[i])))
        .collect::<Vec<TickArrayFacade>>();

    let result: [(Pubkey, TickArrayFacade); 5] = zip(tick_array_addresses, tick_arrays)
        .collect::<Vec<(Pubkey, TickArrayFacade)>>()
        .try_into()
        .map_err(|_| "Failed to convert tick arrays to array".to_string())?;

    Ok(result)
}

pub async fn fetch_oracle(
    rpc: &RpcClient,
    oracle_address: Pubkey,
    whirlpool: &Whirlpool,
) -> Result<Option<Oracle>, Box<dyn Error>> {
    // no need to fetch oracle for non-adaptive fee whirlpools
    if !whirlpool.is_initialized_with_adaptive_fee() {
        return Ok(None);
    }
    let oracle_info = rpc.get_account(&oracle_address).await?;
    Ok(Some(Oracle::from_bytes(&oracle_info.data)?))
}

fn uninitialized_tick_array(start_tick_index: i32) -> TickArrayFacade {
    TickArrayFacade {
        start_tick_index,
        ticks: [TickFacade::default(); TICK_ARRAY_SIZE],
    }
}

pub(crate) fn get_current_transfer_fee(
    mint_account_info: Option<&Account>,
    current_epoch: u64,
) -> Option<TransferFee> {
    let token_mint_data = &mint_account_info?.data;
    let token_mint_unpacked = StateWithExtensions::<Mint>::unpack(token_mint_data).ok()?;

    if let Ok(transfer_fee_config) = token_mint_unpacked.get_extension::<TransferFeeConfig>() {
        let fee = transfer_fee_config.get_epoch_fee(current_epoch);
        return Some(TransferFee {
            fee_bps: fee.transfer_fee_basis_points.into(),
            max_fee: fee.maximum_fee.into(),
        });
    }

    None
}
