use crate::dex::raydium_cpmm::curve::{ConstantProductCurve, Fees};

/// Concrete struct to wrap around the trait object which performs calculation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurveCalculator {}

impl CurveCalculator {
    /// Subtract fees and calculate how much destination token will be provided
    /// given an amount of source token.
    pub fn swap_base_input(
        source_amount: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        trade_fee_rate: u64,
    ) -> Option<u128> {
        // debit the fee to calculate the amount swapped
        let trade_fee = Fees::trading_fee(source_amount, trade_fee_rate)?;

        let source_amount_less_fees = source_amount.checked_sub(trade_fee)?;

        let destination_amount_swapped = ConstantProductCurve::swap_base_input_without_fees(
            source_amount_less_fees,
            swap_source_amount,
            swap_destination_amount,
        );
        Some(destination_amount_swapped)
    }
}
