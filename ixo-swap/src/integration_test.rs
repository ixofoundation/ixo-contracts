#![cfg(test)]

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::str::FromStr;

use cosmwasm_std::{
    coins, to_binary, Addr, Api, Binary, BlockInfo, Coin, CosmosMsg, Decimal, Empty, Querier,
    Storage, Uint128, WasmMsg,
};
use cw1155::{BatchBalanceResponse, Cw1155ExecuteMsg, Cw1155QueryMsg, TokenId};
use cw20::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg, Expiration};
use cw_multi_test::{
    App, Contract, ContractWrapper, Executor, StargateKeeper, StargateMsg, StargateQueryHandler,
};
use prost::Message;

use crate::msg::{
    ExecuteMsg, FeeResponse, InfoResponse, InstantiateMsg, QueryMsg, QueryTokenMetadataRequest,
    QueryTokenMetadataResponse, TokenAmount, TokenSelect, TokenSuppliesResponse,
};
use crate::{
    error::ContractError,
    msg::{Denom, MigrateMsg},
};

#[derive(Clone)]
struct TokenMetadataQueryHandler;
impl StargateQueryHandler for TokenMetadataQueryHandler {
    fn stargate_query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        msg: StargateMsg,
    ) -> anyhow::Result<Binary> {
        let request = QueryTokenMetadataRequest::decode(msg.value.as_slice())?;
        let metadata = QueryTokenMetadataResponse {
            name: request.id.split("/").collect::<Vec<_>>()[0].to_string(),
            decimals: "0".to_string(),
            description: "Test credits".to_string(),
            image: "https://ipfs.io/ipfs/test".to_string(),
            index: "1".to_string(),
        };

        Ok(to_binary(&metadata)?)
    }

    fn register_queries(&'static self, _keeper: &mut StargateKeeper<Empty, Empty>) {}
}

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

fn get_owner_lp_tokens_balance(
    router: &App,
    contract_addr: &Addr,
    tokens_id: &Vec<TokenId>,
) -> TokenSuppliesResponse {
    router
        .wrap()
        .query_wasm_smart(
            contract_addr,
            &QueryMsg::TokenSupplies {
                tokens_id: tokens_id.clone(),
            },
        )
        .unwrap()
}

fn create_amm(
    router: &mut App,
    owner: &Addr,
    token1155_denom: Denom,
    token2_denom: Denom,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Addr {
    // set up amm contract
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom,
        token2_denom,
        lp_token_code_id: cw20_id,
        owner: Some(owner.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient,
    };
    router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
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

fn bank_balance(router: &mut App, addr: &Addr, denom: String) -> Coin {
    router
        .wrap()
        .query_balance(addr.to_string(), denom)
        .unwrap()
}

fn batch_balance_for_owner(
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

    let cw1155_token = create_cw1155(&mut router, &owner);

    let supported_denom = "CARBON".to_string();
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

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
        token1155_denom: Denom::Cw1155(cw1155_token, supported_denom.clone()),
        token2_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
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
fn cw1155_to_cw1155_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        )
    });

    let cw1155_first = create_cw1155(&mut router, &owner);
    let cw1155_second = create_cw1155(&mut router, &owner);

    let token_ids_cw1155_first = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];
    let token_ids_cw1155_second = vec![TokenId::from("SECOND/1"), TokenId::from("SECOND/2")];

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();

    let amm1 = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_first.clone(), "FIRST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );
    let amm2 = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_second.clone(), "SECOND".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    // set up initial balances
    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (
                token_ids_cw1155_first[0].clone(),
                Uint128::new(5000),
                "".to_string(),
            ),
            (
                token_ids_cw1155_first[1].clone(),
                Uint128::new(5000),
                "".to_string(),
            ),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_first.clone(), &mint_msg, &[])
        .unwrap();

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (
                token_ids_cw1155_second[0].clone(),
                Uint128::new(5000),
                "".to_string(),
            ),
            (
                token_ids_cw1155_second[1].clone(),
                Uint128::new(5000),
                "".to_string(),
            ),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_second.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_first, &owner, &token_ids_cw1155_first);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(5000), Uint128::new(5000),]
    );
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_second, &owner, &token_ids_cw1155_second);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(5000), Uint128::new(5000),]
    );

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm1.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_first.clone(), &allowance_msg, &[])
        .unwrap();

    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm2.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_second.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids_cw1155_first[0].clone(), Uint128::new(50)),
            (token_ids_cw1155_first[1].clone(), Uint128::new(50)),
        ]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
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

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids_cw1155_second[0].clone(), Uint128::new(50)),
            (token_ids_cw1155_second[1].clone(), Uint128::new(50)),
        ]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
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

    // Swap cw1155 tokens for specific cw1155 tokens
    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token: TokenSelect::Token1155,
        input_token_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids_cw1155_first[0].clone(), Uint128::new(25)),
            (token_ids_cw1155_first[1].clone(), Uint128::new(25)),
        ])),
        output_min_token: TokenAmount::Multiple(HashMap::from([
            (token_ids_cw1155_second[0].clone(), Uint128::new(12)),
            (token_ids_cw1155_second[1].clone(), Uint128::new(12)),
        ])),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_first, &owner, &token_ids_cw1155_first).balances;
    assert_eq!(owner_balance, [Uint128::new(4925), Uint128::new(4925)]);

    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_second, &owner, &token_ids_cw1155_second).balances;
    assert_eq!(owner_balance, [Uint128::new(4962), Uint128::new(4962)]);

    // Swap cw1155 tokens for any cw1155 tokens
    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token: TokenSelect::Token1155,
        input_token_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids_cw1155_first[0].clone(), Uint128::new(25)),
            (token_ids_cw1155_first[1].clone(), Uint128::new(25)),
        ])),
        output_min_token: TokenAmount::Single(Uint128::new(8)),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_first, &owner, &token_ids_cw1155_first).balances;
    assert_eq!(owner_balance, [Uint128::new(4900), Uint128::new(4900)]);

    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_second, &owner, &token_ids_cw1155_second).balances;
    assert_eq!(owner_balance, [Uint128::new(4970), Uint128::new(4962)]);
}

#[test]
fn cw1155_to_cw20_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let protocol_fee_recipient = Addr::unchecked("protocol_fee_recipient");

    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let cw20_token = create_cw20(
        &mut router,
        &owner,
        "token".to_string(),
        "CWTOKEN".to_string(),
        Uint128::new(150_000),
    );

    let token_ids = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();

    let amm = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), "FIRST".to_string()),
        Denom::Cw20(cw20_token.addr()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient.to_string(),
    );

    // set up initial balances
    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (token_ids[0].clone(), Uint128::new(100_000), "".to_string()),
            (token_ids[1].clone(), Uint128::new(100_000), "".to_string()),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance = batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(100_000), Uint128::new(100_000),]
    );
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(150_000));

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm.to_string(),
        amount: Uint128::new(100_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(50_000)),
            (token_ids[1].clone(), Uint128::new(50_000)),
        ]),
        min_liquidity: Uint128::new(100_000),
        max_token2: Uint128::new(100_000),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &add_liquidity_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(50_000), Uint128::new(50_000)]);
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(50_000));

    // Swap cw1155 for cw20
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1155,
        input_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(25_000)),
            (token_ids[1].clone(), Uint128::new(25_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(33_000)),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(25_000), Uint128::new(25_000)]);
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(83_266));
    let fee_recipient_balance =
        batch_balance_for_owner(&router, &cw1155_token, &protocol_fee_recipient, &token_ids)
            .balances;
    assert_eq!(fee_recipient_balance, [Uint128::new(25), Uint128::new(25)]);

    // Swap cw20 for cw1155
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm.to_string(),
        amount: Uint128::new(60_000),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw20_token.addr(), &allowance_msg, &[])
        .unwrap();

    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: TokenAmount::Single(Uint128::new(60_000)),
        min_output: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(30_000)),
            (token_ids[1].clone(), Uint128::new(30_000)),
        ])),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(60_439), Uint128::new(60_439)]);
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(23_266));
    let fee_recipient_balance = cw20_token
        .balance(&router.wrap(), protocol_fee_recipient.clone())
        .unwrap();
    assert_eq!(fee_recipient_balance, Uint128::new(60));
}

#[test]
fn cw1155_to_native_swap() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let protocol_fee_recipient = Addr::unchecked("protocol_fee_recipient");

    let funds = coins(150_000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let token_ids = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();

    let amm = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), "FIRST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient.to_string(),
    );

    // set up initial balances
    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (token_ids[0].clone(), Uint128::new(100_000), "".to_string()),
            (token_ids[1].clone(), Uint128::new(100_000), "".to_string()),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance = batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(100_000), Uint128::new(100_000),]
    );

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(50_000)),
            (token_ids[1].clone(), Uint128::new(50_000)),
        ]),
        min_liquidity: Uint128::new(100_000),
        max_token2: Uint128::new(100_000),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(100_000),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(50_000), Uint128::new(50_000)]);
    let owner_balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance.amount, Uint128::new(50_000));

    // Swap cw1155 for native
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1155,
        input_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(25_000)),
            (token_ids[1].clone(), Uint128::new(25_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(33_000)),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(25_000), Uint128::new(25_000)]);
    let owner_balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance.amount, Uint128::new(83_266));
    let fee_recipient_balance =
        batch_balance_for_owner(&router, &cw1155_token, &protocol_fee_recipient, &token_ids)
            .balances;
    assert_eq!(fee_recipient_balance, [Uint128::new(25), Uint128::new(25)]);

    // Swap native for cw1155
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: TokenAmount::Single(Uint128::new(60_000)),
        min_output: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(30_000)),
            (token_ids[1].clone(), Uint128::new(30_000)),
        ])),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(60_000),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(60_439), Uint128::new(60_439)]);
    let owner_balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance.amount, Uint128::new(23_266));
    let fee_recipient_balance = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    assert_eq!(fee_recipient_balance.amount, Uint128::new(60));
}

#[test]
// receive cw20 tokens and release upon approval
fn amm_add_and_remove_liquidity() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let supported_denom = "CARBON".to_string();
    let token_ids = vec![
        format!("{}/1", supported_denom),
        format!("{}/2", supported_denom),
    ];
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), supported_denom),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

    let info = get_info(&router, &amm_addr);
    // set up cw20 helpers
    let lp_token = Cw20Contract(Addr::unchecked(info.lp_token_address));

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (token_ids[0].clone(), Uint128::new(5000), "".to_string()),
            (token_ids[1].clone(), Uint128::new(5000), "".to_string()),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance = batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [Uint128::new(5000), Uint128::new(5000)]
    );

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm_addr.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(70)),
            (token_ids[1].clone(), Uint128::new(30)),
        ]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
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
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(4930), Uint128::new(4970)]);
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(token_supplies, [Uint128::new(70), Uint128::new(30)]);
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(amm_balances, [Uint128::new(70), Uint128::new(30)]);
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(100));

    // send tokens to contract address
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([(token_ids[0].clone(), Uint128::new(50))]),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(51),
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
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(4880), Uint128::new(4970)]);
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(token_supplies, [Uint128::new(120), Uint128::new(30)]);
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(amm_balances, [Uint128::new(120), Uint128::new(30)]);
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(150));

    // too low max token error
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([(token_ids[1].clone(), Uint128::new(50))]),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(45),
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
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([(token_ids[1].clone(), Uint128::new(50))]),
        min_liquidity: Uint128::new(500),
        max_token2: Uint128::new(50),
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
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([(token_ids[0].clone(), Uint128::new(50))]),
        min_liquidity: Uint128::new(50),
        max_token2: Uint128::new(50),
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
    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(151),
        min_token1155: TokenAmount::Multiple(HashMap::from([(
            token_ids[1].clone(),
            Uint128::new(0),
        )])),
        min_token2: Uint128::new(0),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
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
        amount: Uint128::new(50),
        min_token1155: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(35)),
            (token_ids[1].clone(), Uint128::new(5)),
        ])),
        min_token2: Uint128::new(50),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap();

    // ensure balances updated

    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(4920), Uint128::new(4980)]);
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(token_supplies, [Uint128::new(80), Uint128::new(20)]);
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(amm_balances, [Uint128::new(80), Uint128::new(20)]);
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
        amount: Uint128::new(100),
        min_token1155: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(80)),
            (token_ids[1].clone(), Uint128::new(20)),
        ])),
        min_token2: Uint128::new(100),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(5000), Uint128::new(5000)]);
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(token_supplies, [Uint128::new(0), Uint128::new(0)]);
}

#[test]
fn remove_liquidity_with_partially_and_any_filling() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let supported_denom = "CARBON".to_string();
    let token_ids = vec![
        format!("{}/1", supported_denom),
        format!("{}/2", supported_denom),
        format!("{}/3", supported_denom),
        format!("{}/4", supported_denom),
    ];
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), supported_denom),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

    let info = get_info(&router, &amm_addr);
    // set up cw20 helpers
    let lp_token = Cw20Contract(Addr::unchecked(info.lp_token_address));

    let mint_msg = Cw1155ExecuteMsg::BatchMint {
        to: owner.clone().into(),
        batch: vec![
            (token_ids[0].clone(), Uint128::new(5000), "".to_string()),
            (token_ids[1].clone(), Uint128::new(5000), "".to_string()),
            (token_ids[2].clone(), Uint128::new(5000), "".to_string()),
            (token_ids[3].clone(), Uint128::new(5000), "".to_string()),
        ],
        msg: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &mint_msg, &[])
        .unwrap();

    // check initial balances
    let owner_balance = batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids);
    assert_eq!(
        owner_balance.balances,
        [
            Uint128::new(5000),
            Uint128::new(5000),
            Uint128::new(5000),
            Uint128::new(5000)
        ]
    );

    // send tokens to contract address
    let allowance_msg = Cw1155ExecuteMsg::ApproveAll {
        operator: amm_addr.clone().into(),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), cw1155_token.clone(), &allowance_msg, &[])
        .unwrap();

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(45)),
            (token_ids[1].clone(), Uint128::new(30)),
            (token_ids[2].clone(), Uint128::new(50)),
            (token_ids[3].clone(), Uint128::new(10)),
        ]),
        min_liquidity: Uint128::new(135),
        max_token2: Uint128::new(135),
        expiration: None,
    };
    let _res = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(135),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(
        owner_balance,
        [
            Uint128::new(4955),
            Uint128::new(4970),
            Uint128::new(4950),
            Uint128::new(4990)
        ]
    );
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(
        token_supplies,
        [
            Uint128::new(45),
            Uint128::new(30),
            Uint128::new(50),
            Uint128::new(10)
        ]
    );
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(
        amm_balances,
        [
            Uint128::new(45),
            Uint128::new(30),
            Uint128::new(50),
            Uint128::new(10)
        ]
    );
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(135));

    // remove liquidity for specific cw1155 tokens
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(80u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(80),
        min_token1155: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(40)),
            (token_ids[1].clone(), Uint128::new(30)),
        ])),
        min_token2: Uint128::new(80),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(
        owner_balance,
        [
            Uint128::new(5000),
            Uint128::new(5000),
            Uint128::new(4950),
            Uint128::new(4990)
        ]
    );
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(
        token_supplies,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(50),
            Uint128::new(10)
        ]
    );
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(
        amm_balances,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(50),
            Uint128::new(10)
        ]
    );
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(55));

    // remove liquidity for any cw1155 tokens
    let allowance_msg = Cw20ExecuteMsg::IncreaseAllowance {
        spender: amm_addr.to_string(),
        amount: Uint128::new(55u128),
        expires: None,
    };
    let _res = router
        .execute_contract(owner.clone(), lp_token.addr(), &allowance_msg, &[])
        .unwrap();

    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(55),
        min_token1155: TokenAmount::Single(Uint128::new(30)),
        min_token2: Uint128::new(30),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(
        owner_balance,
        [
            Uint128::new(5000),
            Uint128::new(5000),
            Uint128::new(5000),
            Uint128::new(4995)
        ]
    );
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(
        token_supplies,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(5)
        ]
    );
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(
        amm_balances,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(5)
        ]
    );
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(0));
}

#[test]
fn migrate() {
    let router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const IBC_TOKEN_DENOM: &str = "atom";

    let amm_id = router.store_code(contract_amm());
    let lp_token_id = router.store_code(contract_cw20());
    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let owner = Addr::unchecked("owner");

    let msg = InstantiateMsg {
        token1155_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
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
        token1155_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(IBC_TOKEN_DENOM.into()),
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
        token1155_amounts: HashMap::from([("TEST/1".to_string(), Uint128::new(50))]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
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
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        Denom::Cw20(cw20_token.addr()),
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
