use solana_sdk::pubkey::Pubkey;

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct TickArray {
    pub discriminator: [u8; 8],
    pub start_tick_index: i32,
    pub ticks: [Tick; 88],
    pub whirlpool: Pubkey,
}

impl TryInto<crate::dex::orca_whirlpools::accounts::tick_array::TickArray> for TickArray {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::tick_array::TickArray, Self::Error> {
        let mut ticks = [crate::dex::orca_whirlpools::accounts::tick_array::Tick::default(); 88];
        for (index, tick) in self.ticks.into_iter().enumerate() {
            ticks[index] = tick.try_into()?;
        }
        Ok(crate::dex::tick_array::TickArray {
            start_tick_index: self.start_tick_index,
            ticks,
            whirlpool: self.whirlpool,
        })
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    pub initialized: bool,
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
    pub fee_growth_outside_a: u128,
    pub fee_growth_outside_b: u128,
    pub reward_growths_outside: [u128; 3],
}

impl TryInto<crate::dex::orca_whirlpools::accounts::tick_array::Tick> for Tick {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crate::dex::tick_array::Tick, Self::Error> {
        Ok(crate::dex::tick_array::Tick {
            initialized: self.initialized,
            liquidity_net: self.liquidity_net,
            liquidity_gross: self.liquidity_gross,
        })
    }
}
