#![cfg(test)]

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::str::FromStr;

use cosmwasm_std::{
    attr, coin, coins, to_json_binary, Addr, Api, Binary, BlockInfo, Coin, Decimal, Empty, Event,
    Querier, StdError, Storage, Uint128, WasmMsg,
};
use cw1155::{BatchBalanceResponse, Cw1155ExecuteMsg, Cw1155QueryMsg, TokenId};
use cw20_lp::{Cw20Coin, Cw20Contract, Cw20ExecuteMsg, Expiration};
use cw_multi_test::{
    App, Contract, ContractWrapper, Executor, StargateKeeper, StargateMsg, StargateQueryHandler,
};
use cw_utils::{parse_instantiate_response_data, PaymentError};
use prost::Message;

use crate::msg::{
    ExecuteMsg, FeeResponse, FreezeStatusResponse, InfoResponse, InstantiateMsg, Metadata,
    OwnershipResponse, QueryDenomMetadataRequest, QueryDenomMetadataResponse, QueryMsg,
    QueryTokenMetadataRequest, QueryTokenMetadataResponse, SlippageResponse, TokenSelect,
    TokenSuppliesResponse,
};
use crate::token_amount::TokenAmount;
use crate::utils::{MIN_FEE_PERCENT, PREDEFINED_MAX_FEES_PERCENT};
use crate::{error::ContractError, msg::Denom};

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

        Ok(to_json_binary(&metadata)?)
    }

    fn register_queries(&'static self, _keeper: &mut StargateKeeper<Empty, Empty>) {}
}

#[derive(Clone)]
struct DenomMetadataQueryHandler;
impl StargateQueryHandler for DenomMetadataQueryHandler {
    fn stargate_query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        msg: StargateMsg,
    ) -> anyhow::Result<Binary> {
        let request = QueryDenomMetadataRequest::decode(msg.value.as_slice())?;
        let metadata = match request.denom.as_str() {
            "Unsupported" => None,
            _ => Some(Metadata {
                ..Default::default()
            }),
        };

        Ok(to_json_binary(&QueryDenomMetadataResponse { metadata })?)
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
    .with_reply(crate::contract::reply);
    Box::new(contract)
}

pub fn contract_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base_lp::contract::execute,
        cw20_base_lp::contract::instantiate,
        cw20_base_lp::contract::query,
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

fn get_freeze_status(router: &App, contract_addr: &Addr) -> FreezeStatusResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::FreezeStatus {})
        .unwrap()
}

fn get_fee(router: &App, contract_addr: &Addr) -> FeeResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Fee {})
        .unwrap()
}

fn get_ownership(router: &App, contract_addr: &Addr) -> OwnershipResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Ownership {})
        .unwrap()
}

fn get_slippage(router: &App, contract_addr: &Addr) -> SlippageResponse {
    router
        .wrap()
        .query_wasm_smart(contract_addr, &QueryMsg::Slippage {})
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
    max_slippage_percent: Decimal,
    lp_fee_percent: Decimal,
    protocol_fee_percent: Decimal,
    protocol_fee_recipient: String,
) -> Addr {
    // set up amm contract
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());

    let msg = InstantiateMsg {
        token1155_denom: token1155_denom.clone(),
        token2_denom: token2_denom.clone(),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: protocol_fee_recipient.clone(),
    };
    let init_msg = to_json_binary(&msg).unwrap();
    let msg = WasmMsg::Instantiate {
        admin: None,
        code_id: amm_id,
        msg: init_msg,
        funds: [].to_vec(),
        label: "amm".to_string(),
    };
    let res = router.execute(owner.clone(), msg.into()).unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "instantiate-ixo-swap"),
        attr("owner", owner.to_string()),
        attr("max_slippage_percent", max_slippage_percent.to_string()),
        attr("lp_fee_percent", lp_fee_percent.to_string()),
        attr("protocol_fee_percent", protocol_fee_percent.to_string()),
        attr("protocol_fee_recipient", protocol_fee_recipient),
        attr("token_1155_denom", token1155_denom.to_string()),
        attr("token_2_denom", token2_denom.to_string()),
    ]);
    assert!(res.has_event(&event));
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "instantiate-lp-token"),
        attr("liquidity_pool_token_address", format!("contract{}", cw20_id)),
    ]);
    assert!(res.has_event(&event));

    let data = parse_instantiate_response_data(res.data.unwrap_or_default().as_slice()).unwrap();
    Addr::unchecked(data.contract_address)
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
    let msg = cw20_base_lp::msg::InstantiateMsg {
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
fn instantiate() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let supported_denom = "CARBON".to_string();
    let lp_fee_percent = Decimal::from_str("0.01").unwrap();
    let protocol_fee_percent = Decimal::zero();

    // instantiate
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    assert_ne!(cw1155_token, amm_addr);

    let info = get_info(&router, &amm_addr);
    assert_eq!(info.lp_token_address, "contract2".to_string());

    let ownership = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, owner.to_string());
    assert_eq!(ownership.pending_owner, None);

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.protocol_fee_recipient, owner.to_string());

    // commenting this test case as removed bank DenomMetadata validation for native denoms
    // // try instantiate with unsupported native denom
    // let cw20_id = router.store_code(contract_cw20());
    // let amm_id = router.store_code(contract_amm());
    // let msg = InstantiateMsg {
    //     token1155_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
    //     token2_denom: Denom::Native("Unsupported".to_string()),
    //     lp_token_code_id: cw20_id,
    //     max_slippage_percent,
    //     lp_fee_percent,
    //     protocol_fee_percent,
    //     protocol_fee_recipient: owner.to_string(),
    // };
    // let err = router
    //     .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
    //     .unwrap_err();
    // assert_eq!(
    //     ContractError::UnsupportedTokenDenom {
    //         id: "Unsupported".to_string()
    //     },
    //     err.downcast().unwrap()
    // );

    // try instantiate with duplicated tokens
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        token2_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(ContractError::InvalidTokenType {}, err.downcast().unwrap());

    // try instantiate with duplicated token addresses
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        token2_denom: Denom::Cw20(cw1155_token.clone()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(
        ContractError::DuplicatedTokenAddress {
            address: cw1155_token.to_string()
        },
        err.downcast().unwrap()
    );

    // instantiate with fee < 0.01% should fail
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let low_protocol_fee = Decimal::from_str("0.001").unwrap();
    let msg = InstantiateMsg {
        token1155_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        token2_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent: low_protocol_fee,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(
        ContractError::FeesTooLow {
            min_fee_percent: Decimal::from_str(MIN_FEE_PERCENT).unwrap(),
            fee_percent: low_protocol_fee
        },
        err.downcast().unwrap()
    );

    // try instantiate with invalid token address
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Cw1155(Addr::unchecked("1"), supported_denom.clone()),
        token2_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(ContractError::Std(StdError::GenericErr { msg: "Invalid input: human address too short for this mock implementation (must be >= 3).".to_string() }), err.downcast().unwrap());

    // try instantiate with non 1155 token for token1155_denom
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(ContractError::InvalidTokenType {}, err.downcast().unwrap());

    // try instantiate with 1155 token for token2_denom
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        token2_denom: Denom::Cw1155(cw1155_token.clone(), supported_denom.clone()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(ContractError::InvalidTokenType {}, err.downcast().unwrap());

    // try instantiate with invalid fee amount
    let lp_fee_percent = Decimal::from_str("5.01").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let cw20_id = router.store_code(contract_cw20());
    let amm_id = router.store_code(contract_amm());
    let msg = InstantiateMsg {
        token1155_denom: Denom::Cw1155(cw1155_token, supported_denom.clone()),
        token2_denom: Denom::Native(NATIVE_TOKEN_DENOM.into()),
        lp_token_code_id: cw20_id,
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        protocol_fee_recipient: owner.to_string(),
    };
    let err = router
        .instantiate_contract(amm_id, owner.clone(), &msg, &[], "amm", None)
        .unwrap_err();
    assert_eq!(
        ContractError::FeesTooHigh {
            max_fee_percent: Decimal::from_str(PREDEFINED_MAX_FEES_PERCENT).unwrap(),
            total_fee_percent: Decimal::from_str("5.01").unwrap()
        },
        err.downcast().unwrap()
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
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_first = create_cw1155(&mut router, &owner);
    let cw1155_second = create_cw1155(&mut router, &owner);

    let token_ids_cw1155_first = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];
    let token_ids_cw1155_second = vec![TokenId::from("SECOND/1"), TokenId::from("SECOND/2")];

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();

    let amm1 = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_first.clone(), "FIRST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );
    let amm2 = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_second.clone(), "SECOND".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
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

    // try swap with unsupported 1155 token
    let swap_msg = ExecuteMsg::PassThroughSwap {
        output_amm_address: amm2.to_string(),
        input_token: TokenSelect::Token1155,
        input_token_amount: TokenAmount::Multiple(HashMap::from([
            ("Unsupported".to_string(), Uint128::new(25)),
            ("Unsupported".to_string(), Uint128::new(25)),
        ])),
        output_min_token: TokenAmount::Multiple(HashMap::from([
            (token_ids_cw1155_second[0].clone(), Uint128::new(12)),
            (token_ids_cw1155_second[1].clone(), Uint128::new(12)),
        ])),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap_err();
    assert_eq!(
        ContractError::UnsupportedTokenDenom {
            id: "Unsupported".to_string()
        },
        err.downcast().unwrap()
    );

    // swap cw1155 tokens for specific cw1155 tokens
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
    let res = router
        .execute_contract(owner.clone(), amm1.clone(), &swap_msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "cross-contract-swap"),
        attr("input_token_enum", "token1155"),
        attr("input_token_amount", Uint128::new(50)),
        attr("output_token_amount", Uint128::new(33)),
        attr("output_amm_address", amm2.to_string()),
        attr("recipient", owner.to_string()),
        attr("token1155_reserve", Uint128::new(150)), // prev amount(100) plus added amount(50)
        attr("token2_reserve", Uint128::new(67)), // prev amount(100) minus removed amount(33)
    ]);
    assert!(res.has_event(&event));

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_first, &owner, &token_ids_cw1155_first).balances;
    assert_eq!(owner_balance, [Uint128::new(4925), Uint128::new(4925)]);

    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_second, &owner, &token_ids_cw1155_second).balances;
    assert_eq!(owner_balance, [Uint128::new(4962), Uint128::new(4962)]);

    // swap cw1155 tokens for any cw1155 tokens
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
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
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

    let max_slippage_percent = Decimal::from_str("8").unwrap();

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();

    let amm = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), "FIRST".to_string()),
        Denom::Cw20(cw20_token.addr()),
        max_slippage_percent,
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

    // try swap with unsupported 1155 token
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1155,
        input_amount: TokenAmount::Multiple(HashMap::from([
            ("Unsupported".to_string(), Uint128::new(25_000)),
            ("Unsupported".to_string(), Uint128::new(25_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(0)),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap_err();
    assert_eq!(
        ContractError::UnsupportedTokenDenom {
            id: "Unsupported".to_string()
        },
        err.downcast().unwrap()
    );

    // try swap with 0 min_output
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1155,
        input_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(25_000)),
            (token_ids[1].clone(), Uint128::new(25_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(0)),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap_err();
    assert_eq!(ContractError::MinTokenError {}, err.downcast().unwrap());

    // swap cw1155 for cw20
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token1155,
        input_amount: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(25_000)),
            (token_ids[1].clone(), Uint128::new(25_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(33_000)),
        expiration: None,
    };
    let res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "swap"),
        attr("sender", owner.clone()),
        attr("recipient", owner.clone()),
        attr("input_token_enum", "token1155"),
        attr("input_token_amount", Uint128::new(50_000)),
        attr("output_token_amount", Uint128::new(33_266)),
        attr("token1155_reserve", Uint128::new(149950)), // prev amount(100_000) plus added amount(50_000) - minus fees(50)
        attr("token2_reserve", Uint128::new(66_734)), // prev amount(100_000) minus removed amount(33_266)
        attr("protocol_fee_amount", Uint128::new(50)),
    ]);
    assert!(res.has_event(&event));

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(25_000), Uint128::new(25_000)]);
    let owner_balance = cw20_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(owner_balance, Uint128::new(83_266));
    let fee_recipient_balance =
        batch_balance_for_owner(&router, &cw1155_token, &protocol_fee_recipient, &token_ids)
            .balances;
    assert_eq!(fee_recipient_balance, [Uint128::new(50), Uint128::new(0)]);

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
            (token_ids[0].clone(), Uint128::new(33_000)),
            (token_ids[1].clone(), Uint128::new(33_000)),
        ])),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(62_878), Uint128::new(58_000)]);
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
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let token_ids = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];

    let max_slippage_percent = Decimal::from_str("8").unwrap();

    let lp_fee_percent = Decimal::from_str("0.2").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.1").unwrap();

    let amm = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), "FIRST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        max_slippage_percent,
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
    assert_eq!(fee_recipient_balance, [Uint128::new(50), Uint128::new(0)]);

    // Swap native for cw1155
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: TokenAmount::Single(Uint128::new(60_000)),
        min_output: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(34_000)),
            (token_ids[1].clone(), Uint128::new(34_000)),
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
    assert_eq!(owner_balance, [Uint128::new(61_878), Uint128::new(59_000)]);
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
fn cw1155_to_native_swap_low_fees() {
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
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);
    let token_ids = vec![TokenId::from("FIRST/1"), TokenId::from("FIRST/2")];

    let max_slippage_percent = Decimal::from_str("8").unwrap();

    let lp_fee_percent = Decimal::from_str("0.0").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.01").unwrap();

    let amm = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token.clone(), "FIRST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.into()),
        max_slippage_percent,
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
            (token_ids[0].clone(), Uint128::new(5_000)),
            (token_ids[1].clone(), Uint128::new(5_000)),
        ])),
        min_output: TokenAmount::Single(Uint128::new(8_362)),
        expiration: None,
    };
    let _res = router
        .execute_contract(owner.clone(), amm.clone(), &swap_msg, &[])
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(45_000), Uint128::new(45_000)]);
    let owner_balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance.amount, Uint128::new(59_090));
    let fee_recipient_balance =
        batch_balance_for_owner(&router, &cw1155_token, &protocol_fee_recipient, &token_ids)
            .balances;
    // should be 1 "FIRST/1" since it alphabetically comes first and with very low protocol fee low inout amount the fee is rounded up to 1
    assert_eq!(fee_recipient_balance, [Uint128::new(1), Uint128::new(0)]);

    // Swap native for cw1155
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: TokenAmount::Single(Uint128::new(7_000)),
        min_output: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(3_700)),
            (token_ids[1].clone(), Uint128::new(3_700)),
        ])),
        expiration: None,
    };
    let _res: cw_multi_test::AppResponse = router
        .execute_contract(
            owner.clone(),
            amm.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(7_000),
            }],
        )
        .unwrap();

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(49_163), Uint128::new(48_700)]);
    let owner_balance: Coin = bank_balance(&mut router, &owner, NATIVE_TOKEN_DENOM.to_string());
    assert_eq!(owner_balance.amount, Uint128::new(52_090));
    let fee_recipient_balance = bank_balance(
        &mut router,
        &protocol_fee_recipient,
        NATIVE_TOKEN_DENOM.to_string(),
    );
    // since very low protocol fee and low input amount the fee is rounded up to 1
    assert_eq!(fee_recipient_balance.amount, Uint128::new(1));


    // Swap input 1 should fail since protocol fee is not zero
    let swap_msg = ExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: TokenAmount::Single(Uint128::new(1)),
        min_output: TokenAmount::Multiple(HashMap::from([
            (token_ids[0].clone(), Uint128::new(1)),
        ])),
        expiration: None,
    };
    let err = router
        .execute_contract(
            owner.clone(),
            amm.clone(),
            &swap_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(1),
            }],
        )
        .unwrap_err();

    assert_eq!(
        ContractError::MinInputTokenAmountError {
            input_token_amount: Uint128::new(1),
            min_allowed: Uint128::new(2),
        },
        err.downcast().unwrap()
    );
}

#[test]
// receive cw20 tokens and release upon approval
fn amm_add_and_remove_liquidity() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";
    const INVALID_NATIVE_TOKEN_DENOM: &str = "ixo";

    let owner = Addr::unchecked("owner");
    let funds = vec![
        coin(2000, NATIVE_TOKEN_DENOM),
        coin(2000, INVALID_NATIVE_TOKEN_DENOM),
    ];
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/ixo.token.v1beta1.Query/TokenMetadata",
            Box::new(TokenMetadataQueryHandler),
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("1").unwrap();

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
        max_slippage_percent,
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

    // try send add liquidity with unsupported 1155 tokens
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            ("Unsupported".to_string(), Uint128::new(70)),
            ("Unsupported".to_string(), Uint128::new(30)),
        ]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };

    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(12),
            }],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::UnsupportedTokenDenom {
            id: "Unsupported".to_string()
        },
        err.downcast().unwrap()
    );

    // try send add liquidity with 0 min_liqudity
    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(70)),
            (token_ids[1].clone(), Uint128::new(30)),
        ]),
        min_liquidity: Uint128::zero(),
        max_token2: Uint128::new(100),
        expiration: None,
    };

    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(12),
            }],
        )
        .unwrap_err();
    assert_eq!(ContractError::MinTokenError {}, err.downcast().unwrap());

    let add_liquidity_msg = ExecuteMsg::AddLiquidity {
        token1155_amounts: HashMap::from([
            (token_ids[0].clone(), Uint128::new(70)),
            (token_ids[1].clone(), Uint128::new(30)),
        ]),
        min_liquidity: Uint128::new(100),
        max_token2: Uint128::new(100),
        expiration: None,
    };

    // try send insufficient amount of native token than provided liquidity to contract
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(12),
            }],
        )
        .unwrap_err();
    assert_eq!(ContractError::InsufficientFunds {}, err.downcast().unwrap());

    // try send invalid denom of native token than provided to contract
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[Coin {
                denom: INVALID_NATIVE_TOKEN_DENOM.into(),
                amount: Uint128::new(12),
            }],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::PaymentError(PaymentError::MissingDenom(NATIVE_TOKEN_DENOM.into())),
        err.downcast().unwrap()
    );

    // try send 2 native tokens to contract
    let err = router
        .execute_contract(
            owner.clone(),
            amm_addr.clone(),
            &add_liquidity_msg,
            &[
                Coin {
                    denom: NATIVE_TOKEN_DENOM.into(),
                    amount: Uint128::new(12),
                },
                Coin {
                    denom: INVALID_NATIVE_TOKEN_DENOM.into(),
                    amount: Uint128::new(12),
                },
            ],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::PaymentError(PaymentError::MultipleDenoms {}),
        err.downcast().unwrap()
    );

    // add liquidity to contract
    let res = router
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
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "add-liquidity"),
        attr("token1155_amount", Uint128::new(100)),
        attr("token2_amount", Uint128::new(100)),
        attr("liquidity_received", Uint128::new(100)),
        attr("liquidity_receiver", owner.to_string()),
        attr("token1155_reserve", Uint128::new(100)),
        attr("token2_reserve", Uint128::new(100)),
    ]);
    assert!(res.has_event(&event));

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

    // try remove liquidity with 0 min_token1155
    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(20),
        min_token1155: TokenAmount::Multiple(HashMap::from([(
            token_ids[1].clone(),
            Uint128::new(0),
        )])),
        min_token2: Uint128::new(1),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap_err();

    assert_eq!(ContractError::MinTokenError {}, err.downcast().unwrap());

    // try remove liquidity with 0 min_token2
    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(20),
        min_token1155: TokenAmount::Multiple(HashMap::from([(
            token_ids[1].clone(),
            Uint128::new(1),
        )])),
        min_token2: Uint128::new(0),
        expiration: None,
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap_err();

    assert_eq!(ContractError::MinTokenError {}, err.downcast().unwrap());

    // try remove more liquidity then owned
    let remove_liquidity_msg = ExecuteMsg::RemoveLiquidity {
        amount: Uint128::new(151),
        min_token1155: TokenAmount::Multiple(HashMap::from([(
            token_ids[1].clone(),
            Uint128::new(1),
        )])),
        min_token2: Uint128::new(1),
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
            (token_ids[0].clone(), Uint128::new(45)),
            (token_ids[1].clone(), Uint128::new(5)),
        ])),
        min_token2: Uint128::new(50),
        expiration: None,
    };
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &remove_liquidity_msg, &[])
        .unwrap();
    let event: Event = Event::new("wasm").add_attributes(vec![
        attr("action", "remove-liquidity"),
        attr("token1155_returned", Uint128::new(50)),
        attr("token2_returned", Uint128::new(50)),
        attr("liquidity_burned", Uint128::new(50)),
        attr("liquidity_provider", owner.to_string()),
        attr("token1155_reserve", Uint128::new(100)), // prev amount(150) minus removed amount(50)
        attr("token2_reserve", Uint128::new(101)), // prev amount(151) minus removed amount(50)
    ]);
    assert!(res.has_event(&event));

    // ensure balances updated
    let owner_balance =
        batch_balance_for_owner(&router, &cw1155_token, &owner, &token_ids).balances;
    assert_eq!(owner_balance, [Uint128::new(4925), Uint128::new(4975)]);
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(token_supplies, [Uint128::new(75), Uint128::new(25)]);
    let amm_balances =
        batch_balance_for_owner(&router, &cw1155_token, &amm_addr, &token_ids).balances;
    assert_eq!(amm_balances, [Uint128::new(75), Uint128::new(25)]);
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
            (token_ids[0].clone(), Uint128::new(75)),
            (token_ids[1].clone(), Uint128::new(25)),
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
        );
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("5").unwrap();

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
        max_slippage_percent,
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
            (token_ids[0].clone(), Uint128::new(41)),
            (token_ids[1].clone(), Uint128::new(30)),
            (token_ids[2].clone(), Uint128::new(5)),
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
            Uint128::new(4955),
            Uint128::new(4990)
        ]
    );
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(
        token_supplies,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(45),
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
            Uint128::new(45),
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
        min_token1155: TokenAmount::Single(Uint128::new(52)),
        min_token2: Uint128::new(52),
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
            Uint128::new(5000)
        ]
    );
    let token_supplies = get_owner_lp_tokens_balance(&router, &amm_addr, &token_ids).supplies;
    assert_eq!(
        token_supplies,
        [
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(0),
            Uint128::new(0)
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
            Uint128::new(0)
        ]
    );
    let crust_balance = lp_token.balance(&router.wrap(), owner.clone()).unwrap();
    assert_eq!(crust_balance, Uint128::new(0));
}

#[test]
fn freeze_pool() {
    let mut router = mock_app();

    let owner = Addr::unchecked("owner");
    let funds = coins(100, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token, "TEST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    let freeze_status = get_freeze_status(&router, &amm_addr);
    assert_eq!(freeze_status.status, false);

    // freeze pool
    let freeze_msg = ExecuteMsg::FreezeDeposits { freeze: true };
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &freeze_msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "freeze-deposits"),
        attr("frozen", "true"),
    ]);
    assert!(res.has_event(&event));

    let freeze_status = get_freeze_status(&router, &amm_addr);
    assert_eq!(freeze_status.status, true);

    // freeze pool with same freeze status
    let freeze_msg = ExecuteMsg::FreezeDeposits { freeze: true };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &freeze_msg, &[])
        .unwrap_err();
    assert_eq!(
        ContractError::DuplicatedFreezeStatus {
            freeze_status: true
        },
        err.downcast().unwrap()
    );

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
fn transfer_ownership() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let new_owner = Addr::unchecked("new-owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token, "TEST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    // Transfer ownership to claim
    let msg = ExecuteMsg::TransferOwnership {
        owner: Some(new_owner.to_string()),
    };
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "transfer-ownership"),
        attr("pending_owner", new_owner.to_string()),
    ]);
    assert!(res.has_event(&event));

    let ownership = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, owner.to_string());
    assert_eq!(ownership.pending_owner, Some(new_owner.to_string()));

    // Try transfer ownership with not owner address
    let msg = ExecuteMsg::TransferOwnership {
        owner: Some(new_owner.to_string()),
    };
    let err = router
        .execute_contract(new_owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    // Try transfer ownership to current owner
    let msg = ExecuteMsg::TransferOwnership {
        owner: Some(owner.to_string()),
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(ContractError::DuplicatedOwner {}, err.downcast().unwrap());

    // Try claim ownership with not new owner address
    let msg = ExecuteMsg::ClaimOwnership {};
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    // Claim ownership
    let msg = ExecuteMsg::ClaimOwnership {};
    let res = router
        .execute_contract(new_owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "claim-ownership"),
        attr("owner", new_owner.to_string()),
    ]);
    assert!(res.has_event(&event));

    let ownership = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, new_owner.to_string());
    assert_eq!(ownership.pending_owner, None);

    // Cancel transferring of ownership
    let msg = ExecuteMsg::TransferOwnership {
        owner: Some(owner.to_string()),
    };
    let _res = router
        .execute_contract(new_owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();

    let ownership: OwnershipResponse = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, new_owner.to_string());
    assert_eq!(ownership.pending_owner, Some(owner.to_string()));

    let msg = ExecuteMsg::TransferOwnership { owner: None };
    let _res = router
        .execute_contract(new_owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();

    let ownership = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, new_owner.to_string());
    assert_eq!(ownership.pending_owner, None);

    // Claim empty pending ownership
    let msg = ExecuteMsg::ClaimOwnership {};
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![attr("action", "claim-ownership")]);
    assert!(res.has_event(&event));

    let ownership = get_ownership(&router, &amm_addr);
    assert_eq!(ownership.owner, new_owner.to_string());
    assert_eq!(ownership.pending_owner, None);
}

#[test]
fn update_slippage() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token, "TEST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    let current_slippage = get_slippage(&router, &amm_addr);
    assert_eq!(max_slippage_percent, current_slippage.max_slippage_percent);

    let new_max_slippage_percent = Decimal::from_str("0.2").unwrap();
    let msg = ExecuteMsg::UpdateSlippage {
        max_slippage_percent: new_max_slippage_percent,
    };
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "update-slippage"),
        attr("max_slippage_percent", new_max_slippage_percent.to_string()),
    ]);
    assert!(res.has_event(&event));

    let current_slippage = get_slippage(&router, &amm_addr);
    assert_eq!(
        new_max_slippage_percent,
        current_slippage.max_slippage_percent
    );
}

#[test]
fn update_fee() {
    let mut router = mock_app();

    const NATIVE_TOKEN_DENOM: &str = "juno";

    let owner = Addr::unchecked("owner");
    let funds = coins(2000, NATIVE_TOKEN_DENOM);
    router.borrow_mut().init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &owner, funds).unwrap();
        router.stargate.register_query(
            "/cosmos.bank.v1beta1.Query/DenomMetadata",
            Box::new(DenomMetadataQueryHandler),
        )
    });

    let cw1155_token = create_cw1155(&mut router, &owner);

    let max_slippage_percent = Decimal::from_str("0.3").unwrap();

    let lp_fee_percent = Decimal::from_str("0.3").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let amm_addr = create_amm(
        &mut router,
        &owner,
        Denom::Cw1155(cw1155_token, "TEST".to_string()),
        Denom::Native(NATIVE_TOKEN_DENOM.to_string()),
        max_slippage_percent,
        lp_fee_percent,
        protocol_fee_percent,
        owner.to_string(),
    );

    let lp_fee_percent = Decimal::from_str("0.15").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.15").unwrap();
    let msg = ExecuteMsg::UpdateFee {
        protocol_fee_recipient: "new_fee_recipient".to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let res = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap();
    let event = Event::new("wasm").add_attributes(vec![
        attr("action", "update-fee"),
        attr("lp_fee_percent", lp_fee_percent.to_string()),
        attr("protocol_fee_percent", protocol_fee_percent.to_string()),
        attr("protocol_fee_recipient", "new_fee_recipient".to_string()),
    ]);
    assert!(res.has_event(&event));

    let fee = get_fee(&router, &amm_addr);
    assert_eq!(fee.protocol_fee_recipient, "new_fee_recipient".to_string());
    assert_eq!(fee.protocol_fee_percent, protocol_fee_percent);
    assert_eq!(fee.lp_fee_percent, lp_fee_percent);

    // Try updating with fee values that are too high
    let lp_fee_percent = Decimal::from_str("5.01").unwrap();
    let protocol_fee_percent = Decimal::zero();
    let msg = ExecuteMsg::UpdateFee {
        protocol_fee_recipient: "new_fee_recipient".to_string(),
        lp_fee_percent,
        protocol_fee_percent,
    };
    let err = router
        .execute_contract(owner.clone(), amm_addr.clone(), &msg, &[])
        .unwrap_err();
    assert_eq!(
        ContractError::FeesTooHigh {
            max_fee_percent: Decimal::from_str(PREDEFINED_MAX_FEES_PERCENT).unwrap(),
            total_fee_percent: Decimal::from_str("5.01").unwrap()
        },
        err.downcast().unwrap()
    );

    // Try updating with invalid owner, show throw unauthoritzed error
    let lp_fee_percent = Decimal::from_str("0.21").unwrap();
    let protocol_fee_percent = Decimal::from_str("0.09").unwrap();
    let msg = ExecuteMsg::UpdateFee {
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
        .unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err.downcast().unwrap());

    // Try updating owner and fee params
    let msg = ExecuteMsg::UpdateFee {
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
}
