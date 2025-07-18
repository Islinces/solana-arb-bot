use bytemuck::{Pod, Zeroable};
use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Debug, Clone, Default)]
#[cfg_attr(test, derive(Copy, Pod, Zeroable))]
pub struct GlobalConfig {
    pub discriminator: [u8; 8],
    pub admin: Pubkey,
    pub lp_fee_basis_points: u64,
    pub protocol_fee_basis_points: u64,
    pub disable_flags: u8,
    pub protocol_fee_recipients: [Pubkey; 8],
    pub coin_creator_fee_basis_points: u64,
}
