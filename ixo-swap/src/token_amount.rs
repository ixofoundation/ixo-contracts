use std::{collections::HashMap, convert::TryFrom};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{CheckedMultiplyFractionError, Decimal, Uint128};
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
    /// Returns clone of the multiple amount of tokens.
    /// If the amount is a single amount, returns an error.
    pub fn get_multiple(&self) -> Result<HashMap<TokenId, Uint128>, ContractError> {
        match self {
            TokenAmount::Multiple(amounts) => Ok(amounts.clone()),
            single_amount => Err(ContractError::InvalidTokenAmount {
                amount: single_amount.get_total(),
            }),
        }
    }

    /// Returns clone of the single amount of tokens.
    /// If the amount is a multiple amount, returns an error.
    pub fn get_single(&self) -> Result<Uint128, ContractError> {
        match self {
            TokenAmount::Single(amount) => Ok(amount.clone()),
            multiple_amount => Err(ContractError::InvalidTokenAmount {
                amount: multiple_amount.get_total(),
            }),
        }
    }

    /// Returns the total amount of tokens.
    /// - If the amount is a single amount, returns the amount.
    /// - If the amount is a multiple amount, returns the sum of all amounts.
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

    /// Wrapper function that returns the amount of tokens for the provided percent.
    /// - If the percent is zero, returns None.
    /// - If the amount is a single amount, returns the single amount after running get_percent_from_single
    /// - If the amount is a multiple amount, returns the multiple amount after running get_percent_from_multiple
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

    /// Calculates the amount of tokens with the given percent from a multiple amount. Does so by:
    /// - getting the total percentage amount wanted, and use this as counter to run loop till it is 0
    /// - sort the input_amounts by amount (ascending), then by id (lexicographical order)
    /// - run for loop and add the amount of tokens wanted to the amounts HashMap, till percent_amount_left is zero
    ///
    /// NOTE: not all the tokens in the input_amounts will be included in the return(fee_amounts), only the ones with the
    /// lowest amounts tot total the percent needed
    fn get_percent_from_multiple(
        input_amounts: HashMap<String, Uint128>,
        percent: Uint128,
    ) -> Result<TokenAmount, ContractError> {
        let mut amounts: HashMap<TokenId, Uint128> = HashMap::new();
        let input_amounts_total = TokenAmount::Multiple(input_amounts.clone()).get_total();

        // Total percentage amount used as counter for the loop
        let mut percent_amount_left =
            Self::get_percent_from_single(input_amounts_total, percent)?.get_single()?;

        // Convert HashMap to Vec of tuples for deterministic sorting
        let mut sorted_input_amounts: Vec<(String, Uint128)> = input_amounts.clone().into_iter().collect();
        // Sort by amount (ascending), then by id (lexicographical order)
        sorted_input_amounts.sort_by(|a, b| {
            if a.1 == b.1 {
                a.0.cmp(&b.0) // Sort by TokenId if amounts are equal
            } else {
                a.1.cmp(&b.1) // Sort by amount (ascending order)
            }
        });

        for (token_id, token_amount) in sorted_input_amounts.into_iter() {
            if percent_amount_left.is_zero() {
                break;
            }

            // Determine the amount to take from the current token
            let take_amount = if token_amount >= percent_amount_left {
                percent_amount_left
            } else {
                token_amount
            };

            // Update the amounts HashMap with the determined take amount
            amounts.insert(token_id.clone(), take_amount);

            // Reduce the remaining amount to be taken
            percent_amount_left -= take_amount;
        }

        Ok(TokenAmount::Multiple(amounts))
    }

    /// Calculates the amount of tokens with the given percent from a single amount.
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
