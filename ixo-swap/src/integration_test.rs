#![cfg(test)]

use std::borrow::BorrowMut;

use cosmwasm_std::{coins, to_binary, Addr, Coin, CosmosMsg, Decimal, Empty, Uint128, WasmMsg};
use cw1155::{BatchBalanceResponse, Cw1155ExecuteMsg, Cw1155QueryMsg, TokenId};

use crate::{error::ContractError, msg::MigrateMsg};
use cw1155_lp::TokenInfo;
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg, Expiration};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use std::str::FromStr;

use crate::msg::{
    Denom, ExecuteMsg, FeeResponse, InfoResponse, InstantiateMsg, QueryMsg, TokenSelect,
};

fn mock_app() -> App {
    App::default()
}

pub fn contract_amm() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply)
    .with_migrate(crate::contract::migrate);
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_cw1155() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw1155_base::contract::execute,
        cw1155_base::contract::instantiate,
        cw1155_base::contract::query,
    );
    Box::new(contract)
}

pub fn contract_cw1155_lp() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw1155_base_lp::contract::execute,
        cw1155_base_lp::contract::instantiate,
        cw1155_base_lp::contract::query,
    );
    Box::new(contract)
}

fn get_info(router: &App, contract_addr: &Addr) -> InfoResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Info {})
        .unwrap()
}

fn get_fee(router: &App, contract_addr: &Addr) -> FeeResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Fee {})
        .unwrap()
}

fn create_amm(
    router: &mut App,
    owner: &Addr,
    token1_denom: Denom,
    token2_denom: Denom,
    lp_token: Option<TokenSelect>,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Addr {
    // set up amm contract
    let lp_token_code_id = if let Some(lp_token) = lp_token.clone() {
        match lp_token {
            TokenSelect::Token1 => {
                if let Denom::Cw20(_) | Denom::Native(_) = token1_denom {
                    router.store_code(contract_cw20())
                } else {
                    router.store_code(contract_cw1155_lp())
                }
            }
            TokenSelect::Token2 => {
                if let Denom::Cw20(_) | Denom::Native(_) = token2_denom {
                    router.store_code(contract_cw20())
                } else {
                    router.store_code(contract_cw1155_lp())
                }
            }
        }
    } else {
        router.store_code(contract_cw20())
    };

    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1_denom,
        token2_denom,
        lp_token,
        lp_token_code_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient,
    };

    router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap()
}

// CreateCW155 create new cw155
fn create_cw1155(router: &mut App, owner: &Addr) -> Addr {
    let cw1155_id = router.store_code(contract_cw1155());
    let msg = cw1155_base::msg::InstantiateMsg {
        minter: owner.into(),
    };

    router
        .instantiate_contract(cw1155_id, owner.clone(), &msg, &[], "CASH", None)
        .unwrap()
}

// CreateCW20 create new cw20 with given initial balance belonging to owner
fn create_cw20(
    router: &mut App,
    owner: &Addr,
    name: String,
    symbol: String,
    balance: Uint128,
) -> Cw20Contract {
    // set up cw20 contract with some tokens
    let cw20_id = router.store_code(contract_cw20());
    let msg = cw20_base::msg::InstantiateMsg {
        name,
        symbol,
        decimals: 6,
        initial_balances: vec![Cw20Coin {
            address: owner.to_string(),
            amount: balance,
        }],
        mint: None,
        marketing: None,
    };
    let addr = router
        .instantiate_contract(cw20_id, owner.clone(), &msg, &[], "CASH", None)
        .unwrap();
    Cw20Contract(addr)
}

fn bank_balance(router: &mut App, addr: &Addr, denom: String) -> Coin {
    router
        .wrap()
        .query_balance(addr.to_string(), denom)
        .unwrap()
}

#[test]
// receive cw20 tokens and release upon approval
fn test_instantiate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw20_token.addr(), amm_addr);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.lp_token_address, "contract2".to_string());

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());
    assert_eq!(fee.owner.unwrap(), owner.to_string());

    // Test instantiation with invalid fee amount
    let lp_fee_percent = Decimal::from_str("1.01").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Cw20(cw20_token.addr()),
        lp_token: Some(TokenSelect::Token2),
        lp_token_code_id: cw20_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        ContractError::FeesTooHigh {
            max_fee_percent: Decimal::from_str("1").unwrap(),
            total_fee_percent: Decimal::from_str("1.01").unwrap()
        },
        err
    );
}

#[test]
fn amm_add_liquidity_cw1155() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let token_ids = vec![TokenId::from("1"), TokenId::from("2"), TokenId::from("3")];
    let token_uris = vec![
        String::from("uri1"),
        String::from("uri2"),
        String::from("uri3"),
    ];
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (
                token_ids[0].clone(),
                Uint128::new(10000),
                token_uris[0].clone(),
            ),
            (
                token_ids[1].clone(),
                Uint128::new(10000),
                token_uris[1].clone(),
            ),
            (
                token_ids[2].clone(),
                Uint128::new(10000),
                token_uris[2].clone(),
            ),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm_addr.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[1].clone()),
            },
        ],
        min_liquidities: vec![Uint128::new(100), Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    let owner_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(9900), Uint128::new(9900), Uint128::new(10000)]
    );
    let amm_balance = get_batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids);
    assert_eq!(
        amm_balance.balances,
        [Uint128::new(100), Uint128::new(100), Uint128::new(0)]
    );

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[2].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[2].clone()),
            },
        ],
        min_liquidities: vec![Uint128::new(100), Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(201),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(201),
            }],
        )
        .unwrap();

    let owner_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(9800), Uint128::new(9900), Uint128::new(9900)]
    );
    let amm_balance = get_batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids);
    assert_eq!(
        amm_balance.balances,
        [Uint128::new(200), Uint128::new(100), Uint128::new(100)]
    );

    let info = get_info(&router, &amm_addr);
    let crust_balance = get_batch_balance(&router, &info.lp_token_address, &token_ids);
    assert_eq!(
        crust_balance.balances,
        [Uint128::new(200), Uint128::new(100), Uint128::new(100)]
    );

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: Some(token_ids[2].clone()),
            amount: Uint128::new(100),
            uri: Some(token_uris[2].clone()),
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(302),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(302),
            }],
        )
        .unwrap();

    let owner_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(9800), Uint128::new(9900), Uint128::new(9800)]
    );
    let amm_balance = get_batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids);
    assert_eq!(
        amm_balance.balances,
        [Uint128::new(200), Uint128::new(100), Uint128::new(200)]
    );

    let info = get_info(&router, &amm_addr);
    let crust_balance = get_batch_balance(&router, &info.lp_token_address, &token_ids);
    assert_eq!(
        crust_balance.balances,
        [Uint128::new(200), Uint128::new(100), Uint128::new(200)]
    );
}

#[test]
fn cw1155_to_cw20_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");

    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token2".to_string(),
        "TOKENTWO".to_string(),
        Uint128::new(5000),
    );

    let token_ids = vec![TokenId::from("1"), TokenId::from("2")];
    let token_uris = vec![String::from("uri1"), String::from("uri2")];
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm1 = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );
    let amm2 = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (
                token_ids[0].clone(),
                Uint128::new(10000),
                token_uris[0].clone(),
            ),
            (
                token_ids[1].clone(),
                Uint128::new(10000),
                token_uris[1].clone(),
            ),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // Add initial liquidity to both pools
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm1.to_string(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(1000),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(1000),
                uri: Some(token_uris[1].clone()),
            },
        ],
        min_liquidities: vec![Uint128::new(1000), Uint128::new(1000)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(1000),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm1.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(1000),
            }],
        )
        .unwrap();

    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(1000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(1000),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(1000)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(1000),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm2.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(1000),
            }],
        )
        .unwrap();

    // Swap cw1155 for cw20
    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[1].clone()),
            },
        ],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(8),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        token1_balance.balances,
        vec![Uint128::new(8990), Uint128::new(8990)]
    );

    let token2_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4008));

    // Swap cw20 for cw1155
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(10),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm1.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[1].clone()),
            },
        ],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm2.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        token1_balance.balances,
        vec![Uint128::new(9000), Uint128::new(9000)]
    );

    let token2_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(3998));
}

fn get_batch_balance_for_owner(
    router: &App,
    contract: &Addr,
    owner: &Addr,
    token_ids: &Vec<String>,
) -> BatchBalanceResponse {
    let query_msg = Cw1155QueryMsg::BatchBalance {
        owner: owner.clone().into(),
        token_ids: token_ids.clone(),
    };

    router
        .wrap()
        .query_wasm_smart(contract, &query_msg)
        .unwrap()
}

fn get_batch_balance(
    router: &App,
    contract: &String,
    token_ids: &Vec<String>,
) -> BatchBalanceResponse {
    let query_msg = cw1155_lp::Cw1155QueryMsg::BatchBalanceForTokens {
        token_ids: token_ids.clone(),
    };

    router
        .wrap()
        .query_wasm_smart(contract, &query_msg)
        .unwrap()
}

#[test]
fn cw1155_to_native_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let token_ids = vec![TokenId::from("1"), TokenId::from("2")];
    let token_uris = vec![String::from("uri1"), String::from("uri2")];
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (
                token_ids[0].clone(),
                Uint128::new(5000),
                token_uris[0].clone(),
            ),
            (
                token_ids[1].clone(),
                Uint128::new(5000),
                token_uris[1].clone(),
            ),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        vec![Uint128::new(5000), Uint128::new(5000)]
    );

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm_addr.to_string(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[1].clone()),
            },
        ],
        min_liquidities: vec![Uint128::new(100), Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(200),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(200),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(100),
                uri: Some(token_uris[1].clone()),
            }
        ]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(200),
            uri: None,
        }]
    );

    // Swap cw1155 for native
    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(10),
                uri: Some(token_uris[1].clone()),
            },
        ],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(9),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &swap_msg, &[])
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(110),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(110),
                uri: Some(token_uris[1].clone()),
            }
        ]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(182),
            uri: None,
        }]
    );

    // ensure balances updated
    let buyer_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        buyer_balance.balances,
        vec![Uint128::new(4890), Uint128::new(4890)]
    );

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1818));

    // Swap native for cw1155
    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(16),
            uri: None,
        }],
        output_min_tokens: vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(8),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(8),
                uri: Some(token_uris[1].clone()),
            },
        ],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(16),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![
            TokenInfo {
                id: Some(token_ids[0].clone()),
                amount: Uint128::new(102),
                uri: Some(token_uris[0].clone()),
            },
            TokenInfo {
                id: Some(token_ids[1].clone()),
                amount: Uint128::new(102),
                uri: Some(token_uris[1].clone()),
            }
        ]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(198),
            uri: None,
        }]
    );

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1802));

    // check owner balance
    let owner_balance = get_batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        vec![Uint128::new(4898), Uint128::new(4898)]
    );
}

#[test]
// receive cw20 tokens and release upon approval
fn amm_add_and_remove_liquidity_cw20() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw20_token.addr(), amm_addr);

    let info = get_info(&router, &amm_addr);
    // set up cw20 helpers
    let lp_token = Cw20Contract(Addr::unchecked(info.lp_token_address));

    // check initial balances
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4900));
    let amm_balance = cw20_token
        .balance(&router.wrap(), amm_addr.clone())
        .unwrap();
    assert_eq!(amm_balance, Uint128::new(100));
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(100));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(50)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(51),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(51),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4850));
    let amm_balance = cw20_token
        .balance(&router.wrap(), amm_addr.clone())
        .unwrap();
    assert_eq!(amm_balance, Uint128::new(150));
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(150));

    // too low max token error
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(50)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(45),
            uri: None,
        }],
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(45),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MaxTokenError {
            max_token: Uint128::new(45),
            tokens_required: Uint128::new(51)
        },
        err.downcast().unwrap()
    );

    // too high min liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(500)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MinLiquidityError {
            min_liquidity: Uint128::new(500),
            liquidity_available: Uint128::new(50)
        },
        err.downcast().unwrap()
    );

    // Expired message
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(51u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(50)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(50),
            uri: None,
        }],
        expiration: Some(Expiration::AtHeight(0)),
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MsgExpirationError {},
        err.downcast().unwrap()
    );

    // Remove more liquidity then owned
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(50u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        input_amounts: vec![Uint128::new(151)],
        min_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        min_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::InsufficientLiquidityError {
            requested: Uint128::new(151),
            available: Uint128::new(150)
        },
        err.downcast().unwrap()
    );

    // Remove some liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(50u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        input_amounts: vec![Uint128::new(50)],
        min_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        min_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4900));
    let amm_balance = cw20_token
        .balance(&router.wrap(), amm_addr.clone())
        .unwrap();
    assert_eq!(amm_balance, Uint128::new(100));
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(100));

    // Remove rest of liquidity
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        input_amounts: vec![Uint128::new(100)],
        min_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        min_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(0),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr,
            &remove_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(50),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));
}

#[test]
fn migrate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const IBC_TOKEN_DENOM: &str = "atom";

    let amm_id = router.store_code(contract_amm());
    let lp_token_id = router.store_code(contract_cw20());
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let owner = Addr::unchecked("owner");

    let msg = InstantiateMsg {
        token1_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
        lp_token: None,
        lp_token_code_id: lp_token_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let amm_addr = router
        .instantiate_contract(
            amm_id,
            owner.clone(),
            &msg,
            &[],
            "amm",
            Some(owner.to_string()),
        )
        .unwrap();

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());

    let migrate_msg = MigrateMsg {
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
        freeze_pool: false,
    };

    router
        .execute(
            owner.clone(),
            CosmosMsg::Wasm(WasmMsg::Migrate {
                contract_addr: amm_addr.to_string(),
                new_code_id: amm_id,
                msg: to_binary(&migrate_msg).unwrap(),
            }),
        )
        .unwrap();

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());
    assert_eq!(fee.owner, Some(owner.to_string()));
}

#[test]
fn migrate_and_freeze_pool() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let funds = coins(100, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const IBC_TOKEN_DENOM: &str = "atom";

    let amm_id = router.store_code(contract_amm());
    let lp_token_id = router.store_code(contract_cw20());
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();

    let msg = InstantiateMsg {
        token1_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
        lp_token: None,
        lp_token_code_id: lp_token_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let amm_addr = router
        .instantiate_contract(
            amm_id,
            owner.clone(),
            &msg,
            &[],
            "amm",
            Some(owner.to_string()),
        )
        .unwrap();

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());

    let migrate_msg = MigrateMsg {
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
        freeze_pool: true,
    };

    router
        .execute(
            owner.clone(),
            CosmosMsg::Wasm(WasmMsg::Migrate {
                contract_addr: amm_addr.to_string(),
                new_code_id: amm_id,
                msg: to_binary(&migrate_msg).unwrap(),
            }),
        )
        .unwrap();

    let _ = get_fee(&router, &amm_addr);

    // now adding liquidity will fail
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner,
            amm_addr,
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap_err();
    assert_eq!(ContractError::FrozenPool {}, err.downcast().unwrap());
}

#[test]
fn swap_tokens_happy_path() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw20_token.addr(), amm_addr);

    // check initial balances
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &buyer, funds).unwrap()
    });

    // Swap native for cw20
    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(9),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(91),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(110),
            uri: None,
        }]
    );

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(9));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1990));

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(7),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(84),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(120),
            uri: None,
        }]
    );

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(16));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1980));

    // Swap cw20 for native

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(16),
        expires: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(16),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(19),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), amm_addr.clone(), &swap_msg, &[])
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(101),
            uri: None,
        }]
    );

    // ensure balances updated
    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(0));

    // Check balances of owner and buyer reflect the sale transaction
    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1999));

    // check owner balance
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4900));

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(3),
            uri: None,
        }],
        recipient: owner.to_string(),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(92),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(111),
            uri: None,
        }]
    );

    // ensure balances updated
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4908));

    // Check balances of owner and buyer reflect the sale transaction
    let balance = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1989));
}

#[test]
fn swap_with_fee_split() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let protocol_fee_recipient = Addr::unchecked("protocol_fee_recipient");
    let funds = coins(2_000_000_000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5_000_000_000),
    );

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient.to_string(),
    );

    assert_ne!(cw20_token.addr(), amm_addr);

    // check initial balances
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(5_000_000_000));

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(100_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100_000_000)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        expiration: None,
    };

    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100_000_000),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }]
    );

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2_000_000_000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &buyer, funds).unwrap()
    });

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10_000_000),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(9_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10_000_000),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(90_933_892),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(109_990_000),
            uri: None,
        }]
    );

    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(9_066_108));

    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1_990_000_000));

    let fee_recipient_balance: Coin = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_balance.amount, Uint128::new(10_000));

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10_000_000),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(7_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10_000_000),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(83_376_282),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(119_980_000),
            uri: None,
        }]
    );

    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(16_623_718));

    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1_980_000_000));

    let fee_recipient_balance: Coin = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_balance.amount, Uint128::new(20_000));

    // Swap token for native

    // send tokens to contract address
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(16_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(16_000_000),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(19_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(buyer.clone(), amm_addr.clone(), &swap_msg, &[])
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(99_360_282),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_711_360),
            uri: None,
        }]
    );

    let buyer_balance = cw20_token.balance(&router.wrap(), buyer.clone()).unwrap();
    assert_eq!(buyer_balance, Uint128::new(623718));

    let balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1_999_268_640));

    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4_900_000_000));

    let fee_recipient_balance = cw20_token
        .balance(&router.wrap(), protocol_fee_recipient.clone())
        .unwrap();
    assert_eq!(fee_recipient_balance, Uint128::new(16_000));

    let swap_msg = ExecuteMsg::SwapAndSendTo {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10_000_000),
            uri: None,
        }],
        recipient: owner.to_string(),
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(3_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10_000_000),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(90_410_067),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(110_701_360),
            uri: None,
        }]
    );

    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(4_908_950_215));

    let balance = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(balance.amount, Uint128::new(1_989_268_640));

    let fee_recipient_balance: Coin = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_balance.amount, Uint128::new(30_000));
}

#[test]
fn update_config() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(5000),
    );

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(cw20_token.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    let lp_fee_percent = Decimal::from_str("0.15").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.15").unwrap();
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some(owner.to_string()),
        protocol_fee_recipient: "new_fee_recpient".to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_recipient, "new_fee_recpient".to_string());
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.owner.unwrap(), owner.to_string());

    // Try updating config with fee values that are too high
    let lp_fee_percent = Decimal::from_str("1.01").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some(owner.to_string()),
        protocol_fee_recipient: "new_fee_recpient".to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        ContractError::FeesTooHigh {
            max_fee_percent: Decimal::from_str("1").unwrap(),
            total_fee_percent: Decimal::from_str("1.01").unwrap()
        },
        err
    );

    // Try updating config with invalid owner, show throw unauthoritzed error
    let lp_fee_percent = Decimal::from_str("0.21").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.09").unwrap();
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some(owner.to_string()),
        protocol_fee_recipient: owner.to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let err = router
        .execute_contract(
            Addr::unchecked("invalid_owner"),
            amm_addr.clone(),
            &msg,
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(ContractError::Unauthorized {}, err);

    // Try updating owner and fee params
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("new_owner".to_string()),
        protocol_fee_recipient: owner.to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.owner.unwrap(), "new_owner".to_string());
}

#[test]
fn swap_native_to_native_tokens_happy_path() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const IBC_TOKEN_DENOM: &str = "atom";

    let owner = Addr::unchecked("owner");
    let funds = vec![
        Coin {
            denom: NATIVE_TOKEN_DENOM.into(),
            amount: Uint128::new(2000),
        },
        Coin {
            denom: IBC_TOKEN_DENOM.into(),
            amount: Uint128::new(5000),
        },
    ];
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let amm_id = router.store_code(contract_amm());
    let lp_token_id = router.store_code(contract_cw20());
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();

    let msg = InstantiateMsg {
        token1_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
        lp_token: None,
        lp_token_code_id: lp_token_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let amm_addr = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap();

    // send tokens to contract address
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner,
            amm_addr.clone(),
            &add_liquidity_msg,
            &[
                Coin {
                    denom: NATIVE_TOKEN_DENOM.into(),
                    amount: Uint128::new(100),
                },
                Coin {
                    denom: IBC_TOKEN_DENOM.into(),
                    amount: Uint128::new(100),
                },
            ],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );

    let buyer = Addr::unchecked("buyer");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &buyer, funds).unwrap()
    });

    let add_liquidity_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(9),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(110),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(91),
            uri: None,
        }]
    );

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1990));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(9));

    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(7),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(10),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(120),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(84),
            uri: None,
        }]
    );

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1980));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(16));

    // Swap token for native
    let swap_msg = ExecuteMsg::Swap {
        input_token_select: TokenSelect::Token2,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(16),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(19),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(
            buyer.clone(),
            amm_addr.clone(),
            &swap_msg,
            &[Coin {
                denom: IBC_TOKEN_DENOM.into(),
                amount: Uint128::new(16),
            }],
        )
        .unwrap();

    let info = get_info(&router, &amm_addr);
    assert_eq!(
        info.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(101),
            uri: None,
        }]
    );
    assert_eq!(
        info.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }]
    );

    // Check balances of owner and buyer reflect the sale transaction
    let native_balance: Coin = bank_balance(&mut router, &buyer, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(native_balance.amount, Uint128::new(1999));
    let ibc_balance: Coin = bank_balance(&mut router, &buyer, IBC_TOKEN_DENOM.to_string());
    assert_eq!(ibc_balance.amount, Uint128::new(0));
}

#[test]
fn token_to_token_swap_with_fee_split() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let protocol_fee_recipient = Addr::unchecked("protocol_fee_recipient");

    let funds = coins(2_000_000_000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let token1 = create_cw20(
        &mut router,
        &owner,
        "token1".to_string(),
        "TOKENONE".to_string(),
        Uint128::new(5_000_000_000),
    );
    let token2 = create_cw20(
        &mut router,
        &owner,
        "token2".to_string(),
        "TOKENTWO".to_string(),
        Uint128::new(5_000_000_000),
    );

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();
    let amm1 = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(token1.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient.to_string(),
    );
    let amm2 = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(token2.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient.to_string(),
    );

    // Add initial liquidity to both pools
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(100_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(10_000_000)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm1.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100_000_000),
            }],
        )
        .unwrap();

    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(100_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100_000_000)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100_000_000),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm2.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100_000_000),
            }],
        )
        .unwrap();

    // Swap token1 for token2
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(10_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10_000_000),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(8_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4_890_000_000));

    let token2_balance = token2.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4_908_289_618));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(90_933_892));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(109_057_042));

    let fee_recipient_token1_balance = token1
        .balance(&router.wrap(), protocol_fee_recipient.clone())
        .unwrap();
    assert_eq!(fee_recipient_token1_balance, Uint128::new(10_000));

    let fee_recipient_native_balance = bank_balance(
        &mut router,
        &protocol_fee_recipient.clone(),
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_native_balance.amount, Uint128::new(9066));

    // Swap token2 for token1
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(10_000_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm1.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10_000_000),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(1_000_000),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm2.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4_901_542_163));

    let token2_balance = token2.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4_898_289_618));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(101_616_497));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(98_363_744));

    let fee_recipient_token2_balance = token2
        .balance(&router.wrap(), protocol_fee_recipient.clone())
        .unwrap();
    assert_eq!(fee_recipient_token2_balance, Uint128::new(10_000));

    let fee_recipient_native_balance = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_native_balance.amount, Uint128::new(19_759));

    // assert internal state is consistent
    let info_amm1 = get_info(&router, &amm1);
    let token1_balance = token1.balance(&router.wrap(), amm1.clone()).unwrap();
    assert_eq!(
        info_amm1.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: token1_balance,
            uri: None,
        }]
    );
    assert_eq!(
        info_amm1.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: amm1_native_balance.amount,
            uri: None,
        }]
    );

    let info_amm2 = get_info(&router, &amm2);
    let token2_balance = token2.balance(&router.wrap(), amm2.clone()).unwrap();
    assert_eq!(
        info_amm2.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: token2_balance,
            uri: None,
        }]
    );
    assert_eq!(
        info_amm2.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: amm2_native_balance.amount,
            uri: None,
        }]
    );
}

#[test]
fn test_pass_through_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap()
    });

    let token1 = create_cw20(
        &mut router,
        &owner,
        "token1".to_string(),
        "TOKENONE".to_string(),
        Uint128::new(5000),
    );
    let token2 = create_cw20(
        &mut router,
        &owner,
        "token2".to_string(),
        "TOKENTWO".to_string(),
        Uint128::new(5000),
    );

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm1 = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(token1.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );
    let amm2 = create_amm(
        &mut router,
        &owner,
        Denom::Cw20(token2.addr()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Some(TokenSelect::Token1),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    // Add initial liquidity to both pools
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(100),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm1.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(100),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        input_token1: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        min_liquidities: vec![Uint128::new(100)],
        max_token2: vec![TokenInfo {
            id: None,
            amount: Uint128::new(100),
            uri: None,
        }],
        expiration: None,
    };
    router
        .execute_contract(
            owner.clone(),
            amm2.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100),
            }],
        )
        .unwrap();

    // Swap token1 for token2
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm1.to_string(),
        amount: Uint128::new(10),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token1.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(8),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4890));

    let token2_balance = token2.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4908));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(91));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(109));

    // Swap token2 for token1
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm2.to_string(),
        amount: Uint128::new(10),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), token2.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm1.to_string(),
        input_token_select: TokenSelect::Token1,
        input_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(10),
            uri: None,
        }],
        output_min_tokens: vec![TokenInfo {
            id: None,
            amount: Uint128::new(1),
            uri: None,
        }],
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm2.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let token1_balance = token1.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token1_balance, Uint128::new(4900));

    let token2_balance = token2.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(token2_balance, Uint128::new(4898));

    let amm1_native_balance = bank_balance(&mut router, &amm1, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm1_native_balance.amount, Uint128::new(101));

    let amm2_native_balance = bank_balance(&mut router, &amm2, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(amm2_native_balance.amount, Uint128::new(99));

    // assert internal state is consistent
    let info_amm1 = get_info(&router, &amm1);
    let token1_balance = token1.balance(&router.wrap(), amm1.clone()).unwrap();
    assert_eq!(
        info_amm1.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: token1_balance,
            uri: None,
        }]
    );
    assert_eq!(
        info_amm1.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: amm1_native_balance.amount,
            uri: None,
        }]
    );

    let info_amm2 = get_info(&router, &amm2);
    let token2_balance = token2.balance(&router.wrap(), amm2.clone()).unwrap();
    assert_eq!(
        info_amm2.token1_reserves,
        vec![TokenInfo {
            id: None,
            amount: token2_balance,
            uri: None,
        }]
    );
    assert_eq!(
        info_amm2.token2_reserves,
        vec![TokenInfo {
            id: None,
            amount: amm2_native_balance.amount,
            uri: None,
        }]
    );
}
