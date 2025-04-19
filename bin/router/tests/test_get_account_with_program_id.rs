use raydium_amm::state::AmmInfo;
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::RpcFilterType;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;
use std::time::Duration;

#[tokio::test]
async fn test_get_account_with_program_id() {
    let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
    let rpc_client = RpcClient::new_with_timeouts_and_commitment(
        "https://solana-rpc.publicnode.com".to_string(),
        Duration::from_secs(60), // request timeout
        CommitmentConfig::confirmed(),
        Duration::from_secs(60), // confirmation timeout
    );
    // let slot = rpc_client.get_slot().await.unwrap();
    // println!("slot: {}", slot);
    let config = RpcProgramAccountsConfig {
        with_context: Some(false),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            min_context_slot: None,
            commitment: Some(CommitmentConfig::finalized()),
            data_slice: None,
        },
        filters: Some(vec![RpcFilterType::DataSize(
            std::mem::size_of::<AmmInfo>() as u64,
        )]),
    };
    let accounts = rpc_client
        .get_program_accounts_with_config(&program_id, config)
        .await
        .unwrap();
    println!("vec1: {:?}", accounts.len());
}
