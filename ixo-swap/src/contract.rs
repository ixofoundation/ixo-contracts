use std::collections::{BTreeMap, HashMap};
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
use crate::utils::{
    calculate_amount_with_percent, decimal_to_uint128, MIN_FEE_PERCENT, PREDEFINED_MAX_FEES_PERCENT, PREDEFINED_MAX_SLIPPAGE_PERCENT, SCALE_FACTOR
};

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
    validate_fee_percent(msg.lp_fee_percent)?;
    validate_fee_percent(msg.protocol_fee_percent)?;
    validate_slippage_percent(msg.max_slippage_percent)?;

    let protocol_fee_recipient = deps.api.addr_validate(&msg.protocol_fee_recipient)?;
    let total_fee_percent = msg.lp_fee_percent + msg.protocol_fee_percent;
    let max_fee_percent = Decimal::from_str(PREDEFINED_MAX_FEES_PERCENT)?;
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
    MAX_SLIPPAGE_PERCENT.save(deps.storage, &msg.max_slippage_percent)?;

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

    Ok(Response::new()
        .add_submessage(reply_msg)
        .add_attributes(vec![
            attr("action", "instantiate-ixo-swap"),
            attr("owner", info.sender),
            attr("max_slippage_percent", msg.max_slippage_percent.to_string()),
            attr("lp_fee_percent", msg.lp_fee_percent.to_string()),
            attr("protocol_fee_percent", msg.protocol_fee_percent.to_string()),
            attr("protocol_fee_recipient", msg.protocol_fee_recipient),
            attr("token_1155_denom", msg.token1155_denom.to_string()),
            attr("token_2_denom", msg.token2_denom.to_string()),
        ])
      )
}

/// Validates that slippage percent is not zero and less than max slippage percent
fn validate_slippage_percent(percent: Decimal) -> Result<(), ContractError> {
    let max_slippage_percent = Decimal::from_str(PREDEFINED_MAX_SLIPPAGE_PERCENT)?;
    if percent.is_zero() || percent > max_slippage_percent {
        return Err(ContractError::InvalidPercent {
            percent,
            max: max_slippage_percent,
        });
    }

    Ok(())
}

/// Validates that fee percent is more than SCALE_FACTOR can handle, or zero
fn validate_fee_percent(percent: Decimal) -> Result<(), ContractError> {
    if percent.is_zero() {
        return Ok(());
    }

    let min_fee_percent = Decimal::from_str(MIN_FEE_PERCENT)?;
    if percent < min_fee_percent {
        return Err(ContractError::FeesTooLow {
            min_fee_percent,
            fee_percent: percent,
        });
    }

    Ok(())
}

/// Validates the input tokens by:
/// - checking that the addresses are valid.
/// - checking that the token addresses are different.
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
        (Denom::Cw1155(token1155_addr, _), Denom::Native(_native_denom)) => {
            deps.api.addr_validate(token1155_addr.as_str())?;
            // Removing this as we deem it unnecessary, since this validates against the bank modules
            // registered DenomMetadata, but we want to allow any native denom to be used, including
            // ibc tokens, without the need to add it to the bank modules DenomMetadata
            // The audited security severity was minor since this just prevents against a misconfiguration
            // by the contract instantiator, thus we deem it okay to bypass this validation
            // validate_native_token_denom(&deps, native_denom)?;
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

/// Executes the `FreezeDeposits` message.
fn execute_freeze_deposits(
    deps: DepsMut,
    sender: Addr,
    freeze: bool,
) -> Result<Response, ContractError> {
    // validate that sender is owner
    if sender != OWNER.load(deps.storage)? {
        return Err(ContractError::UnauthorizedPoolFreeze {});
    }

    // update freeze status and save to storage if not same as current freeze status
    FROZEN.update(deps.storage, |freeze_status| -> Result<_, ContractError> {
        if freeze_status.eq(&freeze) {
            return Err(ContractError::DuplicatedFreezeStatus { freeze_status });
        }
        Ok(freeze)
    })?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "freeze-deposits"),
        attr("frozen", freeze.to_string()),
    ]))
}

/// Validates that expiration is not expired against block height or time
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

/// Calculates the amount of lp token to mint by:
/// - if liquidity supply is zero then return token1_amount
/// - if liquidity supply is not zero then return token1_amount * liquidity_supply / token1_reserve
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

/// Calculates the amount of token2 required for adding to liquidity pool by:
/// - if liquidity supply is zero then return max_token
/// - if liquidity supply is not zero then return token1_amount * token2_reserve / token1_reserve + 1
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

/// Executes the `AddLiquidity` message.
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

    // calculate liquidity amount based on input amounts and do validation
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

    // check that liquidity amount is more than users min liquidity
    if liquidity_amount < min_liquidity {
        return Err(ContractError::MinLiquidityError {
            min_liquidity,
            liquidity_available: liquidity_amount,
        });
    }

    // check that token2 amount that will be used is less than max token provided by user
    if token2_amount > max_token2 {
        return Err(ContractError::MaxTokenError {
            max_token: max_token2,
            tokens_required: token2_amount,
        });
    }

    let mut transfer_msgs: Vec<CosmosMsg> = vec![];
    // add transfer message for 1155 tokens
    if let Denom::Cw1155(addr, _) = token1155.denom {
        transfer_msgs.push(get_cw1155_transfer_msg(
            &info.sender,
            &env.contract.address,
            &addr,
            &token1155_amounts,
        )?)
    }

    // if token2 is cw20 then add transfer message
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

    // update token reserves with newly added amounts
    let updated_token1155 = TOKEN1155.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve += token1155_total_amount;
        Ok(token)
    })?;
    let updated_token2 = TOKEN2.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve += token2_amount;
        Ok(token)
    })?;

    // update lp token supplies to know what 1155 tokens is owned by the contract
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

    // mint lp tokens to user
    transfer_msgs.push(mint_lp_tokens(&info.sender, liquidity_amount, &lp_token_addr)?);

    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_attributes(vec![
            attr("action", "add-liquidity"),
            attr("token1155_amount", token1155_total_amount),
            attr("token2_amount", token2_amount),
            attr("liquidity_received", liquidity_amount),
            attr("liquidity_receiver", info.sender.to_string()),
            attr("token1155_reserve", updated_token1155.reserve),
            attr("token2_reserve", updated_token2.reserve),
        ]))
}

/// Validates that native token denom is supported by the bank module's DebomMetadata
#[allow(dead_code)]
fn validate_native_token_denom(deps: &DepsMut, denom: &String) -> Result<(), ContractError> {
    let denom_metadata: QueryDenomMetadataResponse =
        query_denom_metadata(deps.as_ref(), denom.clone())?;

    if denom_metadata.metadata.is_none() {
        return Err(ContractError::UnsupportedTokenDenom { id: denom.clone() });
    }

    Ok(())
}

/// Validates that 1155 tokens have supported denom as well as query each
/// token id from chain's Token module to ensure that it is a valid token
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

/// Validates that min token is above zero
fn validate_min_token(min_token: Uint128) -> Result<(), ContractError> {
    if min_token.is_zero() {
        return Err(ContractError::MinTokenError {});
    }

    Ok(())
}

/// Validates that slippage is ok by calculating the minimum possible amount based on MAX_SLIPPAGE_PERCENT constant
/// and validates that min_token_amount is more than that minimum amount
fn validate_slippage(
    deps: &DepsMut,
    min_token_amount: Uint128,
    actual_token_amount: Uint128,
) -> Result<(), ContractError> {
    let max_slippage = MAX_SLIPPAGE_PERCENT.load(deps.storage)?;
    let max_slippage_percent = decimal_to_uint128(max_slippage)?;

    let slippage_impact = calculate_amount_with_percent(actual_token_amount, max_slippage_percent)?;

    let min_required_amount = actual_token_amount - slippage_impact;

    if min_token_amount < min_required_amount {
        return Err(ContractError::MinTokenAmountError {
            min_token: min_token_amount,
            min_required: min_required_amount,
        });
    }

    Ok(())
}

/// Creates a ixo1155 transfer message for all tokens in tokens hashmap
fn get_cw1155_transfer_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    tokens: &HashMap<String, Uint128>,
) -> Result<CosmosMsg, ContractError> {
    // Convert HashMap to BTreeMap to maintain deterministic order by key
    let sorted_tokens: BTreeMap<_, _> = tokens.iter().collect();

    let transfer_cw1155_msg = Cw1155ExecuteMsg::BatchSendFrom {
        from: owner.into(),
        to: recipient.into(),
        batch: sorted_tokens
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

/// Queries the total supply of lp token, which is cw20 contract on chain
fn get_lp_token_supply(deps: Deps, lp_token_addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20_lp::TokenInfoResponse = deps
        .querier
        .query_wasm_smart(lp_token_addr, &cw20_base_lp::msg::QueryMsg::TokenInfo {})?;
    Ok(resp.total_supply)
}

/// Creates a mint message for the given amount of lp token to the given recipient
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

/// Queries the balance of the given cw20 contract on chain
fn get_token_balance(deps: Deps, contract: &Addr, addr: &Addr) -> StdResult<Uint128> {
    let resp: cw20_lp::BalanceResponse = deps.querier.query_wasm_smart(
        contract,
        &cw20_base_lp::msg::QueryMsg::Balance {
            address: addr.to_string(),
        },
    )?;
    Ok(resp.balance)
}

/// validates that when input token is Native denom, then info.funds is same as input amount and that user has enough funds
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

/// Creates a cw20 transfer message for the given token amount
fn get_cw20_transfer_from_msg(
    owner: &Addr,
    recipient: &Addr,
    token_addr: &Addr,
    token_amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
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

/// Creates a cw20 increase allowance message for the given token amount and spender
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

/// Executes the `TransferOwnership` message.
pub fn execute_transfer_ownership(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Option<String>,
) -> Result<Response, ContractError> {
    // validate that sender is owner
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut attributes = vec![attr("action", "transfer-ownership")];

    // validate that new owner is valid and not same as current owner
    let new_owner_addr = new_owner
        .as_ref()
        .map(|h| deps.api.addr_validate(h))
        .transpose()?;
    if let Some(new_owner_addr) = new_owner_addr.clone() {
        if owner == new_owner_addr {
            return Err(ContractError::DuplicatedOwner {});
        }

        attributes.push(attr("pending_owner", new_owner_addr.to_string()))
    }

    // save new owner to pending owner
    PENDING_OWNER.save(deps.storage, &new_owner_addr)?;

    Ok(Response::new().add_attributes(attributes))
}

/// Executes the `ClaimOwnership` message.
pub fn execute_claim_ownership(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let pending_owner = PENDING_OWNER.load(deps.storage)?;

    let mut attributes = vec![attr("action", "claim-ownership")];

    // validate that sender is pending owner
    if let Some(pending_owner) = pending_owner {
        if info.sender != pending_owner {
            return Err(ContractError::Unauthorized {});
        }

        // save new owner to storage and remove pending owner
        PENDING_OWNER.save(deps.storage, &None)?;
        OWNER.save(deps.storage, &pending_owner)?;
        attributes.push(attr("owner", pending_owner.to_string()));
    }

    Ok(Response::new().add_attributes(attributes))
}

/// Executes the `UpdateSlippage` message.
pub fn execute_update_slippage(
    deps: DepsMut,
    info: MessageInfo,
    max_slippage_percent: Decimal,
) -> Result<Response, ContractError> {
    // validate that sender is owner
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    // validate slippage and save to storage
    validate_slippage_percent(max_slippage_percent)?;
    MAX_SLIPPAGE_PERCENT.save(deps.storage, &max_slippage_percent)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update-slippage"),
        attr("max_slippage_percent", max_slippage_percent.to_string()),
    ]))
}

/// Executes the `UpdateFee` message.
pub fn execute_update_fee(
    deps: DepsMut,
    info: MessageInfo,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Result<Response, ContractError> {
    // validate that sender is owner
    let owner = OWNER.load(deps.storage)?;
    if info.sender != owner {
        return Err(ContractError::Unauthorized {});
    }

    validate_fee_percent(lp_fee_percent)?;
    validate_fee_percent(protocol_fee_percent)?;

    // validate that total fee percent is less than max fee percent
    let total_fee_percent = lp_fee_percent + protocol_fee_percent;
    let max_fee_percent = Decimal::from_str(PREDEFINED_MAX_FEES_PERCENT)?;
    if total_fee_percent > max_fee_percent {
        return Err(ContractError::FeesTooHigh {
            max_fee_percent,
            total_fee_percent,
        });
    }

    // update fees and save to storage
    let protocol_fee_recipient = deps.api.addr_validate(&protocol_fee_recipient)?;
    let updated_fees = Fees {
        protocol_fee_recipient: protocol_fee_recipient.clone(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    FEES.save(deps.storage, &updated_fees)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "update-fee"),
        attr("lp_fee_percent", lp_fee_percent.to_string()),
        attr("protocol_fee_percent", protocol_fee_percent.to_string()),
        attr("protocol_fee_recipient", protocol_fee_recipient.to_string()),
    ]))
}

/// Executes the `RemoveLiquidity` message.
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
    // get users current liquidity tokens balance
    let balance = get_token_balance(deps.as_ref(), &lp_token_addr, &info.sender)?;
    let lp_token_supply = get_lp_token_supply(deps.as_ref(), &lp_token_addr)?;
    let token1155 = TOKEN1155.load(deps.storage)?;
    let token2 = TOKEN2.load(deps.storage)?;

    // if amount user wants to remove is more than users balance, error
    if amount > balance {
        return Err(ContractError::InsufficientLiquidityError {
            requested: amount,
            available: balance,
        });
    }

    let min_token1155_total_amount = min_token1155.get_total();
    validate_min_token(min_token1155_total_amount)?;
    validate_min_token(min_token2)?;

    // calculate 1155 amount user will get: amount * token1155_reserve / lp_token_supply
    let token1155_amount = amount
        .checked_mul(token1155.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;

    // calculate token2 amount user will get: amount * token2_reserve / lp_token_supply
    let token2_amount = amount
        .checked_mul(token2.reserve)
        .map_err(StdError::overflow)?
        .checked_div(lp_token_supply)
        .map_err(StdError::divide_by_zero)?;

    validate_slippage(&deps, min_token1155_total_amount, token1155_amount)?;
    validate_slippage(&deps, min_token2, token2_amount)?;

    // checks that output tokens is more than users minimum defined
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

    // update token reserves by subtracting input amounts
    let updated_token1155 = TOKEN1155.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(token1155_amount)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;
    let updated_token2 = TOKEN2.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(token2_amount)
            .map_err(StdError::overflow)?;
        Ok(token)
    })?;

    // get the 1155 tokens to transfer, and update the TOKEN_SUPPLIES by subtracting all the tokens from the supply
    let token1155_amounts_to_transfer =
        get_token_amounts_to_transfer(deps.storage, token1155_amount, min_token1155)?;

    let mut msgs: Vec<CosmosMsg> = vec![];
    // add transfer message for 1155 tokens
    if let Denom::Cw1155(addr, _) = token1155.denom {
        msgs.push(get_cw1155_transfer_msg(
            &env.contract.address,
            &info.sender,
            &addr,
            &token1155_amounts_to_transfer,
        )?)
    };

    // add transfer message for token2
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

    // burn lp tokens from user
    msgs.push(get_burn_msg(&lp_token_addr, &info.sender, amount)?);

    Ok(Response::new().add_messages(msgs).add_attributes(vec![
        attr("action", "remove-liquidity"),
        attr("token1155_returned", token1155_amount),
        attr("token2_returned", token2_amount),
        attr("liquidity_burned", amount),
        attr("liquidity_provider", info.sender.to_string()),
        attr("token1155_reserve", updated_token1155.reserve),
        attr("token2_reserve", updated_token2.reserve),
    ]))
}

/// Gets a map of 1155 tokens to transfer from TOKEN_SUPPLIES based on the token1155_amount to transfer by:
/// - if min_token1155 is single then it gets any random tokens from the TOKEN_SUPPLIES till amount is reached
/// - if min_token1155 is multiple then it:
///    - tries to get the min amount per token in the min_token1155, if there isn't then throw error
///    - if the min_token1155 didnt fill the needed token1155_amount, then loop through the min_token1155 ids
///      and get any remaining tokens left in supply for tokens ids, so to first fill the output tokens
///      with same token ids user defined in min_token1155
///    - lastly if ther eis still remaining amount to be filled for transfer, then loop through the
///      TOKEN_SUPPLIES till get the amount of tokens wanted
///
/// NOTE: this assumes that token1155_amount is >= to the total of the min_token1155 amount
/// Please ensure this assumtion is kept by validations before calling this function
fn get_token_amounts_to_transfer(
    storage: &mut dyn Storage,
    token1155_amount: Uint128,
    min_token1155: TokenAmount,
) -> Result<HashMap<TokenId, Uint128>, ContractError> {
    let mut token1155_amount_left_to_transfer = token1155_amount;
    let mut token1155_amounts_to_transfer: HashMap<TokenId, Uint128> = HashMap::new();

    match min_token1155 {
        TokenAmount::Multiple(amounts) => {
            // local cache map of tokens that has remaining supply after the min_token1155 is subtracted
            let mut token1155_supplies: HashMap<TokenId, Uint128> = HashMap::new();

            // map over min_token1155 and per token:
            // - check if the token supply is less than the amount, if so return error
            // - subtract the amount from the token1155_amount_left_to_transfer
            // - update the TOKEN_SUPPLIES and token1155_supplies with the remaining supply for the specific token
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

                update_token_supplies(
                    storage,
                    token_supply - token_amount,
                    token_id,
                    Some(&mut token1155_supplies),
                )?;
            }

            // if there is still has an amount left to transfer, then first loop through the token1155_supplies local
            // cache map, so that first try and fill remaining amount with same tokens as min_token1155
            if !token1155_amount_left_to_transfer.is_zero() {
                let mut sorted_supplies: Vec<(TokenId, Uint128)> = token1155_supplies.into_iter().collect();
                // Sort by amount (ascending), then by token_id for deterministic order
                sorted_supplies.sort_by(|a, b| {
                    if a.1 == b.1 {
                        a.0.cmp(&b.0)
                    } else {
                        a.1.cmp(&b.1)
                    }
                });

                for (token_id, token_supply) in sorted_supplies.into_iter() {
                    if token1155_amount_left_to_transfer.is_zero() {
                        break;
                    }

                    let take_amount = if token_supply >= token1155_amount_left_to_transfer {
                        token1155_amount_left_to_transfer
                    } else {
                        token_supply
                    };

                    token1155_amount_left_to_transfer -= take_amount;
                    *token1155_amounts_to_transfer.entry(token_id.clone()).or_insert(Uint128::zero()) += take_amount;

                    update_token_supplies(
                        storage,
                        token_supply - take_amount,
                        token_id,
                        None,
                    )?;
                }
            }

            // lastly while there is still an amount left to transfer, we run the process_token_supplies_in_chunks to get
            // the amounts from any tokens in the TOKEN_SUPPLIES that are left to transfer
            let res = process_token_supplies_in_chunks(
                storage,
                &mut token1155_amounts_to_transfer,
                &mut token1155_amount_left_to_transfer,
                token1155_amount,
            );
            if let Err(err) = res {
                return Err(err);
            }
        }
        // runs the process_token_supplies_in_chunks to get the amounts from any tokens in the TOKEN_SUPPLIES to transfer
        TokenAmount::Single(_) => {
            let res = process_token_supplies_in_chunks(
                storage,
                &mut token1155_amounts_to_transfer,
                &mut token1155_amount_left_to_transfer,
                token1155_amount,
            );
            if let Err(err) = res {
                return Err(err);
            }
        }
    };

    Ok(token1155_amounts_to_transfer.clone())
}

/// Gets the tokens to transfer by processes the TOKEN_SUPPLIES in chunks of 100 at a time, and updates the
/// mutable token1155_amounts_to_transfer and mutable token1155_amount_left_to_transfer passed by runnig the
/// update_token_amounts per token. see update_token_amounts for more details.
/// Batching of 100 is done to not load the whole TOKEN_SUPPLIES into memory
fn process_token_supplies_in_chunks(
    storage: &mut dyn Storage,
    token1155_amounts_to_transfer: &mut HashMap<TokenId, Uint128>,
    token1155_amount_left_to_transfer: &mut Uint128,
    original_token1155_amount: Uint128,
) -> Result<(), ContractError> {
    while !token1155_amount_left_to_transfer.is_zero() {
        let token_supplies = TOKEN_SUPPLIES
            .range(storage, None, None, Order::Ascending)
            .take(100) // Process in chunks of 100 entries
            .collect::<StdResult<Vec<_>>>()?;

        // this should never happen, but just in case
        if token_supplies.is_empty() {
            return Err(ContractError::InsufficientTokenSupply {
                requested: original_token1155_amount,
                available: original_token1155_amount - *token1155_amount_left_to_transfer,
            });
        }

        for (token_id, token_amount) in token_supplies.into_iter() {
            update_token_amounts(
                storage,
                token1155_amounts_to_transfer,
                token1155_amount_left_to_transfer,
                token_id.clone(),
                token_amount,
                None,
                None,
            )?;

            if token1155_amount_left_to_transfer.is_zero() {
                break;
            }
        }
    }

    Ok(())
}

/// Updates the mutable token_amounts_to_transfer and mutable token_amount_left_to_transfer passed by:
/// - getting the tokens current amount from the token_amounts_to_transfer, aka that is already in map to transfer
/// - if the passed token amount is less than the wanted amount, we:
///   - add the tokens remaining amount to the amount to transfer
///   - make remaining supply zero for the specific token
///   - subtract the taken amount from mutable token_amount_left_to_transfer for next loop iteration
/// - if the passed token amount is >= the wanted amount, we:
///   - add wanted amount to the amount to transfer
///   - subtract the amount we added from the remaining supply for the specific token
///   - if additional_token_amount_to_transfer is none, we set mutable token_amount_left_to_transfer to zero since
///     all the wanted token amount is accounted for then
///   - if additional_token_amount_to_transfer is some, we subtract the additional token amount to transfer from the mutable
/// - then it updates the mutable token_amounts_to_transfer with the new amount to transfer
/// - lastly it updates the TOKEN_SUPPLIES with the remaining supply for the specific token, as well as the mutable
///   token_supplies if provided, see function update_token_supplies for more details
fn update_token_amounts(
    storage: &mut dyn Storage,
    token_amounts_to_transfer: &mut HashMap<String, Uint128>,
    token_amount_left_to_transfer: &mut Uint128,
    token_id: String,
    token_amount: Uint128,
    additional_token_amount_to_transfer: Option<Uint128>,
    token_supplies: Option<&mut HashMap<String, Uint128>>,
) -> StdResult<()> {
    // get the current tokens amount that is already in the token_amounts_to_transfer
    let mut amount_to_transfer = *token_amounts_to_transfer
        .get(&token_id)
        .unwrap_or(&Uint128::zero());

    // token_supply is the amount of tokens that we ideally want to transfer and add to token_amounts_to_transfer
    let mut token_supply = *token_amount_left_to_transfer;
    if let Some(token_amount) = additional_token_amount_to_transfer {
        token_supply = token_amount;
    }

    let remaining_supply;
    // if token amount is >= the amount we want, we:
    // - add wanted amount to the amount to transfer
    // - subtract the amount we want from the remaining supply for the specific token
    // - if additional_token_amount_to_transfer is none, we set mutable token_amount_left_to_transfer to zero since this
    //   if took the amount needed, otherwise we subtract the additional token amount to transfer from the mutable
    if token_amount >= token_supply {
        amount_to_transfer += token_supply;
        remaining_supply = token_amount - token_supply;

        if let Some(token_amount) = additional_token_amount_to_transfer {
            *token_amount_left_to_transfer -= token_amount;
        } else {
            *token_amount_left_to_transfer = Uint128::zero();
        }
    } else {
      // if token amount is < the amount we want, we:
      // - add all the tokens remaining amount to the amount to transfer
      // - make remaining supply zero for the specific token
      // - subtract the taken amount from mutable token_amount_left_to_transfer for next loop iteration
        amount_to_transfer += token_amount;
        remaining_supply = Uint128::zero();
        *token_amount_left_to_transfer -= token_amount;
    }

    token_amounts_to_transfer.insert(token_id.clone(), amount_to_transfer);
    update_token_supplies(storage, remaining_supply, token_id.clone(), token_supplies)
}

/// Updates the TOKEN_SUPPLIES, and mutable token_supplies(if it is provided) passed by:
/// - if the remaining supply is zero, we remove the token id from the TOKEN_SUPPLIES and token_supplies
/// - otherwise we update the supply for the token id in the TOKEN_SUPPLIES and token_supplies
///
/// NOTE: the token_supplies is mutable so the passed in token_supplies will be updated if provided
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

/// Creates a burn message for the given amount of lp token from the given owner
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

/// Creates a cw20 transfer message for the given token amount, from contract to recipient
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

/// Creates a bank transfer message for the given amount for a native denom, will always be from contract to recipient
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

/// Creates a transfer message for the given amount and denom, from sender to recipient
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

/// Calculates the amount of tokens the user bought by:.
/// - if either reserve is zero throws error
/// - create fee percent with SCALE_FACTOR and calculate input_amount_with_fee
/// - calculate numerator: input_amount_with_fee * output_reserve
/// - calculate denominator: input_reserve * SCALE_FACTOR + input_amount_with_fee
/// - calculate amount bought: numerator / denominator
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

/// Calculates the amount of tokens the reserves get, aka input amount minus the protocol fees, by:
/// - if fee amount is none then return input amount
/// - if fee amount is some then return input amount - fee amount
///
/// For single token input amount, it is input amount - fee amount
/// For multiple token input amount, it is input amount - fee amount if it exists in the fee amount per token
fn get_amount_without_fee(
    input_amount: &TokenAmount,
    fee_amount: Option<TokenAmount>,
) -> Result<TokenAmount, ContractError> {
    if let Some(fee_amount) = fee_amount {
        match input_amount.clone() {
            TokenAmount::Multiple(mut input_amounts) => {
                let fee_amounts = fee_amount.get_multiple()?;

                let zero = Uint128::zero();
                for (token_id, token_amount) in input_amounts.iter_mut() {
                    // it is possible that there are some tokens that is not in the fee_amount, due to the way it is calculated
                    // refer to get_percent_from_multiple for more details
                    let fee_amount = fee_amounts.get(token_id).unwrap_or(&zero);

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

// Executes the `Swap` message.
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

    // map tokens to type and load from storage
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

    // can do early validation, if protocol fee is not 0 and input == 1, then can throw error already, since
    // protocol fees are rounded up so it will be minimum 1
    if fees.protocol_fee_percent > Decimal::zero() && input_amount_total == Uint128::one() {
        return Err(ContractError::MinInputTokenAmountError {
            input_token_amount: input_amount_total,
            min_allowed: Uint128::from_str("2")?,
        });
    }

    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    // get the calculated amount of tokens bought
    let token_bought = get_input_price(
        input_amount_total,
        input_token.reserve,
        output_token.reserve,
        total_fee_percent,
    )?;

    validate_slippage(&deps, min_token_total, token_bought)?;

    // check that token_bought is more than min_token_total provided by user
    if min_token_total > token_bought {
        return Err(ContractError::SwapMinError {
            min: min_token_total,
            available: token_bought,
        });
    }

    let protocol_fee_amount = input_amount.get_percent(fees.protocol_fee_percent)?;
    let input_amount_without_protocol_fee =
        get_amount_without_fee(&input_amount, protocol_fee_amount.clone())?;

    let mut msgs = vec![];
    // switch on input token denom and add transfer message from user to contract
    // no need for native transfer as info.funds is same as input amount and will be transfered to contract
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

    let mut protocol_fee_amount_total = Uint128::zero();
    // Send protocol fee to protocol fee recipient
    if let Some(protocol_fee_amount) = protocol_fee_amount {
        protocol_fee_amount_total = protocol_fee_amount.get_total();
        msgs.push(get_fee_transfer_msg(
            &info.sender,
            &fees.protocol_fee_recipient,
            &input_token.denom,
            protocol_fee_amount,
        )?)
    }

    let recipient = deps.api.addr_validate(&recipient)?;
    // switch on output token denom and add transfer message from contract to recipient(user)
    msgs.push(match output_token.denom {
        Denom::Cw1155(addr, _) => {
            let tokens_to_transfer =
                get_token_amounts_to_transfer(deps.storage, token_bought, min_token)?;

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

    // update input token reserve adding input amount without protocol fee
    let updated_input_token = input_token_item.update(
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

    // update output token reserve by subtracting token_bought
    let updated_output_token = output_token_item.update(
        deps.storage,
        |mut output_token| -> Result<_, ContractError> {
            output_token.reserve = output_token
                .reserve
                .checked_sub(token_bought)
                .map_err(StdError::overflow)?;
            Ok(output_token)
        },
    )?;

    // update lp token supplies by adding input amount if it multiple as it is 1155 tokens then and need to keep track of id and amount
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

    // Attributes for response
    let mut attributes = vec![
        attr("action", "swap"),
        attr("sender", info.sender.to_string()),
        attr("recipient", recipient.to_string()),
        attr("input_token_enum", input_token_enum.to_string()),
        attr("input_token_amount", input_amount_total),
        attr("output_token_amount", token_bought),
        attr("protocol_fee_amount", protocol_fee_amount_total),
    ];

    // Add updated reserves based on the token type
    match input_token_enum {
        TokenSelect::Token1155 => {
            attributes.push(attr("token1155_reserve", updated_input_token.reserve));
            attributes.push(attr("token2_reserve", updated_output_token.reserve));
        }
        TokenSelect::Token2 => {
            attributes.push(attr("token1155_reserve", updated_output_token.reserve));
            attributes.push(attr("token2_reserve", updated_input_token.reserve));
        }
    }


    Ok(Response::new().add_messages(msgs).add_attributes(attributes))
}

// Executes the `PassThroughSwap` message.
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

    // map tokens to type and load from storage
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

    let input_amount_total = input_token_amount.get_total();
    let fees = FEES.load(deps.storage)?;

    // can do early validation, if protocol fee is not 0 and input == 1, then can throw error already, since
    // protocol fees are rounded up so it will be minimum 1
    if fees.protocol_fee_percent > Decimal::zero() && input_amount_total == Uint128::one() {
        return Err(ContractError::MinInputTokenAmountError {
            input_token_amount: input_amount_total,
            min_allowed: Uint128::from_str("2")?,
        });
    }


    let total_fee_percent = fees.lp_fee_percent + fees.protocol_fee_percent;
    // get the calculated amount of tokens bought
    let amount_to_transfer = get_input_price(
        input_amount_total,
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

    let mut protocol_fee_amount_total = Uint128::zero();
    // Send protocol fee to protocol fee recipient
    if let Some(protocol_fee_amount) = protocol_fee_amount {
        protocol_fee_amount_total = protocol_fee_amount.get_total();
        msgs.push(get_fee_transfer_msg(
            &info.sender,
            &fees.protocol_fee_recipient,
            &input_token.denom,
            protocol_fee_amount,
        )?)
    }

    let output_amm_address = deps.api.addr_validate(&output_amm_address)?;

    // Increase allowance of output contract if transfer token is cw20
    if let Denom::Cw20(addr) = &transfer_token.denom {
        msgs.push(get_cw20_increase_allowance_msg(
            addr,
            &output_amm_address,
            amount_to_transfer,
            Some(Expiration::AtHeight(env.block.height + 1)),
        )?)
    };

    // query info of output amm
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

    // create swap and send to message
    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token: transfer_input_token_enum,
        input_amount: TokenAmount::Single(amount_to_transfer),
        recipient: info.sender.to_string(),
        min_token: output_min_token,
        expiration,
    };

    let output_amm_address_clone = output_amm_address.clone();
    msgs.push(
        WasmMsg::Execute {
            contract_addr: output_amm_address_clone.into(),
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

    // update input token reserve by adding input amount without protocol fee
    let updated_input_token = input_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        // Add input amount - protocol fee to input token reserve
        let input_amount_without_protocol_fee_total = input_amount_without_protocol_fee.get_total();
        token.reserve = token
            .reserve
            .checked_add(input_amount_without_protocol_fee_total)
            .map_err(StdError::overflow)?;

        Ok(token)
    })?;

    // update output token reserve by subtracting amount to transfer
    let updated_transfer_token = transfer_token_state.update(deps.storage, |mut token| -> Result<_, ContractError> {
        token.reserve = token
            .reserve
            .checked_sub(amount_to_transfer)
            .map_err(StdError::overflow)?;

        Ok(token)
    })?;

    // update lp token supplies by adding input amount if it multiple as it is 1155 tokens then and need to keep track of id and amount
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

    // Attributes for response
    let mut attributes = vec![
        attr("action", "cross-contract-swap"),
        attr("input_token_enum", input_token_enum.to_string()),
        attr("input_token_amount", input_amount_total),
        attr("output_token_amount", amount_to_transfer),
        attr("output_amm_address", output_amm_address.to_string()),
        attr("recipient", info.sender.to_string()),
        attr("protocol_fee_amount", protocol_fee_amount_total),
    ];

    // Add updated reserves based on the token type
    match input_token_enum {
        TokenSelect::Token1155 => {
            attributes.push(attr("token1155_reserve", updated_input_token.reserve));
            attributes.push(attr("token2_reserve", updated_transfer_token.reserve));
        }
        TokenSelect::Token2 => {
            attributes.push(attr("token1155_reserve", updated_transfer_token.reserve));
            attributes.push(attr("token2_reserve", updated_input_token.reserve));
        }
    }

    Ok(Response::new().add_messages(msgs).add_attributes(attributes))
}

// Queries for the contract state.
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

// Queries for token module token metadata.
pub fn query_token_metadata(deps: Deps, id: String) -> StdResult<QueryTokenMetadataResponse> {
    deps.querier.query(&QueryRequest::Stargate {
        path: "/ixo.token.v1beta1.Query/TokenMetadata".to_string(),
        data: Binary::from(QueryTokenMetadataRequest { id }.encode_to_vec()),
    })
}

// Queries for bank module denom metadata.
pub fn query_denom_metadata(deps: Deps, denom: String) -> StdResult<QueryDenomMetadataResponse> {
    deps.querier.query(&QueryRequest::Stargate {
        path: "/cosmos.bank.v1beta1.Query/DenomMetadata".to_string(),
        data: Binary::from(QueryDenomMetadataRequest { denom }.encode_to_vec()),
    })
}

/// Queries the total supply of lp tokens for the given tokens id
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

/// Queries the MAX slippage percent
pub fn query_slippage(deps: Deps) -> StdResult<SlippageResponse> {
    let max_slippage_percent = MAX_SLIPPAGE_PERCENT.load(deps.storage)?;

    Ok(SlippageResponse {
        max_slippage_percent,
    })
}

/// Queries the info of the contract, includes token reserves/denoms and lp supply/token address
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

/// Queries the ownership of the contract, includes owner and pending owner
pub fn query_ownership(deps: Deps) -> StdResult<OwnershipResponse> {
    let owner = OWNER.load(deps.storage)?.to_string();
    let pending_owner = PENDING_OWNER.load(deps.storage)?.map(|o| o.into_string());

    Ok(OwnershipResponse {
        owner,
        pending_owner,
    })
}

/// Queries the freeze status of the contract
pub fn query_freeze_status(deps: Deps) -> StdResult<FreezeStatusResponse> {
    let freeze_status = FROZEN.load(deps.storage)?;

    Ok(FreezeStatusResponse {
        status: freeze_status,
    })
}

/// Queries the price in token2 for the wanted token1155 amount
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

/// Queries the price in token1155 for the wanted token2 amount
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

/// Queries the fees of the contract, includes lp fee percent, protocol fee percent and protocol fee recipient
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

            Ok(Response::new().add_attributes(vec![
                attr("action", "instantiate-lp-token"),
                attr("liquidity_pool_token_address", cw20_addr),
            ]))
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

        // let token_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        // let mut token_amounts = HashMap::new();
        let mut token_supplies = HashMap::new();

        let mut tokens = HashMap::new();
        tokens.insert("1".to_string(), Uint128::new(500));
        tokens.insert("2".to_string(), Uint128::new(500));
        tokens.insert("3".to_string(), Uint128::new(500));

        let mut min_token_amounts = HashMap::new();
        min_token_amounts.insert("1".to_string(), Uint128::new(100));

        // map through tokens and add to token_supplies
        for (token_id, token_amount) in tokens.clone().into_iter() {
            // token_amounts.insert(token_id.clone(), Uint128::new(100));
            token_supplies.insert(token_id.clone(), token_amount);
            TOKEN_SUPPLIES
                .save(&mut deps.storage, token_id.clone(), &token_amount)
                .unwrap();
        }

        let token_amounts_to_transfer = get_token_amounts_to_transfer(
            &mut deps.storage,
            Uint128::new(501),
            TokenAmount::Multiple(min_token_amounts.clone()),
        )
        .unwrap();

        assert_token_supplies(
            &mut deps.storage,
            &token_amounts_to_transfer,
            &token_supplies,
        );
        assert_eq!(
            TokenAmount::Multiple(token_amounts_to_transfer).get_total(),
            Uint128::new(501)
        )
    }

    #[test]
    fn should_fail_token_amounts_to_transfer_is_less_than_supply() {
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
        .unwrap_err();

        assert_eq!(
            token_amounts_to_transfer,
            ContractError::InsufficientTokenSupply {
                requested: Uint128::new(1_000),
                available: Uint128::new(600)
            }
        );
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
            .save(&mut deps.storage, &Decimal::from_str("2").unwrap())
            .unwrap();

        let min_token_amount = Uint128::new(95_000);
        let actual_token_amount = Uint128::new(110_000);
        let err =
            validate_slippage(&deps.as_mut(), min_token_amount, actual_token_amount).unwrap_err();

        assert_eq!(
            err,
            ContractError::MinTokenAmountError {
                min_token: min_token_amount,
                min_required: Uint128::new(107_800) // 110_000 * 0.98 (2% slippage) = 107_800
            }
        );
    }

    #[test]
    fn should_pass_slippage_validation_when_minimum_amount_greater_then_minimum_required() {
        let mut deps = mock_dependencies();

        MAX_SLIPPAGE_PERCENT
            .save(&mut deps.storage, &Decimal::from_str("0.1").unwrap())
            .unwrap();

        let min_token_amount = Uint128::new(109_900); // 110_000 * 0.999 (0.1% slippage) = 109_890
        let actual_token_amount = Uint128::new(110_000);
        let res = validate_slippage(&deps.as_mut(), min_token_amount, actual_token_amount).unwrap();

        assert_eq!(res, ());
    }

    #[test]
    fn should_fail_slippage_percent_validation_when_slippage_too_high() {
        let percent = Decimal::from_str("10.1").unwrap();
        let err = validate_slippage_percent(percent).unwrap_err();
        assert_eq!(
            err,
            ContractError::InvalidPercent {
                percent,
                max: Decimal::from_str(PREDEFINED_MAX_SLIPPAGE_PERCENT).unwrap()
            }
        );
    }

    #[test]
    fn should_fail_slippage_percent_validation_when_percent_is_0() {
        let percent = Decimal::from_str("0").unwrap();
        let err = validate_slippage_percent(percent).unwrap_err();
        assert_eq!(
            err,
            ContractError::InvalidPercent {
                percent,
                max: Decimal::from_str(PREDEFINED_MAX_SLIPPAGE_PERCENT).unwrap()
            }
        );
    }

    #[test]
    fn should_pass_slippage_percent_validation_when_percent_in_allowed_range() {
        let res = validate_slippage_percent(Decimal::from_str("0.5").unwrap()).unwrap();
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
