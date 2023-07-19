//! Multitest is a design to simulate a blockchain environment in pure Rust.
//! This allows us to run unit tests that involve contract -> contract,
//! and contract -> bank interactions. This is not intended to be a full blockchain app
//! but to simulate the Cosmos SDK x/wasm module close enough to gain confidence in
//! multi-contract deployements before testing them on a live blockchain.
//!
//! To understand the design of this module, please refer to `../DESIGN.md`

mod app;
mod bank;
#[allow(clippy::type_complexity)]
mod contracts;
pub mod custom_handler;
pub mod error;
mod executor;
mod gov;
mod ibc;
mod module;
mod prefixed_storage;
mod staking;
mod stargate;
mod test_helpers;
mod transactions;
mod wasm;

pub use crate::app::{
    custom_app, next_block, App, AppBuilder, BasicApp, BasicAppBuilder, CosmosRouter, Router,
    SudoMsg,
};
pub use crate::bank::{Bank, BankKeeper, BankSudo};
pub use crate::contracts::{Contract, ContractWrapper};
pub use crate::executor::{AppResponse, Executor};
pub use crate::ibc::Ibc;
pub use crate::module::{FailingModule, Module};
pub use crate::staking::{DistributionKeeper, StakeKeeper, Staking, StakingInfo, StakingSudo};
pub use crate::wasm::{AddressGenerator, Wasm, WasmKeeper, WasmSudo};
pub use stargate::*;

pub const QUERY_ALL_BALANCES_PATH: &str = "/cosmos.bank.v1beta1.Query/AllBalances";
pub const QUERY_BALANCE_PATH: &str = "/cosmos.bank.v1beta1.Query/Balance";
pub const QUERY_SUPPLY_PATH: &str = "/cosmos.bank.v1beta1.Query/SupplyOf";
pub const QUERY_WASM_CONTRACT_SMART_PATH: &str = "/cosmwasm.wasm.v1.Query/SmartContractState";
pub const QUERY_WASM_CONTRACT_RAW_PATH: &str = "/cosmwasm.wasm.v1.Query/RawContractState";
pub const QUERY_WASM_CONTRACT_INFO_PATH: &str = "/cosmwasm.wasm.v1.Query/ContractInfo";
pub const QUERY_WASM_CODE_INFO_PATH: &str = "/cosmwasm.wasm.v1.Query/CodeInfo";
