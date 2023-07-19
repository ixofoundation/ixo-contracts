use std::collections::HashMap;

use anyhow::bail;
use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CosmosMsg, CustomQuery, Empty, Querier, Storage};

use crate::{AppResponse, CosmosRouter};

// TODO: turn into extensions of Fn trait
pub trait StargateQueryHandler {
    fn stargate_query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: StargateMsg,
    ) -> anyhow::Result<Binary>;

    fn register_queries(&'static self, keeper: &mut StargateKeeper<Empty, Empty>);
}

pub trait StargateMessageHandler<ExecC, QueryC: CustomQuery> {
    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<AppResponse>;

    fn register_msgs(&'static self, keeper: &mut StargateKeeper<Empty, Empty>);
}

pub struct StargateKeeper<ExecC, QueryC> {
    messages: HashMap<String, Box<dyn StargateMessageHandler<ExecC, QueryC>>>,
    queries: HashMap<String, Box<dyn StargateQueryHandler>>,
}

impl<'a, ExecC, QueryC> StargateKeeper<ExecC, QueryC> {
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            queries: HashMap::new(),
        }
    }

    pub fn register_msg(
        &mut self,
        type_url: &str,
        handler: Box<dyn StargateMessageHandler<ExecC, QueryC>>,
    ) {
        self.messages.insert(type_url.to_string(), handler);
    }

    pub fn register_query(&mut self, type_url: &str, handler: Box<dyn StargateQueryHandler>) {
        self.queries.insert(type_url.to_string(), handler);
    }
}

pub struct StargateMsg {
    pub type_url: String,
    pub value: Binary,
}

impl From<StargateMsg> for CosmosMsg {
    fn from(msg: StargateMsg) -> Self {
        CosmosMsg::Stargate {
            type_url: msg.type_url,
            value: msg.value,
        }
    }
}

pub trait Stargate<ExecC, QueryC> {
    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<crate::AppResponse>;

    fn sudo(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &cosmwasm_std::BlockInfo,
        msg: Empty,
    ) -> anyhow::Result<crate::AppResponse>;

    fn query(
        &self,
        api: &dyn cosmwasm_std::Api,
        storage: &dyn cosmwasm_std::Storage,
        querier: &dyn cosmwasm_std::Querier,
        block: &cosmwasm_std::BlockInfo,
        request: StargateMsg,
    ) -> anyhow::Result<cosmwasm_std::Binary>;
}

impl<'a, ExecC, QueryC: CustomQuery> Stargate<ExecC, QueryC> for StargateKeeper<ExecC, QueryC> {
    fn execute(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargateMsg,
    ) -> anyhow::Result<crate::AppResponse> {
        match self.messages.get(&msg.type_url.to_string()) {
            Some(handler) => handler.execute(api, storage, router, block, sender, msg),
            None => bail!("Unsupported stargate message: {}", msg.type_url),
        }
    }

    fn sudo(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _msg: Empty,
    ) -> anyhow::Result<crate::AppResponse> {
        bail!("StargateKeeper does not support sudo")
    }

    fn query(
        &self,
        api: &dyn cosmwasm_std::Api,
        storage: &dyn cosmwasm_std::Storage,
        querier: &dyn cosmwasm_std::Querier,
        block: &cosmwasm_std::BlockInfo,
        request: StargateMsg,
    ) -> anyhow::Result<cosmwasm_std::Binary> {
        match self.queries.get(&request.type_url.to_string()) {
            Some(handler) => handler.stargate_query(api, storage, querier, block, request),
            None => bail!("Unsupported stargate query: {}", request.type_url),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Ok;
    use cosmwasm_std::{coin, from_binary, to_binary, Coin, CosmosMsg, Event, QueryRequest};
    use osmosis_std::types::cosmos::bank::v1beta1::{
        QueryAllBalancesRequest, QueryBalanceRequest, QuerySupplyOfRequest,
    };

    use crate::{BasicAppBuilder, Executor};

    use super::*;

    #[derive(Clone)]
    struct FooHandler;
    impl StargateMessageHandler<Empty, Empty> for FooHandler {
        fn execute(
            &self,
            _api: &dyn Api,
            _storage: &mut dyn Storage,
            _router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
            _block: &BlockInfo,
            _sender: Addr,
            msg: StargateMsg,
        ) -> anyhow::Result<AppResponse> {
            let mut res = AppResponse::default();
            let num: u64 = from_binary(&msg.value)?;
            res.events
                .push(Event::new("foo").add_attribute("bar", num.to_string()));
            Ok(res)
        }

        fn register_msgs(&'static self, keeper: &mut StargateKeeper<Empty, Empty>) {
            keeper.register_msg("foo", Box::new(self.clone()))
        }
    }
    const FOO_HANDLER: FooHandler = FooHandler;

    #[derive(Clone)]
    struct FooQueryHandler;
    impl StargateQueryHandler for FooQueryHandler {
        fn stargate_query(
            &self,
            _api: &dyn Api,
            _storage: &dyn Storage,
            _querier: &dyn Querier,
            _block: &BlockInfo,
            msg: StargateMsg,
        ) -> anyhow::Result<Binary> {
            let num: u64 = from_binary(&msg.value)?;
            let bin = to_binary(&format!("bar{:?}", num)).unwrap();
            Ok(bin)
        }

        fn register_queries(&'static self, keeper: &mut StargateKeeper<Empty, Empty>) {
            keeper.register_query("foo", Box::new(self.clone()))
        }
    }
    const FOO_QUERY_HANDLER: FooQueryHandler = FooQueryHandler;

    #[derive(Clone)]
    struct BarHandler;
    impl StargateMessageHandler<Empty, Empty> for BarHandler {
        fn execute(
            &self,
            api: &dyn Api,
            storage: &mut dyn Storage,
            router: &dyn CosmosRouter<ExecC = Empty, QueryC = Empty>,
            block: &BlockInfo,
            _sender: Addr,
            msg: StargateMsg,
        ) -> anyhow::Result<AppResponse> {
            let query_res = router.query(
                api,
                storage,
                block,
                QueryRequest::Stargate {
                    path: "foo".to_string(),
                    data: msg.value,
                },
            )?;

            let mut res = AppResponse::default();
            res.data = Some(query_res);

            Ok(res)
        }

        fn register_msgs(&'static self, keeper: &mut StargateKeeper<Empty, Empty>) {
            keeper.register_msg("bar", Box::new(self.clone()))
        }
    }
    const BAR_HANDLER: BarHandler = BarHandler;

    #[test]
    fn new_stargate_keeper() {
        StargateKeeper::<Empty, Empty>::new();
    }

    #[test]
    fn register_and_call_stargate_msg() {
        let mut stargate_keeper = StargateKeeper::new();
        stargate_keeper.register_msg("foo", Box::new(FOO_HANDLER));

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        let res = app
            .execute(
                Addr::unchecked("unchecked"),
                CosmosMsg::Stargate {
                    type_url: "foo".to_string(),
                    value: to_binary(&1337u64).unwrap(),
                },
            )
            .unwrap();

        res.assert_event(&Event::new("foo").add_attribute("bar", "1337"));
    }

    #[test]
    fn register_and_call_stargate_query() {
        let mut stargate_keeper = StargateKeeper::new();
        stargate_keeper.register_query("foo", Box::new(FOO_QUERY_HANDLER));

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        let querier = app.wrap();

        let res: String = querier
            .query(&QueryRequest::Stargate {
                path: "foo".to_string(),
                data: to_binary(&1337u64).unwrap(),
            })
            .unwrap();

        assert_eq!(res, "bar1337".to_string());
    }

    #[test]
    fn query_inside_execution() {
        let mut stargate_keeper = StargateKeeper::new();
        stargate_keeper.register_msg("bar", Box::new(BAR_HANDLER));
        stargate_keeper.register_query("foo", Box::new(FOO_QUERY_HANDLER));

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|_, _, _| {});

        let res = app
            .execute(
                Addr::unchecked("unchecked"),
                CosmosMsg::Stargate {
                    type_url: "bar".to_string(),
                    value: to_binary(&1337u64).unwrap(),
                },
            )
            .unwrap();

        let x: String = from_binary(&res.data.unwrap()).unwrap();
        assert_eq!(x, "bar1337");
    }

    #[test]
    fn query_bank_module_via_stargate() {
        let stargate_keeper = StargateKeeper::new();

        let owner = Addr::unchecked("owner");
        let init_funds = vec![coin(20, "btc"), coin(100, "eth")];

        let app = BasicAppBuilder::<Empty, Empty>::new()
            .with_stargate(stargate_keeper)
            .build(|router, _, storage| {
                router
                    .bank
                    .init_balance(storage, &owner, init_funds.clone())
                    .unwrap();
            });

        let querier = app.wrap();

        // QueryAllBalancesRequest
        let res = QueryAllBalancesRequest {
            address: owner.to_string(),
            pagination: None,
        }
        .query(&querier)
        .unwrap();
        let blances: Vec<Coin> = res
            .balances
            .into_iter()
            .map(|c| Coin::new(u128::from_str(&c.amount).unwrap(), c.denom))
            .collect();
        assert_eq!(blances, init_funds);

        // QueryBalanceRequest
        let res = QueryBalanceRequest {
            address: owner.to_string(),
            denom: "eth".to_string(),
        }
        .query(&querier)
        .unwrap();
        let balance = res.balance.unwrap();
        assert_eq!(balance.amount, init_funds[1].amount.to_string());
        assert_eq!(balance.denom, init_funds[1].denom);

        // QueryTotalSupplyRequest
        let res = QuerySupplyOfRequest {
            denom: "eth".to_string(),
        };
        let res = res.query(&querier).unwrap();
        let supply = res.amount.unwrap();
        assert_eq!(supply.amount, init_funds[1].amount.to_string());
        assert_eq!(supply.denom, init_funds[1].denom);
    }
}
