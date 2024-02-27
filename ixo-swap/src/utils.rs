use cosmwasm_std::{Decimal, StdError, StdResult, Uint128};

pub const SCALE_FACTOR: Uint128 = Uint128::new(10_000);
pub const MAX_FEE_PERCENT: &str = "1";
pub const MAX_PERCENT: &str = "100";
pub const DECIMAL_PRECISION: Uint128 = Uint128::new(10u128.pow(20));

pub fn decimal_to_uint128(decimal: Decimal) -> StdResult<Uint128> {
    let result: Uint128 = decimal
        .atomics()
        .checked_mul(SCALE_FACTOR)
        .map_err(StdError::overflow)?;

    Ok(result / DECIMAL_PRECISION)
}
