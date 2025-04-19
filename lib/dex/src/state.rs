use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentLevel;
use yellowstone_grpc_proto::geyser::SubscribeUpdateAccountInfo;

#[derive(Default, Debug)]
pub struct FetchConfig {
    pub commitment_level: Option<CommitmentLevel>,
    pub subscribe_mints: Vec<Pubkey>,
    pub grpc_url: String,
    pub rpc_url: String,
}

pub enum SourceMessage {
    GrpcAccountUpdate(GrpcAccountUpdateType, SubscribeUpdateAccountInfo),
}

#[derive(Eq, PartialEq, Hash)]
pub enum GrpcAccountUpdateType {
    PoolState,
    MintVault,
}

impl From<usize> for GrpcAccountUpdateType {
    fn from(value: usize) -> Self {
        match value {
            0 => GrpcAccountUpdateType::PoolState,
            1 => GrpcAccountUpdateType::MintVault,
            _ => panic!("Invalid value"),
        }
    }
}
