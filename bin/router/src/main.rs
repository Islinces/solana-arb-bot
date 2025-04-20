use chrono::Local;
use dex::interface::{DexInterface, DexPoolInterface, GrpcSubscriber};
use dex::state::FetchConfig;
use dex::util::tokio_spawn;
use log::{error, info};
use raydium_amm::raydium_amm_dex::{RaydiumAmmDex, RaydiumAmmGrpcSubscriber};
use router::router::Routing;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentLevel;
use std::env;
use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    env::set_var(
        env_logger::DEFAULT_FILTER_ENV,
        env::var_os(env_logger::DEFAULT_FILTER_ENV).unwrap_or_else(|| "info".into()),
    );
    env_logger::builder()
        .format(|buf, record| {
            write!(
                buf,
                "{} [{}] - {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S%.6f"),
                record.level(),
                record.args()
            )
        })
        .init();
    let grpc_url = "https://solana-yellowstone-grpc.publicnode.com";
    let rpc_url = "https://solana-rpc.publicnode.com";
    let fetch_config = Arc::new(FetchConfig {
        grpc_url: grpc_url.to_string(),
        rpc_url: rpc_url.to_string(),
        commitment_level: Some(CommitmentLevel::Finalized),
        subscribe_mints: vec![
            Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
        ],
    });
    let rpc_client = RpcClient::new(rpc_url.to_string());
    let raydium_amm_dex = RaydiumAmmDex::fetch_pool_base_info(&rpc_client, &fetch_config)
        .await
        .unwrap();
    let mut routing = Routing::from(vec![raydium_amm_dex.clone()]);
    let (snapshot_sender, mut snapshot_receiver) =
        tokio::sync::mpsc::unbounded_channel::<Box<dyn DexPoolInterface>>();
    let (trigger_route_sender, mut trigger_route_receiver) =
        tokio::sync::mpsc::unbounded_channel::<Box<dyn DexPoolInterface>>();

    tokio_spawn("raydium dex sub", async move {
        RaydiumAmmGrpcSubscriber::subscribe(
            raydium_amm_dex,
            fetch_config,
            snapshot_sender.clone(),
            trigger_route_sender.clone(),
        )
        .await;
    });
    loop {
        tokio::select! {
            data = snapshot_receiver.recv() => {
                match data {
                    Some(data) => {
                        info!("snapshot_receiver:{:?}",data);
                        if let Err(e)=routing.fill_snapshot(data){
                            error!("填充快照失败:{:?}",e);
                        }
                    }
                    None => {
                        error!("snapshot_receiver:None");
                    }
                }
            },
            data = trigger_route_receiver.recv()=>{
                match data {
                    Some(data) => {
                        info!("trigger_route_receiver:{:?}",data);
                        match &routing.trigger_after_update_pool(data,
                            Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
                            10_u64.pow(6)) {
                            Ok(pool)=>{
                                info!("swap成功:{:?}",pool);
                            },
                            Err(e)=>{
                                error!("swap失败:{}",e);
                            }
                        }
                    }
                    None => {
                        error!("trigger_route_receiver:None");
                    }
                }
            }
        }
    }
}
