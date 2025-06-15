use solana_sdk::pubkey::Pubkey;

const TICK_ARRAY_SIZE_USIZE: usize = 60;
const REWARD_NUM: usize = 3;
#[repr(C, packed)]
pub struct TickArrayState {
    pub pool_id: Pubkey,
    pub start_tick_index: i32,
    pub ticks: [TickState; TICK_ARRAY_SIZE_USIZE],
    pub initialized_tick_count: u8,
    // account update recent epoch
    pub recent_epoch: u64,
    // Unused bytes for future upgrades.
    pub padding: [u8; 107],
}

impl TryInto<crate::dex::raydium_clmm::TickArrayState> for TickArrayState {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::TickArrayState, Self::Error> {
        let mut ticks = [crate::dex::raydium_clmm::TickState::default(); 60];
        for (index, tick) in self.ticks.into_iter().enumerate() {
            ticks[index] = tick.try_into()?;
        }
        Ok(crate::dex::TickArrayState {
            pool_id: self.pool_id,
            start_tick_index: self.start_tick_index,
            ticks,
        })
    }
}

#[repr(C, packed)]
#[derive(Default, Debug)]
pub struct TickState {
    pub tick: i32,
    /// Amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
    pub liquidity_net: i128,
    /// The total position liquidity that references this tick
    pub liquidity_gross: u128,

    /// Fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
    /// only has relative meaning, not absolute — the value depends on when the tick is initialized
    pub fee_growth_outside_0_x64: u128,
    pub fee_growth_outside_1_x64: u128,

    // Reward growth per unit of liquidity like fee, array of Q64.64
    pub reward_growths_outside_x64: [u128; REWARD_NUM],
    // Unused bytes for future upgrades.
    pub padding: [u32; 13],
}

impl TryInto<crate::dex::raydium_clmm::TickState> for TickState {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::TickState, Self::Error> {
        Ok(crate::dex::TickState {
            tick: self.tick,
            liquidity_net: self.liquidity_net,
            liquidity_gross: self.liquidity_gross,
        })
    }
}
