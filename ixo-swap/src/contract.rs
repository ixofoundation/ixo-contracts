use cosmwasm_std::{
    attr, entry_point, from_binary, to_binary, to_vec, Addr, Binary, BlockInfo, Coin,
    ContractResult, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply,
    Response, StdError, StdResult, SubMsg, SystemResult, Uint128, Uint256, Uint512, WasmMsg,
};
use cw0::parse_reply_instantiate_data;
use cw1155::{BatchBalanceResponse, Cw1155ExecuteMsg, Cw1155QueryMsg};
use cw1155_lp::{BatchBalanceForAllResponse, TokenInfo};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Expiration, MinterResponse};
use cw20_base::contract::query_balance;
use std::convert::TryFrom;
use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::{
    Denom, ExecuteMsg, FeeResponse, InfoResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    Token1ForToken2PriceResponse, Token2ForToken1PriceResponse, TokenResponse, TokenSelect, A,
};
use crate::state::{Fees, Token, FEES, FROZEN, LP_TOKEN, LP_TOKEN_ADDRESS, OWNER, TOKEN1, TOKEN2};

// Version info for migration info
pub const CONTRACT_NAME: &str = "crates.io:ixoswap";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_LP_TOKEN_REPLY_ID: u64 = 0;

const FEE_SCALE_FACTOR: Uint128 = Uint128::new(10_000);
const MAX_FEE_PERCENT: &str = "1";
const FEE_DECIMAL_PRECISION: Uint128 = Uint128::new(10u128.pow(20));

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let token1 = Token {
        reserves: vec![],
        denom: msg.token1_denom.clone(),
    };
    TOKEN1.save(deps.storage, &token1)?;

    let token2 = Token {
        reserves: vec![],
        denom: msg.token2_denom.clone(),
    };
    TOKEN2.save(deps.storage, &token2)?;

    LP_TOKEN.save(deps.storage, &msg.lp_token)?;

    let owner = msg.owner.map(|h| deps.api.addr_validate(&h)).transpose()?;
    OWNER.save(deps.storage, &owner)?;

    let protocol_fee_recipient = deps.api.addr_validate(&msg.protocol_fee_recipient)?;
    let total_fee_percent = msg.lp_fee_percent + msg.protocol_fee_percent;
    let max_fee_percent = Decimal::from_str(MAX_FEE_PERCENT)?;
    if total_fee_percent > max_fee_percent {
        return Err(ContractError::FeesTooHigh {
            max_fee_percent,
            total_fee_percent,
        });
    }

    let fees = Fees {
        lp_fee_percent: msg.lp_fee_percent,
        protocol_fee_percent: msg.protocol_fee_percent,
        protocol_fee_recipient,
    };
    FEES.save(deps.storage, &fees)?;

    // Depositing is not frozen by default
    FROZEN.save(deps.storage, &false)?;

    let submsg = if let Some(lp_token) = msg.lp_token {
        match lp_token {
            TokenSelect::Token1 => get_lp_token_instantiation_submessage(
                token1,
                msg.lp_token_code_id,
                env.contract.address.clone().into(),
            )
            .unwrap(),
            TokenSelect::Token2 => get_lp_token_instantiation_submessage(
                token2,
                msg.lp_token_code_id,
                env.contract.address.clone().into(),
            )
            .unwrap(),
        }
    } else {
        get_default_instantiation_submessage(
            msg.lp_token_code_id,
            env.contract.address.clone().into(),
        )
    };

    Ok(Response::new().add_submessage(submsg))
}

fn get_lp_token_instantiation_submessage(
    token: Token,
    code_id: u64,
    minter: String,
) -> Option<SubMsg> {
    match token.denom {
        Denom::Cw20(_) => Some(get_default_instantiation_submessage(code_id, minter)),
        Denom::Cw1155(_) => {
            let instantiate_lp_token_msg = WasmMsg::Instantiate {
                code_id,
                funds: vec![],
                admin: None,
                label: "lp_token".to_string(),
                msg: to_binary(&cw1155_base::msg::InstantiateMsg { minter }).unwrap(),
            };

            Some(SubMsg::reply_on_success(
                instantiate_lp_token_msg,
                INSTANTIATE_LP_TOKEN_REPLY_ID,
            ))
        }
        Denom::Native(_) => None,
    }
}

fn get_default_instantiation_submessage(code_id: u64, minter: String) -> SubMsg {
    let instantiate_lp_token_msg = WasmMsg::Instantiate {
        code_id,
        funds: vec![],
        admin: None,
        label: "lp_token".to_string(),
        msg: to_binary(&cw20_base::msg::InstantiateMsg {
            name: "IxoSwap_Liquidity_Token".into(),
            symbol: "islpt".into(),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse { minter, cap: None }),
            marketing: None,
        })
        .unwrap(),
    };

    SubMsg::reply_on_success(instantiate_lp_token_msg, INSTANTIATE_LP_TOKEN_REPLY_ID)
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AddLiquidity {
            input_token1,
            max_token2,
            min_liquidities,
            expiration,
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_add_liquidity(
                deps,
                &info,
                env,
                input_token1,
                max_token2,
                min_liquidities,
                expiration,
            )
        }
        ExecuteMsg::RemoveLiquidity {
            amounts,
            min_token1,
            min_token2,
            expiration,
        } => execute_remove_liquidity(deps, info, env, amounts, min_token1, min_token2, expiration),
        ExecuteMsg::Swap {
            input_token,
            input_amounts,
            min_outputs,
            expiration,
            ..
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_swap(
                deps,
                &info,
                input_amounts,
                env,
                input_token,
                info.sender.to_string(),
                min_outputs,
                expiration,
            )
        }
        ExecuteMsg::PassThroughSwap {
            output_amm_address,
            input_token,
            input_tokens_amount,
            output_min_tokens,
            expiration,
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_pass_through_swap(
                deps,
                info,
                env,
                output_amm_address,
                input_token,
                input_tokens_amount,
                output_min_tokens,
                expiration,
            )
        }
        ExecuteMsg::SwapAndSendTo {
            input_token,
            input_amounts,
            recipient,
            min_tokens,
            expiration,
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_swap(
                deps,
                &info,
                input_amounts,
                env,
                input_token,
                recipient,
                min_tokens,
                expiration,
            )
        }
        ExecuteMsg::UpdateConfig {
            owner,
            protocol_fee_recipient,
            lp_fee_percent,
            protocol_fee_percent,
        } => execute_update_config(
            deps,
            info,
            owner,
            lp_fee_percent,
            protocol_fee_percent,
            protocol_fee_recipient,
        ),
        ExecuteMsg::FreezeDeposits { freeze } => execute_freeze_deposits(deps, info.sender, freeze),
    }
}

fn execute_freeze_deposits(
    deps: DepsMut,
    sender: Addr,
    freeze: bool,
) -> Result<Response, ContractError> {
    if let Some(owner) = OWNER.load(deps.storage)? {
        if sender != owner {
            return Err(ContractError::UnauthorizedPoolFreeze {});
        }
    } else {
        return Err(ContractError::UnauthorizedPoolFreeze {});
    }

    FROZEN.save(deps.storage, &freeze)?;
    Ok(Response::new().add_attribute("action", "freezing-contracts"))
}

fn check_expiration(
    expiration: &Option<Expiration>,
    block: &BlockInfo,
) -> Result<(), ContractError> {
    match expiration {
        Some(e) => {
            if e.is_expired(block) {
                return Err(ContractError::MsgExpirationError {});
            }
            Ok(())
        }
        None => Ok(()),
    }
}

pub fn execute_add_liquidity(
    deps: DepsMut,
    info: &MessageInfo,
    env: Env,
    input_token1: Vec<TokenInfo>,
    max_token2: Vec<TokenInfo>,
    min_liquidities: Vec<Uint128>,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let lp_token_addr = LP_TOKEN_ADDRESS.load(deps.storage)?;

    // validate funds
    validate_input_amount(
        &info.funds,
        input_token1.first().unwrap().amount,
        &token1.denom,
    )?;
    validate_input_amount(
        &info.funds,
        max_token2.first().unwrap().amount,
        &token2.denom,
    )?;

    let lp_tokens = get_lp_tokens(input_token1, max_token2);
    let lp_token_supplies =
        get_lp_token_supplies(deps.as_ref(), &token1, &token2, &lp_token_addr, lp_tokens)?;
    let liquidity_amounts =
        get_lp_token_amounts_to_mint(&input_token1, &lp_token_supplies, &token1.reserves)?;

    let token2_amounts = get_token2_amounts_required(
        &max_token2,
        &token1_amounts,
        &lp_token_supply,
        &token2.reserves,
        &token1.reserves,
    )?;

    let invalid_liquidity = liquidity_amounts
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, amount)| !amount.is_zero() && amount < &min_liquidities[*index]);

    if let Some((index, amount)) = invalid_liquidity {
        return Err(ContractError::MinLiquidityError {
            min_liquidity: min_liquidities[index],
            liquidity_available: amount,
        });
    }

    let invalid_token2_amount = token2_amounts
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, amount)| amount > &max_token2[*index]);

    if let Some((index, amount)) = invalid_token2_amount {
        return Err(ContractError::MaxTokenError {
            max_token: max_token2[index],
            tokens_required: amount,
        });
    }

    // Generate cw20/cw1155 transfer messages if necessary
    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    match token1.denom {
        Denom::Cw20(addr) => transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            *token1_amounts.first().unwrap(),
        )?),
        Denom::Cw1155(denom) => transfer_msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &denom.address,
            &denom.tokens,
            &token1_amounts,
        )?),
        Denom::Native(_) => {}
    }

    match token2.denom.clone() {
        Denom::Cw20(addr) => transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            *token2_amounts.first().unwrap(),
        )?),
        Denom::Cw1155(denom) => transfer_msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &denom.address,
            &denom.tokens,
            &token2_amounts,
        )?),
        Denom::Native(_) => {}
    }

    // Refund token 2 if is a native token and not all is spent
    if let Denom::Native(ref denom) = token2.denom {
        let token2_amount = *token2_amounts.first().unwrap();
        let token2_max_amount = *max_token2.first().unwrap();

        if token2_amount < token2_max_amount {
            transfer_msgs.push(get_bank_transfer_to_msg(
                &info.sender,
                &denom,
                token2_max_amount - token2_amount,
            ))
        }
    }

    TOKEN1.update(deps.storage, |mut token1| -> Result<_, ContractError> {
        token1.reserves = token1
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, amount)| amount.checked_add(token1_amounts[index]).unwrap())
            .collect::<Vec<Uint128>>();

        Ok(token1)
    })?;
    TOKEN2.update(deps.storage, |mut token2| -> Result<_, ContractError> {
        token2.reserves = token2
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, amount)| amount.checked_add(token2_amounts[index]).unwrap())
            .collect::<Vec<Uint128>>();

        Ok(token2)
    })?;

    let mint_msg = mint_lp_tokens(&info.sender, &token1, &liquidity_amounts, &lp_token_addr)?;

    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_message(mint_msg)
        .add_attributes(vec![
            attr("token1_amount", format!("{:?}", token1_amounts)),
            attr("token2_amount", format!("{:?}", token2_amounts)),
            attr("liquidity_received", format!("{:?}", liquidity_amounts)),
        ]))
}

fn get_lp_token_amounts_to_mint(
    token1_amounts: &Vec<TokenInfo>,
    liquidity_supplies: &Vec<TokenInfo>,
    token1_reserves: &Vec<TokenInfo>,
) -> Result<Vec<TokenInfo>, ContractError> {
    Ok(liquidity_supplies
        .into_iter()
        .enumerate()
        .map(|(index, liquidity)| {
            if liquidity.amount.is_zero() {
                token1_amounts[index]
            } else {
                let reserve = token1_reserves
                    .into_iter()
                    .find(|reserve| reserve.id.unwrap() == liquidity.id.unwrap());
                let reserve_amount = if let Some(reserve) = reserve {
                    reserve.amount
                } else if let Some(reserve) = token1_reserves.first() {
                    reserve.amount
                } else {
                    Uint128::zero()
                };

                liquidity.amount = liquidity
                    .amount
                    .checked_mul(token1_amounts[index].amount)
                    .map_err(StdError::overflow)
                    .unwrap()
                    .checked_div(reserve_amount)
                    .map_err(StdError::divide_by_zero)
                    .unwrap();

                *liquidity
            }
        })
        .collect())
}

fn get_token2_amounts_required(
    max_token: &Vec<Uint128>,
    token1_amounts: &Vec<Uint128>,
    liquidity_supplies: &Vec<Uint128>,
    token2_reserves: &Vec<Uint128>,
    token1_reserves: &Vec<Uint128>,
) -> Result<Vec<Uint128>, StdError> {
    Ok(liquidity_supplies
        .into_iter()
        .enumerate()
        .map(|(index, liquidity)| {
            if liquidity.is_zero() {
                get_indexed_value(max_token, liquidity_supplies, index)
            } else {
                let amount = get_indexed_value(token1_amounts, liquidity_supplies, index);

                amount
                    .checked_mul(get_indexed_value(
                        token2_reserves,
                        liquidity_supplies,
                        index,
                    ))
                    .map_err(StdError::overflow)
                    .unwrap()
                    .checked_div(get_indexed_value(
                        token1_reserves,
                        liquidity_supplies,
                        index,
                    ))
                    .map_err(StdError::divide_by_zero)
                    .unwrap()
                    .checked_add(Uint128::new(1))
                    .map_err(StdError::overflow)
                    .unwrap()
            }
        })
        .collect())
}

fn get_indexed_value<T, U>(first_vec: &Vec<T>, second_vec: &Vec<U>, index: usize) -> T
where
    T: Copy,
{
    if first_vec.len() == second_vec.len() {
        first_vec[index]
    } else {
        *first_vec.first().unwrap()
    }
}

fn get_lp_tokens(
    token1_info: Vec<TokenInfo>,
    token2_info: Vec<TokenInfo>,
) -> Option<Vec<TokenInfo>> {
    let token1_ids: Vec<TokenInfo> = get_ids_from_tokens_info(token1_info);
    let token2_ids: Vec<TokenInfo> = get_ids_from_tokens_info(token2_info);
    let lp_tokens = [token1_ids, token2_ids].concat();

    if lp_tokens.len() != 0 {
        Some(lp_tokens)
    } else {
        None
    }
}

fn get_ids_from_tokens_info(token_info: Vec<TokenInfo>) -> Vec<TokenInfo> {
    token_info
        .into_iter()
        .filter(|info| info.id.is_some())
        .collect()
}

fn get_lp_token_supplies(
    deps: Deps,
    token1: &Token,
    token2: &Token,
    lp_token_addr: &Addr,
    lp_tokens: Option<Vec<TokenInfo>>,
) -> StdResult<Vec<TokenInfo>> {
    let lp_token = LP_TOKEN.load(deps.storage)?;

    if let Some(lp_token) = LP_TOKEN.load(deps.storage)? {
        match lp_token {
            TokenSelect::Token1 => {
                get_token_supply_by_denom(deps, lp_token_addr, lp_tokens, token1.denom)
            }
            TokenSelect::Token2 => {
                get_token_supply_by_denom(deps, lp_token_addr, lp_tokens, token2.denom)
            }
        }
    } else {
        let resp: cw20::TokenInfoResponse = deps
            .querier
            .query_wasm_smart(lp_token_addr, &cw20_base::msg::QueryMsg::TokenInfo {})?;

        Ok(vec![TokenInfo {
            id: None,
            amount: resp.total_supply,
            uri: None,
        }])
    }
}

fn get_token_supply_by_denom(
    deps: Deps,
    lp_token_addr: &Addr,
    lp_tokens: Option<Vec<TokenInfo>>,
    denom: Denom,
) -> StdResult<Vec<TokenInfo>> {
    match denom {
        Denom::Cw20(_) | Denom::Native(_) => {
            let resp: cw20::TokenInfoResponse = deps
                .querier
                .query_wasm_smart(lp_token_addr, &cw20_base::msg::QueryMsg::TokenInfo {})?;

            Ok(vec![TokenInfo {
                id: None,
                amount: resp.total_supply,
                uri: None,
            }])
        }
        Denom::Cw1155(_) => {
            if let Some(lp_tokens) = lp_tokens {
                let resp: BatchBalanceResponse = deps.querier.query_wasm_smart(
                    lp_token_addr,
                    &cw1155_lp::Cw1155QueryMsg::BatchBalanceForTokens {
                        token_ids: lp_tokens.into_iter().map(|info| info.id.unwrap()).collect(),
                    },
                )?;

                Ok(lp_tokens
                    .into_iter()
                    .enumerate()
                    .map(|(index, info)| TokenInfo {
                        id: info.id,
                        amount: resp.balances[index],
                        uri: info.uri,
                    })
                    .collect())
            } else {
                let resp: BatchBalanceForAllResponse = deps.querier.query_wasm_smart(
                    lp_token_addr,
                    &cw1155_lp::Cw1155QueryMsg::BatchBalanceForAll {},
                )?;

                Ok(resp.balances)
            }
        }
    }
}

fn mint_lp_tokens(
    recipient: &Addr,
    token: &Token,
    liquidity_amounts: &Vec<Uint128>,
    lp_token_address: &Addr,
) -> StdResult<CosmosMsg> {
    let mint_execute_msg = match &token.denom {
        Denom::Cw20(_) | Denom::Native(_) => {
            let mint_msg = cw20_base::msg::ExecuteMsg::Mint {
                recipient: recipient.into(),
                amount: *liquidity_amounts.first().unwrap(),
            };

            WasmMsg::Execute {
                contract_addr: lp_token_address.to_string(),
                msg: to_binary(&mint_msg)?,
                funds: vec![],
            }
        }
        Denom::Cw1155(denom) => {
            let mint_msg = cw1155_lp::Cw1155ExecuteMsg::BatchMint {
                to: recipient.into(),
                batch: denom
                    .tokens
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(index, (id, uri))| {
                        (
                            id,
                            get_indexed_value(liquidity_amounts, &denom.tokens, index),
                            uri,
                        )
                    })
                    .collect(),
                msg: None,
            };

            WasmMsg::Execute {
                contract_addr: lp_token_address.to_string(),
                msg: to_binary(&mint_msg)?,
                funds: vec![],
            }
        }
    };

    Ok(mint_execute_msg.into())
}

fn get_token_balances(
    deps: Deps,
    token1: &Token,
    token2: &Token,
    contract: &Addr,
    addr: &Addr,
) -> StdResult<Vec<Uint128>> {
    let mut denom = token1.denom.clone();

    match &token2.denom {
        Denom::Cw20(_) => denom = token2.denom.clone(),
        Denom::Cw1155(_) => denom = token2.denom.clone(),
        Denom::Native(_) => {}
    }

    match denom {
        Denom::Cw20(_) | Denom::Native(_) => {
            let resp: cw20::BalanceResponse = deps.querier.query_wasm_smart(
                contract,
                &cw20_base::msg::QueryMsg::Balance {
                    address: addr.to_string(),
                },
            )?;
            Ok(vec![resp.balance])
        }
        Denom::Cw1155(denom) => {
            let resp: BatchBalanceResponse = deps.querier.query_wasm_smart(
                contract,
                &Cw1155QueryMsg::BatchBalance {
                    owner: addr.to_string(),
                    token_ids: denom.tokens.into_iter().map(|(id, _)| id).collect(),
                },
            )?;

            Ok(resp.balances)
        }
    }
}

fn validate_input_amount(
    actual_funds: &[Coin],
    given_amount: Uint128,
    given_denom: &Denom,
) -> Result<(), ContractError> {
    match given_denom {
        Denom::Cw20(_) => Ok(()),
        Denom::Cw1155(_) => Ok(()),
        Denom::Native(denom) => {
            let actual = get_amount_for_denom(actual_funds, denom);
            if actual.amount != given_amount {
                return Err(ContractError::InsufficientFunds {});
            }
            if &actual.denom != denom {
                return Err(ContractError::IncorrectNativeDenom {
                    provided: actual.denom,
                    required: denom.to_string(),
                });
            };
            Ok(())
        }
    }
}

fn get_cw20_transfer_from_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::TransferFrom {
        owner: owner.into(),
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        funds: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();
    Ok(cw20_transfer_cosmos_msg)
}

fn get_cw20_increase_allowance_msg(
    token_addr: &Addr,
    spender: &Addr,
    amount: Uint128,
    expires: Option<Expiration>,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let increase_allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: spender.to_string(),
        amount,
        expires,
    };
    let exec_allowance = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&increase_allowance_msg)?,
        funds: vec![],
    };
    Ok(exec_allowance.into())
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Option<String>,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if Some(info.sender) != owner {
        return Err(ContractError::Unauthorized {});
    }

    let new_owner_addr = new_owner
        .as_ref()
        .map(|h| deps.api.addr_validate(h))
        .transpose()?;
    OWNER.save(deps.storage, &new_owner_addr)?;

    let total_fee_percent = lp_fee_percent + protocol_fee_percent;
    let max_fee_percent = Decimal::from_str(MAX_FEE_PERCENT)?;
    if total_fee_percent > max_fee_percent {
        return Err(ContractError::FeesTooHigh {
            max_fee_percent,
            total_fee_percent,
        });
    }

    let protocol_fee_recipient = deps.api.addr_validate(&protocol_fee_recipient)?;
    let updated_fees = Fees {
        protocol_fee_recipient: protocol_fee_recipient.clone(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    FEES.save(deps.storage, &updated_fees)?;

    let new_owner = new_owner.unwrap_or_default();
    Ok(Response::new().add_attributes(vec![
        attr("new_owner", new_owner),
        attr("lp_fee_percent", lp_fee_percent.to_string()),
        attr("protocol_fee_percent", protocol_fee_percent.to_string()),
        attr("protocol_fee_recipient", protocol_fee_recipient.to_string()),
    ]))
}

pub fn execute_remove_liquidity(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    amounts: Vec<Uint128>,
    min_token1: Vec<Uint128>,
    min_token2: Vec<Uint128>,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let lp_token_addr = LP_TOKEN_ADDRESS.load(deps.storage)?;
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let balances = get_token_balances(
        deps.as_ref(),
        &token1,
        &token2,
        &lp_token_addr,
        &info.sender,
    )?;
    let lp_token_supplies = get_lp_token_supplies(deps.as_ref(), &token1, &token2, &lp_token_addr)?;

    let insufficient_amount = amounts
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, amount)| amount > &balances[*index]);

    if let Some((index, amount)) = insufficient_amount {
        return Err(ContractError::InsufficientLiquidityError {
            requested: amount,
            available: balances[index],
        });
    }

    let token1_amounts = amounts
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, amount)| {
            amount
                .checked_mul(token1.reserves[index])
                .map_err(StdError::overflow)
                .unwrap()
                .checked_div(lp_token_supplies[index])
                .map_err(StdError::divide_by_zero)
                .unwrap()
        })
        .collect::<Vec<Uint128>>();

    let invalid_min_token1 = token1_amounts
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, amount)| amount < &min_token1[*index]);

    if let Some((index, token1_amount)) = invalid_min_token1 {
        return Err(ContractError::MinToken1Error {
            requested: min_token1[index],
            available: token1_amount,
        });
    }

    let token2_amounts = amounts
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, amount)| {
            amount
                .checked_mul(token2.reserves[index])
                .map_err(StdError::overflow)
                .unwrap()
                .checked_div(lp_token_supplies[index])
                .map_err(StdError::divide_by_zero)
                .unwrap()
        })
        .collect::<Vec<Uint128>>();

    let invalid_min_token2 = token2_amounts
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, amount)| amount < &min_token2[*index]);

    if let Some((index, token2_amount)) = invalid_min_token2 {
        return Err(ContractError::MinToken2Error {
            requested: min_token2[index],
            available: token2_amount,
        });
    }

    TOKEN1.update(deps.storage, |mut token1| -> Result<_, ContractError> {
        token1.reserves = token1
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, reserve)| {
                reserve
                    .checked_sub(token1_amounts[index])
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect();

        Ok(token1)
    })?;

    TOKEN2.update(deps.storage, |mut token2| -> Result<_, ContractError> {
        token2.reserves = token2
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, reserve)| {
                reserve
                    .checked_sub(token2_amounts[index])
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect();

        Ok(token2)
    })?;

    let token1_transfer_msg = match token1.denom.clone() {
        Denom::Cw20(addr) => {
            get_cw20_transfer_to_msg(&info.sender, &addr, *token1_amounts.first().unwrap())?
        }
        Denom::Cw1155(denom) => get_cw1155_transfer_msg(
            &env.contract.address,
            &info.sender,
            &denom.address,
            &denom.tokens,
            &token1_amounts,
        )?,
        Denom::Native(denom) => {
            get_bank_transfer_to_msg(&info.sender, &denom, *token1_amounts.first().unwrap())
        }
    };
    let token2_transfer_msg = match token2.denom.clone() {
        Denom::Cw20(addr) => {
            get_cw20_transfer_to_msg(&info.sender, &addr, *token2_amounts.first().unwrap())?
        }
        Denom::Cw1155(denom) => get_cw1155_transfer_msg(
            &env.contract.address,
            &info.sender,
            &denom.address,
            &denom.tokens,
            &token2_amounts,
        )?,
        Denom::Native(denom) => {
            get_bank_transfer_to_msg(&info.sender, &denom, *token2_amounts.first().unwrap())
        }
    };

    let lp_token = get_lp_token(deps, token1, token2);
    let lp_token_burn_msg = get_burn_msg(&lp_token_addr, &info.sender, &amounts, lp_token)?;

    Ok(Response::new()
        .add_messages(vec![
            token1_transfer_msg,
            token2_transfer_msg,
            lp_token_burn_msg,
        ])
        .add_attributes(vec![
            attr("liquidity_burned", format!("{:?}", amounts)),
            attr("token1_returned", format!("{:?}", token1_amounts)),
            attr("token2_returned", format!("{:?}", token2_amounts)),
        ]))
}

fn get_lp_token(deps: DepsMut, token1: Token, token2: Token) -> Option<Token> {
    if let Some(lp_token) = LP_TOKEN.load(deps.storage).unwrap() {
        match lp_token {
            TokenSelect::Token1 {} => Some(token1),
            TokenSelect::Token2 {} => Some(token2),
        }
    } else {
        None
    }
}

fn get_burn_msg(
    contract: &Addr,
    owner: &Addr,
    amounts: &Vec<Uint128>,
    token: Option<Token>,
) -> StdResult<CosmosMsg> {
    if token.is_none() {
        get_cw20_burn_msg(contract, owner, amounts)
    } else {
        let token = token.unwrap();

        match token.denom {
            Denom::Cw1155(denom) => get_cw1155_burn_msg(denom, contract, owner, amounts),
            _ => get_cw20_burn_msg(contract, owner, amounts),
        }
    }
}

fn get_cw1155_burn_msg(
    denom: Cw1155Denom,
    contract: &Addr,
    owner: &Addr,
    amounts: &Vec<Uint128>,
) -> StdResult<CosmosMsg> {
    let msg = cw1155_lp::Cw1155ExecuteMsg::BatchBurn {
        from: owner.to_string(),
        batch: denom
            .tokens
            .into_iter()
            .enumerate()
            .map(|(index, (id, uri))| (id, amounts[index], uri))
            .collect(),
    };

    Ok(WasmMsg::Execute {
        contract_addr: contract.to_string(),
        msg: to_binary(&msg)?,
        funds: vec![],
    }
    .into())
}

fn get_cw20_burn_msg(
    contract: &Addr,
    owner: &Addr,
    amounts: &Vec<Uint128>,
) -> StdResult<CosmosMsg> {
    let msg = cw20_base::msg::ExecuteMsg::BurnFrom {
        owner: owner.to_string(),
        amount: *amounts.first().unwrap(),
    };

    Ok(WasmMsg::Execute {
        contract_addr: contract.to_string(),
        msg: to_binary(&msg)?,
        funds: vec![],
    }
    .into())
}

fn get_cw20_transfer_to_msg(
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> StdResult<CosmosMsg> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::Transfer {
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw20_msg)?,
        funds: vec![],
    };
    let cw20_transfer_cosmos_msg: CosmosMsg = exec_cw20_transfer.into();

    Ok(cw20_transfer_cosmos_msg)
}

fn get_cw1155_transfer_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    tokens: &Vec<(String, String)>,
    token_amounts: &Vec<Uint128>,
) -> StdResult<CosmosMsg> {
    // create transfer cw1155 msg
    let transfer_cw1155_msg = Cw1155ExecuteMsg::BatchSendFrom {
        from: owner.into(),
        to: recipient.into(),
        batch: tokens
            .to_vec()
            .into_iter()
            .enumerate()
            .map(|(index, (id, uri))| (id, token_amounts[index], uri))
            .collect(),
        msg: None,
    };
    let exec_cw1155_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_binary(&transfer_cw1155_msg)?,
        funds: vec![],
    };
    let cw1155_transfer_cosmos_msg: CosmosMsg = exec_cw1155_transfer.into();

    Ok(cw1155_transfer_cosmos_msg)
}

fn get_bank_transfer_to_msg(recipient: &Addr, denom: &str, native_amount: Uint128) -> CosmosMsg {
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: native_amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg
}

fn get_fee_transfer_msg(
    sender: &Addr,
    recipient: &Addr,
    fee_denom: &Denom,
    amounts: Vec<Uint128>,
) -> StdResult<CosmosMsg> {
    match fee_denom {
        Denom::Cw20(addr) => {
            get_cw20_transfer_from_msg(sender, recipient, addr, *amounts.first().unwrap())
        }
        Denom::Cw1155(denom) => {
            get_cw1155_transfer_msg(sender, recipient, &denom.address, &denom.tokens, &amounts)
        }
        Denom::Native(denom) => Ok(get_bank_transfer_to_msg(
            recipient,
            denom,
            *amounts.first().unwrap(),
        )),
    }
}

fn fee_decimal_to_uint128(decimal: Decimal) -> StdResult<Uint128> {
    let result: Uint128 = decimal
        .atomics()
        .checked_mul(FEE_SCALE_FACTOR)
        .map_err(StdError::overflow)?;

    Ok(result / FEE_DECIMAL_PRECISION)
}

fn get_input_price(
    input_amounts: &Vec<Uint128>,
    input_reserves: &Vec<Uint128>,
    output_reserves: &Vec<Uint128>,
    fee_percent: Decimal,
) -> StdResult<Vec<Uint128>> {
    if input_reserves.first().unwrap().is_zero() || output_reserves.first().unwrap().is_zero() {
        return Err(StdError::generic_err("No liquidity"));
    };

    let fee_percent = fee_decimal_to_uint128(fee_percent)?;
    let fee_reduction_percent = FEE_SCALE_FACTOR - fee_percent;
    let input_amounts_with_fee = &input_amounts
        .into_iter()
        .map(|amount| Uint512::from(amount.full_mul(fee_reduction_percent)))
        .collect::<Vec<Uint512>>();
    let numerators: Vec<Uint512> = if output_reserves.len() < input_amounts_with_fee.len() {
        input_amounts_with_fee
            .to_vec()
            .into_iter()
            .enumerate()
            .map(|(index, amount)| {
                amount
                    .checked_mul(Uint512::from(get_indexed_value(
                        output_reserves,
                        input_amounts_with_fee,
                        index,
                    )))
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect()
    } else {
        output_reserves
            .to_vec()
            .into_iter()
            .enumerate()
            .map(|(index, amount)| {
                Uint512::from(amount)
                    .checked_mul(get_indexed_value(
                        input_amounts_with_fee,
                        output_reserves,
                        index,
                    ))
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect()
    };
    let denominators: Vec<Uint512> = input_reserves
        .to_vec()
        .into_iter()
        .enumerate()
        .map(|(index, reserve)| {
            Uint512::from(reserve)
                .checked_mul(Uint512::from(FEE_SCALE_FACTOR))
                .map_err(StdError::overflow)
                .unwrap()
                .checked_add(input_amounts_with_fee[index])
                .map_err(StdError::overflow)
                .unwrap()
        })
        .collect();

    Ok(numerators
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, numerator)| {
            Uint128::try_from(
                numerator
                    .checked_div(get_indexed_value(&denominators, &numerators, index))
                    .map_err(StdError::divide_by_zero)
                    .unwrap(),
            )
            .unwrap()
        })
        .collect())
}

fn get_protocol_fee_amount(
    input_amounts: &Vec<Uint128>,
    fee_percent: Decimal,
) -> StdResult<Vec<Uint128>> {
    if fee_percent.is_zero() {
        return Ok(vec![Uint128::zero()]);
    }

    let fee_percent = fee_decimal_to_uint128(fee_percent)?;
    Ok(input_amounts
        .into_iter()
        .map(|input_amount| {
            Uint128::try_from(
                input_amount
                    .full_mul(fee_percent)
                    .checked_div(Uint256::from(FEE_SCALE_FACTOR))
                    .map_err(StdError::divide_by_zero)
                    .unwrap(),
            )
            .unwrap()
        })
        .collect())
}

fn get_amount_for_denom(coins: &[Coin], denom: &str) -> Coin {
    let amount: Uint128 = coins
        .iter()
        .filter(|c| c.denom == denom)
        .map(|c| c.amount)
        .sum();

    Coin {
        amount,
        denom: denom.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_swap(
    deps: DepsMut,
    info: &MessageInfo,
    input_amounts: Vec<Uint128>,
    _env: Env,
    input_token_enum: TokenSelect,
    recipient: String,
    min_tokens: Vec<Uint128>,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let input_token_item = match input_token_enum {
        TokenSelect::Token1 => TOKEN1,
        TokenSelect::Token2 => TOKEN2,
    };
    let input_token = input_token_item.load(deps.storage)?;
    let output_token_item = match input_token_enum {
        TokenSelect::Token1 => TOKEN2,
        TokenSelect::Token2 => TOKEN1,
    };
    let output_token = output_token_item.load(deps.storage)?;

    // validate input_amount if native input token
    validate_input_amount(
        &info.funds,
        *input_amounts.first().unwrap(),
        &input_token.denom,
    )?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let tokens_bought = get_input_price(
        &input_amounts,
        &input_token.reserves,
        &output_token.reserves,
        total_fee_percent,
    )?;

    let invalid_token = min_tokens
        .clone()
        .into_iter()
        .enumerate()
        .find(|(index, min_token)| {
            if min_tokens.len() == tokens_bought.len() {
                min_token > &tokens_bought[*index]
            } else {
                min_token > tokens_bought.first().unwrap()
            }
        });

    if let Some((index, min_token)) = invalid_token {
        return Err(ContractError::SwapMinError {
            min: min_token,
            available: tokens_bought[index],
        });
    }

    // Calculate fees
    let protocol_fee_amounts = get_protocol_fee_amount(&input_amounts, fees.protocol_fee_percent)?;
    let input_amounts_minus_protocol_fee = input_amounts
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, input_amount)| {
            input_amount
                .checked_sub(get_indexed_value(
                    &protocol_fee_amounts,
                    &input_amounts,
                    index,
                ))
                .unwrap()
        })
        .collect::<Vec<Uint128>>();

    let mut msgs = match input_token.denom.clone() {
        Denom::Cw20(addr) => vec![get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            *input_amounts_minus_protocol_fee.first().unwrap(),
        )?],
        Denom::Cw1155(denom) => vec![get_cw1155_transfer_msg(
            &info.sender,
            &_env.contract.address,
            &denom.address,
            &denom.tokens,
            &input_amounts_minus_protocol_fee,
        )?],
        Denom::Native(_) => vec![],
    };

    // Send protocol fee to protocol fee recipient
    protocol_fee_amounts.into_iter().for_each(|fee_amount| {
        if !fee_amount.is_zero() {
            msgs.push(
                get_fee_transfer_msg(
                    &info.sender,
                    &fees.protocol_fee_recipient,
                    &input_token.denom,
                    vec![fee_amount],
                )
                .unwrap(),
            )
        }
    });

    let recipient = deps.api.addr_validate(&recipient)?;
    // Create transfer to message
    msgs.push(match output_token.denom {
        Denom::Cw20(addr) => {
            get_cw20_transfer_to_msg(&recipient, &addr, *tokens_bought.first().unwrap())?
        }
        Denom::Cw1155(denom) => get_cw1155_transfer_msg(
            &_env.contract.address,
            &recipient,
            &denom.address,
            &denom.tokens,
            &tokens_bought,
        )?,
        Denom::Native(denom) => {
            get_bank_transfer_to_msg(&recipient, &denom, *tokens_bought.first().unwrap())
        }
    });

    input_token_item.update(
        deps.storage,
        |mut input_token| -> Result<_, ContractError> {
            input_token.reserves = input_token
                .reserves
                .into_iter()
                .enumerate()
                .map(|(index, reserve)| {
                    reserve
                        .checked_add(input_amounts_minus_protocol_fee[index])
                        .map_err(StdError::overflow)
                        .unwrap()
                })
                .collect();

            Ok(input_token)
        },
    )?;

    output_token_item.update(
        deps.storage,
        |mut output_token| -> Result<_, ContractError> {
            output_token.reserves = output_token
                .reserves
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, reserve)| {
                    reserve
                        .checked_sub(get_indexed_value(
                            &tokens_bought,
                            &output_token.reserves,
                            index,
                        ))
                        .map_err(StdError::overflow)
                        .unwrap()
                })
                .collect();

            Ok(output_token)
        },
    )?;

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("native_sold", format!("{:?}", input_amounts)),
        attr("token_bought", format!("{:?}", tokens_bought)),
    ]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_pass_through_swap(
    deps: DepsMut,
    info: MessageInfo,
    _env: Env,
    output_amm_address: String,
    input_token_enum: TokenSelect,
    input_tokens_amount: Vec<Uint128>,
    output_min_tokens: Vec<Uint128>,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &_env.block)?;

    let input_token_state = match input_token_enum {
        TokenSelect::Token1 => TOKEN1,
        TokenSelect::Token2 => TOKEN2,
    };
    let input_token = input_token_state.load(deps.storage)?;
    let transfer_token_state = match input_token_enum {
        TokenSelect::Token1 => TOKEN2,
        TokenSelect::Token2 => TOKEN1,
    };
    let transfer_token = transfer_token_state.load(deps.storage)?;

    validate_input_amount(
        &info.funds,
        *input_tokens_amount.first().unwrap(),
        &input_token.denom,
    )?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let amounts_to_transfer = get_input_price(
        &input_tokens_amount,
        &input_token.reserves,
        &transfer_token.reserves,
        total_fee_percent,
    )?;

    // Calculate fees
    let protocol_fee_amounts =
        get_protocol_fee_amount(&input_tokens_amount, fees.protocol_fee_percent)?;
    let input_amounts_minus_protocol_fee = input_tokens_amount
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, input_amount)| {
            input_amount
                .checked_sub(get_indexed_value(
                    &protocol_fee_amounts,
                    &input_tokens_amount,
                    index,
                ))
                .unwrap()
        })
        .collect::<Vec<Uint128>>();

    // Transfer input amount - protocol fee to contract
    let mut msgs = match input_token.denom.clone() {
        Denom::Cw20(addr) => vec![get_cw20_transfer_from_msg(
            &info.sender,
            &_env.contract.address,
            &addr,
            *input_amounts_minus_protocol_fee.first().unwrap(),
        )?],
        Denom::Cw1155(denom) => vec![get_cw1155_transfer_msg(
            &info.sender,
            &_env.contract.address,
            &denom.address,
            &denom.tokens,
            &input_amounts_minus_protocol_fee,
        )?],
        Denom::Native(_) => vec![],
    };

    // Send protocol fee to protocol fee recipient
    protocol_fee_amounts.into_iter().for_each(|fee_amount| {
        if !fee_amount.is_zero() {
            msgs.push(
                get_fee_transfer_msg(
                    &info.sender,
                    &fees.protocol_fee_recipient,
                    &input_token.denom,
                    vec![fee_amount],
                )
                .unwrap(),
            )
        }
    });

    let output_amm_address = deps.api.addr_validate(&output_amm_address)?;

    // Increase allowance of output contract is transfer token is cw20
    if let Denom::Cw20(addr) = &transfer_token.denom {
        msgs.push(get_cw20_increase_allowance_msg(
            addr,
            &output_amm_address,
            *amounts_to_transfer.first().unwrap(),
            Some(Expiration::AtHeight(_env.block.height + 1)),
        )?)
    };

    let resp: InfoResponse = deps
        .querier
        .query_wasm_smart(&output_amm_address, &QueryMsg::Info {})?;

    let transfer_input_token_enum = if transfer_token.denom == resp.token1_denom {
        Ok(TokenSelect::Token1)
    } else if transfer_token.denom == resp.token2_denom {
        Ok(TokenSelect::Token2)
    } else {
        Err(ContractError::InvalidOutputPool {})
    }?;

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token: transfer_input_token_enum,
        input_amounts: amounts_to_transfer.clone(),
        recipient: info.sender.to_string(),
        min_tokens: output_min_tokens,
        expiration,
    };

    msgs.push(
        WasmMsg::Execute {
            contract_addr: output_amm_address.into(),
            msg: to_binary(&swap_msg)?,
            funds: match transfer_token.denom {
                Denom::Cw20(_) => vec![],
                Denom::Cw1155(_) => vec![],
                Denom::Native(denom) => vec![Coin {
                    denom,
                    amount: *amounts_to_transfer.first().unwrap(),
                }],
            },
        }
        .into(),
    );

    input_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        // Add input amount - protocol fee to input token reserve
        token.reserves = token
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, reserve)| {
                reserve
                    .checked_add(input_amounts_minus_protocol_fee[index])
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect();

        Ok(token)
    })?;

    transfer_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserves = token
            .reserves
            .into_iter()
            .enumerate()
            .map(|(index, reserve)| {
                reserve
                    .checked_sub(amounts_to_transfer[index])
                    .map_err(StdError::overflow)
                    .unwrap()
            })
            .collect();

        Ok(token)
    })?;

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("input_token_amount", format!("{:?}", input_tokens_amount)),
        attr("native_transferred", format!("{:?}", amounts_to_transfer)),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Info {} => to_binary(&query_info(deps)?),
        QueryMsg::Token1ForToken2Price { token1_amounts } => {
            to_binary(&query_token1_for_token2_price(deps, token1_amounts)?)
        }
        QueryMsg::Token2ForToken1Price { token2_amounts } => {
            to_binary(&query_token2_for_token1_price(deps, token2_amounts)?)
        }
        QueryMsg::Fee {} => to_binary(&query_fee(deps)?),
        QueryMsg::Token { token_id } => to_binary(&query_token(deps, token_id)?),
    }
}

pub fn query_token(deps: Deps, token_id: String) -> StdResult<TokenResponse> {
    // let query_request = QueryRequest::Stargate {
    //     path: "/ixo.token.v1beta1.Query/TokenMetadata".to_string(),
    //     data: Binary::from(format!(r#"{{ "id": "{}"}}"#, token_id).as_bytes()),
    // };

    let query_request = QueryRequest::Custom(A {
        path: "/ixo.token.v1beta1.Query/TokenMetadata".to_string(),
        data: Binary::from(format!(r#"{{ "id": "{}"}}"#, token_id).as_bytes()),
    });
    let raw = to_vec(&query_request)?;

    match deps.querier.raw_query(&raw) {
        SystemResult::Err(system_err) => Err(StdError::generic_err(format!(
            "Querier system error: {}",
            system_err
        ))),
        SystemResult::Ok(ContractResult::Err(contract_err)) => Err(StdError::generic_err(format!(
            "Querier contract error: {}",
            contract_err
        ))),
        SystemResult::Ok(ContractResult::Ok(value)) => from_binary(&value),
    }
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let lp_token_address = LP_TOKEN_ADDRESS.load(deps.storage)?;

    // TODO get total supply
    Ok(InfoResponse {
        token1_reserves: token1.reserves.clone(),
        token2_reserves: token2.reserves.clone(),
        token1_denom: token1.denom.clone(),
        token2_denom: token2.denom.clone(),
        lp_token_supplies: get_lp_token_supplies(deps, &token1, &token2, &lp_token_address, None)?,
        lp_token_address: lp_token_address.into_string(),
    })
}

pub fn query_token1_for_token2_price(
    deps: Deps,
    token1_amounts: Vec<Uint128>,
) -> StdResult<Token1ForToken2PriceResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token2_amounts = get_input_price(
        &token1_amounts,
        &token1.reserves,
        &token2.reserves,
        total_fee_percent,
    )?;

    Ok(Token1ForToken2PriceResponse { token2_amounts })
}

pub fn query_token2_for_token1_price(
    deps: Deps,
    token2_amount: Vec<Uint128>,
) -> StdResult<Token2ForToken1PriceResponse> {
    let token1 = TOKEN1.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token1_amounts = get_input_price(
        &token2_amount,
        &token2.reserves,
        &token1.reserves,
        total_fee_percent,
    )?;

    Ok(Token2ForToken1PriceResponse { token1_amounts })
}

pub fn query_fee(deps: Deps) -> StdResult<FeeResponse> {
    let fees = FEES.load(deps.storage)?;
    let owner = OWNER.load(deps.storage)?.map(|o| o.into_string());

    Ok(FeeResponse {
        owner,
        lp_fee_percent: fees.lp_fee_percent,
        protocol_fee_percent: fees.protocol_fee_percent,
        protocol_fee_recipient: fees.protocol_fee_recipient.into_string(),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_LP_TOKEN_REPLY_ID {
        return Err(ContractError::UnknownReplyId { id: msg.id });
    };
    let res = parse_reply_instantiate_data(msg);
    match res {
        Ok(res) => {
            // Validate contract address
            let lp_contract_addr = deps.api.addr_validate(&res.contract_address)?;

            // Save gov token
            LP_TOKEN_ADDRESS.save(deps.storage, &lp_contract_addr)?;

            Ok(Response::new())
        }
        Err(_) => Err(ContractError::InstantiateLpTokenError {}),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    let owner = match msg.owner {
        None => None,
        Some(o) => Some(deps.api.addr_validate(&o)?),
    };
    OWNER.save(deps.storage, &owner)?;

    let protocol_fee_recipient = deps.api.addr_validate(&msg.protocol_fee_recipient)?;
    let total_fee_percent = msg.lp_fee_percent + msg.protocol_fee_percent;
    let max_fee_percent = Decimal::from_str(MAX_FEE_PERCENT)?;
    if total_fee_percent > max_fee_percent {
        return Err(ContractError::FeesTooHigh {
            max_fee_percent,
            total_fee_percent,
        });
    }

    let fees = Fees {
        lp_fee_percent: msg.lp_fee_percent,
        protocol_fee_percent: msg.protocol_fee_percent,
        protocol_fee_recipient,
    };
    FEES.save(deps.storage, &fees)?;

    // By default deposits are not frozen
    FROZEN.save(deps.storage, &msg.freeze_pool)?;

    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_liquidity_amount() {
        let liquidity = get_lp_token_amounts_to_mint(
            &vec![Uint128::new(100)],
            &vec![Uint128::zero()],
            &vec![Uint128::zero()],
        )
        .unwrap();
        assert_eq!(liquidity, vec![Uint128::new(100)]);

        let liquidity = get_lp_token_amounts_to_mint(
            &vec![Uint128::new(100)],
            &vec![Uint128::new(50)],
            &vec![Uint128::new(25)],
        )
        .unwrap();
        assert_eq!(liquidity, vec![Uint128::new(200)]);
    }

    #[test]
    fn test_get_token_amount() {
        let liquidity = get_token2_amounts_required(
            &vec![Uint128::new(100)],
            &vec![Uint128::new(50)],
            &vec![Uint128::zero()],
            &vec![Uint128::zero()],
            &vec![Uint128::zero()],
        )
        .unwrap();
        assert_eq!(liquidity, vec![Uint128::new(100)]);

        let liquidity = get_token2_amounts_required(
            &vec![Uint128::new(200)],
            &vec![Uint128::new(50)],
            &vec![Uint128::new(50)],
            &vec![Uint128::new(100)],
            &vec![Uint128::new(25)],
        )
        .unwrap();
        assert_eq!(liquidity, vec![Uint128::new(201)]);
    }

    #[test]
    fn test_get_input_price() {
        let fee_percent = Decimal::from_str("0.3").unwrap();
        // Base case
        assert_eq!(
            get_input_price(
                &vec![Uint128::new(10)],
                &vec![Uint128::new(100)],
                &vec![Uint128::new(100)],
                fee_percent
            )
            .unwrap(),
            vec![Uint128::new(9)]
        );

        // No input reserve error
        let err = get_input_price(
            &vec![Uint128::new(10)],
            &vec![Uint128::new(0)],
            &vec![Uint128::new(100)],
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));

        // No output reserve error
        let err = get_input_price(
            &vec![Uint128::new(10)],
            &vec![Uint128::new(100)],
            &vec![Uint128::new(0)],
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));

        // No reserve error
        let err = get_input_price(
            &vec![Uint128::new(10)],
            &vec![Uint128::new(0)],
            &vec![Uint128::new(0)],
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));
    }
}
