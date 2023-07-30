use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;

use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Decimal, Deps, DepsMut,
    Env, MessageInfo, Order, QueryRequest, Reply, Response, StdError, StdResult, Storage, SubMsg,
    Uint128, Uint256, Uint512, WasmMsg,
};
use cw1155::{Cw1155ExecuteMsg, TokenId};
use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Expiration, MinterResponse};
use cw20_base::contract::query_balance;
use cw_utils::parse_reply_instantiate_data;
use prost::Message;

use crate::error::ContractError;
use crate::msg::{
    Denom, ExecuteMsg, FeeResponse, InfoResponse, InstantiateMsg, MigrateMsg, QueryMsg,
    QueryTokenMetadataRequest, QueryTokenMetadataResponse, Token1ForToken2PriceResponse,
    Token2ForToken1PriceResponse, TokenAmount, TokenSelect, TokenSuppliesResponse,
};
use crate::random::seed::get_seed;
use crate::random::{pcg64_from_seed, Pcg64};
use crate::state::{
    Fees, Token, FEES, FROZEN, LP_ADDRESS, OWNER, TOKEN1155, TOKEN2, TOKEN_SUPPLIES,
};

// Version info for migration info
pub const CONTRACT_NAME: &str = "crates.io:wasmswap";
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

    let token1155 = Token {
        denom: msg.token1_denom.clone(),
        reserve: Uint128::zero(),
    };
    TOKEN1155.save(deps.storage, &token1155)?;

    let token2 = Token {
        denom: msg.token2_denom.clone(),
        reserve: Uint128::zero(),
    };
    TOKEN2.save(deps.storage, &token2)?;

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

    let instantiate_lp_token_msg = WasmMsg::Instantiate {
        code_id: msg.lp_token_code_id,
        funds: vec![],
        admin: None,
        label: "lp_token".to_string(),
        msg: to_binary(&cw20_base::msg::InstantiateMsg {
            name: "WasmSwap_Liquidity_Token".into(),
            symbol: "wslpt".into(),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: env.contract.address.into(),
                cap: None,
            }),
            marketing: None,
        })?,
    };

    let reply_msg =
        SubMsg::reply_on_success(instantiate_lp_token_msg, INSTANTIATE_LP_TOKEN_REPLY_ID);

    Ok(Response::new().add_submessage(reply_msg))
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
            token1155_amounts,
            min_liquidity,
            max_token2,
            expiration,
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_add_liquidity(
                deps,
                &info,
                env,
                min_liquidity,
                token1155_amounts,
                max_token2,
                expiration,
            )
        }
        ExecuteMsg::RemoveLiquidity {
            amount,
            min_token1155,
            min_token2,
            expiration,
        } => execute_remove_liquidity(
            deps,
            info,
            env,
            amount,
            min_token1155,
            min_token2,
            expiration,
        ),
        ExecuteMsg::Swap {
            input_token,
            input_amount,
            min_output,
            expiration,
            ..
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_swap(
                deps,
                &info,
                input_amount,
                env,
                input_token,
                info.sender.to_string(),
                min_output,
                expiration,
            )
        }
        ExecuteMsg::PassThroughSwap {
            output_amm_address,
            input_token,
            input_token_amount,
            output_min_token,
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
                input_token_amount,
                output_min_token,
                expiration,
            )
        }
        ExecuteMsg::SwapAndSendTo {
            input_token,
            input_amount,
            recipient,
            min_token,
            expiration,
        } => {
            if FROZEN.load(deps.storage)? {
                return Err(ContractError::FrozenPool {});
            }
            execute_swap(
                deps,
                &info,
                input_amount,
                env,
                input_token,
                recipient,
                min_token,
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

fn get_lp_token_amount_to_mint(
    token1_amount: Uint128,
    liquidity_supply: Uint128,
    token1_reserve: Uint128,
) -> Result<Uint128, ContractError> {
    if liquidity_supply == Uint128::zero() {
        Ok(token1_amount)
    } else {
        Ok(token1_amount
            .checked_mul(liquidity_supply)
            .map_err(StdError::overflow)?
            .checked_div(token1_reserve)
            .map_err(StdError::divide_by_zero)?)
    }
}

fn get_token2_amount_required(
    max_token: Uint128,
    token1_amount: Uint128,
    liquidity_supply: Uint128,
    token2_reserve: Uint128,
    token1_reserve: Uint128,
) -> Result<Uint128, StdError> {
    if liquidity_supply == Uint128::zero() {
        Ok(max_token)
    } else {
        Ok(token1_amount
            .checked_mul(token2_reserve)
            .map_err(StdError::overflow)?
            .checked_div(token1_reserve)
            .map_err(StdError::divide_by_zero)?
            .checked_add(Uint128::new(1))
            .map_err(StdError::overflow)?)
    }
}

fn get_token_total_amount(token_amounts: &HashMap<TokenId, Uint128>) -> Uint128 {
    token_amounts
        .clone()
        .into_iter()
        .map(|(_, amount)| amount)
        .reduce(|acc, e| acc + e)
        .unwrap()
}

pub fn execute_add_liquidity(
    deps: DepsMut,
    info: &MessageInfo,
    env: Env,
    min_liquidity: Uint128,
    token1155_amounts: HashMap<TokenId, Uint128>,
    max_token2: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let lp_token_addr = LP_ADDRESS.load(deps.storage)?;

    validate_token1155_denom(&deps, &token1155.denom, &token1155_amounts)?;
    validate_input_amount(&info.funds, max_token2, &token2.denom)?;

    let token1155_total_amount = get_token_total_amount(&token1155_amounts);
    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
    let liquidity_amount =
        get_lp_token_amount_to_mint(token1155_total_amount, lp_token_supply, token1155.reserve)?;

    let token2_amount = get_token2_amount_required(
        max_token2,
        token1155_total_amount,
        lp_token_supply,
        token2.reserve,
        token1155.reserve,
    )?;

    if liquidity_amount < min_liquidity {
        return Err(ContractError::MinLiquidityError {
            min_liquidity,
            liquidity_available: liquidity_amount,
        });
    }

    if token2_amount > max_token2 {
        return Err(ContractError::MaxTokenError {
            max_token: max_token2,
            tokens_required: token2_amount,
        });
    }

    // Generate cw20 transfer messages if necessary
    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    if let Denom::Cw1155(addr, _) = token1155.denom {
        transfer_msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &token1155_amounts,
        )?)
    }
    if let Denom::Cw20(addr) = token2.denom.clone() {
        transfer_msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            token2_amount,
        )?)
    }

    // Refund token 2 if is a native token and not all is spent
    if let Denom::Native(denom) = token2.denom {
        if token2_amount < max_token2 {
            transfer_msgs.push(get_bank_transfer_to_msg(
                &info.sender,
                &denom,
                max_token2 - token2_amount,
            ))
        }
    }

    TOKEN1155.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve += token1155_total_amount;
        Ok(token)
    })?;
    TOKEN2.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve += token2_amount;
        Ok(token)
    })?;

    for (token_id, token_amount) in token1155_amounts.into_iter() {
        TOKEN_SUPPLIES.update(
            deps.storage,
            token_id.clone(),
            |lp_token_supply| -> Result<_, ContractError> {
                match lp_token_supply {
                    Some(lp_token_supply) => Ok(lp_token_supply + token_amount),
                    None => Ok(token_amount),
                }
            },
        )?;
    }

    let mint_msg = mint_lp_tokens(&info.sender, liquidity_amount, &lp_token_addr)?;
    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_message(mint_msg)
        .add_attributes(vec![
            attr("token1155_amount", token1155_total_amount),
            attr("token2_amount", token2_amount),
            attr("liquidity_received", liquidity_amount),
        ]))
}

fn validate_token1155_denom(
    deps: &DepsMut,
    denom: &Denom,
    tokens: &HashMap<TokenId, Uint128>,
) -> Result<(), ContractError> {
    match denom {
        Denom::Cw1155(_, supported_denom) => {
            for (token_id, _) in tokens.into_iter() {
                let token_metadata: QueryTokenMetadataResponse =
                    query_token_metadata(deps.as_ref(), token_id.clone())?;

                if token_metadata.name != *supported_denom {
                    return Err(ContractError::UnsupportedTokenDenom {
                        id: token_id.clone(),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn get_cw1155_transfer_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    tokens: &HashMap<String, Uint128>,
) -> StdResult<CosmosMsg> {
    // create transfer cw1155 msg
    let transfer_cw1155_msg = Cw1155ExecuteMsg::BatchSendFrom {
        from: owner.into(),
        to: recipient.into(),
        batch: tokens
            .into_iter()
            .map(|(token_id, amount)| (token_id.clone(), amount.clone(), "".to_string()))
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

fn get_lp_token_supply(deps: Deps, lp_token_addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20::TokenInfoResponse = deps
        .querier
        .query_wasm_smart(lp_token_addr, &cw20_base::msg::QueryMsg::TokenInfo {})?;
    Ok(resp.total_supply)
}

fn mint_lp_tokens(
    recipient: &Addr,
    liquidity_amount: Uint128,
    lp_token_address: &Addr,
) -> StdResult<CosmosMsg> {
    let mint_msg = cw20_base::msg::ExecuteMsg::Mint {
        recipient: recipient.into(),
        amount: liquidity_amount,
    };
    Ok(WasmMsg::Execute {
        contract_addr: lp_token_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    }
    .into())
}

fn get_token_balance(deps: Deps, contract: &Addr, addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20::BalanceResponse = deps.querier.query_wasm_smart(
        contract,
        &cw20_base::msg::QueryMsg::Balance {
            address: addr.to_string(),
        },
    )?;
    Ok(resp.balance)
}

fn validate_input_amount(
    actual_funds: &[Coin],
    given_amount: Uint128,
    given_denom: &Denom,
) -> Result<(), ContractError> {
    match given_denom {
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
        _ => Ok(()),
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
    amount: Uint128,
    min_token1155: HashMap<TokenId, Uint128>,
    min_token2: Uint128,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let lp_token_addr = LP_ADDRESS.load(deps.storage)?;
    let balance = get_token_balance(deps.as_ref(), &lp_token_addr, &info.sender)?;
    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    if amount > balance {
        return Err(ContractError::InsufficientLiquidityError {
            requested: amount,
            available: balance,
        });
    }

    let min_token1155_total_amount = get_token_total_amount(&min_token1155);
    let token1155_amount = amount
        .checked_mul(token1155.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;
    if token1155_amount < min_token1155_total_amount {
        return Err(ContractError::MinToken1155Error {
            requested: min_token1155_total_amount,
            available: token1155_amount,
        });
    }

    let token2_amount = amount
        .checked_mul(token2.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;
    if token2_amount < min_token2 {
        return Err(ContractError::MinToken2Error {
            requested: min_token2,
            available: token2_amount,
        });
    }

    TOKEN1155.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(token1155_amount)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;

    TOKEN2.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(token2_amount)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;

    let ref mut rng = pcg64_from_seed(&get_seed(token1155_amount, env.block.height))?;
    let token1155_amounts_to_transfer =
        get_token_amounts_to_transfer(deps.storage, token1155_amount, min_token1155, rng)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    if let Denom::Cw1155(addr, _) = token1155.denom {
        msgs.push(get_cw1155_transfer_msg(
            &env.contract.address,
            &info.sender,
            &addr,
            &token1155_amounts_to_transfer,
        )?)
    };

    match token2.denom {
        Denom::Cw20(addr) => msgs.push(get_cw20_transfer_to_msg(
            &info.sender,
            &addr,
            token2_amount,
        )?),
        Denom::Native(denom) => msgs.push(get_bank_transfer_to_msg(
            &info.sender,
            &denom,
            token2_amount,
        )),
        _ => {}
    };

    msgs.push(get_burn_msg(&lp_token_addr, &info.sender, amount)?);

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("liquidity_burned", amount),
        attr("token1_returned", token1155_amount),
        attr("token2_returned", token2_amount),
    ]))
}

fn get_token_amounts_to_transfer(
    storage: &mut dyn Storage,
    token1155_amount: Uint128,
    min_token1155: HashMap<TokenId, Uint128>,
    rng: &mut Pcg64,
) -> Result<HashMap<TokenId, Uint128>, ContractError> {
    let mut token1155_amount_left_to_transfer = token1155_amount;
    let mut token1155_amounts_to_transfer: HashMap<TokenId, Uint128> = HashMap::new();
    for (token_id, token_amount) in min_token1155.clone().into_iter() {
        let token_supply = TOKEN_SUPPLIES
            .may_load(storage, token_id.clone())?
            .unwrap_or_default();

        if token_supply < token_amount {
            return Err(ContractError::MinToken1155Error {
                requested: token_amount,
                available: token_supply,
            });
        }

        token1155_amount_left_to_transfer -= token_amount;
        token1155_amounts_to_transfer.insert(token_id.clone(), token_amount);

        let remaining_supply = token_supply - token_amount;
        if remaining_supply.is_zero() {
            TOKEN_SUPPLIES.remove(storage, token_id);
        } else {
            TOKEN_SUPPLIES.save(storage, token_id, &remaining_supply)?;
        }
    }

    if !token1155_amount_left_to_transfer.is_zero() {
        let additional_amount_to_transfer = token1155_amount_left_to_transfer
            .checked_div(Uint128::from(min_token1155.len() as u32))
            .map_err(StdError::divide_by_zero)?;

        for (token_id, token_amount) in token1155_amounts_to_transfer.iter_mut() {
            let token_supply = TOKEN_SUPPLIES
                .may_load(storage, token_id.clone())?
                .unwrap_or_default();

            let mut remaining_supply = Uint128::zero();
            if token_supply > additional_amount_to_transfer {
                *token_amount += additional_amount_to_transfer;
                token1155_amount_left_to_transfer -= additional_amount_to_transfer;
                remaining_supply = token_supply - additional_amount_to_transfer;
            } else {
                *token_amount += token_supply;
                token1155_amount_left_to_transfer -= token_supply;
            }

            if remaining_supply.is_zero() {
                TOKEN_SUPPLIES.remove(storage, token_id.clone());
            } else {
                TOKEN_SUPPLIES.save(storage, token_id.clone(), &remaining_supply)?;
            }
        }
    }

    while !token1155_amount_left_to_transfer.is_zero() {
        let token_supplies = TOKEN_SUPPLIES
            .range(storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()?;
        let supply_index = rng.next_u64() % token_supplies.len() as u64;
        let token_supply = token_supplies[supply_index as usize].clone();
        let mut token_amount =
            if let Some(token_amount) = token1155_amounts_to_transfer.get(&token_supply.0) {
                token_amount.clone()
            } else {
                Uint128::zero()
            };

        let mut remaining_supply = Uint128::zero();
        if token_supply.1 > token1155_amount_left_to_transfer {
            token_amount += token1155_amount_left_to_transfer;
            remaining_supply = token_supply.1 - token1155_amount_left_to_transfer;
            token1155_amount_left_to_transfer = Uint128::zero();
        } else {
            token_amount += token_supply.1;
            token1155_amount_left_to_transfer -= token_supply.1;
        }

        if remaining_supply.is_zero() {
            TOKEN_SUPPLIES.remove(storage, token_supply.0.clone());
        } else {
            TOKEN_SUPPLIES.save(storage, token_supply.0.clone(), &remaining_supply)?;
        }

        token1155_amounts_to_transfer.insert(token_supply.0.clone(), token_amount);
    }

    Ok(token1155_amounts_to_transfer)
}

fn get_burn_msg(contract: &Addr, owner: &Addr, amount: Uint128) -> StdResult<CosmosMsg> {
    let msg = cw20_base::msg::ExecuteMsg::BurnFrom {
        owner: owner.to_string(),
        amount,
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
    amount: TokenAmount,
) -> StdResult<CosmosMsg> {
    match fee_denom {
        Denom::Cw1155(addr, _) => {
            get_cw1155_transfer_msg(sender, recipient, addr, &amount.get_token1155())
        }
        Denom::Cw20(addr) => {
            get_cw20_transfer_from_msg(sender, recipient, addr, amount.get_token2())
        }
        Denom::Native(denom) => Ok(get_bank_transfer_to_msg(
            recipient,
            denom,
            amount.get_token2(),
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
    input_amount: Uint128,
    input_reserve: Uint128,
    output_reserve: Uint128,
    fee_percent: Decimal,
) -> StdResult<Uint128> {
    if input_reserve == Uint128::zero() || output_reserve == Uint128::zero() {
        return Err(StdError::generic_err("No liquidity"));
    };

    let fee_percent = fee_decimal_to_uint128(fee_percent)?;
    let fee_reduction_percent = FEE_SCALE_FACTOR - fee_percent;
    let input_amount_with_fee = Uint512::from(input_amount.full_mul(fee_reduction_percent));
    let numerator = input_amount_with_fee
        .checked_mul(Uint512::from(output_reserve))
        .map_err(StdError::overflow)?;
    let denominator = Uint512::from(input_reserve)
        .checked_mul(Uint512::from(FEE_SCALE_FACTOR))
        .map_err(StdError::overflow)?
        .checked_add(input_amount_with_fee)
        .map_err(StdError::overflow)?;

    Ok(numerator
        .checked_div(denominator)
        .map_err(StdError::divide_by_zero)?
        .try_into()?)
}

fn get_protocol_fee_amount(
    input_amount: &TokenAmount,
    fee_percent: Decimal,
) -> StdResult<Option<TokenAmount>> {
    if fee_percent.is_zero() {
        return Ok(None);
    }

    let fee_percent = fee_decimal_to_uint128(fee_percent)?;
    match input_amount.clone() {
        TokenAmount::Token1155(mut input_amounts) => {
            for (_, token_amount) in input_amounts.iter_mut() {
                *token_amount = token_amount
                    .full_mul(fee_percent)
                    .checked_div(Uint256::from(FEE_SCALE_FACTOR))
                    .map_err(StdError::divide_by_zero)?
                    .try_into()?;
            }

            Ok(Some(TokenAmount::Token1155(input_amounts)))
        }
        TokenAmount::Token2(input_amount) => Ok(Some(TokenAmount::Token2(
            input_amount
                .full_mul(fee_percent)
                .checked_div(Uint256::from(FEE_SCALE_FACTOR))
                .map_err(StdError::divide_by_zero)?
                .try_into()?,
        ))),
    }
}

fn get_amount_without_fee(
    input_amount: &TokenAmount,
    fee_amount: Option<TokenAmount>,
) -> TokenAmount {
    if let Some(fee_amount) = fee_amount {
        match input_amount.clone() {
            TokenAmount::Token1155(mut input_amount) => {
                let fee_amount = fee_amount.get_token1155();

                for (token_id, token_amount) in input_amount.iter_mut() {
                    let fee_amount = fee_amount.get(token_id).unwrap();

                    *token_amount -= fee_amount
                }

                TokenAmount::Token1155(input_amount)
            }
            TokenAmount::Token2(input_amount) => {
                let fee_amount = fee_amount.get_token2();

                TokenAmount::Token2(input_amount - fee_amount)
            }
        }
    } else {
        input_amount.clone()
    }
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
    input_amount: TokenAmount,
    env: Env,
    input_token_enum: TokenSelect,
    recipient: String,
    min_token: TokenAmount,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let input_token_item = match input_token_enum {
        TokenSelect::Token1155 => TOKEN1155,
        TokenSelect::Token2 => TOKEN2,
    };
    let input_token = input_token_item.load(deps.storage)?;
    let output_token_item = match input_token_enum {
        TokenSelect::Token1155 => TOKEN2,
        TokenSelect::Token2 => TOKEN1155,
    };
    let output_token = output_token_item.load(deps.storage)?;

    if let TokenAmount::Token2(amount) = input_amount {
        validate_input_amount(&info.funds, amount, &input_token.denom)?;
    }

    let input_amount_total = match input_amount {
        TokenAmount::Token1155(ref amounts) => get_token_total_amount(amounts),
        TokenAmount::Token2(amount) => amount,
    };
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token_bought = get_input_price(
        input_amount_total,
        input_token.reserve,
        output_token.reserve,
        total_fee_percent,
    )?;

    let min_token_total = match min_token {
        TokenAmount::Token1155(ref amounts) => get_token_total_amount(amounts),
        TokenAmount::Token2(amount) => amount,
    };
    if min_token_total > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token_total,
            available: token_bought,
        });
    }
    // Calculate fees
    let protocol_fee_amount = get_protocol_fee_amount(&input_amount, fees.protocol_fee_percent)?;
    let input_amount_without_protocol_fee =
        get_amount_without_fee(&input_amount, protocol_fee_amount.clone());

    let mut msgs = vec![];
    match input_token.denom.clone() {
        Denom::Cw1155(addr, _) => msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &input_amount_without_protocol_fee.get_token1155(),
        )?),
        Denom::Cw20(addr) => msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            input_amount_without_protocol_fee.get_token2(),
        )?),
        _ => {}
    };

    // Send protocol fee to protocol fee recipient
    if let Some(protocol_fee_amount) = protocol_fee_amount {
        msgs.push(get_fee_transfer_msg(
            &info.sender,
            &fees.protocol_fee_recipient,
            &input_token.denom,
            protocol_fee_amount,
        )?)
    }

    let recipient = deps.api.addr_validate(&recipient)?;
    // Create transfer to message
    msgs.push(match output_token.denom {
        Denom::Cw1155(addr, _) => {
            let ref mut rng = pcg64_from_seed(&get_seed(token_bought, env.block.height))?;
            let tokens_to_transfer = get_token_amounts_to_transfer(
                deps.storage,
                token_bought,
                min_token.get_token1155(),
                rng,
            )?;

            get_cw1155_transfer_msg(
                &env.contract.address,
                &recipient,
                &addr,
                &tokens_to_transfer,
            )?
        }
        Denom::Cw20(addr) => get_cw20_transfer_to_msg(&recipient, &addr, token_bought)?,
        Denom::Native(denom) => get_bank_transfer_to_msg(&recipient, &denom, token_bought),
    });

    input_token_item.update(
        deps.storage,
        |mut input_token| -> Result<_, ContractError> {
            let input_amount_without_protocol_fee_total = match input_amount_without_protocol_fee {
                TokenAmount::Token1155(amounts) => get_token_total_amount(&amounts),
                TokenAmount::Token2(amount) => amount,
            };
            input_token.reserve = input_token
                .reserve
                .checked_add(input_amount_without_protocol_fee_total)
                .map_err(StdError::overflow)?;

            Ok(input_token)
        },
    )?;

    output_token_item.update(
        deps.storage,
        |mut output_token| -> Result<_, ContractError> {
            output_token.reserve = output_token
                .reserve
                .checked_sub(token_bought)
                .map_err(StdError::overflow)?;
            Ok(output_token)
        },
    )?;

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("native_sold", input_amount_total),
        attr("token_bought", token_bought),
    ]))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_pass_through_swap(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    output_amm_address: String,
    input_token_enum: TokenSelect,
    input_token_amount: TokenAmount,
    output_min_token: TokenAmount,
    expiration: Option<Expiration>,
) -> Result<Response, ContractError> {
    check_expiration(&expiration, &env.block)?;

    let input_token_state = match input_token_enum {
        TokenSelect::Token1155 => TOKEN1155,
        TokenSelect::Token2 => TOKEN2,
    };
    let input_token = input_token_state.load(deps.storage)?;
    let transfer_token_state = match input_token_enum {
        TokenSelect::Token1155 => TOKEN2,
        TokenSelect::Token2 => TOKEN1155,
    };
    let transfer_token = transfer_token_state.load(deps.storage)?;

    if let TokenAmount::Token2(amount) = input_token_amount {
        validate_input_amount(&info.funds, amount, &input_token.denom)?;
    }

    let input_token_amount_total = match input_token_amount {
        TokenAmount::Token1155(ref amounts) => get_token_total_amount(&amounts),
        TokenAmount::Token2(amount) => amount,
    };
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let amount_to_transfer = get_input_price(
        input_token_amount_total,
        input_token.reserve,
        transfer_token.reserve,
        total_fee_percent,
    )?;

    // Calculate fees
    let protocol_fee_amount =
        get_protocol_fee_amount(&input_token_amount, fees.protocol_fee_percent)?;
    let input_amount_without_protocol_fee =
        get_amount_without_fee(&input_token_amount, protocol_fee_amount.clone());

    // Transfer input amount - protocol fee to contract
    let mut msgs: Vec<CosmosMsg> = vec![];
    match input_token.denom.clone() {
        Denom::Cw1155(addr, _) => msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &input_amount_without_protocol_fee.get_token1155(),
        )?),
        Denom::Cw20(addr) => msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            input_amount_without_protocol_fee.get_token2(),
        )?),
        _ => {}
    };

    // Send protocol fee to protocol fee recipient
    if let Some(protocol_fee_amount) = protocol_fee_amount {
        msgs.push(get_fee_transfer_msg(
            &info.sender,
            &fees.protocol_fee_recipient,
            &input_token.denom,
            protocol_fee_amount,
        )?)
    }

    let output_amm_address = deps.api.addr_validate(&output_amm_address)?;

    // Increase allowance of output contract is transfer token is cw20
    if let Denom::Cw20(addr) = &transfer_token.denom {
        msgs.push(get_cw20_increase_allowance_msg(
            addr,
            &output_amm_address,
            amount_to_transfer,
            Some(Expiration::AtHeight(env.block.height + 1)),
        )?)
    };

    let resp: InfoResponse = deps
        .querier
        .query_wasm_smart(&output_amm_address, &QueryMsg::Info {})?;

    let transfer_input_token_enum = if transfer_token.denom == resp.token1_denom {
        Ok(TokenSelect::Token1155)
    } else if transfer_token.denom == resp.token2_denom {
        Ok(TokenSelect::Token2)
    } else {
        Err(ContractError::InvalidOutputPool {})
    }?;

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token: transfer_input_token_enum,
        input_amount: TokenAmount::Token2(amount_to_transfer),
        recipient: info.sender.to_string(),
        min_token: output_min_token,
        expiration,
    };

    msgs.push(
        WasmMsg::Execute {
            contract_addr: output_amm_address.into(),
            msg: to_binary(&swap_msg)?,
            funds: match transfer_token.denom {
                Denom::Native(denom) => vec![Coin {
                    denom,
                    amount: amount_to_transfer,
                }],
                _ => vec![],
            },
        }
        .into(),
    );

    input_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        // Add input amount - protocol fee to input token reserve
        let input_amount_without_protocol_fee_total = match input_amount_without_protocol_fee {
            TokenAmount::Token1155(amounts) => get_token_total_amount(&amounts),
            TokenAmount::Token2(amount) => amount,
        };
        token.reserve = token
            .reserve
            .checked_add(input_amount_without_protocol_fee_total)
            .map_err(StdError::overflow)?;

        Ok(token)
    })?;

    transfer_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(amount_to_transfer)
            .map_err(StdError::overflow)?;

        Ok(token)
    })?;

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("input_token_amount", input_token_amount_total),
        attr("native_transferred", amount_to_transfer),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Info {} => to_binary(&query_info(deps)?),
        QueryMsg::Token1ForToken2Price { token1_amount } => {
            to_binary(&query_token1_for_token2_price(deps, token1_amount)?)
        }
        QueryMsg::Token2ForToken1Price { token2_amount } => {
            to_binary(&query_token2_for_token1_price(deps, token2_amount)?)
        }
        QueryMsg::Fee {} => to_binary(&query_fee(deps)?),
        QueryMsg::TokenSupplies { tokens_id } => to_binary(&query_tokens_supply(deps, tokens_id)?),
    }
}

pub fn query_token_metadata(deps: Deps, id: String) -> StdResult<QueryTokenMetadataResponse> {
    deps.querier.query(&QueryRequest::Stargate {
        path: "/ixo.token.v1beta1.Query/TokenMetadata".to_string(),
        data: Binary::from(QueryTokenMetadataRequest { id }.encode_to_vec()),
    })
}

pub fn query_tokens_supply(
    deps: Deps,
    tokens_id: Vec<TokenId>,
) -> StdResult<TokenSuppliesResponse> {
    let mut supplies = vec![];

    for token_id in tokens_id.into_iter() {
        let lp_token = TOKEN_SUPPLIES.may_load(deps.storage, token_id)?;

        supplies.push(lp_token.unwrap_or_default());
    }

    Ok(TokenSuppliesResponse { supplies })
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let lp_token_address = LP_ADDRESS.load(deps.storage)?;

    // TODO get total supply
    Ok(InfoResponse {
        token1_reserve: token1155.reserve,
        token1_denom: token1155.denom,
        token2_reserve: token2.reserve,
        token2_denom: token2.denom,
        lp_token_supply: get_lp_token_supply(deps, &lp_token_address)?,
        lp_token_address: lp_token_address.into_string(),
    })
}

pub fn query_token1_for_token2_price(
    deps: Deps,
    token1_amount: Uint128,
) -> StdResult<Token1ForToken2PriceResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token2_amount = get_input_price(
        token1_amount,
        token1155.reserve,
        token2.reserve,
        total_fee_percent,
    )?;
    Ok(Token1ForToken2PriceResponse { token2_amount })
}

pub fn query_token2_for_token1_price(
    deps: Deps,
    token2_amount: Uint128,
) -> StdResult<Token2ForToken1PriceResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token1_amount = get_input_price(
        token2_amount,
        token2.reserve,
        token1155.reserve,
        total_fee_percent,
    )?;
    Ok(Token2ForToken1PriceResponse { token1_amount })
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
            let cw20_addr = deps.api.addr_validate(&res.contract_address)?;

            // Save gov token
            LP_ADDRESS.save(deps.storage, &cw20_addr)?;

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
        let liquidity =
            get_lp_token_amount_to_mint(Uint128::new(100), Uint128::zero(), Uint128::zero())
                .unwrap();
        assert_eq!(liquidity, Uint128::new(100));

        let liquidity =
            get_lp_token_amount_to_mint(Uint128::new(100), Uint128::new(50), Uint128::new(25))
                .unwrap();
        assert_eq!(liquidity, Uint128::new(200));
    }

    #[test]
    fn test_get_token_amount() {
        let liquidity = get_token2_amount_required(
            Uint128::new(100),
            Uint128::new(50),
            Uint128::zero(),
            Uint128::zero(),
            Uint128::zero(),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128::new(100));

        let liquidity = get_token2_amount_required(
            Uint128::new(200),
            Uint128::new(50),
            Uint128::new(50),
            Uint128::new(100),
            Uint128::new(25),
        )
        .unwrap();
        assert_eq!(liquidity, Uint128::new(201));
    }

    #[test]
    fn test_get_input_price() {
        let fee_percent = Decimal::from_str("0.3").unwrap();
        // Base case
        assert_eq!(
            get_input_price(
                Uint128::new(10),
                Uint128::new(100),
                Uint128::new(100),
                fee_percent
            )
            .unwrap(),
            Uint128::new(9)
        );

        // No input reserve error
        let err = get_input_price(
            Uint128::new(10),
            Uint128::new(0),
            Uint128::new(100),
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));

        // No output reserve error
        let err = get_input_price(
            Uint128::new(10),
            Uint128::new(100),
            Uint128::new(0),
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));

        // No reserve error
        let err = get_input_price(
            Uint128::new(10),
            Uint128::new(0),
            Uint128::new(0),
            fee_percent,
        )
        .unwrap_err();
        assert_eq!(err, StdError::generic_err("No liquidity"));
    }
}
