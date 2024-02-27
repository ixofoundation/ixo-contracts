use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;

use cosmwasm_std::{
    attr, entry_point, to_json_binary, Addr, Binary, BlockInfo, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, Order, QueryRequest, Reply, Response, StdError, StdResult, Storage,
    SubMsg, Uint128, Uint512, WasmMsg,
};
use cw1155::{Cw1155ExecuteMsg, TokenId};
use cw2::set_contract_version;
use cw20_lp::{Cw20ExecuteMsg, Expiration, MinterResponse};
use cw_utils::{must_pay, parse_reply_instantiate_data};
use prost::Message;

use crate::error::ContractError;
use crate::msg::{
    Denom, ExecuteMsg, FeeResponse, FreezeStatusResponse, InfoResponse, InstantiateMsg,
    OwnershipResponse, QueryDenomMetadataRequest, QueryDenomMetadataResponse, QueryMsg,
    QueryTokenMetadataRequest, QueryTokenMetadataResponse, SlippageResponse,
    Token1155ForToken2PriceResponse, Token2ForToken1155PriceResponse, TokenSelect,
    TokenSuppliesResponse,
};
use crate::state::{
    Fees, Token, FEES, FROZEN, LP_ADDRESS, MAX_SLIPPAGE_PERCENT, OWNER, PENDING_OWNER, TOKEN1155,
    TOKEN2, TOKEN_SUPPLIES,
};
use crate::token_amount::TokenAmount;
use crate::utils::{decimal_to_uint128, MAX_FEE_PERCENT, MAX_PERCENT, SCALE_FACTOR};

// Version info for migration info
pub const CONTRACT_NAME: &str = "crates.io:ixoswap";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_LP_TOKEN_REPLY_ID: u64 = 0;

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    validate_input_tokens(&deps, &msg.token1155_denom, &msg.token2_denom)?;

    let token1155 = Token {
        denom: msg.token1155_denom.clone(),
        reserve: Uint128::zero(),
    };
    TOKEN1155.save(deps.storage, &token1155)?;

    let token2 = Token {
        denom: msg.token2_denom.clone(),
        reserve: Uint128::zero(),
    };
    TOKEN2.save(deps.storage, &token2)?;

    OWNER.save(deps.storage, &info.sender)?;
    PENDING_OWNER.save(deps.storage, &None)?;

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

    validate_percent(msg.max_slippage_percent)?;
    MAX_SLIPPAGE_PERCENT.save(deps.storage, &msg.max_slippage_percent)?;

    // Depositing is not frozen by default
    FROZEN.save(deps.storage, &false)?;

    let instantiate_lp_token_msg = WasmMsg::Instantiate {
        code_id: msg.lp_token_code_id,
        funds: vec![],
        admin: None,
        label: "lp_token".to_string(),
        msg: to_json_binary(&cw20_base_lp::msg::InstantiateMsg {
            name: "IxoSwap_Liquidity_Token".into(),
            symbol: "islpt".into(),
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

fn validate_percent(percent: Decimal) -> Result<(), ContractError> {
    if percent.is_zero() || percent > Decimal::from_str(MAX_PERCENT)? {
        return Err(ContractError::InvalidPercent { percent });
    }

    Ok(())
}

fn validate_input_tokens(
    deps: &DepsMut,
    token1155_denom: &Denom,
    token2_denom: &Denom,
) -> Result<(), ContractError> {
    match (token1155_denom, token2_denom) {
        (Denom::Cw1155(token1155_addr, _), Denom::Cw20(token2_addr)) => {
            deps.api.addr_validate(token1155_addr.as_str())?;
            deps.api.addr_validate(token2_addr.as_str())?;

            if token1155_addr == token2_addr {
                return Err(ContractError::DuplicatedTokenAddress {
                    address: token1155_addr.to_string(),
                });
            }
        }
        (Denom::Cw1155(token1155_addr, _), Denom::Native(native_denom)) => {
            deps.api.addr_validate(token1155_addr.as_str())?;
            validate_native_token_denom(&deps, native_denom)?;
        }
        _ => return Err(ContractError::InvalidTokenType {}),
    }

    Ok(())
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
        ExecuteMsg::UpdateFee {
            protocol_fee_recipient,
            lp_fee_percent,
            protocol_fee_percent,
        } => execute_update_fee(
            deps,
            info,
            lp_fee_percent,
            protocol_fee_percent,
            protocol_fee_recipient,
        ),
        ExecuteMsg::UpdateSlippage {
            max_slippage_percent,
        } => execute_update_slippage(deps, info, max_slippage_percent),
        ExecuteMsg::TransferOwnership { owner } => execute_transfer_ownership(deps, info, owner),
        ExecuteMsg::ClaimOwnership {} => execute_claim_ownership(deps, info),
        ExecuteMsg::FreezeDeposits { freeze } => execute_freeze_deposits(deps, info.sender, freeze),
    }
}

fn execute_freeze_deposits(
    deps: DepsMut,
    sender: Addr,
    freeze: bool,
) -> Result<Response, ContractError> {
    if sender != OWNER.load(deps.storage)? {
        return Err(ContractError::UnauthorizedPoolFreeze {});
    }

    FROZEN.update(deps.storage, |freeze_status| -> Result<_, ContractError> {
        if freeze_status.eq(&freeze) {
            return Err(ContractError::DuplicatedFreezeStatus { freeze_status });
        }
        Ok(freeze)
    })?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "freeze-contracts"),
        attr("freeze_status", freeze.to_string()),
    ]))
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

    validate_min_token(min_liquidity)?;
    validate_token1155_denom(&deps, &token1155.denom, &token1155_amounts)?;
    validate_input_amount(
        &info.funds,
        &TokenAmount::Single(max_token2),
        &token2.denom,
        &info.sender,
    )?;

    let token1155_total_amount = TokenAmount::Multiple(token1155_amounts.clone()).get_total();
    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
    let liquidity_amount =
        get_lp_token_amount_to_mint(token1155_total_amount, lp_token_supply, token1155.reserve)?;

    validate_slippage(&deps, min_liquidity, liquidity_amount)?;

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
            attr("action", "add-liquidity"),
            attr("token1155_amount", token1155_total_amount),
            attr("token2_amount", token2_amount),
            attr("liquidity_received", liquidity_amount),
        ]))
}

fn validate_native_token_denom(deps: &DepsMut, denom: &String) -> Result<(), ContractError> {
    let denom_metadata: QueryDenomMetadataResponse =
        query_denom_metadata(deps.as_ref(), denom.clone())?;

    if denom_metadata.metadata.is_none() {
        return Err(ContractError::UnsupportedTokenDenom { id: denom.clone() });
    }

    Ok(())
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

fn validate_min_token(min_token: Uint128) -> Result<(), ContractError> {
    if min_token.is_zero() {
        return Err(ContractError::MinTokenError {});
    }

    Ok(())
}

fn validate_slippage(
    deps: &DepsMut,
    min_token_amount: Uint128,
    actual_token_amount: Uint128,
) -> Result<(), ContractError> {
    let max_slippage_percent = MAX_SLIPPAGE_PERCENT.load(deps.storage)?;

    let actual_token_decimal_amount = Decimal::from_str(actual_token_amount.to_string().as_str())?;
    let min_required_decimal_amount = actual_token_decimal_amount
        - (actual_token_decimal_amount * max_slippage_percent) / Decimal::from_str(MAX_PERCENT)?;
    let min_required_amount = min_required_decimal_amount.to_uint_floor();

    if min_token_amount < min_required_amount {
        return Err(ContractError::MinTokenAmountError {
            min_token: min_token_amount,
            min_required: min_required_amount,
        });
    }

    Ok(())
}

fn get_cw1155_transfer_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    tokens: &HashMap<String, Uint128>,
) -> Result<CosmosMsg, ContractError> {
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
        msg: to_json_binary(&transfer_cw1155_msg)?,
        funds: vec![],
    };
    let cw1155_transfer_cosmos_msg: CosmosMsg = exec_cw1155_transfer.into();

    Ok(cw1155_transfer_cosmos_msg)
}

fn get_lp_token_supply(deps: Deps, lp_token_addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20_lp::TokenInfoResponse = deps
        .querier
        .query_wasm_smart(lp_token_addr, &cw20_base_lp::msg::QueryMsg::TokenInfo {})?;
    Ok(resp.total_supply)
}

fn mint_lp_tokens(
    recipient: &Addr,
    liquidity_amount: Uint128,
    lp_token_address: &Addr,
) -> StdResult<CosmosMsg> {
    let mint_msg = cw20_base_lp::msg::ExecuteMsg::Mint {
        recipient: recipient.into(),
        amount: liquidity_amount,
    };
    Ok(WasmMsg::Execute {
        contract_addr: lp_token_address.to_string(),
        msg: to_json_binary(&mint_msg)?,
        funds: vec![],
    }
    .into())
}

fn get_token_balance(deps: Deps, contract: &Addr, addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20_lp::BalanceResponse = deps.querier.query_wasm_smart(
        contract,
        &cw20_base_lp::msg::QueryMsg::Balance {
            address: addr.to_string(),
        },
    )?;
    Ok(resp.balance)
}

fn validate_input_amount(
    actual_funds: &[Coin],
    given_amount: &TokenAmount,
    given_denom: &Denom,
    sender: &Addr,
) -> Result<(), ContractError> {
    match given_denom {
        Denom::Native(denom) => {
            let actual_amount = must_pay(
                &MessageInfo {
                    sender: sender.clone(),
                    funds: actual_funds.to_vec(),
                },
                denom,
            )?;
            if actual_amount != given_amount.get_single()? {
                return Err(ContractError::InsufficientFunds {});
            }

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
) -> Result<CosmosMsg, ContractError> {
    // create transfer cw20 msg
    let transfer_cw20_msg = Cw20ExecuteMsg::TransferFrom {
        owner: owner.into(),
        recipient: recipient.into(),
        amount: token_amount,
    };
    let exec_cw20_transfer = WasmMsg::Execute {
        contract_addr: token_addr.into(),
        msg: to_json_binary(&transfer_cw20_msg)?,
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
        msg: to_json_binary(&increase_allowance_msg)?,
        funds: vec![],
    };
    Ok(exec_allowance.into())
}

pub fn execute_transfer_ownership(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Option<String>,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut attributes = vec![attr("action", "transfer-ownership")];
    let new_owner_addr = new_owner
        .as_ref()
        .map(|h| deps.api.addr_validate(h))
        .transpose()?;
    if let Some(new_owner_addr) = new_owner_addr.clone() {
        if owner == new_owner_addr {
            return Err(ContractError::DuplicatedOwner {});
        }

        attributes.push(attr("pending_owner", new_owner_addr))
    }

    PENDING_OWNER.save(deps.storage, &new_owner_addr)?;

    Ok(Response::new().add_attributes(attributes))
}

pub fn execute_claim_ownership(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let pending_owner = PENDING_OWNER.load(deps.storage)?;

    let mut attributes = vec![attr("action", "claim-ownership")];
    if let Some(pending_owner) = pending_owner {
        if info.sender != pending_owner {
            return Err(ContractError::Unauthorized {});
        }

        PENDING_OWNER.save(deps.storage, &None)?;
        OWNER.save(deps.storage, &pending_owner)?;
        attributes.push(attr("new_owner", pending_owner));
    }

    Ok(Response::new().add_attributes(attributes))
}

pub fn execute_update_slippage(
    deps: DepsMut,
    info: MessageInfo,
    max_slippage_percent: Decimal,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    validate_percent(max_slippage_percent)?;
    MAX_SLIPPAGE_PERCENT.save(deps.storage, &max_slippage_percent)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update-slippage"),
        attr("max_slippage_percent", max_slippage_percent.to_string()),
    ]))
}

pub fn execute_update_fee(
    deps: DepsMut,
    info: MessageInfo,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

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

    Ok(Response::new().add_attributes(vec![
        attr("action", "update-config"),
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
    min_token1155: TokenAmount,
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

    let min_token1155_total_amount = min_token1155.get_total();
    validate_min_token(min_token1155_total_amount)?;
    validate_min_token(min_token2)?;

    let token1155_amount = amount
        .checked_mul(token1155.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;
    let token2_amount = amount
        .checked_mul(token2.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;

    validate_slippage(&deps, min_token1155_total_amount, token1155_amount)?;
    validate_slippage(&deps, min_token2, token2_amount)?;

    if token1155_amount < min_token1155_total_amount {
        return Err(ContractError::MinToken1155Error {
            requested: min_token1155_total_amount,
            available: token1155_amount,
        });
    }
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

    let token1155_amounts_to_transfer =
        get_token_amounts_to_transfer(deps.storage, token1155_amount, min_token1155)?;

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
        attr("action", "remove-liquidity"),
        attr("liquidity_burned", amount),
        attr("token1155_returned", token1155_amount),
        attr("token2_returned", token2_amount),
    ]))
}

fn get_token_amounts_to_transfer(
    storage: &mut dyn Storage,
    token1155_amount: Uint128,
    min_token1155: TokenAmount,
) -> Result<HashMap<TokenId, Uint128>, ContractError> {
    let mut token1155_amount_left_to_transfer = token1155_amount;
    let mut token1155_amounts_to_transfer: HashMap<TokenId, Uint128> = HashMap::new();

    match min_token1155 {
        TokenAmount::Multiple(amounts) => {
            let mut token1155_supplies: HashMap<TokenId, Uint128> = HashMap::new();

            for (token_id, token_amount) in amounts.into_iter() {
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
                update_token_supplies(
                    storage,
                    remaining_supply,
                    token_id,
                    Some(&mut token1155_supplies),
                )?;
            }

            while !token1155_amount_left_to_transfer.is_zero() && !token1155_supplies.is_empty() {
                let additional_amount_to_transfer = token1155_amount_left_to_transfer
                    .checked_div(Uint128::from(token1155_supplies.len() as u32))
                    .map_err(StdError::divide_by_zero)?;

                for (token_id, token_amount) in token1155_supplies.clone().into_iter() {
                    if token1155_amount_left_to_transfer.is_zero() {
                        break;
                    }

                    let mut optional_additional_amount = None;
                    let mut optional_supplies = None;
                    if !additional_amount_to_transfer.is_zero() {
                        optional_additional_amount = Some(additional_amount_to_transfer);
                        optional_supplies = Some(&mut token1155_supplies);
                    };

                    update_token_amounts(
                        storage,
                        &mut token1155_amounts_to_transfer,
                        &mut token1155_amount_left_to_transfer,
                        token_id,
                        token_amount,
                        optional_additional_amount,
                        optional_supplies,
                    )?;
                }
            }
        }
        TokenAmount::Single(_) => {
            let token_supplies = TOKEN_SUPPLIES
                .range(storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<_>>>()?;

            let mut supply_index = 0;
            while !token1155_amount_left_to_transfer.is_zero() {
                let (token_id, token_amount) = token_supplies[supply_index].clone();

                update_token_amounts(
                    storage,
                    &mut token1155_amounts_to_transfer,
                    &mut token1155_amount_left_to_transfer,
                    token_id,
                    token_amount,
                    None,
                    None,
                )?;
                supply_index += 1;
            }
        }
    };

    Ok(token1155_amounts_to_transfer.clone())
}

fn update_token_amounts(
    storage: &mut dyn Storage,
    token_amounts_to_transfer: &mut HashMap<String, Uint128>,
    token_amount_left_to_transfer: &mut Uint128,
    token_id: String,
    token_amount: Uint128,
    additional_token_amount_to_transfer: Option<Uint128>,
    token_supplies: Option<&mut HashMap<String, Uint128>>,
) -> StdResult<()> {
    let mut amount_to_transfer = *token_amounts_to_transfer
        .get(&token_id)
        .unwrap_or(&Uint128::zero());

    let mut token_supply = *token_amount_left_to_transfer;
    if let Some(token_amount) = additional_token_amount_to_transfer {
        token_supply = token_amount;
    }

    let remaining_supply;
    if token_amount >= token_supply {
        amount_to_transfer += token_supply;
        remaining_supply = token_amount - token_supply;

        if let Some(token_amount) = additional_token_amount_to_transfer {
            *token_amount_left_to_transfer -= token_amount;
        } else {
            *token_amount_left_to_transfer = Uint128::zero();
        }
    } else {
        amount_to_transfer += token_amount;
        remaining_supply = Uint128::zero();
        *token_amount_left_to_transfer -= token_amount;
    }

    token_amounts_to_transfer.insert(token_id.clone(), amount_to_transfer);
    update_token_supplies(storage, remaining_supply, token_id.clone(), token_supplies)
}

fn update_token_supplies(
    storage: &mut dyn Storage,
    remaining_supply: Uint128,
    token_id: String,
    token_supplies: Option<&mut HashMap<String, Uint128>>,
) -> StdResult<()> {
    if remaining_supply.is_zero() {
        TOKEN_SUPPLIES.remove(storage, token_id.clone());

        if let Some(token_supplies) = token_supplies {
            token_supplies.remove(&token_id);
        }
    } else {
        TOKEN_SUPPLIES.save(storage, token_id.clone(), &remaining_supply)?;

        if let Some(token_supplies) = token_supplies {
            token_supplies.insert(token_id, remaining_supply);
        }
    }

    Ok(())
}

fn get_burn_msg(contract: &Addr, owner: &Addr, amount: Uint128) -> StdResult<CosmosMsg> {
    let msg = cw20_base_lp::msg::ExecuteMsg::BurnFrom {
        owner: owner.to_string(),
        amount,
    };
    Ok(WasmMsg::Execute {
        contract_addr: contract.to_string(),
        msg: to_json_binary(&msg)?,
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
        msg: to_json_binary(&transfer_cw20_msg)?,
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
) -> Result<CosmosMsg, ContractError> {
    match fee_denom {
        Denom::Cw1155(addr, _) => {
            get_cw1155_transfer_msg(sender, recipient, addr, &amount.get_multiple()?)
        }
        Denom::Cw20(addr) => {
            get_cw20_transfer_from_msg(sender, recipient, addr, amount.get_single()?)
        }
        Denom::Native(denom) => Ok(get_bank_transfer_to_msg(
            recipient,
            denom,
            amount.get_single()?,
        )),
    }
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

    let fee_percent = decimal_to_uint128(fee_percent)?;
    let fee_reduction_percent = SCALE_FACTOR - fee_percent;
    let input_amount_with_fee = Uint512::from(input_amount.full_mul(fee_reduction_percent));
    let numerator = input_amount_with_fee
        .checked_mul(Uint512::from(output_reserve))
        .map_err(StdError::overflow)?;
    let denominator = Uint512::from(input_reserve)
        .checked_mul(Uint512::from(SCALE_FACTOR))
        .map_err(StdError::overflow)?
        .checked_add(input_amount_with_fee)
        .map_err(StdError::overflow)?;

    Ok(numerator
        .checked_div(denominator)
        .map_err(StdError::divide_by_zero)?
        .try_into()?)
}

fn get_amount_without_fee(
    input_amount: &TokenAmount,
    fee_amount: Option<TokenAmount>,
) -> Result<TokenAmount, ContractError> {
    if let Some(fee_amount) = fee_amount {
        match input_amount.clone() {
            TokenAmount::Multiple(mut input_amounts) => {
                let fee_amounts = fee_amount.get_multiple()?;

                for (token_id, token_amount) in input_amounts.iter_mut() {
                    let fee_amount = fee_amounts.get(token_id).unwrap();

                    *token_amount -= fee_amount
                }

                Ok(TokenAmount::Multiple(input_amounts))
            }
            TokenAmount::Single(input_amount) => {
                let fee_amount = fee_amount.get_single()?;

                Ok(TokenAmount::Single(input_amount - fee_amount))
            }
        }
    } else {
        Ok(input_amount.clone())
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

    if input_token_enum == TokenSelect::Token1155 {
        validate_token1155_denom(&deps, &input_token.denom, &input_amount.get_multiple()?)?;
    }

    let min_token_total = min_token.get_total();
    validate_min_token(min_token_total)?;
    validate_input_amount(&info.funds, &input_amount, &input_token.denom, &info.sender)?;

    let input_amount_total = input_amount.get_total();
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let mut token_bought = get_input_price(
        input_amount_total,
        input_token.reserve,
        output_token.reserve,
        total_fee_percent,
    )?;

    validate_slippage(&deps, min_token_total, token_bought)?;

    if min_token_total > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token_total,
            available: token_bought,
        });
    }
    // Calculate fees
    let protocol_fee_amount = input_amount.get_percent(fees.protocol_fee_percent)?;
    let input_amount_without_protocol_fee =
        get_amount_without_fee(&input_amount, protocol_fee_amount.clone())?;

    let mut msgs = vec![];
    match input_token.denom.clone() {
        Denom::Cw1155(addr, _) => msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &input_amount_without_protocol_fee.get_multiple()?,
        )?),
        Denom::Cw20(addr) => msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            input_amount_without_protocol_fee.get_single()?,
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
            let tokens_to_transfer =
                get_token_amounts_to_transfer(deps.storage, token_bought, min_token)?;
            token_bought = TokenAmount::Multiple(tokens_to_transfer.clone()).get_total();

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
            let input_amount_without_protocol_fee_total =
                input_amount_without_protocol_fee.get_total();
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

    if let TokenAmount::Multiple(input_amounts) = input_amount_without_protocol_fee {
        for (token_id, token_amount) in input_amounts.into_iter() {
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
    }

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "swap"),
        attr("recipient", recipient),
        attr("token_sold", input_amount_total),
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

    if input_token_enum == TokenSelect::Token1155 {
        validate_token1155_denom(
            &deps,
            &input_token.denom,
            &input_token_amount.get_multiple()?,
        )?;
    }

    validate_input_amount(
        &info.funds,
        &input_token_amount,
        &input_token.denom,
        &info.sender,
    )?;

    let input_token_amount_total = input_token_amount.get_total();
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let amount_to_transfer = get_input_price(
        input_token_amount_total,
        input_token.reserve,
        transfer_token.reserve,
        total_fee_percent,
    )?;

    // Calculate fees
    let protocol_fee_amount = input_token_amount.get_percent(fees.protocol_fee_percent)?;
    let input_amount_without_protocol_fee =
        get_amount_without_fee(&input_token_amount, protocol_fee_amount.clone())?;

    // Transfer input amount - protocol fee to contract
    let mut msgs: Vec<CosmosMsg> = vec![];
    match input_token.denom.clone() {
        Denom::Cw1155(addr, _) => msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &input_amount_without_protocol_fee.get_multiple()?,
        )?),
        Denom::Cw20(addr) => msgs.push(get_cw20_transfer_from_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            input_amount_without_protocol_fee.get_single()?,
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

    let transfer_input_token_enum = if transfer_token.denom == resp.token1155_denom {
        Ok(TokenSelect::Token1155)
    } else if transfer_token.denom == resp.token2_denom {
        Ok(TokenSelect::Token2)
    } else {
        Err(ContractError::InvalidOutputPool {})
    }?;

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token: transfer_input_token_enum,
        input_amount: TokenAmount::Single(amount_to_transfer),
        recipient: info.sender.to_string(),
        min_token: output_min_token,
        expiration,
    };

    msgs.push(
        WasmMsg::Execute {
            contract_addr: output_amm_address.into(),
            msg: to_json_binary(&swap_msg)?,
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
        let input_amount_without_protocol_fee_total = input_amount_without_protocol_fee.get_total();
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

    if let TokenAmount::Multiple(input_amounts) = input_amount_without_protocol_fee {
        for (token_id, token_amount) in input_amounts.into_iter() {
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
    }

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "cross-contract-swap"),
        attr("input_token_amount", input_token_amount_total),
        attr("native_transferred", amount_to_transfer),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Info {} => to_json_binary(&query_info(deps)?),
        QueryMsg::Token1155ForToken2Price { token1155_amount } => {
            to_json_binary(&query_token1155_for_token2_price(deps, token1155_amount)?)
        }
        QueryMsg::Token2ForToken1155Price { token2_amount } => {
            to_json_binary(&query_token2_for_token1155_price(deps, token2_amount)?)
        }
        QueryMsg::Fee {} => to_json_binary(&query_fee(deps)?),
        QueryMsg::TokenSupplies { tokens_id } => {
            to_json_binary(&query_tokens_supply(deps, tokens_id)?)
        }
        QueryMsg::FreezeStatus {} => to_json_binary(&query_freeze_status(deps)?),
        QueryMsg::Ownership {} => to_json_binary(&query_ownership(deps)?),
        QueryMsg::Slippage {} => to_json_binary(&query_slippage(deps)?),
    }
}

pub fn query_token_metadata(deps: Deps, id: String) -> StdResult<QueryTokenMetadataResponse> {
    deps.querier.query(&QueryRequest::Stargate {
        path: "/ixo.token.v1beta1.Query/TokenMetadata".to_string(),
        data: Binary::from(QueryTokenMetadataRequest { id }.encode_to_vec()),
    })
}

pub fn query_denom_metadata(deps: Deps, denom: String) -> StdResult<QueryDenomMetadataResponse> {
    deps.querier.query(&QueryRequest::Stargate {
        path: "/cosmos.bank.v1beta1.Query/DenomMetadata".to_string(),
        data: Binary::from(QueryDenomMetadataRequest { denom }.encode_to_vec()),
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

pub fn query_slippage(deps: Deps) -> StdResult<SlippageResponse> {
    let max_slippage_percent = MAX_SLIPPAGE_PERCENT.load(deps.storage)?;

    Ok(SlippageResponse {
        max_slippage_percent,
    })
}

pub fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;
    let lp_token_address = LP_ADDRESS.load(deps.storage)?;

    Ok(InfoResponse {
        token1155_reserve: token1155.reserve,
        token1155_denom: token1155.denom,
        token2_reserve: token2.reserve,
        token2_denom: token2.denom,
        lp_token_supply: get_lp_token_supply(deps, &lp_token_address)?,
        lp_token_address: lp_token_address.into_string(),
    })
}

pub fn query_ownership(deps: Deps) -> StdResult<OwnershipResponse> {
    let owner = OWNER.load(deps.storage)?.to_string();
    let pending_owner = PENDING_OWNER.load(deps.storage)?.map(|o| o.into_string());

    Ok(OwnershipResponse {
        owner,
        pending_owner,
    })
}

pub fn query_freeze_status(deps: Deps) -> StdResult<FreezeStatusResponse> {
    let freeze_status = FROZEN.load(deps.storage)?;

    Ok(FreezeStatusResponse {
        status: freeze_status,
    })
}

pub fn query_token1155_for_token2_price(
    deps: Deps,
    token1155_amount: TokenAmount,
) -> StdResult<Token1155ForToken2PriceResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let token1155_amount_total = token1155_amount.get_total();
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token2_amount = get_input_price(
        token1155_amount_total,
        token1155.reserve,
        token2.reserve,
        total_fee_percent,
    )?;

    Ok(Token1155ForToken2PriceResponse { token2_amount })
}

pub fn query_token2_for_token1155_price(
    deps: Deps,
    token2_amount: TokenAmount,
) -> StdResult<Token2ForToken1155PriceResponse> {
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    let token2_amount_total = token2_amount.get_total();
    let fees = FEES.load(deps.storage)?;
    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    let token1155_amount = get_input_price(
        token2_amount_total,
        token2.reserve,
        token1155.reserve,
        total_fee_percent,
    )?;

    Ok(Token2ForToken1155PriceResponse { token1155_amount })
}

pub fn query_fee(deps: Deps) -> StdResult<FeeResponse> {
    let fees = FEES.load(deps.storage)?;

    Ok(FeeResponse {
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

            Ok(Response::new().add_attribute("liquidity_pool_token_address", cw20_addr))
        }
        Err(_) => Err(ContractError::InstantiateLpTokenError {}),
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_dependencies;

    use super::*;

    #[test]
    fn should_return_lp_token_amount() {
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
    fn should_return_token_amount() {
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
    fn should_return_input_price() {
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
    }

    #[test]
    fn should_fail_returning_input_price() {
        let fee_percent = Decimal::from_str("0.3").unwrap();

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

    #[test]
    fn should_fail_returning_token_amounts_to_transfer_when_insufficient_supply_of_min_token() {
        let mut deps = mock_dependencies();

        let token_id = "1".to_string();
        let token_amounts = HashMap::from([(token_id.clone(), Uint128::new(100))]);

        TOKEN_SUPPLIES
            .save(&mut deps.storage, token_id.clone(), &Uint128::new(1))
            .unwrap();

        let err = get_token_amounts_to_transfer(
            &mut deps.storage,
            Uint128::new(500),
            TokenAmount::Multiple(token_amounts.clone()),
        )
        .unwrap_err();

        assert_eq!(
            err,
            ContractError::MinToken1155Error {
                available: Uint128::new(1),
                requested: Uint128::new(100)
            }
        );
    }

    #[test]
    fn should_return_token_amounts_to_transfer_when_multiple_input_token_provided() {
        let mut deps = mock_dependencies();

        let token_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let mut token_amounts = HashMap::new();
        let mut token_supplies = HashMap::new();

        for token_id in token_ids {
            token_amounts.insert(token_id.clone(), Uint128::new(100));
            token_supplies.insert(token_id.clone(), Uint128::new(1000));
            TOKEN_SUPPLIES
                .save(&mut deps.storage, token_id.clone(), &Uint128::new(1000))
                .unwrap();
        }

        let token_amounts_to_transfer = get_token_amounts_to_transfer(
            &mut deps.storage,
            Uint128::new(500),
            TokenAmount::Multiple(token_amounts.clone()),
        )
        .unwrap();

        assert_token_supplies(
            &mut deps.storage,
            &token_amounts_to_transfer,
            &token_supplies,
        );
        assert_eq!(
            TokenAmount::Multiple(token_amounts_to_transfer).get_total(),
            Uint128::new(500)
        )
    }

    #[test]
    fn should_return_partial_token_amounts_to_transfer_when_multiple_input_token_provided() {
        let mut deps = mock_dependencies();

        let token_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let mut token_amounts = HashMap::new();
        let mut token_supplies = HashMap::new();

        for (index, token_id) in token_ids.into_iter().enumerate() {
            let token_index = Uint128::new(index as u128 + 1);
            let token_amount = Uint128::new(100).checked_mul(token_index).unwrap();

            token_amounts.insert(token_id.clone(), token_amount);
            token_supplies.insert(token_id.clone(), token_amount);
            TOKEN_SUPPLIES
                .save(&mut deps.storage, token_id.clone(), &token_amount)
                .unwrap();
        }

        let token_amounts_to_transfer = get_token_amounts_to_transfer(
            &mut deps.storage,
            Uint128::new(1000),
            TokenAmount::Multiple(token_amounts.clone()),
        )
        .unwrap();

        assert_token_supplies(
            &mut deps.storage,
            &token_amounts_to_transfer,
            &token_supplies,
        );
        assert_eq!(
            TokenAmount::Multiple(token_amounts_to_transfer).get_total(),
            Uint128::new(600)
        )
    }

    #[test]
    fn should_return_any_token_amounts_to_transfer_when_single_input_token_provided() {
        let mut deps = mock_dependencies();

        let token_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let mut token_supplies = HashMap::new();

        for token_id in token_ids {
            token_supplies.insert(token_id.clone(), Uint128::new(1000));
            TOKEN_SUPPLIES
                .save(&mut deps.storage, token_id.clone(), &Uint128::new(1000))
                .unwrap();
        }

        let token_amounts_to_transfer = get_token_amounts_to_transfer(
            &mut deps.storage,
            Uint128::new(500),
            TokenAmount::Single(Uint128::new(300)),
        )
        .unwrap();

        assert_token_supplies(
            &mut deps.storage,
            &token_amounts_to_transfer,
            &token_supplies,
        );
        assert_eq!(
            TokenAmount::Multiple(token_amounts_to_transfer).get_total(),
            Uint128::new(500)
        )
    }

    #[test]
    fn should_fail_slippage_validation_when_min_token_amount_less_than_minimum_required() {
        let mut deps = mock_dependencies();

        MAX_SLIPPAGE_PERCENT
            .save(&mut deps.storage, &Decimal::from_str("10").unwrap())
            .unwrap();

        let min_token_amount = Uint128::new(95_000);
        let actual_token_amount = Uint128::new(110_000);
        let err =
            validate_slippage(&deps.as_mut(), min_token_amount, actual_token_amount).unwrap_err();

        assert_eq!(
            err,
            ContractError::MinTokenAmountError {
                min_token: min_token_amount,
                min_required: Uint128::new(99_000)
            }
        );
    }

    #[test]
    fn should_pass_slippage_validation_when_minimum_amount_greater_then_minimum_required() {
        let mut deps = mock_dependencies();

        MAX_SLIPPAGE_PERCENT
            .save(&mut deps.storage, &Decimal::from_str("10").unwrap())
            .unwrap();

        let min_token_amount = Uint128::new(105_000);
        let actual_token_amount = Uint128::new(110_000);
        let res = validate_slippage(&deps.as_mut(), min_token_amount, actual_token_amount).unwrap();

        assert_eq!(res, ());
    }

    #[test]
    fn should_fail_percent_validation_when_percent_greater_than_100() {
        let percent = Decimal::from_str("110").unwrap();
        let err = validate_percent(percent).unwrap_err();
        assert_eq!(err, ContractError::InvalidPercent { percent });
    }

    #[test]
    fn should_fail_percent_validation_when_percent_in_0() {
        let percent = Decimal::from_str("0").unwrap();
        let err = validate_percent(percent).unwrap_err();
        assert_eq!(err, ContractError::InvalidPercent { percent });
    }

    #[test]
    fn should_pass_percent_validation_when_percent_in_allowed_range() {
        let res = validate_percent(Decimal::from_str("50").unwrap()).unwrap();
        assert_eq!(res, ());
    }

    fn assert_token_supplies(
        storage: &mut dyn Storage,
        token_amounts: &HashMap<String, Uint128>,
        token_supplies: &HashMap<String, Uint128>,
    ) {
        for (id, amount) in token_amounts {
            let initial_token_supply = token_supplies.get(id).unwrap();
            let updated_token_supply = TOKEN_SUPPLIES
                .may_load(storage, id.clone())
                .unwrap()
                .unwrap_or(Uint128::zero());

            assert_eq!(updated_token_supply + amount, initial_token_supply)
        }
    }
}
