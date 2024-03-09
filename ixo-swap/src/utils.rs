use cosmwasm_std::{Decimal, StdError, StdResult, Uint128};

pub const SCALE_FACTOR: Uint128 = Uint128::new(10_000);
pub const PREDEFINED_MAX_PERCENT: &str = "1";
pub const PREDEFINED_MAX_SLIPPAGE_PERCENT: &str = "0.5";
pub const DECIMAL_PRECISION: Uint128 = Uint128::new(10u128.pow(20));

pub fn decimal_to_uint128(decimal: Decimal) -> StdResult<Uint128> {
    let result: Uint128 = decimal
        .atomics()
        .checked_mul(SCALE_FACTOR)
        .map_err(StdError::overflow)?;

    Ok(result / DECIMAL_PRECISION)
}
