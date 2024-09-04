use std::convert::TryFrom;

use cosmwasm_std::{CheckedMultiplyFractionError, Decimal, StdError, StdResult, Uint128};

/// The minimum fee percent allowed is 0.01%, based of the SCALE_FACTOR,
/// otherwise it will always end up with 0 fee if lower than 0.01%
pub const MIN_FEE_PERCENT: &str = "0.01";
pub const SCALE_FACTOR: Uint128 = Uint128::new(10_000);
pub const PREDEFINED_MAX_FEES_PERCENT: &str = "5";
pub const PREDEFINED_MAX_SLIPPAGE_PERCENT: &str = "10";
pub const DECIMAL_PRECISION: Uint128 = Uint128::new(10u128.pow(20));

/// Converts a Decimal to a Uint128 with the SCALE_FACTOR applied, so that Uint128::1 is 0.01%
pub fn decimal_to_uint128(decimal: Decimal) -> StdResult<Uint128> {
    let result: Uint128 = decimal
        .atomics()
        .checked_mul(SCALE_FACTOR)
        .map_err(StdError::overflow)?;

    Ok(result / DECIMAL_PRECISION)
}

// Utility function to calculate amount based on percent
pub fn calculate_amount_with_percent(
    input_amount: Uint128,
    percent: Uint128,
) -> Result<Uint128, CheckedMultiplyFractionError> {
    if percent.is_zero() || input_amount.is_zero() {
        return Ok(Uint128::zero());
    }

    let fraction = (SCALE_FACTOR.u128(), 1u128);
    let result = input_amount
        .full_mul(percent)
        .checked_div_ceil(fraction)
        .map_err(|err| err)?;
    Ok(Uint128::try_from(result)?)
}