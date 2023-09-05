use std::{collections::HashMap, convert::TryInto};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, StdError, StdResult, Uint128, Uint256};
use cw1155::TokenId;

use crate::utils::{decimal_to_uint128, SCALE_FACTOR};

#[cw_serde]
pub enum TokenAmount {
    Multiple(HashMap<TokenId, Uint128>),
    Single(Uint128),
}

impl TokenAmount {
    pub fn get_multiple(&self) -> HashMap<TokenId, Uint128> {
        match self {
            TokenAmount::Multiple(amounts) => amounts.clone(),
            TokenAmount::Single(e) => {
                panic!("{}: {:?}", "Incompatible amount", &e)
            }
        }
    }

    pub fn get_single(&self) -> Uint128 {
        match self {
            TokenAmount::Single(amount) => amount.clone(),
            TokenAmount::Multiple(e) => {
                panic!("{}: {:?}", "Incompatible amount", &e)
            }
        }
    }

    pub fn get_total_amount(&self) -> Uint128 {
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

    pub fn get_percent_from_amount(&self, percent: Decimal) -> StdResult<Option<TokenAmount>> {
        if percent.is_zero() {
            return Ok(None);
        }

        let percent = decimal_to_uint128(percent)?;
        match self {
            TokenAmount::Multiple(amounts) => Ok(Some(Self::get_percent_from_multiple_amount(
                amounts.clone(),
                percent,
            )?)),
            TokenAmount::Single(amount) => Ok(Some(Self::get_percent_from_single_amount(
                amount.clone(),
                percent,
            )?)),
        }
    }

    fn get_percent_from_multiple_amount(
        input_amounts: HashMap<String, Uint128>,
        percent: Uint128,
    ) -> StdResult<TokenAmount> {
        let mut amounts: HashMap<TokenId, Uint128> = HashMap::new();
        let input_amounts_total = TokenAmount::Multiple(input_amounts.clone()).get_total_amount();
        let mut amount_left =
            Self::get_percent_from_single_amount(input_amounts_total, percent)?.get_single();

        while !amount_left.is_zero() {
            let amount_per_token = amount_left
                .checked_div(Uint128::from(input_amounts.len() as u32))
                .map_err(StdError::divide_by_zero)?;

            for (token_id, token_amount) in input_amounts.clone().into_iter() {
                if amount_left.is_zero() {
                    break;
                }

                let mut taken_amount_per_token =
                    *amounts.get(&token_id).unwrap_or(&Uint128::zero());
                if taken_amount_per_token == token_amount {
                    continue;
                }

                let token_amount_left = token_amount - taken_amount_per_token;
                let token_supply = if amount_per_token.is_zero() {
                    amount_left
                } else {
                    amount_per_token
                };

                if token_amount_left >= token_supply {
                    taken_amount_per_token += token_supply;

                    if amount_per_token.is_zero() {
                        amount_left = Uint128::zero();
                    } else {
                        amount_left -= amount_per_token;
                    }
                } else {
                    taken_amount_per_token += token_amount_left;
                    amount_left -= token_amount_left;
                }

                amounts.insert(token_id, taken_amount_per_token);
            }
        }

        Ok(TokenAmount::Multiple(amounts))
    }

    fn get_percent_from_single_amount(
        input_amount: Uint128,
        percent: Uint128,
    ) -> StdResult<TokenAmount> {
        Ok(TokenAmount::Single(
            input_amount
                .full_mul(percent)
                .checked_div(Uint256::from(SCALE_FACTOR))
                .map_err(StdError::divide_by_zero)?
                .try_into()?,
        ))
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
            .get_percent_from_amount(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total_amount(), Uint128::new(260))
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
            .get_percent_from_amount(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total_amount(), Uint128::new(260))
    }

    #[test]
    fn should_return_fee_amount_when_multiple_input_token_provided() {
        let token_amount = TokenAmount::Multiple(HashMap::from([
            ("1".to_string(), Uint128::new(9621)),
            ("2".to_string(), Uint128::new(15123)),
            ("3".to_string(), Uint128::new(1256)),
        ]));
        let fee = token_amount
            .get_percent_from_amount(Decimal::from_str("1").unwrap())
            .unwrap()
            .unwrap();

        assert_eq!(fee.get_total_amount(), Uint128::new(260))
    }
}
