use crate::communication::{AndromedaMsg, AndromedaQuery};
use cosmwasm_std::{Event, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The address authorized to mint new receipts
    pub minter: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
/// A struct representation of a receipt. Contains a vector of CosmWasm [Event](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.Event.html) structs.
pub struct Receipt {
    /// A vector of CosmWasm [Event](https://docs.rs/cosmwasm-std/0.16.0/cosmwasm_std/struct.Event.html) structs related to the receipt
    pub events: Vec<Event>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// The address authorized to mint new receipts
    pub minter: String,
    /// Optional list of moderating addresses authorized to update receipts, defaults to an empty vector
    pub operators: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    /// Mint a new receipt. Only executable by the assigned `minter` address. Generates a receipt ID.
    StoreReceipt {
        receipt: Receipt,
    },
    /// Edit a receipt by ID. Only executable by the assigned `minter` address or a valid `operator`.
    EditReceipt {
        receipt_id: Uint128,
        receipt: Receipt,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    AndrQuery(AndromedaQuery),
    /// Query receipt by its generated ID.
    Receipt {
        receipt_id: Uint128,
    },
    /// The current contract config.
    ContractInfo {},
    IsOperator {
        address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ContractInfoResponse {
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ReceiptResponse {
    pub receipt: Receipt,
}
