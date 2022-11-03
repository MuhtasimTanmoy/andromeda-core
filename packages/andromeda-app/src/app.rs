use common::ado_base::{AndromedaMsg, AndromedaQuery};
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Binary;

#[cw_serde]
pub struct AppComponent {
    pub name: String,
    pub ado_type: String,
    pub instantiate_msg: Binary,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub app_components: Vec<AppComponent>,
    pub name: String,
    pub primitive_contract: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    AndrReceive(AndromedaMsg),
    AddAppComponent { component: AppComponent },
    ClaimOwnership { name: Option<String> },
    ProxyMessage { name: String, msg: Binary },
    UpdateAddress { name: String, addr: String },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(AndromedaQuery)]
    #[returns(AndromedaQuery)]
    AndrQuery(AndromedaQuery),
    #[returns(String)]
    GetAddress { name: String },
    #[returns(AppComponent)]
    GetComponents {},
    #[returns(bool)]
    ComponentExists { name: String },
    #[returns(Vec<AppComponent>)]
    GetAddresses {},
    #[returns(ConfigResponse)]
    Config {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub name: String,
}

#[cw_serde]
pub struct ComponentAddress {
    pub name: String,
    pub address: String,
}

#[cfg(test)]
mod tests {
    // use super::*;
}
