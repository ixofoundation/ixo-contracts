use std::{collections::HashMap, convert::TryFrom};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CheckedMultiplyFractionError, Decimal, StdError, Uint128};
use cw1155::TokenId;

use crate::{
    error::ContractError,
    utils::{decimal_to_uint128, SCALE_FACTOR},
};

#[cw_serde]
pub enum TokenAmount {
    Multiple(HashMap<TokenId, Uint128>),
    Single(Uint128),
}

impl TokenAmount {
    pub fn get_multiple(&self) -> Result<HashMap<TokenId, Uint128>, ContractError> {
        match self {
            TokenAmount::Multiple(amounts) => Ok(amounts.clone()),
            single_amount => Err(ContractError::InvalidTokenAmount {
                amount: single_amount.get_total(),
            }),
        }
    }

    pub fn get_single(&self) -> Result<Uint128, ContractError> {
        match self {
            TokenAmount::Single(amount) => Ok(amount.clone()),
            multiple_amount => Err(ContractError::InvalidTokenAmount {
                amount: multiple_amount.get_total(),
            }),
        }
    }

    pub fn get_total(&self) -> Uint128 {
        match self {
            TokenAmount::Multiple(amounts) => amounts
                .clone()
                .into_iter()
                .map(|(_, amount)| amount)
                .reduce(|acc, e| acc + e)
                .unwrap(),
            TokenAmount::Single(amount) => *amount,
        }
    }

    pub fn get_percent(&self, percent: Decimal) -> Result<Option<TokenAmount>, ContractError> {
        if percent.is_zero() {
            return Ok(None);
        }

        let percent = decimal_to_uint128(percent)?;
        match self {
            TokenAmount::Multiple(amounts) => Ok(Some(Self::get_percent_from_multiple(
                amounts.clone(),
                percent,
            )?)),
            TokenAmount::Single(amount) => Ok(Some(Self::get_percent_from_single(
                amount.clone(),
                percent,
            )?)),
        }
    }

    fn get_percent_from_multiple(
        input_amounts: HashMap<String, Uint128>,
        percent: Uint128,
    ) -> Result<TokenAmount, ContractError> {
        let mut amounts: HashMap<TokenId, Uint128> = HashMap::new();
        let input_amounts_total = TokenAmount::Multiple(input_amounts.clone()).get_total();
        let mut percent_amount_left =
            Self::get_percent_from_single(input_amounts_total, percent)?.get_single()?;

        while !percent_amount_left.is_zero() {
            let percent_amount_per_token = percent_amount_left
                .checked_div(Uint128::from(input_amounts.len() as u32))
                .map_err(StdError::divide_by_zero)?;

            for (token_id, token_amount) in input_amounts.clone().into_iter() {
                if percent_amount_left.is_zero() {
                    break;
                }

                let mut taken_percent_amount_per_token =
                    *amounts.get(&token_id).unwrap_or(&Uint128::zero());
                if taken_percent_amount_per_token == token_amount {
                    continue;
                }

                let token_amount_left = token_amount - taken_percent_amount_per_token;
                let percent_amount = if percent_amount_per_token.is_zero() {
                    percent_amount_left
                } else {
                    percent_amount_per_token
                };

                if token_amount_left >= percent_amount {
                    taken_percent_amount_per_token += percent_amount;

                    if percent_amount_per_token.is_zero() {
                        percent_amount_left = Uint128::zero();
                    } else {
                        percent_amount_left -= percent_amount_per_token;
                    }
                } else {
                    taken_percent_amount_per_token += token_amount_left;
                    percent_amount_left -= token_amount_left;
                }

                amounts.insert(token_id, taken_percent_amount_per_token);
            }
        }

        Ok(TokenAmount::Multiple(amounts))
    }

    fn get_percent_from_single(
        input_amount: Uint128,
        percent: Uint128,
    ) -> Result<TokenAmount, CheckedMultiplyFractionError> {
        let fraction = (SCALE_FACTOR.u128(), 1u128);
        let result = input_amount
            .full_mul(percent)
            .checked_div_ceil(fraction)
            .map_err(|err| err)?;
        Ok(TokenAmount::Single(Uint128::try_from(result)?))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn should_return_fee_amount_when_single_input_token_provided() {
        let token_amount = TokenAmount::Single(Uint128::new(26000));
        let fee = token_amount
            .get_percent(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total(), Uint128::new(260))
    }

    #[test]
    fn should_return_fee_amount_when_multiple_input_token_provided_and_two_token_amount_are_over() {
        let token_amount = TokenAmount::Multiple(HashMap::from([
            ("1".to_string(), Uint128::new(10)),
            ("2".to_string(), Uint128::new(17234)),
            ("3".to_string(), Uint128::new(10)),
            ("4".to_string(), Uint128::new(8746)),
        ]));
        let fee = token_amount
            .get_percent(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total(), Uint128::new(260))
    }

    #[test]
    fn should_return_fee_amount_when_multiple_input_token_provided() {
        let token_amount = TokenAmount::Multiple(HashMap::from([
            ("1".to_string(), Uint128::new(9621)),
            ("2".to_string(), Uint128::new(15123)),
            ("3".to_string(), Uint128::new(1256)),
        ]));
        let fee = token_amount
            .get_percent(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total(), Uint128::new(260))
    }

    #[test]
    fn should_return_error_when_get_multiple_called_for_single_amount() {
        let token_amount = TokenAmount::Single(Uint128::new(26000));
        let error = token_amount.get_multiple().err().unwrap();

        assert_eq!(
            ContractError::InvalidTokenAmount {
                amount: Uint128::new(26000)
            },
            error
        )
    }

    #[test]
    fn should_return_error_when_get_single_called_for_multiple_amount() {
        let token_amount = TokenAmount::Multiple(HashMap::from([
            ("1".to_string(), Uint128::new(1234)),
            ("2".to_string(), Uint128::new(4321)),
        ]));
        let error = token_amount.get_single().err().unwrap();

        assert_eq!(
            ContractError::InvalidTokenAmount {
                amount: Uint128::new(5555)
            },
            error
        )
    }
}
