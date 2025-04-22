use arrayref::{array_ref, array_refs};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tracing::error;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestAccountsDataSlice,
    SubscribeRequestFilterAccounts, SubscribeUpdateAccount,
};
use yellowstone_grpc_proto::tonic::service::Interceptor;

#[derive(Debug, Copy, Clone)]
pub struct MintVaultUpdate {
    pub pubkey: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

impl MintVaultUpdate {
    pub fn subscribe_request_data_slices() -> Vec<SubscribeRequestAccountsDataSlice> {
        vec![
            // mint
            SubscribeRequestAccountsDataSlice {
                offset: 0,
                length: 32,
            },
            // amount
            SubscribeRequestAccountsDataSlice {
                offset: 64,
                length: 8,
            },
            // state
            SubscribeRequestAccountsDataSlice {
                offset: 108,
                length: 1,
            },
        ]
    }

    pub fn parse_mint_vault_by_filter_name(
        mint_vault_key: Pubkey,
        data: Vec<u8>,
        filter_name: String,
    ) -> (
        Option<Pubkey>,
        Option<MintVaultUpdate>,
        Option<MintVaultUpdate>,
    ) {
        let filter_split = filter_name.split(":").take(2).collect::<Vec<_>>();
        if filter_split[1].eq("0") {
            (
                Some(Pubkey::from_str(filter_split[0]).unwrap()),
                Some(MintVaultUpdate::from((mint_vault_key, data))),
                None,
            )
        } else {
            (
                Some(Pubkey::from_str(filter_split[0]).unwrap()),
                None,
                Some(MintVaultUpdate::from((mint_vault_key, data))),
            )
        }
    }
}

impl From<(Pubkey, Vec<u8>)> for MintVaultUpdate {
    fn from(value: (Pubkey, Vec<u8>)) -> Self {
        let src = array_ref![value.1.as_slice(), 0, 41];
        let (mint, amount, _state) = array_refs![src, 32, 8, 1];
        // let _state = AccountState::try_from(state[0]).unwrap();
        Self {
            pubkey: value.0,
            mint: Pubkey::from(mint.to_owned()),
            amount: u64::from_le_bytes(*amount),
        }
    }
}

pub struct GrpcClientCreator;

impl GrpcClientCreator {
    pub async fn create() -> anyhow::Result<GeyserGrpcClient<impl Interceptor>> {
        let mut builder =
            GeyserGrpcClient::build_from_static("https://solana-yellowstone-grpc.publicnode.com");
        builder = builder
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .buffer_size(65536)
            .initial_connection_window_size(5242880)
            .initial_stream_window_size(4194304)
            .connect_timeout(Duration::from_millis(10 * 1000));
        builder.connect().await.map_err(|e| {
            error!("failed to connect: {e}");
            anyhow::anyhow!(e)
        })
    }
}

pub struct MintVaultSubscribe;

impl MintVaultSubscribe {
    pub fn create_subscribe_request(
        pools: Vec<(Pubkey, Pubkey, Pubkey)>,
        commitment_level: Option<CommitmentLevel>,
    ) -> SubscribeRequest {
        SubscribeRequest {
            accounts: pools
                .into_iter()
                .flat_map(|(pool_id, mint_0_vault, mint_1_vault)| {
                    [
                        (
                            // mint_vault账户上没有关联的pool_id信息
                            // 通过filter_name在grpc推送消息时确定关联的pool
                            format!("{}:{}", pool_id, 0),
                            SubscribeRequestFilterAccounts {
                                account: vec![mint_0_vault.to_string()],
                                ..Default::default()
                            },
                        ),
                        (
                            format!("{}:{}", pool_id, 1),
                            SubscribeRequestFilterAccounts {
                                account: vec![mint_1_vault.to_string()],
                                ..Default::default()
                            },
                        ),
                    ]
                })
                .collect::<HashMap<_, _>>(),
            commitment: Some(commitment_level.unwrap_or(CommitmentLevel::Processed))
                .map(|x| x as i32),
            accounts_data_slice: MintVaultUpdate::subscribe_request_data_slices(),
            ..Default::default()
        }
    }
}
