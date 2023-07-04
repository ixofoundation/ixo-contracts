use cosmwasm_std::{Addr, Deps, Env, Order, StdResult};
use cw1155_lp::{
    ApprovedForAllResponse, BalanceResponse, BatchBalanceForAllResponse, BatchBalanceResponse,
    IsApprovedForAllResponse, TokenInfo, TokenInfoResponse, TokensResponse,
};
use cw_storage_plus::Bound;
use cw_utils::{maybe_addr, Expiration};

use crate::{
    helpers::check_can_approve,
    state::{APPROVES, BALANCES, SUPPLIES, TOKENS},
};

pub const DEFAULT_LIMIT: u32 = 10;
pub const MAX_LIMIT: u32 = 30;

pub fn balance(deps: Deps, owner: String, token_id: String) -> StdResult<BalanceResponse> {
    let owner = deps.api.addr_validate(&owner)?;

    let balance = BALANCES
        .may_load(deps.storage, (&owner, &token_id))?
        .unwrap_or_default();

    Ok(BalanceResponse { balance })
}

pub fn batch_balance(
    deps: Deps,
    owner: String,
    token_ids: Vec<String>,
) -> StdResult<BatchBalanceResponse> {
    let owner = deps.api.addr_validate(&owner)?;

    let balances = token_ids
        .into_iter()
        .map(|token_id| -> StdResult<_> {
            Ok(BALANCES
                .may_load(deps.storage, (&owner, &token_id))?
                .unwrap_or_default())
        })
        .collect::<StdResult<_>>()?;

    Ok(BatchBalanceResponse { balances })
}

pub fn batch_balance_for_tokens(
    deps: Deps,
    token_ids: Vec<String>,
) -> StdResult<BatchBalanceResponse> {
    let balances = token_ids
        .into_iter()
        .map(|token_id| -> StdResult<_> {
            Ok(SUPPLIES
                .may_load(deps.storage, &token_id)?
                .unwrap_or_default())
        })
        .collect::<StdResult<_>>()?;

    Ok(BatchBalanceResponse { balances })
}

pub fn batch_balance_for_all(deps: Deps) -> StdResult<BatchBalanceForAllResponse> {
    let balances = SUPPLIES
        .range(deps.storage, None, None, Order::Ascending)
        .map(|supply| {
            supply.map(|(id, amount)| {
                let uri = TOKENS.load(deps.storage, &id).unwrap();

                TokenInfo {
                    id: Some(id),
                    amount,
                    uri: Some(uri),
                }
            })
        })
        .collect::<StdResult<_>>()?;

    Ok(BatchBalanceForAllResponse { balances })
}

fn build_approval(item: StdResult<(Addr, Expiration)>) -> StdResult<cw1155_lp::Approval> {
    item.map(|(addr, expires)| cw1155_lp::Approval {
        spender: addr.into(),
        expires,
    })
}

pub fn approved_for_all(
    deps: Deps,
    env: Env,
    owner: String,
    include_expired: bool,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<ApprovedForAllResponse> {
    let owner = deps.api.addr_validate(&owner)?;
    let start_after = maybe_addr(deps.api, start_after)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(Bound::exclusive);

    let operators = APPROVES
        .prefix(&owner)
        .range(deps.storage, start, None, Order::Ascending)
        .filter(|r| include_expired || r.is_err() || !r.as_ref().unwrap().1.is_expired(&env.block))
        .take(limit)
        .map(build_approval)
        .collect::<StdResult<_>>()?;

    Ok(ApprovedForAllResponse { operators })
}

pub fn token_info(deps: Deps, token_id: String) -> StdResult<TokenInfoResponse> {
    let url = TOKENS.load(deps.storage, &token_id)?;

    Ok(TokenInfoResponse { url })
}

pub fn is_approved_for_all(
    deps: Deps,
    env: Env,
    owner: String,
    operator: String,
) -> StdResult<IsApprovedForAllResponse> {
    let owner_addr = deps.api.addr_validate(&owner)?;
    let operator_addr = deps.api.addr_validate(&operator)?;

    let approved = check_can_approve(deps, &env, &owner_addr, &operator_addr)?;

    Ok(IsApprovedForAllResponse { approved })
}

pub fn tokens(
    deps: Deps,
    owner: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let owner = deps.api.addr_validate(&owner)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(|s| Bound::exclusive(s.as_str()));

    let tokens = BALANCES
        .prefix(&owner)
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<StdResult<_>>()?;

    Ok(TokensResponse { tokens })
}

pub fn all_tokens(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<TokensResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.as_ref().map(|s| Bound::exclusive(s.as_str()));

    let tokens = TOKENS
        .keys(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<StdResult<_>>()?;

    Ok(TokensResponse { tokens })
}
