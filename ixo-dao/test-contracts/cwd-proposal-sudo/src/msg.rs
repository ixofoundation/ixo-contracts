use cosmwasm_std::CosmosMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cwd_macros::{info_query, proposal_module_query};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub root: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Execute { msgs: Vec<CosmosMsg> },
}

#[proposal_module_query]
#[info_query]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Admin {},
}
