#![cfg(all(not(target_arch = "wasm32"), feature = "testing"))]

use crate::contract::{execute, instantiate, query};
use andromeda_fungible_tokens::cw20::{ExecuteMsg, InstantiateMsg, QueryMsg};
use common::ado_base::modules::Module;
use cosmwasm_std::{Binary, Empty, Uint128};
use cw20::MinterResponse;
use cw_multi_test::{Contract, ContractWrapper};

pub fn mock_andromeda_cw20() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new_with_empty(execute, instantiate, query);
    Box::new(contract)
}

pub fn mock_minter(minter: String, cap: Option<Uint128>) -> MinterResponse {
    MinterResponse { minter, cap }
}

pub fn mock_cw20_instantiate_msg(
    name: impl Into<String>,
    symbol: impl Into<String>,
    decimals: u8,
    initial_balances: Vec<cw20::Cw20Coin>,
    mint: Option<MinterResponse>,
    modules: Option<Vec<Module>>,
) -> InstantiateMsg {
    InstantiateMsg {
        name: name.into(),
        symbol: symbol.into(),
        decimals,
        initial_balances,
        mint,
        marketing: None,
        modules,
    }
}

pub fn mock_get_cw20_balance(address: impl Into<String>) -> QueryMsg {
    QueryMsg::Balance {
        address: address.into(),
    }
}

pub fn mock_cw20_send(contract: impl Into<String>, amount: Uint128, msg: Binary) -> ExecuteMsg {
    ExecuteMsg::Send {
        contract: contract.into(),
        amount,
        msg,
    }
}

pub fn mock_cw20_transfer(recipient: impl Into<String>, amount: Uint128) -> ExecuteMsg {
    ExecuteMsg::Transfer {
        recipient: recipient.into(),
        amount,
    }
}
